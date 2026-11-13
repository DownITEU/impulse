use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

const CURRENT: &str = "impulse.log";
const PREVIOUS: &str = "impulse.old.log";

/// The log file. Lines are written unbuffered, so a crash loses nothing.
pub struct Sink {
    dir: PathBuf,
    file: File,
    written: u64,
    limit: u64,
}

impl Sink {
    pub fn open(dir: &Path, limit: u64) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let file = append(dir)?;
        let written = file.metadata()?.len();
        Ok(Self { dir: dir.to_owned(), file, written, limit })
    }

    pub fn write(&mut self, line: &str) -> io::Result<()> {
        if self.written > self.limit {
            self.rotate()?;
        }
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.written += line.len() as u64 + 1;
        Ok(())
    }

    // Keeps one previous file, so the logs never exceed about twice the limit.
    fn rotate(&mut self) -> io::Result<()> {
        fs::rename(self.dir.join(CURRENT), self.dir.join(PREVIOUS))?;
        self.file = append(&self.dir)?;
        self.written = 0;
        Ok(())
    }
}

fn append(dir: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(dir.join(CURRENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_past_the_limit() {
        let dir = std::env::temp_dir().join(format!("impulse-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut sink = Sink::open(&dir, 10).unwrap();
        sink.write("first line, over the limit").unwrap();
        sink.write("second").unwrap();
        drop(sink);
        let read = |name| fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(read(PREVIOUS), "first line, over the limit\n");
        assert_eq!(read(CURRENT), "second\n");
        // A new run appends to the current file.
        Sink::open(&dir, 10).unwrap().write("third").unwrap();
        assert_eq!(read(CURRENT), "second\nthird\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
