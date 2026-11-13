//! Frames on the window's own device: decoded there by the GPU's video engine when it can, then copied into
//! textures the UI draws, so a frame never leaves the GPU.

use std::sync::mpsc::{Receiver, Sender, channel};

use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{IMFDXGIBuffer, IMFDXGIDeviceManager, MFCreateDXGIDeviceManager};
use windows::core::{Error, Interface, Result};

/// Hands `device` to Media Foundation; it must be multithread protected, as GPUI's is.
pub fn manager(device: &ID3D11Device) -> Result<IMFDXGIDeviceManager> {
    let (mut token, mut manager) = (0, None);
    // SAFETY: `token` and `manager` are valid outputs that live through the call.
    unsafe { MFCreateDXGIDeviceManager(&mut token, &mut manager) }?;
    let manager = manager.ok_or_else(Error::empty)?;
    // SAFETY: `manager` is live, `device` is a live device and `token` came from creating this manager.
    unsafe { manager.ResetDevice(device, token) }?;
    Ok(manager)
}

/// The part of a decoded picture that shows; even sides, as NV12 needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

/// NV12 textures the frames are copied into. A frame's comes back once it is dropped, so a few go round.
pub struct Pool {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    free: Receiver<ID3D11Texture2D>,
    back: Sender<ID3D11Texture2D>,
    // The software decoder's picture, uploaded whole before its shown part is copied out.
    upload: Option<ID3D11Texture2D>,
}

impl Pool {
    pub fn new(device: &ID3D11Device) -> Result<Pool> {
        // SAFETY: `device` is a live COM object.
        let context = unsafe { device.GetImmediateContext() }?;
        let (back, free) = channel();
        Ok(Pool { device: device.clone(), context, free, back, upload: None })
    }

    /// The GPU decoder's picture in `buffer`.
    pub fn copy_decoded(&mut self, buffer: &IMFDXGIBuffer, area: Area) -> Result<Texture> {
        // SAFETY: `raw` is a valid output; `from_raw` only runs after `?` shows GetResource succeeded, so it owns a live texture.
        let source = unsafe {
            let mut raw = std::ptr::null_mut();
            buffer.GetResource(&ID3D11Texture2D::IID, &mut raw)?;
            ID3D11Texture2D::from_raw(raw)
        };
        // SAFETY: `buffer` is a live COM object.
        let index = unsafe { buffer.GetSubresourceIndex() }?;
        self.copy(&source, index, area)
    }

    /// The software decoder's picture of `size`: rows `pitch` bytes apart from `top`, the chroma after the luma.
    ///
    /// # Safety
    /// `top` points to an NV12 picture of `size` at `pitch`, readable for the whole call.
    pub unsafe fn upload(&mut self, top: *const u8, pitch: u32, size: (u32, u32), area: Area) -> Result<Texture> {
        let upload = match &self.upload {
            Some(upload) if size_of(upload) == size => upload.clone(),
            _ => self.upload.insert(self.create(size, 0)?).clone(),
        };
        // SAFETY: `upload` is a live texture of this device and `top` is as the caller promised.
        unsafe { self.context.UpdateSubresource(&upload, 0, None, top.cast(), pitch, 0) };
        self.copy(&upload, 0, area)
    }

    fn copy(&mut self, source: &ID3D11Texture2D, index: u32, area: Area) -> Result<Texture> {
        let texture = self.take((area.width, area.height))?;
        let part =
            D3D11_BOX { left: area.left, top: area.top, front: 0, right: area.left + area.width, bottom: area.top + area.height, back: 1 };
        // On the immediate context, so a draw issued after it sees the copy done.
        // SAFETY: both textures are live on this device, `index` is the source's own subresource and `part` lies inside the picture.
        unsafe { self.context.CopySubresourceRegion(&texture, 0, 0, 0, 0, source, index, Some(&part)) };
        Ok(Texture { texture: Some(texture), back: self.back.clone() })
    }

    fn take(&self, size: (u32, u32)) -> Result<ID3D11Texture2D> {
        // One of another size is from before the picture changed size: it goes.
        while let Ok(texture) = self.free.try_recv() {
            if size_of(&texture) == size {
                return Ok(texture);
            }
        }
        self.create(size, D3D11_BIND_SHADER_RESOURCE.0 as u32)
    }

    fn create(&self, (width, height): (u32, u32), bind: u32) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: `desc` and `texture` are valid for the call and no initial data is passed.
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture)) }?;
        texture.ok_or_else(Error::empty)
    }
}

fn size_of(texture: &ID3D11Texture2D) -> (u32, u32) {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: `texture` is a live COM object and `desc` a valid output.
    unsafe { texture.GetDesc(&mut desc) };
    (desc.Width, desc.Height)
}

/// An NV12 texture of the decoder's pool; back in it once dropped.
pub struct Texture {
    texture: Option<ID3D11Texture2D>,
    back: Sender<ID3D11Texture2D>,
}

impl Texture {
    pub fn get(&self) -> &ID3D11Texture2D {
        self.texture.as_ref().expect("only a drop takes it")
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        if let Some(texture) = self.texture.take() {
            let _ = self.back.send(texture);
        }
    }
}
