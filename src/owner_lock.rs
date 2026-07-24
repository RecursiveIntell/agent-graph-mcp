//! Process-lifetime exclusive ownership of a durable data directory.
//!
//! Uses `fs2` for safe file locking without `unsafe` code.
//! The lock is automatically released when the owning process exits
//! or when the `OwnerLock` is dropped.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Error message when the data directory is already owned by another process.
pub const DATA_DIR_ALREADY_OWNED: &str =
    "DATA_DIR_ALREADY_OWNED: another process owns this data directory";

/// A process-lifetime exclusive lock on a data directory.
///
/// The lock file is created at `{data_dir}/.owner.lock` with mode 0600.
/// The lock is held for the lifetime of this struct (RAII).
/// When dropped, the file handle is closed which automatically releases
/// the advisory lock.
#[derive(Debug)]
pub struct OwnerLock {
    _file: File,
    pub path: PathBuf,
}

impl OwnerLock {
    /// Attempt to acquire an exclusive, non-blocking lock on the data directory.
    ///
    /// Returns `Err(DATA_DIR_ALREADY_OWNED)` if another process already holds the lock.
    /// No database files are opened or modified during lock acquisition.
    pub fn acquire(data_dir: &Path) -> io::Result<Self> {
        // Ensure the directory exists before creating the lock file
        std::fs::create_dir_all(data_dir)?;

        let path = data_dir.join(".owner.lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;

        // Try non-blocking exclusive lock
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                DATA_DIR_ALREADY_OWNED,
            )),
        }
    }
}

impl Drop for OwnerLock {
    fn drop(&mut self) {
        // fs2 unlocks automatically when the File is dropped.
        // The _file field will be dropped after this returns,
        // releasing the lock.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn acquire_and_reacquire_after_drop() {
        let dir = tempdir().unwrap();
        let lock1 = OwnerLock::acquire(dir.path());
        assert!(lock1.is_ok(), "first acquire should succeed");

        // Drop the lock
        drop(lock1);

        // Should be able to reacquire
        let lock2 = OwnerLock::acquire(dir.path());
        assert!(lock2.is_ok(), "reacquire after drop should succeed");
    }

    #[test]
    fn second_acquire_fails() {
        let dir = tempdir().unwrap();
        let _lock1 = OwnerLock::acquire(dir.path()).expect("first acquire");

        let lock2 = OwnerLock::acquire(dir.path());
        assert!(lock2.is_err(), "second concurrent acquire should fail");
        let err = lock2.unwrap_err();
        assert!(
            err.to_string().contains("ALREADY_OWNED"),
            "error should mention ALREADY_OWNED"
        );
    }

    #[test]
    fn lock_file_has_private_permissions() {
        let dir = tempdir().unwrap();
        let _lock = OwnerLock::acquire(dir.path()).expect("acquire");

        let meta = std::fs::metadata(dir.path().join(".owner.lock")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "lock file should have 0600 permissions, got 0o{:o}",
            mode
        );
    }
}
