use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFDXGIBuffer, IMFDXGIDeviceManager, IMFMediaBuffer, IMFMediaType, IMFSample,
    IMFSourceReader, MF_LOW_LATENCY, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
    MF_MT_MINIMUM_DISPLAY_APERTURE, MF_MT_SUBTYPE, MF_PD_DURATION, MF_SOURCE_READER_ALL_STREAMS,
    MF_SOURCE_READER_D3D_MANAGER, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READER_MEDIASOURCE,
    MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READERF_ERROR, MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL,
    MFMediaType_Video, MFVideoArea, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::core::{GUID, HSTRING, Interface};

use crate::gpu::{self, Area, Pool};
use crate::runtime::Runtime;
use crate::{Frame, Texture};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const MEDIA_SOURCE: u32 = MF_SOURCE_READER_MEDIASOURCE.0 as u32;
const ALL_STREAMS: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
const TICKS_PER_SECOND: u64 = 10_000_000;

/// What a decoded NV12 picture is: its whole size and the part that shows.
#[derive(Clone, Copy)]
struct Layout {
    size: (u32, u32),
    area: Area,
    /// Bytes between rows in memory, when a buffer does not tell.
    stride: u32,
}

/// A source reader that yields NV12 frames at the clip's own size, as textures on one device.
pub struct Reader {
    reader: IMFSourceReader,
    pool: Pool,
    layout: Layout,
    duration: Duration,
    // Last, so it outlives the reader above.
    _runtime: Runtime,
}

impl Reader {
    pub fn open(file: &Path, device: &ID3D11Device) -> Result<Reader> {
        let runtime = Runtime::start().context("cannot start Media Foundation")?;
        // The GPU's video engine leaves the CPU to the game; the software decoder is the fallback.
        let on_gpu = gpu::manager(device)
            .map_err(anyhow::Error::from)
            .and_then(|manager| configure(file, Some(&manager)));
        let reader = match on_gpu {
            Ok(reader) => reader,
            Err(e) => {
                log::info!("video not decoded on the GPU: {e:#}");
                configure(file, None)?
            }
        };
        let layout = layout_of(&reader)?;
        let duration = duration_of(&reader)?;
        Ok(Reader {
            reader,
            pool: Pool::new(device).context("cannot use the window's device")?,
            layout,
            duration,
            _runtime: runtime,
        })
    }

    /// The length of one pass over the clip.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// The next frame, or `None` at the end of the clip.
    pub fn read(&mut self) -> Result<Option<Frame>> {
        loop {
            let (mut flags, mut ticks, mut sample) = (0u32, 0i64, None::<IMFSample>);
            unsafe {
                self.reader.ReadSample(
                    VIDEO,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut ticks),
                    Some(&mut sample),
                )
            }
            .context("cannot read a video sample")?;
            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                bail!("the source reader reported an error");
            }
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                self.layout = layout_of(&self.reader)?;
            }
            if let Some(sample) = sample {
                let at = Duration::from_nanos(ticks.max(0) as u64 * 100);
                return Ok(Some(self.picture(&sample, at)?));
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok(None);
            }
        }
    }

    /// Back to the first frame.
    pub fn rewind(&mut self) -> Result<()> {
        let start = PROPVARIANT::from(0i64);
        unsafe { self.reader.SetCurrentPosition(&GUID::zeroed(), &start) }
            .context("cannot seek to the start")
    }

    fn picture(&mut self, sample: &IMFSample, at: Duration) -> Result<Frame> {
        let area = self.layout.area;
        let buffer = unsafe { sample.ConvertToContiguousBuffer() }?;
        let texture = match buffer.cast::<IMFDXGIBuffer>() {
            Ok(on_gpu) => self.pool.copy_decoded(&on_gpu, area)?,
            Err(_) => self.uploaded(&buffer)?,
        };
        Ok(Frame {
            texture,
            width: area.width,
            height: area.height,
            at,
        })
    }

    /// The software decoder's picture, from memory onto the device.
    fn uploaded(&mut self, buffer: &IMFMediaBuffer) -> Result<Texture> {
        let Layout { size, area, stride } = self.layout;
        let texture = match buffer.cast::<IMF2DBuffer>() {
            Ok(buffer2d) => {
                let (mut top, mut pitch) = (std::ptr::null_mut(), 0i32);
                unsafe { buffer2d.Lock2D(&mut top, &mut pitch) }?;
                let texture = self.pool.upload(top, pitch as u32, size, area);
                let _ = unsafe { buffer2d.Unlock2D() };
                texture
            }
            Err(_) => {
                let mut top = std::ptr::null_mut();
                unsafe { buffer.Lock(&mut top, None, None) }?;
                let texture = self.pool.upload(top, stride, size, area);
                let _ = unsafe { buffer.Unlock() };
                texture
            }
        };
        Ok(texture?)
    }
}

/// A reader of `file`'s video alone in NV12, decoded through `gpu` when there is one.
fn configure(file: &Path, gpu: Option<&IMFDXGIDeviceManager>) -> Result<IMFSourceReader> {
    let reader = create(file, gpu).with_context(|| format!("cannot open {}", file.display()))?;
    // A selected stream nobody reads (audio) queues its samples.
    unsafe {
        reader
            .SetStreamSelection(ALL_STREAMS, false)
            .and_then(|()| reader.SetStreamSelection(VIDEO, true))
    }
    .context("no video stream")?;
    let out = nv12().context("cannot describe the output type")?;
    unsafe { reader.SetCurrentMediaType(VIDEO, None, &out) }
        .context("no decoder for this video")?;
    Ok(reader)
}

fn create(
    file: &Path,
    gpu: Option<&IMFDXGIDeviceManager>,
) -> windows::core::Result<IMFSourceReader> {
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 3) }?;
    let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
    // H.264 decodes to NV12; this converts any other kind of clip.
    unsafe { attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1) }?;
    // The decoder holds fewer frames: a 1080p clip takes about half the memory, its frames unchanged.
    unsafe { attributes.SetUINT32(&MF_LOW_LATENCY, 1) }?;
    if let Some(gpu) = gpu {
        unsafe { attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, gpu) }?;
    }
    unsafe { MFCreateSourceReaderFromURL(&HSTRING::from(file.as_os_str()), &attributes) }
}

fn nv12() -> windows::core::Result<IMFMediaType> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
    }
    Ok(media_type)
}

/// What the reader really delivers: a decoder pads its pictures (1080 rows to 1088), the aperture says what shows.
fn layout_of(reader: &IMFSourceReader) -> Result<Layout> {
    let media_type = unsafe { reader.GetCurrentMediaType(VIDEO) }?;
    let packed = unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) }.context("no frame size")?;
    let size = ((packed >> 32) as u32, packed as u32);
    let mut aperture = MFVideoArea::default();
    let shown = unsafe {
        media_type.GetBlob(
            &MF_MT_MINIMUM_DISPLAY_APERTURE,
            std::slice::from_raw_parts_mut((&raw mut aperture).cast(), size_of::<MFVideoArea>()),
            None,
        )
    };
    let area = match shown {
        Ok(()) => Area {
            left: aperture.OffsetX.value.max(0) as u32,
            top: aperture.OffsetY.value.max(0) as u32,
            width: aperture.Area.cx.max(0) as u32,
            height: aperture.Area.cy.max(0) as u32,
        },
        Err(_) => Area {
            left: 0,
            top: 0,
            width: size.0,
            height: size.1,
        },
    };
    let area = even(area, size).context("no picture in the frame")?;
    let stride = unsafe { media_type.GetUINT32(&MF_MT_DEFAULT_STRIDE) }.unwrap_or(size.0);
    Ok(Layout { size, area, stride })
}

/// `area` inside a picture of `size`, its corner and sides even.
fn even(area: Area, size: (u32, u32)) -> Option<Area> {
    let (left, top) = (area.left & !1, area.top & !1);
    let width = area.width.min(size.0.saturating_sub(left)) & !1;
    let height = area.height.min(size.1.saturating_sub(top)) & !1;
    (width > 0 && height > 0).then_some(Area {
        left,
        top,
        width,
        height,
    })
}

fn duration_of(reader: &IMFSourceReader) -> Result<Duration> {
    let value = unsafe { reader.GetPresentationAttribute(MEDIA_SOURCE, &MF_PD_DURATION) }
        .context("the clip has no duration")?;
    let ticks = u64::try_from(&value).context("the clip has no duration")?;
    if ticks == 0 {
        bail!("the clip has no duration");
    }
    Ok(Duration::from_nanos(
        ticks * (1_000_000_000 / TICKS_PER_SECOND),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(left: u32, top: u32, width: u32, height: u32) -> Area {
        Area {
            left,
            top,
            width,
            height,
        }
    }

    #[test]
    fn the_shown_area_is_even_and_inside_the_picture() {
        assert_eq!(
            even(area(0, 0, 1920, 1080), (1920, 1088)),
            Some(area(0, 0, 1920, 1080))
        );
        assert_eq!(
            even(area(1, 3, 641, 361), (642, 364)),
            Some(area(0, 2, 640, 360))
        );
        assert_eq!(
            even(area(0, 0, 2000, 2000), (1280, 720)),
            Some(area(0, 0, 1280, 720))
        );
        assert_eq!(even(area(0, 0, 1, 1), (1280, 720)), None);
    }
}
