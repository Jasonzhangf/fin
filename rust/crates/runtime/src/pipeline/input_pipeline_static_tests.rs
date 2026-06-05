#[test]
fn input_pipeline_uses_unique_node_type_names_and_adjacent_builders() {
    let pipeline = include_str!("input_pipeline.rs");
    for node in [
        "InputIn01ChannelRaw",
        "InputIn02Normalized",
        "InputIn03Operation",
        "InputIn04SessionBound",
        "InputIn05ReasoningSeed",
    ] {
        assert!(
            pipeline.contains(&format!("struct {node}")),
            "missing unique input pipeline node {node}"
        );
    }

    for owner in [
        "InputIn02NormalizedBuilder",
        "InputIn03OperationBuilder",
        "InputIn04SessionBoundBuilder",
        "InputIn05ReasoningSeedBuilder",
    ] {
        assert!(
            pipeline.contains(owner),
            "missing owning input builder {owner}"
        );
    }

    let runtime = include_str!("../lib.rs");
    assert!(
        runtime.contains("InputIn01ChannelRaw")
            && runtime.contains("InputIn05ReasoningSeedBuilder.build(session_bound)?"),
        "InferenceOperationBuilder must enter InputIn01 and exit through InputIn05"
    );
}

#[test]
fn input_pipeline_has_no_from_conversions_or_intermediate_numbering() {
    let pipeline = include_str!("input_pipeline.rs");
    assert!(
        !pipeline.contains("impl From<"),
        "InputIn chain must use owning builders, not scattered From conversions"
    );
    for forbidden in ["InputIn03a", "InputIn03_", "InputIn03.", "InputInV2"] {
        assert!(
            !pipeline.contains(forbidden),
            "forbidden input chain numbering {forbidden}"
        );
    }
}
