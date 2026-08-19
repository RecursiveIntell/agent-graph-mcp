use serde_json::{json, Value};

/// List all available built-in templates.
///
/// Templates marked `executable: true` have passed semantic black-box tests.
/// Templates that require unavailable capabilities are listed as unavailable
/// with a typed reason.
pub fn list() -> Value {
    json!({
      "available": [
        {
          "id": "council_deliberation",
          "version": "2",
          "description": "Three-analyst parallel council: coordinator splits work into workstreams, parallel analysts each consume their assigned workstream, join synthesizes, final report produced.",
          "params": ["name"],
          "storage_class": "server_builtin",
          "executable": true
        },
        {
          "id": "parallel_council",
          "version": "1",
          "description": "Two-perspective debate: optimist vs skeptic with judge synthesis.",
          "params": ["name"],
          "storage_class": "server_builtin",
          "executable": true
        },
        {
          "id": "plan_critique_refine",
          "version": "1",
          "description": "Sequential plan→critique→refine pipeline.",
          "params": ["name"],
          "storage_class": "server_builtin",
          "executable": true
        },
        {
          "id": "analysis_pipeline",
          "version": "1",
          "description": "Structured LLM analysis: planner→researcher→extractor→synthesizer→validator with conditional correction loop. This is model-knowledge synthesis, NOT web research or source verification.",
          "params": ["name"],
          "storage_class": "server_builtin",
          "executable": true
        },
        {
          "id": "classifier_router",
          "version": "2",
          "description": "LLM classifier routes input to category-specific handlers (bug/feature/question). Original input is preserved separately from the classification label.",
          "params": ["name"],
          "storage_class": "server_builtin",
          "executable": true
        }
      ],
      "unavailable": [
        {
          "id": "approval_gated_action",
          "reason": "requires authenticated human approval (HITL) which is not available until the operator authority subsystem is installed and verified"
        },
        {
          "id": "research_pipeline",
          "reason": "renamed to analysis_pipeline; true web research requires source-witness contracts and external tool integration not yet implemented"
        },
        {
          "id": "map_reduce",
          "reason": "requires dynamic parallel branch count from input data"
        }
      ]
    })
}

/// Instantiate a template by ID, producing a valid GraphSpec JSON.
pub fn instantiate(id: &str, name: &str) -> Result<Value, String> {
    match id {
        // ── plan_critique_refine ──────────────────────────────────────
        "plan_critique_refine" => Ok(json!({
            "spec_version": "2",
            "name": name,
            "entry": "plan",
            "output_key": "final",
            "max_iterations": 12,
            "nodes": [
                {"id": "plan", "type": "llm", "prompt": "Create a concise plan for: {input}", "config": {"output_key": "draft"}},
                {"id": "critique", "type": "llm", "prompt": "Critique this plan: {input}", "config": {"input_key": "draft", "output_key": "critique"}},
                {"id": "refine", "type": "llm", "prompt": "Refine using this critique: {input}", "config": {"input_key": "critique", "output_key": "final"}}
            ],
            "edges": [
                {"from": "plan", "to": "critique"},
                {"from": "critique", "to": "refine"},
                {"from": "refine", "to": "END"}
            ]
        })),

        // ── parallel_council (2-person debate) ────────────────────────
        "parallel_council" => Ok(json!({
            "spec_version": "2",
            "name": name,
            "entry": "fanout",
            "output_key": "decision",
            "max_iterations": 8,
            "max_parallelism": 2,
            "nodes": [
                {"id": "fanout", "type": "passthrough"},
                {"id": "optimist", "type": "llm", "prompt": "Give the strongest case for: {input}", "config": {"output_key": "optimist"}},
                {"id": "skeptic", "type": "llm", "prompt": "Give the strongest critique of: {input}", "config": {"output_key": "skeptic"}},
                {"id": "join", "type": "join", "config": {"inputs": ["optimist", "skeptic"], "output": "council", "mode": "collect_array"}},
                {"id": "judge", "type": "llm", "prompt": "Judge these ordered views and produce a decision: {input}", "config": {"input_key": "council", "output_key": "decision"}}
            ],
            "edges": [
                {"from": "fanout", "to": "optimist"},
                {"from": "fanout", "to": "skeptic"},
                {"from": "optimist", "to": "join"},
                {"from": "skeptic", "to": "join"},
                {"from": "join", "to": "judge"},
                {"from": "judge", "to": "END"}
            ]
        })),

        // ── council_deliberation (3-analyst council, fixed v2) ────────
        // FIX AG-006.4: Each analyst now consumes its assigned workstream
        // from the coordinator output, not the original input.
        "council_deliberation" => Ok(json!({
            "spec_version": "2",
            "name": name,
            "entry": "coordinator",
            "output_key": "final_report",
            "max_iterations": 16,
            "max_parallelism": 3,
            "reducers": {"workstreams": "last_write_wins"},
            "nodes": [
                {"id": "coordinator", "type": "llm", "prompt": "You are a research coordinator. Break this question into 3 distinct research workstreams. Output JSON: {\"workstreams\": [{\"id\":\"ws0\",\"query\":\"...\"}, {\"id\":\"ws1\",\"query\":\"...\"}, {\"id\":\"ws2\",\"query\":\"...\"}]}\n\nQuestion: {input}", "json_mode": true, "config": {"output_key": "workstreams"}},
                {"id": "fanout", "type": "passthrough", "config": {"input_key": "workstreams"}},
                // FIX: each analyst reads its specific workstream query via input_key
                // G4/G6: bounded output budgets (3k analysts / 7k synthesizer) and an
                // explicit output contract guard against truncation + tool-call echoes.
                {"id": "analyst_0", "type": "llm", "prompt": "Research this workstream thoroughly: {input}\n\nThe workstreams JSON is available. Find the workstream with id 'ws0' and address its query.\n\nOUTPUT CONTRACT: respond with plain markdown only — never emit tool-call JSON.", "config": {"input_key": "workstreams", "output_key": "ws0_result", "max_tokens": 3000}},
                {"id": "analyst_1", "type": "llm", "prompt": "Research this workstream thoroughly: {input}\n\nThe workstreams JSON is available. Find the workstream with id 'ws1' and address its query.\n\nOUTPUT CONTRACT: respond with plain markdown only — never emit tool-call JSON.", "config": {"input_key": "workstreams", "output_key": "ws1_result", "max_tokens": 3000}},
                {"id": "analyst_2", "type": "llm", "prompt": "Research this workstream thoroughly: {input}\n\nThe workstreams JSON is available. Find the workstream with id 'ws2' and address its query.\n\nOUTPUT CONTRACT: respond with plain markdown only — never emit tool-call JSON.", "config": {"input_key": "workstreams", "output_key": "ws2_result", "max_tokens": 3000}},
                {"id": "join", "type": "join", "config": {"inputs": ["ws0_result", "ws1_result", "ws2_result"], "output": "findings", "mode": "collect_array"}},
                {"id": "synthesize", "type": "llm", "prompt": "Synthesize these three research findings into a unified report with recommendations: {input}\n\nOUTPUT CONTRACT: respond with plain markdown only — never emit tool-call JSON.", "config": {"input_key": "findings", "output_key": "final_report", "max_tokens": 7000}}
            ],
            "edges": [
                {"from": "coordinator", "to": "fanout"},
                {"from": "fanout", "to": "analyst_0"},
                {"from": "fanout", "to": "analyst_1"},
                {"from": "fanout", "to": "analyst_2"},
                {"from": "analyst_0", "to": "join"},
                {"from": "analyst_1", "to": "join"},
                {"from": "analyst_2", "to": "join"},
                {"from": "join", "to": "synthesize"},
                {"from": "synthesize", "to": "END"}
            ]
        })),

        // ── analysis_pipeline (renamed from research_pipeline, fixed) ─
        // FIX AG-006.1: Renamed to analysis_pipeline — this is LLM knowledge
        // synthesis, NOT web research or source verification.
        // FIX AG-006.3: Validator now has conditional routing. On validation
        // failure, it routes back to synthesizer for correction. On success,
        // it routes to formatter. A loop limit is enforced by max_iterations.
        "analysis_pipeline" => Ok(json!({
            "spec_version": "2",
            "name": name,
            "entry": "planner",
            "output_key": "final",
            "max_iterations": 20,
            "nodes": [
                {"id": "planner", "type": "llm", "prompt": "Create a research plan for: {input}. Output JSON with 'steps' array.", "json_mode": true, "config": {"output_key": "plan"}},
                {"id": "researcher", "type": "llm", "prompt": "Execute this research step using your knowledge (no web access): {input}", "config": {"input_key": "plan", "output_key": "research"}},
                {"id": "extractor", "type": "llm", "prompt": "Extract key claims and evidence from: {input}", "config": {"input_key": "research", "output_key": "claims"}},
                {"id": "synthesizer", "type": "llm", "prompt": "Synthesize these claims into a coherent summary: {input}", "config": {"input_key": "claims", "output_key": "summary"}},
                {"id": "validator", "type": "llm", "prompt": "Validate this summary. Is it accurate and complete? Respond with JSON: {\"valid\": true/false, \"issues\": [...]}", "json_mode": true, "config": {"input_key": "summary", "output_key": "validation"}},
                // Conditional router: check validation.valid
                {"id": "validation_router", "type": "router", "config": {
                    "rules": [
                        {"path": "validation.valid", "op": "eq", "value": true, "targets": ["formatter"]},
                        {"path": "validation.valid", "op": "eq", "value": false, "targets": ["corrector"]}
                    ],
                    "default": ["formatter"]
                }},
                // Correction loop: feed issues back to synthesizer
                {"id": "corrector", "type": "llm", "prompt": "The validator found these issues: {input}. Revise the summary to address them. Previous claims: {claims}", "config": {"input_key": "validation", "output_key": "summary"}},
                {"id": "formatter", "type": "llm", "prompt": "Format the final output. Summary: {summary}. Validation: {validation}", "config": {"output_key": "final"}}
            ],
            "edges": [
                {"from": "planner", "to": "researcher"},
                {"from": "researcher", "to": "extractor"},
                {"from": "extractor", "to": "synthesizer"},
                {"from": "synthesizer", "to": "validator"},
                {"from": "validator", "to": "validation_router"},
                // Conditional routes
                {"from": "validation_router", "to": "formatter", "condition": "valid"},
                {"from": "validation_router", "to": "corrector", "condition": "invalid"},
                // Correction loop back to validator
                {"from": "corrector", "to": "validator"},
                {"from": "formatter", "to": "END"}
            ]
        })),

        // ── classifier_router (fixed v2) ──────────────────────────────
        // FIX AG-006.5: Classifier writes to `classification.label`, NOT
        // `__input__`. Downstream handlers receive the original input plus
        // the classification label.
        "classifier_router" => Ok(json!({
            "spec_version": "2",
            "name": name,
            "entry": "classifier",
            "output_key": "response",
            "max_iterations": 8,
            "reducers": {"original_input": "last_write_wins"},
            "nodes": [
                // Classifier writes to classification.label, preserving __input__
                {"id": "classifier", "type": "llm", "prompt": "Classify this input. Respond with exactly one word: 'bug', 'feature', or 'question'.\n\nInput: {input}", "config": {"output_key": "classification.label"}},
                {"id": "router", "type": "router", "config": {
                    "rules": [
                        {"path": "classification.label", "op": "contains", "value": "bug", "targets": ["bug_handler"]},
                        {"path": "classification.label", "op": "contains", "value": "feature", "targets": ["feature_handler"]},
                        {"path": "classification.label", "op": "contains", "value": "question", "targets": ["question_handler"]}
                    ],
                    "default": ["general_handler"]
                }},
                // Handlers receive original input (via __input__), not the label
                {"id": "bug_handler", "type": "llm", "prompt": "Analyze this bug report and suggest a fix: {input}", "config": {"output_key": "response"}},
                {"id": "feature_handler", "type": "llm", "prompt": "Evaluate this feature request: {input}", "config": {"output_key": "response"}},
                {"id": "question_handler", "type": "llm", "prompt": "Answer this question thoroughly: {input}", "config": {"output_key": "response"}},
                {"id": "general_handler", "type": "llm", "prompt": "Handle this general input: {input}", "config": {"output_key": "response"}}
            ],
            "edges": [
                {"from": "classifier", "to": "router"},
                {"from": "bug_handler", "to": "END"},
                {"from": "feature_handler", "to": "END"},
                {"from": "question_handler", "to": "END"},
                {"from": "general_handler", "to": "END"}
            ]
        })),

        // ── approval_gated_action (NOT available) ─────────────────────
        // This template is unavailable until the authenticated operator
        // authority subsystem (Phase 5) is installed and verified.
        // The human_approval node type requires HITL capability which
        // is currently not available.
        "approval_gated_action" => Err(
            "template 'approval_gated_action' is unavailable: requires authenticated human approval (HITL) which is not yet implemented. See ADR for operator authority subsystem."
                .to_string(),
        ),

        // ── legacy alias: research_pipeline → analysis_pipeline ──────
        "research_pipeline" => {
            let mut spec = instantiate("analysis_pipeline", name)?;
            if let Some(obj) = spec.as_object_mut() {
                obj.insert("name".to_string(), json!(name));
            }
            Ok(spec)
        }

        _ => Err(format!("template '{id}' is unavailable")),
    }
}

#[cfg(test)]
mod tests {
    use super::{instantiate, list};
    use crate::spec::GraphSpec;

    #[test]
    fn executable_templates_declare_explicit_terminal_outputs() {
        let catalog = list();
        let available = catalog["available"]
            .as_array()
            .expect("available templates");
        for template in available {
            if template["executable"] != true {
                continue;
            }
            let id = template["id"].as_str().expect("template id");
            let spec: GraphSpec =
                serde_json::from_value(instantiate(id, "contract-test").expect("template"))
                    .expect("valid graph spec");
            assert!(
                spec.output_key.is_some(),
                "template '{id}' must declare output_key"
            );
        }
    }

    // AG-006.1: research_pipeline is renamed to analysis_pipeline
    #[test]
    fn research_pipeline_renamed_to_analysis_pipeline() {
        let catalog = list();
        let available = catalog["available"].as_array().unwrap();
        let has_research = available
            .iter()
            .any(|t| t["id"].as_str() == Some("research_pipeline"));
        assert!(
            !has_research,
            "research_pipeline should not be in available templates"
        );
        let has_analysis = available
            .iter()
            .any(|t| t["id"].as_str() == Some("analysis_pipeline"));
        assert!(has_analysis, "analysis_pipeline should be available");
        let unavailable = catalog["unavailable"].as_array().unwrap();
        let has_research_unavail = unavailable
            .iter()
            .any(|t| t["id"].as_str() == Some("research_pipeline"));
        assert!(has_research_unavail);
    }

    // AG-006.2: approval_gated_action is unavailable
    #[test]
    fn approval_gated_action_is_unavailable() {
        let catalog = list();
        let available = catalog["available"].as_array().unwrap();
        let has_approval = available
            .iter()
            .any(|t| t["id"].as_str() == Some("approval_gated_action"));
        assert!(
            !has_approval,
            "approval_gated_action must not be listed as available/executable"
        );
        let unavailable = catalog["unavailable"].as_array().unwrap();
        let has_approval_unavail = unavailable
            .iter()
            .any(|t| t["id"].as_str() == Some("approval_gated_action"));
        assert!(
            has_approval_unavail,
            "approval_gated_action must be listed as unavailable with reason"
        );
    }

    // AG-006.3: analysis_pipeline has conditional routing for validation
    #[test]
    fn analysis_pipeline_has_conditional_validation_routing() {
        let spec = instantiate("analysis_pipeline", "test").unwrap();
        let edges = spec["edges"].as_array().unwrap();
        // Must have conditional edges (not purely linear)
        let has_condition = edges.iter().any(|e| e.get("condition").is_some());
        assert!(
            has_condition,
            "analysis_pipeline must have conditional routing for validation success/failure"
        );
        // Must have a corrector node for the correction loop
        let nodes = spec["nodes"].as_array().unwrap();
        let has_corrector = nodes.iter().any(|n| n["id"].as_str() == Some("corrector"));
        assert!(
            has_corrector,
            "analysis_pipeline must have a corrector node"
        );
    }

    // AG-006.4: council analysts consume coordinator workstreams, not original input
    #[test]
    fn council_analysts_consume_workstreams_not_input() {
        let spec = instantiate("council_deliberation", "test").unwrap();
        let nodes = spec["nodes"].as_array().unwrap();
        for analyst in ["analyst_0", "analyst_1", "analyst_2"] {
            let node = nodes
                .iter()
                .find(|n| n["id"].as_str() == Some(analyst))
                .expect(analyst);
            let input_key = node["config"]["input_key"].as_str().unwrap_or("__input__");
            assert_eq!(
                input_key, "workstreams",
                "{analyst} must read from workstreams, not __input__"
            );
        }
    }

    // AG-006.5: classifier writes to classification.label, not __input__
    #[test]
    fn classifier_writes_to_label_not_input() {
        let spec = instantiate("classifier_router", "test").unwrap();
        let nodes = spec["nodes"].as_array().unwrap();
        let classifier = nodes
            .iter()
            .find(|n| n["id"].as_str() == Some("classifier"))
            .expect("classifier node");
        let output_key = classifier["config"]["output_key"]
            .as_str()
            .expect("output_key");
        assert_ne!(
            output_key, "__input__",
            "classifier must NOT write to __input__"
        );
        assert_eq!(
            output_key, "classification.label",
            "classifier should write to classification.label"
        );
    }

    // Legacy alias: research_pipeline still instantiates (as analysis_pipeline)
    #[test]
    fn research_pipeline_legacy_alias_works() {
        let result = instantiate("research_pipeline", "legacy-test");
        assert!(
            result.is_ok(),
            "research_pipeline should still instantiate as legacy alias"
        );
        let spec = result.unwrap();
        assert_eq!(spec["spec_version"], "2");
    }
}
