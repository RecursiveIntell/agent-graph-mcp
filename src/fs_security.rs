//! Fail-closed filesystem security checks for durable storage.
//!
//! Ensures data directories and files are private (0700/0600) and owned
//! by the current user. Rejects unsafe symlinks, foreign ownership, and
//! permissive permissions.

use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Error message when the data directory is already owned by another process.
pub const DATA_DIR_ALREADY_OWNED: &str = "DATA_DIR_ALREADY_OWNED";

/// Check if a mode is private (no group/other permissions).
fn is_private_mode(mode: u32) -> bool {
    mode & 0o077 == 0
}

/// Get the effective UID safely without `unsafe`.
fn current_uid() -> u32 {
    // std::process::id() gives PID, not UID. We need geteuid().
    // Use the nix crate if available, or use the `users` crate.
    // For now, use a safe FFI wrapper through libc but without inline unsafe.
    // The libc crate's functions are already marked unsafe, but we can
    // wrap them in a safe function since geteuid() is inherently safe
    // (it cannot fail and has no side effects).
    //
    // We use `#[allow(unsafe_code)]` on this specific function.
    // This is the only place unsafe is needed in this module.
    current_euid()
}

#[allow(unsafe_code)]
fn current_euid() -> u32 {
    // SAFETY: geteuid() is a read-only syscall that cannot fail.
    // It returns the effective user ID of the calling process.
    // There are no safety invariants to uphold.
    unsafe { libc::geteuid() }
}

/// Check if a file/directory is owned by the current user.
fn is_owned_by_current_user(meta: &fs::Metadata) -> bool {
    meta.uid() == current_uid()
}

/// Ensure a directory exists and has private permissions (0700).
///
/// If the directory doesn't exist, creates it with 0700.
/// If it exists, verifies it's a real directory (not symlink), owned by
/// current user, and has no group/other permissions.
pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    if path.exists() {
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("data directory is a symlink: {}", path.display()),
            ));
        }
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("data path is not a directory: {}", path.display()),
            ));
        }
        if !is_owned_by_current_user(&meta) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "data directory not owned by current user: {}",
                    path.display()
                ),
            ));
        }
        if !is_private_mode(meta.mode()) {
            // Try to fix permissions
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            // Re-check
            let meta = fs::metadata(path)?;
            if !is_private_mode(meta.mode()) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "data directory has permissive permissions: 0o{:o}",
                        meta.mode() & 0o777
                    ),
                ));
            }
        }
    } else {
        fs::create_dir_all(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Check that a file (if it exists) is a regular file, owned by current user,
/// and has private permissions (0600 or stricter).
///
/// If `required` is true, the file must exist.
/// If `required` is false, a missing file is OK.
pub fn check_private_file(path: &Path, required: bool) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("file is a symlink: {}", path.display()),
                ));
            }
            if !meta.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("path is not a regular file: {}", path.display()),
                ));
            }
            if !is_owned_by_current_user(&meta) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("file not owned by current user: {}", path.display()),
                ));
            }
            if !is_private_mode(meta.mode()) {
                // Try to fix permissions for existing files
                fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
                // Re-check
                let meta = fs::metadata(path)?;
                if !is_private_mode(meta.mode()) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "file has permissive permissions: 0o{:o}",
                            meta.mode() & 0o777
                        ),
                    ));
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound && !required => return Ok(()),
        Err(e) => return Err(e),
    }
    Ok(())
}

/// Create a new file with private permissions (0600).
pub fn secure_create_file(path: &Path) -> io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    check_private_file(path, true)?;
    Ok(file)
}

/// Validate the entire data store layout: directory, database files, and
/// optional integrity key.
///
/// This should be called before opening the SQLite database.
pub fn validate_data_store(data_dir: &Path, key: Option<&Path>) -> io::Result<()> {
    ensure_private_dir(data_dir)?;

    // Check database files (WAL/SHM may not exist yet — that's OK)
    for name in ["agent-graph.db", "agent-graph.db-wal", "agent-graph.db-shm"] {
        check_private_file(&data_dir.join(name), false)?;
    }

    // Check integrity key if provided
    if let Some(key) = key {
        check_private_file(key, true)?;
    }

    Ok(())
}

/// Re-check file permissions after WAL creation (SQLite creates WAL files
/// with default umask, which may be too permissive).
pub fn recheck_wal_permissions(data_dir: &Path) -> io::Result<()> {
    for name in ["agent-graph.db-wal", "agent-graph.db-shm"] {
        let path = data_dir.join(name);
        if path.exists() {
            check_private_file(&path, false)?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _path(_: PathBuf) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_private_dir_creates_with_0700() {
        let dir = tempdir().unwrap();
        let new_dir = dir.path().join("new_private");
        ensure_private_dir(&new_dir).unwrap();

        let meta = fs::metadata(&new_dir).unwrap();
        assert_eq!(meta.mode() & 0o777, 0o700);
    }

    #[test]
    fn ensure_private_dir_rejects_symlink() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("link");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let result = ensure_private_dir(&link);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("symlink"));
    }

    #[test]
    fn check_private_file_missing_optional_ok() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nonexistent");
        let result = check_private_file(&missing, false);
        assert!(result.is_ok());
    }

    #[test]
    fn check_private_file_missing_required_fails() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nonexistent");
        let result = check_private_file(&missing, true);
        assert!(result.is_err());
    }

    #[test]
    fn check_private_file_rejects_symlink() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real_file");
        let link = dir.path().join("link_file");
        fs::write(&real, "test").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let result = check_private_file(&link, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("symlink"));
    }

    #[test]
    fn validate_data_store_succeeds_on_fresh_dir() {
        let dir = tempdir().unwrap();
        let result = validate_data_store(dir.path(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn secure_create_file_creates_with_0600() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_file");
        let _file = secure_create_file(&file_path).unwrap();

        let meta = fs::metadata(&file_path).unwrap();
        assert_eq!(meta.mode() & 0o777, 0o600);
    }

    #[test]
    fn recheck_wal_permissions_ok_when_no_wal() {
        let dir = tempdir().unwrap();
        let result = recheck_wal_permissions(dir.path());
        assert!(result.is_ok());
    }
}
