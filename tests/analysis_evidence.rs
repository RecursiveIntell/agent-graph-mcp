use agent_graph_mcp::{spec::parse_and_validate, templates};
use serde_json::json;

#[test]
fn planning_template_carries_evidence_draft_and_dissent_to_finalizer() {
    let spec = templates::instantiate("plan_critique_refine", "grounded-plan").unwrap();
    parse_and_validate(&spec).unwrap();
    let nodes = spec["nodes"].as_array().unwrap();
    let finalizer = nodes.iter().find(|n| n["id"] == "refine").unwrap();
    let context_key = finalizer["config"]["input_key"].as_str().unwrap();
    let gather = nodes
        .iter()
        .find(|n| n["config"]["output"] == context_key)
        .expect("explicit final evidence join");
    assert_eq!(gather["type"], "join");
    assert_eq!(
        gather["config"]["inputs"],
        json!(["__input__", "draft", "critique"])
    );
    let critique = nodes.iter().find(|n| n["id"] == "critique").unwrap();
    let ck = critique["config"]["input_key"].as_str().unwrap();
    let cg = nodes
        .iter()
        .find(|n| n["config"]["output"] == ck)
        .expect("explicit critic evidence join");
    assert_eq!(cg["config"]["inputs"], json!(["__input__", "draft"]));
}
