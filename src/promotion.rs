//! Canonical, operator-authorized template promotion.
//! Outcome evidence is advisory; only an authenticated operator receipt can approve.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCandidateState {
    LegacyUnverified,
    Candidate,
    Quarantined,
    Approved,
    Revoked,
}

impl TemplateCandidateState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyUnverified => "legacy_unverified",
            Self::Candidate => "candidate",
            Self::Quarantined => "quarantined",
            Self::Approved => "approved",
            Self::Revoked => "revoked",
        }
    }
    pub fn can_transition(self, to: Self) -> bool {
        match self {
            Self::LegacyUnverified => to == Self::Quarantined,
            Self::Candidate => matches!(to, Self::Candidate | Self::Quarantined | Self::Approved),
            Self::Approved => matches!(to, Self::Approved | Self::Quarantined | Self::Revoked),
            Self::Quarantined => to == Self::Candidate,
            Self::Revoked => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateOutcome {
    pub template_id: String,
    pub run_id: String,
    pub terminal_receipt_id: String,
    pub receipt_digest: String,
    pub disposition: String,
    pub evidence_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorReceipt {
    pub operator_id: String,
    pub nonce: String,
    pub authenticated: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionError {
    Unauthorized,
    AuthorizationReplayed,
    NotFound,
    InvalidTransition,
    NotEligible,
    ReceiptMismatch(String),
    NonTerminal,
}

static NONCES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
fn nonces() -> &'static Mutex<HashSet<String>> {
    NONCES.get_or_init(|| Mutex::new(HashSet::new()))
}

#[derive(Debug, Clone)]
pub struct PromotionStore {
    pub template_id: String,
    pub spec_digest: String,
    pub graph_id: String,
    pub graph_version: String,
    pub state: TemplateCandidateState,
    pub outcomes: Vec<TemplateOutcome>,
    pub decisions: Vec<String>,
}
impl PromotionStore {
    pub fn new(template_id: &str, spec_digest: &str, graph_id: &str, graph_version: &str) -> Self {
        Self {
            template_id: template_id.into(),
            spec_digest: spec_digest.into(),
            graph_id: graph_id.into(),
            graph_version: graph_version.into(),
            state: TemplateCandidateState::Candidate,
            outcomes: vec![],
            decisions: vec![],
        }
    }
    pub fn add_outcome(&mut self, o: TemplateOutcome) -> Result<(), PromotionError> {
        if o.template_id != self.template_id {
            return Err(PromotionError::ReceiptMismatch("template_id".into()));
        }
        if self.outcomes.iter().any(|x| x.run_id == o.run_id) {
            return Ok(());
        }
        if o.disposition.eq_ignore_ascii_case("bad")
            || o.disposition.eq_ignore_ascii_case("failed")
            || o.disposition.eq_ignore_ascii_case("contradicted")
        {
            self.state = TemplateCandidateState::Quarantined;
        }
        self.outcomes.push(o);
        Ok(())
    }
    pub fn eligible(&self) -> bool {
        self.state == TemplateCandidateState::Candidate
            && self
                .outcomes
                .iter()
                .filter(|o| {
                    o.disposition.eq_ignore_ascii_case("good")
                        || o.disposition.eq_ignore_ascii_case("supported")
                })
                .count()
                >= 3
    }
    pub fn promote_template(&mut self, receipt: OperatorReceipt) -> Result<(), PromotionError> {
        if !receipt.authenticated || !receipt.operator_id.starts_with("operator:") {
            return Err(PromotionError::Unauthorized);
        }
        let mut used = nonces().lock().unwrap();
        if !used.insert(receipt.nonce.clone()) {
            return Err(PromotionError::AuthorizationReplayed);
        }
        if !self.eligible() {
            return Err(PromotionError::NotEligible);
        }
        if !self.state.can_transition(TemplateCandidateState::Approved) {
            return Err(PromotionError::InvalidTransition);
        }
        self.state = TemplateCandidateState::Approved;
        self.decisions.push(receipt.nonce);
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct ReceiptIdentity<'a> {
    pub run_id: &'a str,
    pub receipt_digest: &'a str,
    pub graph_id: &'a str,
    pub graph_version: &'a str,
    pub template_id: &'a str,
    pub spec_digest: &'a str,
}

pub fn verify_canonical_receipt(
    expected: &ReceiptIdentity<'_>,
    actual: &ReceiptIdentity<'_>,
    terminal: bool,
) -> Result<(), PromotionError> {
    for (name, a, b) in [
        ("run_id", expected.run_id, actual.run_id),
        (
            "receipt_digest",
            expected.receipt_digest,
            actual.receipt_digest,
        ),
        ("graph_id", expected.graph_id, actual.graph_id),
        (
            "graph_version",
            expected.graph_version,
            actual.graph_version,
        ),
        ("template_id", expected.template_id, actual.template_id),
        ("spec_digest", expected.spec_digest, actual.spec_digest),
    ] {
        if a != b {
            return Err(PromotionError::ReceiptMismatch(name.into()));
        }
    }
    if !terminal {
        return Err(PromotionError::NonTerminal);
    }
    Ok(())
}
pub fn evidence_digest(outcomes: &[TemplateOutcome]) -> String {
    let mut h = Sha256::new();
    for o in outcomes {
        h.update(format!(
            "{}:{}:{}:{}:{}:{}\n",
            o.template_id,
            o.run_id,
            o.terminal_receipt_id,
            o.receipt_digest,
            o.disposition,
            o.evidence_digest
        ));
    }
    format!("{:x}", h.finalize())
}
