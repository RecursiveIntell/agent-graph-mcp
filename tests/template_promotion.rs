use agent_graph_mcp::promotion::{
    verify_canonical_receipt, OperatorReceipt, PromotionError, PromotionStore,
    TemplateCandidateState, TemplateOutcome,
};

fn outcome(run_id: &str, disposition: &str) -> TemplateOutcome {
    TemplateOutcome {
        template_id: "template-a".into(),
        run_id: run_id.into(),
        terminal_receipt_id: format!("receipt-{run_id}"),
        receipt_digest: format!("digest-{run_id}"),
        disposition: disposition.into(),
        evidence_digest: format!("evidence-{run_id}"),
    }
}

fn eligible_store(suffix: &str) -> PromotionStore {
    let mut store = PromotionStore::new("template-a", "spec-1", "graph-a", "v1");
    for run in ["one", "two", "three"] {
        store
            .add_outcome(outcome(&format!("{suffix}-{run}"), "good"))
            .unwrap();
    }
    store
}

#[test]
fn three_good_outcomes_need_operator_authority() {
    let mut store = eligible_store("good");
    assert!(store.eligible());
    assert_eq!(store.state, TemplateCandidateState::Candidate);
    assert_eq!(
        store.promote_template(OperatorReceipt {
            operator_id: "user".into(),
            nonce: "n-good-unauth".into(),
            authenticated: false
        }),
        Err(PromotionError::Unauthorized)
    );
    assert_eq!(store.state, TemplateCandidateState::Candidate);
    store
        .promote_template(OperatorReceipt {
            operator_id: "operator:alice".into(),
            nonce: "n-good".into(),
            authenticated: true,
        })
        .unwrap();
    assert_eq!(store.state, TemplateCandidateState::Approved);
}

#[test]
fn bad_outcome_quarantines_candidate() {
    let mut store = PromotionStore::new("template-a", "spec-1", "graph-a", "v1");
    store.add_outcome(outcome("bad-run", "bad")).unwrap();
    assert_eq!(store.state, TemplateCandidateState::Quarantined);
    assert!(!store.eligible());
}

#[test]
fn mismatched_and_nonterminal_receipts_are_rejected() {
    let mismatch = verify_canonical_receipt(
        "run", "receipt", "graph", "v1", "template", "spec", "other", "receipt", "graph", "v1",
        "template", "spec", true,
    );
    assert_eq!(
        mismatch,
        Err(PromotionError::ReceiptMismatch("run_id".into()))
    );
    let nonterminal = verify_canonical_receipt(
        "run", "receipt", "graph", "v1", "template", "spec", "run", "receipt", "graph", "v1",
        "template", "spec", false,
    );
    assert_eq!(nonterminal, Err(PromotionError::NonTerminal));
}

#[test]
fn duplicate_run_ids_do_not_increase_eligibility() {
    let mut store = PromotionStore::new("template-a", "spec-1", "graph-a", "v1");
    store.add_outcome(outcome("same", "good")).unwrap();
    store.add_outcome(outcome("same", "good")).unwrap();
    assert_eq!(store.outcomes.len(), 1);
    assert!(!store.eligible());
}

#[test]
fn nonce_replay_is_rejected() {
    let mut first = eligible_store("nonce-one");
    first
        .promote_template(OperatorReceipt {
            operator_id: "operator:alice".into(),
            nonce: "replay-nonce".into(),
            authenticated: true,
        })
        .unwrap();
    let mut second = eligible_store("nonce-two");
    assert_eq!(
        second.promote_template(OperatorReceipt {
            operator_id: "operator:alice".into(),
            nonce: "replay-nonce".into(),
            authenticated: true
        }),
        Err(PromotionError::AuthorizationReplayed)
    );
}

#[test]
fn cloned_store_preserves_restart_state() {
    let mut store = eligible_store("restart");
    store
        .promote_template(OperatorReceipt {
            operator_id: "operator:alice".into(),
            nonce: "restart-nonce".into(),
            authenticated: true,
        })
        .unwrap();
    let restarted = store.clone();
    assert_eq!(restarted.state, TemplateCandidateState::Approved);
    assert_eq!(restarted.outcomes, store.outcomes);
    assert_eq!(restarted.decisions, vec!["restart-nonce"]);
}
