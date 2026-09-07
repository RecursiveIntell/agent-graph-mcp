//! Operator-approved Git source boundary, narrower than cargo-deny's URL allowlist.
const REPOSITORY: &str = "https://github.com/RecursiveIntell/proveKV.git";
const REVISION: &str = "8dd2bd4ce6ee9c2247f47823151b185a5c5511b9";

fn approved_source(source: &str) -> bool {
    source == format!("git+{REPOSITORY}?rev={REVISION}#{REVISION}")
}

#[test]
fn manifest_and_lock_use_only_the_operator_approved_git_revision() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: toml::Value = std::fs::read_to_string(root.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        manifest["dependencies"]["provekv"]["git"].as_str(),
        Some(REPOSITORY)
    );
    assert_eq!(
        manifest["dependencies"]["provekv"]["rev"].as_str(),
        Some(REVISION)
    );
    let lock: toml::Value = std::fs::read_to_string(root.join("Cargo.lock"))
        .unwrap()
        .parse()
        .unwrap();
    let packages = lock["package"].as_array().unwrap();
    assert!(packages.iter().all(|p| p["name"].as_str() != Some("paste")));
    let mut simba_versions = Vec::new();
    for package in packages {
        if package["name"].as_str() == Some("simba") {
            assert!(approved_source(package["source"].as_str().unwrap()));
            simba_versions.push(package["version"].as_str().unwrap());
        }
    }
    simba_versions.sort();
    assert_eq!(simba_versions, ["0.8.1", "0.9.1"]);
    let sources: Vec<_> = lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p.get("source").and_then(toml::Value::as_str))
        .filter(|s| s.starts_with("git+"))
        .collect();
    assert!(
        !sources.is_empty(),
        "approved Git dependency unexpectedly absent"
    );
    for source in sources {
        assert!(approved_source(source), "unapproved Git source: {source}");
    }
}

#[test]
fn changed_revision_branch_repository_and_resolved_commit_are_rejected() {
    let approved = format!("git+{REPOSITORY}?rev={REVISION}#{REVISION}");
    assert!(approved_source(&approved));
    for rejected in [
        approved.replace("?rev=", "?branch="),
        approved.replace("proveKV.git", "other.git"),
        approved.replace(REVISION, "1111111111111111111111111111111111111111"),
        format!("git+{REPOSITORY}?rev={REVISION}#1111111111111111111111111111111111111111"),
    ] {
        assert!(!approved_source(&rejected));
    }
}
