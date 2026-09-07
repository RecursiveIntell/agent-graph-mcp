use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn collect(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "build identity does not admit source symlinks",
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            collect(&entry?.path(), files)?;
        }
    } else if metadata.is_file() {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Scope is explicit: these are crate build inputs, not an entire repository
    // or transitive dependency attestation. The deployment manifest owns those.
    let mut files = vec![PathBuf::from("build.rs"), PathBuf::from("Cargo.toml")];
    if Path::new("Cargo.lock").exists() {
        files.push(PathBuf::from("Cargo.lock"));
    }
    collect(Path::new("src"), &mut files)?;
    files.sort();
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let mut source = Sha256::new();
    for path in &files {
        let name = path.to_str().ok_or("non-UTF-8 source path")?;
        println!("cargo:rerun-if-changed={name}");
        let bytes = fs::read(path)?;
        source.update((name.len() as u64).to_be_bytes());
        source.update(name.as_bytes());
        source.update((bytes.len() as u64).to_be_bytes());
        source.update(&bytes);
    }
    // Resolve worktree .git files through Git; do not assume .git is a directory.
    for name in ["HEAD", "index", "refs", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let revision = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unavailable".into());
    let dirty = git(&["status", "--porcelain=v1"])
        .map(|value| (!value.is_empty()).to_string())
        .unwrap_or_else(|| "unknown".into());
    let lock = match fs::read("Cargo.lock") {
        Ok(bytes) => format!("sha256:{:x}", Sha256::digest(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => "unavailable".into(),
        Err(error) => return Err(error.into()),
    };
    println!("cargo:rustc-env=AGENT_GRAPH_BUILD_GIT_SHA={revision}");
    println!("cargo:rustc-env=AGENT_GRAPH_BUILD_GIT_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=AGENT_GRAPH_BUILD_SOURCE_SHA256=sha256:{:x}",
        source.finalize()
    );
    println!("cargo:rustc-env=AGENT_GRAPH_BUILD_LOCK_SHA256={lock}");
    Ok(())
}
