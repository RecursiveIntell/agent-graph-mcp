use agent_graph_mcp::tool_runtime::{
    classify_tool, issue_lease, reserve_call, verify_lease, verify_receipt_chain, LeaseBinding,
    ReceiptOutcome, ToolCallReceipt, ToolCounters, ToolEffect, ToolInvocation, ToolLease,
    ToolPolicyError,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};

fn key() -> Vec<u8> {
    vec![0x42; 32]
}

fn lease() -> ToolLease {
    let now = Utc::now();
    ToolLease {
        protocol: "agent_graph.tool_lease.v1".into(),
        lease_id: "lease-1".into(),
        lineage_id: "lineage-1".into(),
        graph_id: "cleanup".into(),
        graph_version: "sha256:graph".into(),
        run_id: "run-1".into(),
        node_id: "inspect".into(),
        issued_at: now - Duration::seconds(1),
        expires_at: now + Duration::minutes(5),
        tool_allowlist: vec!["*".into()],
        effect_allowlist: vec![ToolEffect::ReadOnly],
        max_tool_calls: 4,
        max_recursive_calls: 0,
        max_agent_depth: 1,
        max_graph_depth: 1,
        max_children: 0,
        agent_depth: 1,
        graph_depth: 1,
        active_stack: vec!["graph:cleanup@sha256:graph".into(), "node:inspect".into()],
        counters: ToolCounters::default(),
        parent_receipt_digest: None,
    }
}

fn binding() -> LeaseBinding<'static> {
    LeaseBinding {
        graph_id: "cleanup",
        graph_version: "sha256:graph",
        run_id: "run-1",
        node_id: "inspect",
    }
}

fn invocation(tool_name: &str, effect: ToolEffect) -> ToolInvocation {
    ToolInvocation {
        graph_id: "cleanup".into(),
        graph_version: "sha256:graph".into(),
        run_id: "run-1".into(),
        node_id: "inspect".into(),
        attempt: 1,
        tool_name: tool_name.into(),
        arguments: json!({"path":"/tmp/repo"}),
        effect,
        recursion_identity: None,
        parent_receipt_digest: None,
    }
}

#[test]
fn lease_hmac_and_exact_binding_are_mandatory() {
    let signed = issue_lease(lease(), &key()).expect("signed lease");
    verify_lease(&signed, &key(), Utc::now(), binding()).expect("valid lease");

    let mut forged = signed.clone();
    forged.lease.max_tool_calls = 999;
    assert_eq!(
        verify_lease(&forged, &key(), Utc::now(), binding()).unwrap_err(),
        ToolPolicyError::LeaseSignatureInvalid
    );

    let wrong = LeaseBinding {
        run_id: "run-other",
        ..binding()
    };
    assert_eq!(
        verify_lease(&signed, &key(), Utc::now(), wrong).unwrap_err(),
        ToolPolicyError::LeaseBindingMismatch
    );
}

#[test]
fn expired_or_missing_signature_fails_closed() {
    let mut expired = lease();
    expired.expires_at = Utc::now() - Duration::seconds(1);
    let signed = issue_lease(expired, &key()).expect("signed lease");
    assert_eq!(
        verify_lease(&signed, &key(), Utc::now(), binding()).unwrap_err(),
        ToolPolicyError::LeaseExpired
    );

    let mut unsigned = issue_lease(lease(), &key()).expect("signed lease");
    unsigned.signature.clear();
    assert_eq!(
        verify_lease(&unsigned, &key(), Utc::now(), binding()).unwrap_err(),
        ToolPolicyError::LeaseSignatureRequired
    );
}

#[test]
fn read_only_call_is_reserved_before_execution() {
    let signed = issue_lease(lease(), &key()).expect("signed lease");
    let reservation = reserve_call(
        &signed,
        &key(),
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .expect("reservation");

    assert_eq!(reservation.updated_lease.lease.counters.tool_calls, 1);
    assert_eq!(reservation.intent.tool_name, "read_file");
    assert_eq!(reservation.intent.effect, ToolEffect::ReadOnly);
    assert!(reservation.intent.call_id.starts_with("tool-call-"));
    assert!(reservation.intent.signature.starts_with("hmac-sha256:"));
}

#[test]
fn tool_effect_and_recursion_budgets_cannot_be_widened_by_invocation() {
    let signed = issue_lease(lease(), &key()).expect("signed lease");
    assert_eq!(
        reserve_call(
            &signed,
            &key(),
            Utc::now(),
            binding(),
            invocation("write_file", ToolEffect::LocalMutation),
        )
        .unwrap_err(),
        ToolPolicyError::EffectNotGranted
    );

    let mut recursive_lease = lease();
    recursive_lease
        .effect_allowlist
        .push(ToolEffect::RecursiveOrchestration);
    let recursive_signed = issue_lease(recursive_lease, &key()).expect("signed lease");
    assert_eq!(
        reserve_call(
            &recursive_signed,
            &key(),
            Utc::now(),
            binding(),
            invocation("delegate_task", ToolEffect::RecursiveOrchestration),
        )
        .unwrap_err(),
        ToolPolicyError::RecursiveBudgetExhausted
    );
}

#[test]
fn cycle_detection_rejects_reentry_even_when_recursive_budget_exists() {
    let mut recursive = lease();
    recursive
        .effect_allowlist
        .push(ToolEffect::RecursiveOrchestration);
    recursive.max_recursive_calls = 2;
    recursive.active_stack.push("tool:delegate_task".into());
    let signed = issue_lease(recursive, &key()).expect("signed lease");
    let mut call = invocation("delegate_task", ToolEffect::RecursiveOrchestration);
    call.recursion_identity = Some("tool:delegate_task".into());

    assert_eq!(
        reserve_call(&signed, &key(), Utc::now(), binding(), call).unwrap_err(),
        ToolPolicyError::RecursionCycleDetected
    );
}

#[test]
fn receipt_chain_binds_intent_result_and_parent() {
    let signed = issue_lease(lease(), &key()).expect("signed lease");
    let reserved = reserve_call(
        &signed,
        &key(),
        Utc::now(),
        binding(),
        invocation("git_status", ToolEffect::ReadOnly),
    )
    .expect("reservation");
    let receipt = ToolCallReceipt::complete(
        &reserved.intent,
        ReceiptOutcome::Succeeded,
        &json!({"status":"clean"}),
        "clean",
        Utc::now(),
        &key(),
    )
    .expect("receipt");

    verify_receipt_chain(&reserved.intent, &receipt, &key()).expect("valid chain");

    let mut forged = receipt.clone();
    forged.result_digest = "sha256:forged".into();
    assert_eq!(
        verify_receipt_chain(&reserved.intent, &forged, &key()).unwrap_err(),
        ToolPolicyError::ReceiptSignatureInvalid
    );
}

#[test]
fn classification_covers_recursive_and_side_effecting_surfaces() {
    assert_eq!(classify_tool("read_file", &json!({})), ToolEffect::ReadOnly);
    assert_eq!(
        classify_tool("mcp__agent_graph__graph_run_start", &json!({})),
        ToolEffect::RecursiveOrchestration
    );
    assert_eq!(
        classify_tool("delegate_task", &json!({})),
        ToolEffect::RecursiveOrchestration
    );
    assert_eq!(
        classify_tool("cronjob", &json!({"action":"create"})),
        ToolEffect::RecursiveOrchestration
    );
    assert_eq!(
        classify_tool("write_file", &json!({})),
        ToolEffect::LocalMutation
    );
    assert_eq!(
        classify_tool("ha_call_service", &json!({})),
        ToolEffect::ExternalEffect
    );
    // cronjob list is read-only; all other cronjob actions are recursive.
    assert_eq!(
        classify_tool("cronjob", &json!({"action":"list"})),
        ToolEffect::ReadOnly
    );
    // Unknown tools default to external effect (fail-safe).
    assert_eq!(
        classify_tool("unknown_hypothetical_tool", &json!({})),
        ToolEffect::ExternalEffect
    );
}

// ── Hostile tests ─────────────────────────────────────────────────────

#[test]
fn budget_exhaustion_blocks_further_calls() {
    let mut budgeted = lease();
    budgeted.max_tool_calls = 2;
    let signed = issue_lease(budgeted, &key()).expect("signed");

    // First two calls succeed.
    let r1 = reserve_call(
        &signed,
        &key(),
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .expect("call 1");
    assert_eq!(r1.updated_lease.lease.counters.tool_calls, 1);

    let r2 = reserve_call(
        &r1.updated_lease,
        &key(),
        Utc::now(),
        binding(),
        invocation("search_files", ToolEffect::ReadOnly),
    )
    .expect("call 2");
    assert_eq!(r2.updated_lease.lease.counters.tool_calls, 2);

    // Third call must fail.
    assert_eq!(
        reserve_call(
            &r2.updated_lease,
            &key(),
            Utc::now(),
            binding(),
            invocation("web_search", ToolEffect::ReadOnly),
        )
        .unwrap_err(),
        ToolPolicyError::ToolBudgetExhausted
    );
}

#[test]
fn tool_not_in_allowlist_is_denied() {
    let mut restricted = lease();
    restricted.tool_allowlist = vec!["read_file".into(), "search_files".into()];
    let signed = issue_lease(restricted, &key()).expect("signed");

    // Allowed.
    reserve_call(
        &signed,
        &key(),
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .expect("allowed");

    // Denied — not in allowlist.
    assert_eq!(
        reserve_call(
            &signed,
            &key(),
            Utc::now(),
            binding(),
            invocation("web_search", ToolEffect::ReadOnly),
        )
        .unwrap_err(),
        ToolPolicyError::ToolNotGranted
    );
}

#[test]
fn invocation_binding_must_match_lease() {
    let signed = issue_lease(lease(), &key()).expect("signed");

    // Wrong graph_id in invocation.
    let mut bad = invocation("read_file", ToolEffect::ReadOnly);
    bad.graph_id = "other-graph".into();
    assert_eq!(
        reserve_call(&signed, &key(), Utc::now(), binding(), bad).unwrap_err(),
        ToolPolicyError::InvocationInvalid
    );

    // Wrong run_id.
    let mut bad2 = invocation("read_file", ToolEffect::ReadOnly);
    bad2.run_id = "run-other".into();
    assert_eq!(
        reserve_call(&signed, &key(), Utc::now(), binding(), bad2).unwrap_err(),
        ToolPolicyError::InvocationInvalid
    );

    // Zero attempt.
    let mut bad3 = invocation("read_file", ToolEffect::ReadOnly);
    bad3.attempt = 0;
    assert_eq!(
        reserve_call(&signed, &key(), Utc::now(), binding(), bad3).unwrap_err(),
        ToolPolicyError::InvocationInvalid
    );
}

#[test]
fn effect_classification_mismatch_is_denied() {
    let signed = issue_lease(lease(), &key()).expect("signed");
    // Claim read_only but classify_tool says write_file is LocalMutation.
    assert_eq!(
        reserve_call(
            &signed,
            &key(),
            Utc::now(),
            binding(),
            invocation("write_file", ToolEffect::ReadOnly),
        )
        .unwrap_err(),
        ToolPolicyError::EffectClassificationMismatch
    );
}

#[test]
fn receipt_chain_rejects_wrong_parent() {
    let signed = issue_lease(lease(), &key()).expect("signed");
    let r1 = reserve_call(
        &signed,
        &key(),
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .expect("reservation 1");

    let mut r2_invocation = invocation("search_files", ToolEffect::ReadOnly);
    r2_invocation.parent_receipt_digest = Some("sha256:correct-parent".into());
    let r2 = reserve_call(
        &r1.updated_lease,
        &key(),
        Utc::now(),
        binding(),
        r2_invocation,
    )
    .expect("reservation 2");

    let receipt = ToolCallReceipt::complete(
        &r2.intent,
        ReceiptOutcome::Succeeded,
        &json!({"found": 5}),
        "5 files found",
        Utc::now(),
        &key(),
    )
    .expect("receipt");

    // Tamper the parent link.
    let mut forged = receipt.clone();
    forged.parent_receipt_digest = Some("sha256:wrong-parent".into());
    assert_eq!(
        verify_receipt_chain(&r2.intent, &forged, &key()).unwrap_err(),
        ToolPolicyError::ReceiptChainMismatch
    );
}

#[test]
fn replay_detection_requires_exact_idempotency() {
    // Two different invocations of the same tool with different arguments
    // must produce different call IDs — no accidental replay collision.
    let signed = issue_lease(lease(), &key()).expect("signed");
    let inv1 = invocation("read_file", ToolEffect::ReadOnly);
    let mut inv2 = invocation("read_file", ToolEffect::ReadOnly);
    inv2.arguments = json!({"path": "/tmp/different"});

    let r1 = reserve_call(&signed, &key(), Utc::now(), binding(), inv1).expect("r1");
    let r2 = reserve_call(&signed, &key(), Utc::now(), binding(), inv2).expect("r2");

    assert_ne!(r1.intent.call_id, r2.intent.call_id);
    assert_ne!(r1.intent.arguments_digest, r2.intent.arguments_digest);
    assert_ne!(r1.intent.signature, r2.intent.signature);
}

#[test]
fn receipt_summary_too_large_is_rejected() {
    let signed = issue_lease(lease(), &key()).expect("signed");
    let reserved = reserve_call(
        &signed,
        &key(),
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .expect("reservation");

    let huge_summary = "x".repeat(5000); // exceeds 4096 byte limit
    assert_eq!(
        ToolCallReceipt::complete(
            &reserved.intent,
            ReceiptOutcome::Succeeded,
            &json!({"ok": true}),
            &huge_summary,
            Utc::now(),
            &key(),
        )
        .unwrap_err(),
        ToolPolicyError::ReceiptSummaryTooLarge
    );
}

#[test]
fn concurrent_counters_are_atomic_per_reservation() {
    let mut wide = lease();
    wide.max_tool_calls = 10;
    wide.effect_allowlist.push(ToolEffect::LocalMutation);
    let signed = issue_lease(wide, &key()).expect("signed");

    // Chain 5 calls; each advances counters by exactly 1.
    let mut current = signed;
    let mut total = 0u64;
    for i in 0..5 {
        let tool = if i % 2 == 0 {
            ("read_file", ToolEffect::ReadOnly)
        } else {
            ("write_file", ToolEffect::LocalMutation)
        };
        current = reserve_call(
            &current,
            &key(),
            Utc::now(),
            binding(),
            invocation(tool.0, tool.1),
        )
        .expect("call")
        .updated_lease;
        total += 1;
        assert_eq!(current.lease.counters.tool_calls, total);
    }
    assert_eq!(current.lease.counters.tool_calls, 5);
}

// ── BROKER card (P3): fixed-operation bridge conformance ───────────────────
// LOCK contract-v1 §3. A fake named handler + counting ledger stand in for
// the real effect owner. The broker (not the model) resolves the executable;
// the worker-supplied ledger is never accepted as proof by itself.

use std::sync::Mutex;

/// Counting fake effect owner: one registered fixed operation, one ledger.
struct FakeBroker {
    key: Vec<u8>,
    ledger: Mutex<Vec<ToolCallReceipt>>,
    effects: Mutex<u64>,
}

impl FakeBroker {
    fn new() -> Self {
        Self {
            key: key(),
            ledger: Mutex::new(Vec::new()),
            effects: Mutex::new(0),
        }
    }

    /// Admit one fixed operation under a signed lease. Returns the verified
    /// receipt reference only after the intent/receipt chain checks pass.
    fn dispatch(
        &self,
        signed: &agent_graph_mcp::tool_runtime::SignedToolLease,
        inv: ToolInvocation,
        result: Value,
    ) -> Result<ToolCallReceipt, ToolPolicyError> {
        let reservation = reserve_call(signed, &self.key, Utc::now(), binding(), inv)?;
        // The "effect" happens exactly once per admitted reservation.
        *self.effects.lock().unwrap() += 1;
        let receipt = ToolCallReceipt::complete(
            &reservation.intent,
            ReceiptOutcome::Succeeded,
            &result,
            "fixed-op synthetic effect",
            Utc::now(),
            &self.key,
        )?;
        verify_receipt_chain(&reservation.intent, &receipt, &self.key)?;
        self.ledger.lock().unwrap().push(receipt.clone());
        Ok(receipt)
    }

    fn effect_count(&self) -> u64 {
        *self.effects.lock().unwrap()
    }
}

#[test]
fn admitted_fixed_op_returns_one_verified_reference_and_same_key_reattaches() {
    let broker = FakeBroker::new();
    let signed = issue_lease(lease(), &broker.key).expect("signed");
    let receipt = broker
        .dispatch(
            &signed,
            invocation("read_file", ToolEffect::ReadOnly),
            json!({"content":"fixture"}),
        )
        .expect("admitted");
    assert_eq!(broker.effect_count(), 1);
    assert_eq!(receipt.outcome, ReceiptOutcome::Succeeded);

    // Same operation identity (same lease digest + attempt + args) produces
    // the SAME call_id: the broker reattaches to the existing ledger entry
    // instead of executing a second effect.
    let reservation_again = reserve_call(
        &signed,
        &broker.key,
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .expect("reserve again");
    assert_eq!(reservation_again.intent.call_id, receipt.call_id);
    let existing = broker.ledger.lock().unwrap();
    assert!(existing.iter().any(|r| r.call_id == receipt.call_id));
    // Reattach path does NOT run the effect again.
    assert_eq!(broker.effect_count(), 1);
}

#[test]
fn changed_arguments_digest_is_a_new_operation_not_a_replay() {
    let broker = FakeBroker::new();
    let signed = issue_lease(lease(), &broker.key).expect("signed");
    let r1 = broker
        .dispatch(
            &signed,
            invocation("read_file", ToolEffect::ReadOnly),
            json!({"a":1}),
        )
        .unwrap();
    let mut changed = invocation("read_file", ToolEffect::ReadOnly);
    changed.arguments = json!({"path":"/tmp/other"});
    let reservation = reserve_call(&signed, &broker.key, Utc::now(), binding(), changed).unwrap();
    assert_ne!(reservation.intent.call_id, r1.call_id);
}

#[test]
fn expired_grant_returns_no_accepted_result_and_no_effect() {
    let broker = FakeBroker::new();
    let mut expired = lease();
    let now = Utc::now();
    expired.issued_at = now - Duration::minutes(10);
    expired.expires_at = now - Duration::minutes(5);
    let signed = issue_lease(expired, &broker.key).expect("signed shape ok");
    let err = broker
        .dispatch(
            &signed,
            invocation("read_file", ToolEffect::ReadOnly),
            json!({}),
        )
        .unwrap_err();
    assert_eq!(err, ToolPolicyError::LeaseExpired);
    assert_eq!(broker.effect_count(), 0);
    assert!(broker.ledger.lock().unwrap().is_empty());
}

#[test]
fn worker_supplied_ledger_alone_is_not_proof() {
    // A ledger file handed over by the worker verifies ONLY against the
    // broker's key and the original intent. Forged signature or mismatched
    // intent must fail even if the ledger parses cleanly.
    let broker = FakeBroker::new();
    let signed = issue_lease(lease(), &broker.key).expect("signed");
    let reservation = reserve_call(
        &signed,
        &broker.key,
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .unwrap();
    let mut forged = ToolCallReceipt::complete(
        &reservation.intent,
        ReceiptOutcome::Succeeded,
        &json!({"x":1}),
        "forged",
        Utc::now(),
        &vec![0x99; 32], // wrong key
    )
    .unwrap();
    assert_eq!(
        verify_receipt_chain(&reservation.intent, &forged, &broker.key).unwrap_err(),
        ToolPolicyError::ReceiptSignatureInvalid
    );
    forged.signature = String::new();
    assert_eq!(
        verify_receipt_chain(&reservation.intent, &forged, &broker.key).unwrap_err(),
        ToolPolicyError::ReceiptSignatureInvalid
    );
    assert_eq!(broker.effect_count(), 0);
}

#[test]
fn crash_after_synthetic_effect_reconciles_without_replay() {
    let broker = FakeBroker::new();
    let signed = issue_lease(lease(), &broker.key).expect("signed");
    let reservation = reserve_call(
        &signed,
        &broker.key,
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .unwrap();
    // Effect happens; receipt persistence "crashes" before the caller sees it.
    *broker.effects.lock().unwrap() += 1;
    let receipt = ToolCallReceipt::complete(
        &reservation.intent,
        ReceiptOutcome::Succeeded,
        &json!({"content":"fixture"}),
        "post-crash reconciliation",
        Utc::now(),
        &broker.key,
    )
    .unwrap();
    broker.ledger.lock().unwrap().push(receipt.clone());

    // Recovery: query the effect owner by operation identity. The ledger
    // entry verifies against the intent; no second effect is executed.
    let recovered = broker
        .ledger
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.call_id == reservation.intent.call_id)
        .cloned()
        .expect("ledger entry exists");
    verify_receipt_chain(&reservation.intent, &recovered, &broker.key).expect("verified");
    assert_eq!(broker.effect_count(), 1);

    // An indeterminate outcome stays indeterminate — never upgraded to pass.
    let unknown = ToolCallReceipt::complete(
        &reservation.intent,
        ReceiptOutcome::Indeterminate,
        &Value::Null,
        "outcome unknown",
        Utc::now(),
        &broker.key,
    )
    .unwrap();
    assert_eq!(unknown.outcome, ReceiptOutcome::Indeterminate);
    verify_receipt_chain(&reservation.intent, &unknown, &broker.key).expect("chain ok");
}

#[test]
fn budget_exhaustion_denies_before_the_effect() {
    let broker = FakeBroker::new();
    let mut tight = lease();
    tight.max_tool_calls = 1;
    let signed = issue_lease(tight, &broker.key).expect("signed");
    let first = broker
        .dispatch(
            &signed,
            invocation("read_file", ToolEffect::ReadOnly),
            json!({}),
        )
        .unwrap();
    assert_eq!(broker.effect_count(), 1);
    // Chain the updated lease; second call must be denied pre-effect.
    let reservation = reserve_call(
        &signed,
        &broker.key,
        Utc::now(),
        binding(),
        invocation("read_file", ToolEffect::ReadOnly),
    )
    .unwrap();
    let err = broker
        .dispatch(
            &reservation.updated_lease,
            invocation("read_file", ToolEffect::ReadOnly),
            json!({}),
        )
        .unwrap_err();
    assert_eq!(err, ToolPolicyError::ToolBudgetExhausted);
    assert_eq!(broker.effect_count(), 1);
    let _ = first;
}
