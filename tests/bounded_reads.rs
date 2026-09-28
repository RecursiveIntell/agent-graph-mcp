use agent_graph_mcp::run_artifact::{page_verified_artifact, ArtifactKind, ArtifactPageError};
use serde_json::json;

fn wrapper(run_id: &str, version: &str, body: &str) -> serde_json::Value {
    json!({
        "receipt": {"run_id": run_id, "graph_version": version, "terminal_output": {"state_key": "answer"}},
        "bundle": {"payload": {"run_id": run_id, "graph_version": version, "output": {"answer": body}}}
    })
}

#[test]
fn pages_reconstruct_canonical_utf8_json_and_digest() {
    let value = wrapper("r1", "v1", "a🦀b");
    let full = serde_json::to_vec(&value["bundle"]["payload"]["output"]["answer"]).unwrap();
    let first =
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 0, 7, None, None).unwrap();
    assert!(first.data.len() <= 7);
    assert!(first.next_offset.is_some());
    let second = page_verified_artifact(
        &value,
        "r1",
        ArtifactKind::Output,
        first.next_offset.unwrap(),
        7,
        Some(&first.digest),
        Some(&first.artifact_id),
    )
    .unwrap();
    let mut assembled = first.data.into_bytes();
    assembled.extend_from_slice(second.data.as_bytes());
    assert_eq!(assembled, full);
    assert_eq!(second.digest, first.digest);
    assert!(second.done);
}

#[test]
fn rejects_invalid_limits_offsets_and_small_nonprogress_pages() {
    let value = wrapper("r1", "v1", "🦀");
    assert!(matches!(
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 0, 0, None, None),
        Err(ArtifactPageError::InvalidLimit)
    ));
    assert!(matches!(
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 0, 16385, None, None),
        Err(ArtifactPageError::InvalidLimit)
    ));
    let initial =
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 0, 1, None, None).unwrap();
    assert!(matches!(
        page_verified_artifact(
            &value,
            "r1",
            ArtifactKind::Output,
            2,
            10,
            Some(&initial.digest),
            Some(&initial.artifact_id)
        ),
        Err(ArtifactPageError::InvalidOffset)
    ));
    assert!(matches!(
        page_verified_artifact(
            &value,
            "r1",
            ArtifactKind::Output,
            1,
            1,
            Some(&initial.digest),
            Some(&initial.artifact_id)
        ),
        Err(ArtifactPageError::LimitTooSmall)
    ));
    assert!(matches!(
        page_verified_artifact(
            &value,
            "r1",
            ArtifactKind::Output,
            u64::MAX,
            10,
            Some(&initial.digest),
            Some(&initial.artifact_id)
        ),
        Err(ArtifactPageError::InvalidOffset)
    ));
}

#[test]
fn continuation_requires_and_checks_digest_and_artifact_identity() {
    let value = wrapper("r1", "v1", "abc");
    let first =
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 0, 1, None, None).unwrap();
    assert!(matches!(
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 1, 1, None, None),
        Err(ArtifactPageError::ContinuationIdentityRequired)
    ));
    assert!(matches!(
        page_verified_artifact(
            &value,
            "r1",
            ArtifactKind::Output,
            1,
            1,
            Some("wrong"),
            Some(&first.artifact_id)
        ),
        Err(ArtifactPageError::DigestMismatch)
    ));
    assert!(matches!(
        page_verified_artifact(
            &value,
            "r1",
            ArtifactKind::Output,
            1,
            1,
            Some(&first.digest),
            Some("wrong")
        ),
        Err(ArtifactPageError::ArtifactIdentityMismatch)
    ));
}

#[test]
fn artifact_identity_binds_run_and_kind_even_for_identical_bytes() {
    let a = wrapper("r1", "v1", "same");
    let b = wrapper("r2", "v1", "same");
    let out_a =
        page_verified_artifact(&a, "r1", ArtifactKind::Output, 0, 16384, None, None).unwrap();
    let out_b =
        page_verified_artifact(&b, "r2", ArtifactKind::Output, 0, 16384, None, None).unwrap();
    let same_bytes = json!({
        "receipt": {"run_id": "r1", "graph_version": "v1", "terminal_output": {"state_key": "answer"}},
        "bundle": {"payload": {"run_id": "r1", "graph_version": "v1", "output": {
            "answer": {"run_id": "r1", "graph_version": "v1", "terminal_output": {"state_key": "answer"}}
        }}}
    });
    let same_output = page_verified_artifact(
        &same_bytes,
        "r1",
        ArtifactKind::Output,
        0,
        16384,
        None,
        None,
    )
    .unwrap();
    let receipt = page_verified_artifact(
        &same_bytes,
        "r1",
        ArtifactKind::Receipt,
        0,
        16384,
        None,
        None,
    )
    .unwrap();
    assert_eq!(out_a.data, out_b.data);
    assert_ne!(out_a.artifact_id, out_b.artifact_id);
    assert_eq!(same_output.data, receipt.data);
    assert_ne!(same_output.artifact_id, receipt.artifact_id);
}

#[test]
fn absent_declared_output_is_unavailable() {
    let mut value = wrapper("r1", "v1", "unused");
    value["bundle"]["payload"]["output"] = json!({"other": 2});
    assert!(matches!(
        page_verified_artifact(&value, "r1", ArtifactKind::Output, 0, 20, None, None),
        Err(ArtifactPageError::Unavailable)
    ));
}
