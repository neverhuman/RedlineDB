use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::{Error, ErrorCode, Result};

pub(super) struct NativeRoot {
    dir: File,
    dev: u64,
    ino: u64,
    bindings: Vec<RootBinding>,
}

struct RootBinding {
    parent: File,
    name: CString,
    dev: u64,
    ino: u64,
}

impl NativeRoot {
    pub(super) fn open(path: &Path) -> Result<Self> {
        Self::open_with_create(path, false)
    }

    pub(super) fn prepare_empty(path: &Path, message: &'static str) -> Result<Self> {
        let root = Self::open_with_create(path, true)?;
        root.revalidate_path(path)?;
        if fs::read_dir(root.proc_path())?.next().is_some() {
            return Err(Error::new(ErrorCode::Busy, message));
        }
        Ok(root)
    }

    fn open_with_create(path: &Path, create: bool) -> Result<Self> {
        if path.as_os_str().is_empty() {
            return Err(Error::new(ErrorCode::Corrupt, "native root path is empty"));
        }
        let start = if path.is_absolute() { "/" } else { "." };
        let mut dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(start)?;
        let mut bindings = Vec::new();
        for component in path.components() {
            let component = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(component) => component,
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(Error::new(
                        ErrorCode::Corrupt,
                        "native root path contains an invalid component",
                    ));
                }
            };
            let name = component_name(component)?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let opened = match openat_name(&dir, &name, flags, 0) {
                Ok(opened) => opened,
                Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                    mkdirat_name(&dir, &name)?;
                    dir.sync_all()?;
                    openat_name(&dir, &name, flags, 0)?
                }
                Err(error) => return Err(error.into()),
            };
            let metadata = opened.metadata()?;
            let observed = statat_name(&dir, &name)?;
            if !metadata.file_type().is_dir()
                || observed.st_mode & libc::S_IFMT != libc::S_IFDIR
                || observed.st_dev as u64 != metadata.dev()
                || observed.st_ino as u64 != metadata.ino()
            {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "native root ancestry is not stable",
                ));
            }
            bindings.push(RootBinding {
                parent: dir,
                name,
                dev: metadata.dev(),
                ino: metadata.ino(),
            });
            dir = opened;
        }
        let metadata = dir.metadata()?;
        if !metadata.file_type().is_dir() {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "native root is not a directory",
            ));
        }
        let root = Self {
            dir,
            dev: metadata.dev(),
            ino: metadata.ino(),
            bindings,
        };
        root.revalidate_ancestry()?;
        Ok(root)
    }

    pub(super) fn proc_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.dir.as_raw_fd()))
    }

    pub(super) fn sync_all(&self) -> Result<()> {
        self.dir.sync_all()?;
        Ok(())
    }

    pub(super) fn revalidate_path(&self, path: &Path) -> Result<()> {
        self.revalidate_ancestry()?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir()
            || metadata.dev() != self.dev
            || metadata.ino() != self.ino
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "native root identity changed",
            ));
        }
        self.revalidate_ancestry()?;
        Ok(())
    }

    fn revalidate_ancestry(&self) -> Result<()> {
        for binding in &self.bindings {
            let observed = statat_name(&binding.parent, &binding.name)?;
            if observed.st_mode & libc::S_IFMT != libc::S_IFDIR
                || observed.st_dev as u64 != binding.dev
                || observed.st_ino as u64 != binding.ino
            {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "native root ancestry changed",
                ));
            }
        }
        let metadata = self.dir.metadata()?;
        if !metadata.file_type().is_dir()
            || metadata.dev() != self.dev
            || metadata.ino() != self.ino
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "native root identity changed",
            ));
        }
        Ok(())
    }

    pub(super) fn open_regular_file(&self, rel: &Path) -> Result<File> {
        self.revalidate_ancestry()?;
        let components = normal_components(rel)?;
        let mut parent = self.dir.try_clone()?;
        for (index, component) in components.iter().enumerate() {
            let last = index + 1 == components.len();
            let flags = if last {
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC
            } else {
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
            };
            let opened = openat_component(&parent, component, flags, 0)?;
            let metadata = opened.metadata()?;
            if (last && (!metadata.file_type().is_file() || metadata.nlink() != 1))
                || (!last && !metadata.file_type().is_dir())
            {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "backup path is not a native regular file",
                ));
            }
            parent = opened;
        }
        self.revalidate_ancestry()?;
        Ok(parent)
    }

    pub(super) fn create_regular_file(&self, rel: &Path) -> Result<File> {
        self.revalidate_ancestry()?;
        let components = normal_components(rel)?;
        let (file_name, parents) = components
            .split_last()
            .ok_or_else(|| Error::new(ErrorCode::Corrupt, "empty backup path"))?;
        let mut parent = self.dir.try_clone()?;
        for component in parents {
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let opened = match openat_component(&parent, component, flags, 0) {
                Ok(opened) => opened,
                Err(error) if error.code() == ErrorCode::NotFound => {
                    mkdirat_component(&parent, component)?;
                    parent.sync_all()?;
                    openat_component(&parent, component, flags, 0)?
                }
                Err(error) => return Err(error),
            };
            if !opened.metadata()?.file_type().is_dir() {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "restore ancestor is not a native directory",
                ));
            }
            parent = opened;
        }
        let output = openat_component(
            &parent,
            file_name,
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )?;
        parent.sync_all()?;
        self.revalidate_ancestry()?;
        Ok(output)
    }

    pub(super) fn write_json_atomic<T: Serialize>(&self, rel: &Path, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(value)?;
        self.write_bytes_atomic(rel, &bytes)
    }

    pub(super) fn write_text_atomic(&self, rel: &Path, text: &str) -> Result<()> {
        self.write_bytes_atomic(rel, text.as_bytes())
    }

    fn write_bytes_atomic(&self, rel: &Path, bytes: &[u8]) -> Result<()> {
        self.revalidate_ancestry()?;
        let (parent, name) = self.open_parent(rel, true)?;
        match statat_name(&parent, &name) {
            Ok(metadata)
                if metadata.st_mode & libc::S_IFMT == libc::S_IFREG && metadata.st_nlink == 1 => {}
            Ok(_) => {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "atomic target is not a native regular file",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let mut temporary_bytes = name.to_bytes().to_vec();
        temporary_bytes.extend_from_slice(b".tmp");
        let temporary = CString::new(temporary_bytes)
            .map_err(|_| Error::new(ErrorCode::Corrupt, "invalid atomic temporary name"))?;
        let result = (|| {
            let mut file = openat_name(
                &parent,
                &temporary,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )?;
            file.write_all(bytes)?;
            file.sync_all()?;
            parent.sync_all()?;
            self.revalidate_ancestry()?;
            renameat_name(&parent, &temporary, &name)?;
            parent.sync_all()?;
            let published = openat_name(
                &parent,
                &name,
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0,
            )?;
            let metadata = published.metadata()?;
            let observed = statat_name(&parent, &name)?;
            if !metadata.file_type().is_file()
                || metadata.nlink() != 1
                || metadata.len() != bytes.len() as u64
                || observed.st_mode & libc::S_IFMT != libc::S_IFREG
                || observed.st_nlink != 1
                || observed.st_size < 0
                || observed.st_size as u64 != bytes.len() as u64
                || observed.st_dev as u64 != metadata.dev()
                || observed.st_ino as u64 != metadata.ino()
            {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "atomic publication identity changed",
                ));
            }
            self.revalidate_ancestry()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = unlinkat_name(&parent, &temporary);
        }
        result
    }

    fn open_parent(&self, rel: &Path, create: bool) -> Result<(File, CString)> {
        let components = normal_components(rel)?;
        let (file_name, parents) = components
            .split_last()
            .ok_or_else(|| Error::new(ErrorCode::Corrupt, "empty backup path"))?;
        let mut parent = self.dir.try_clone()?;
        for component in parents {
            let name = component_name(component)?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let opened = match openat_name(&parent, &name, flags, 0) {
                Ok(opened) => opened,
                Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                    mkdirat_name(&parent, &name)?;
                    parent.sync_all()?;
                    openat_name(&parent, &name, flags, 0)?
                }
                Err(error) => return Err(error.into()),
            };
            let metadata = opened.metadata()?;
            let observed = statat_name(&parent, &name)?;
            if !metadata.file_type().is_dir()
                || observed.st_mode & libc::S_IFMT != libc::S_IFDIR
                || observed.st_dev as u64 != metadata.dev()
                || observed.st_ino as u64 != metadata.ino()
            {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "atomic publication ancestry changed",
                ));
            }
            parent = opened;
        }
        Ok((parent, component_name(file_name)?))
    }
}

fn normal_components(path: &Path) -> Result<Vec<&std::ffi::OsStr>> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "invalid backup manifest path",
            ));
        };
        components.push(component);
    }
    if components.is_empty() {
        return Err(Error::new(ErrorCode::Corrupt, "empty backup manifest path"));
    }
    Ok(components)
}

fn component_name(component: &std::ffi::OsStr) -> Result<CString> {
    CString::new(component.as_bytes())
        .map_err(|_| Error::new(ErrorCode::Corrupt, "backup path contains NUL"))
}

fn openat_component(
    parent: &File,
    component: &std::ffi::OsStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> Result<File> {
    let component = component_name(component)?;
    openat_name(parent, &component, flags, mode).map_err(Error::from)
}

fn openat_name(
    parent: &File,
    name: &CStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> std::io::Result<File> {
    // SAFETY: `parent` is retained for the call, `name` is NUL-terminated,
    // and a successful call returns a new owned descriptor.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags, mode) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: the successful `openat` result is uniquely owned here.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn statat_name(parent: &File, name: &CStr) -> std::io::Result<libc::stat> {
    // SAFETY: `libc::stat` is a C data record whose all-zero bit pattern is
    // valid; `fstatat` overwrites it on success.
    let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
    // SAFETY: `parent` remains open, `name` is NUL-terminated, and a
    // successful `fstatat` initializes the provided `stat` storage.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(stat)
    }
}

fn mkdirat_name(parent: &File, name: &CStr) -> std::io::Result<()> {
    // SAFETY: `parent` and the NUL-terminated `name` remain valid for the
    // complete call.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    Ok(())
}

fn mkdirat_component(parent: &File, component: &std::ffi::OsStr) -> Result<()> {
    let component = component_name(component)?;
    mkdirat_name(parent, &component).map_err(Error::from)
}

fn renameat_name(parent: &File, old: &CStr, new: &CStr) -> std::io::Result<()> {
    // SAFETY: both names and the retained directory descriptor remain valid
    // for the complete call.
    if unsafe {
        libc::renameat(
            parent.as_raw_fd(),
            old.as_ptr(),
            parent.as_raw_fd(),
            new.as_ptr(),
        )
    } != 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn unlinkat_name(parent: &File, name: &CStr) -> std::io::Result<()> {
    // SAFETY: `parent` and `name` remain valid for the complete call.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error);
        }
    }
    Ok(())
}
