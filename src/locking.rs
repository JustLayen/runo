//! Vim/nano-compatible advisory lock header. The lock never contains file data.
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
pub struct Lock {
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl Lock {
    pub fn acquire(name: &str) -> io::Result<Self> {
        let path = Path::new(name);
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = parent.join(format!(
            ".{}.swp",
            path.file_name()
                .ok_or_else(|| io::Error::other("Invalid lock filename"))?
                .to_string_lossy()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let mut bytes = vec![0u8; 1024];
        bytes[..2].copy_from_slice(b"b0");
        put(&mut bytes[2..24], &format!("runo {}", crate::defs::VERSION));
        bytes[24..28].copy_from_slice(&std::process::id().to_le_bytes());
        put(
            &mut bytes[28..68],
            &std::env::var("USER").unwrap_or_else(|_| "unknown".into()),
        );
        let mut host = [0u8; 256];
        unsafe {
            libc::gethostname(host.as_mut_ptr().cast(), host.len());
        }
        put(
            &mut bytes[68..108],
            &String::from_utf8_lossy(
                &host[..host.iter().position(|&c| c == 0).unwrap_or(host.len())],
            ),
        );
        put(
            &mut bytes[108..1008],
            &fs::canonicalize(name)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(name))
                .to_string_lossy(),
        );
        let metadata = file.metadata()?;
        let lock = Self {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        file.write_all(&bytes)?;
        Ok(lock)
    }
}
fn put(dst: &mut [u8], text: &str) {
    let n = text.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&text.as_bytes()[..n]);
}
impl Drop for Lock {
    fn drop(&mut self) {
        if let Ok(m) = fs::symlink_metadata(&self.path) {
            if m.dev() == self.device && m.ino() == self.inode {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}
