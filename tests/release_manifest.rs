use std::fs;

#[test]
fn release_schema_and_scripts_exist() {
    let root = env!("CARGO_MANIFEST_DIR");
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(format!("{root}/docs/release/manifest-v2.schema.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(schema["properties"]["manifest_version"]["const"], 2);
    for name in [
        "build-release.sh",
        "validate-release.py",
        "install-release.sh",
    ] {
        assert!(
            fs::metadata(format!("{root}/scripts/{name}")).is_ok(),
            "missing {name}"
        );
    }
}
