#[path = "../src/lifecycle.rs"]
mod lifecycle;
#[test]
fn lifecycle_mapping_has_one_owner() {
    for (s, e) in [
        ("accepted", lifecycle::Lifecycle::Accepted),
        ("running", lifecycle::Lifecycle::Running),
        ("completed", lifecycle::Lifecycle::Completed),
        ("failed", lifecycle::Lifecycle::Failed),
        ("cancelled", lifecycle::Lifecycle::Cancelled),
        ("interrupted", lifecycle::Lifecycle::Interrupted),
    ] {
        assert_eq!(lifecycle::Lifecycle::classify(s), e);
    }
}
