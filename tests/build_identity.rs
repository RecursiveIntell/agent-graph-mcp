use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn collect(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            collect(&entry.unwrap().path(), files);
        }
    } else {
        assert!(!path.is_symlink());
        files.push(path.to_path_buf());
    }
}

#[test]
fn embedded_digest_matches_current_crate_build_inputs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![
        root.join("build.rs"),
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
    ];
    collect(&root.join("src"), &mut files);
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        hash.update((relative.len() as u64).to_be_bytes());
        hash.update(relative.as_bytes());
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    let build = agent_graph_mcp::transport::build_identity();
    assert_eq!(
        build["source_content_sha256"],
        format!("sha256:{:x}", hash.finalize())
    );
    assert_eq!(
        build["cargo_lock_sha256"],
        format!(
            "sha256:{:x}",
            Sha256::digest(std::fs::read(root.join("Cargo.lock")).unwrap())
        )
    );
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(head.status.success());
    assert_eq!(
        build["source_revision"],
        String::from_utf8(head.stdout).unwrap().trim()
    );
}
