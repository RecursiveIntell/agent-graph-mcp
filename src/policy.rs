use crate::spec::{GraphSpec, MAX_INPUT_BYTES, MAX_ITERATIONS, MAX_OUTPUT_BYTES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyFinding {
    pub code: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyReport {
    pub substantive: bool,
    pub findings: Vec<PolicyFinding>,
}

/// Non-authoritative admission preflight. It never grants authorization.
pub fn preflight(spec: &GraphSpec, provider: &str) -> PolicyReport {
    let mut findings = Vec::new();
    if spec.spec_version != "2" {
        findings.push(PolicyFinding {
            code: "GRAPH_VERSION_UNSUPPORTED",
            detail: spec.spec_version.clone(),
        });
    }
    if spec.max_iterations.unwrap_or(MAX_ITERATIONS) > MAX_ITERATIONS {
        findings.push(PolicyFinding {
            code: "BUDGET_EXCEEDED",
            detail: "max_iterations".into(),
        });
    }
    if provider.trim().is_empty() {
        findings.push(PolicyFinding {
            code: "PROVIDER_DESTINATION_MISSING",
            detail: "provider destination is empty".into(),
        });
    }
    if spec
        .nodes
        .iter()
        .any(|n| n.max_tokens.unwrap_or(0) > MAX_OUTPUT_BYTES)
    {
        findings.push(PolicyFinding {
            code: "OUTPUT_BUDGET_EXCEEDED",
            detail: "max_tokens".into(),
        });
    }
    let _ = MAX_INPUT_BYTES;
    for node in &spec.nodes {
        if let Err(error) = GraphSpec::executable_node_type(&node.node_type) {
            findings.push(PolicyFinding {
                code: "UNSUPPORTED_NODE_TYPE",
                detail: error,
            });
        }
        if node.evidence_required {
            findings.push(PolicyFinding {
                code: "WITNESS_REQUIREMENT",
                detail: node.id.clone(),
            });
        }
    }
    PolicyReport {
        substantive: true,
        findings,
    }
}
