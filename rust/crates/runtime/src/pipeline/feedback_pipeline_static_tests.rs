#[test]
fn feedback_pipeline_uses_unique_node_type_names_and_adjacent_parsers() {
    let pipeline = include_str!("feedback_pipeline.rs");
    for node in [
        "FeedbackResp01ModelRaw",
        "FeedbackResp02TaggedBlocks",
        "FeedbackResp03UserVisible",
        "FeedbackResp04ControlFeedback",
        "FeedbackResp05ToolIntent",
        "FeedbackResp06SessionMaterialized",
        "FeedbackResp07ChannelRender",
    ] {
        assert!(
            pipeline.contains(&format!("struct {node}")),
            "missing unique feedback pipeline node {node}"
        );
    }
    for owner in [
        "FeedbackResp02TaggedBlocksParser",
        "FeedbackResp03UserVisibleBuilder",
        "FeedbackResp04ControlFeedbackParser",
        "FeedbackResp05ToolIntentParser",
        "FeedbackResp06SessionMaterializedBuilder",
        "FeedbackResp07ChannelRenderBuilder",
    ] {
        assert!(
            pipeline.contains(owner),
            "missing owning feedback builder/parser {owner}"
        );
    }
}

#[test]
fn feedback_pipeline_rejects_silent_invalid_and_no_fallback_truth() {
    let pipeline = include_str!("feedback_pipeline.rs");
    assert!(
        !pipeline.contains("impl From<"),
        "Feedback chain must use owning parsers/builders, not From conversions"
    );
    for forbidden in [
        "FeedbackResp03a",
        "FeedbackResp04_",
        "FeedbackResp04.",
        "FeedbackRespV2",
    ] {
        assert!(
            !pipeline.contains(forbidden),
            "forbidden feedback chain numbering {forbidden}"
        );
    }
    assert!(
        !pipeline.contains("fallback_ok") && !pipeline.contains("treat_invalid_as_success"),
        "Feedback chain must not silently salvage invalid control/tool blocks"
    );
}
