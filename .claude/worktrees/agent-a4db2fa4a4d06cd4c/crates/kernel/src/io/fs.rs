use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::Result;

pub trait FileHandle {
    fn len(&self) -> Result<u64>;
    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    fn read_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()>;
    fn write_all_at(&mut self, offset: u64, buf: &[u8]) -> Result<()>;
    fn sync_data(&self) -> Result<()>;
    fn set_len(&self, len: u64) -> Result<()>;
}

pub trait FileSystem {
    type File: FileHandle;

    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn read_dir_names(&self, path: &Path) -> Result<Vec<String>>;
    fn open_rw_create(&self, path: &Path) -> Result<Self::File>;
    fn open_rw_existing(&self, path: &Path) -> Result<Self::File>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    type File = StdFileHandle;

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path)?;
        Ok(())
    }

    fn read_dir_names(&self, path: &Path) -> Result<Vec<String>> {
        let mut names = Vec::new();
        if !path.exists() {
            return Ok(names);
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    fn open_rw_create(&self, path: &Path) -> Result<Self::File> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        Ok(StdFileHandle(file))
    }

    fn open_rw_existing(&self, path: &Path) -> Result<Self::File> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(StdFileHandle(file))
    }
}

#[derive(Debug)]
pub struct StdFileHandle(File);

impl FileHandle for StdFileHandle {
    fn len(&self) -> Result<u64> {
        Ok(self.0.metadata()?.len())
    }

    fn read_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.read_exact(buf)?;
        Ok(())
    }

    fn write_all_at(&mut self, offset: u64, buf: &[u8]) -> Result<()> {
        self.0.seek(SeekFrom::Start(offset))?;
        self.0.write_all(buf)?;
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        self.0.sync_data()?;
        Ok(())
    }

    fn set_len(&self, len: u64) -> Result<()> {
        self.0.set_len(len)?;
        Ok(())
    }
}
