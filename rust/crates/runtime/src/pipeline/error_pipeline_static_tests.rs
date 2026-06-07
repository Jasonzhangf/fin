#[test]
fn error_pipeline_uses_unique_node_type_names_and_no_swallow() {
    let pipeline = include_str!("error_pipeline.rs");
    for node in [
        "ErrorErr01Detected",
        "ErrorErr02SourceClassified",
        "ErrorErr03RuntimeClassified",
        "ErrorErr04SessionRecorded",
        "ErrorErr05UserVisible",
    ] {
        assert!(
            pipeline.contains(&format!("struct {node}")),
            "missing unique error pipeline node {node}"
        );
    }
    for owner in [
        "ErrorErr01DetectedBuilder",
        "ErrorErr02SourceClassifiedBuilder",
        "ErrorErr03RuntimeClassifiedBuilder",
        "ErrorErr04SessionRecordedBuilder",
        "ErrorErr05UserVisibleBuilder",
    ] {
        assert!(
            pipeline.contains(owner),
            "missing owning error builder {owner}"
        );
    }
    assert!(
        !pipeline.contains("treat_invalid_as_success") && !pipeline.contains("fallback_ok"),
        "error chain must not silently swallow or fall back to success"
    );
}

#[test]
fn error_pipeline_classifies_source_and_runtime_decision() {
    let pipeline = include_str!("error_pipeline.rs");
    for source in ["Input", "Provider", "Model", "Tool", "Runtime", "Channel"] {
        assert!(
            pipeline.contains(source),
            "ErrorErr02 source class missing {source}"
        );
    }
    for forbidden in ["ErrorErr03a", "ErrorErr04_", "ErrorErr04.", "ErrorErrV2"] {
        assert!(
            !pipeline.contains(forbidden),
            "forbidden error chain numbering {forbidden}"
        );
    }
}
