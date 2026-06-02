#[test]
fn provider_hub_uses_unique_node_type_names_and_no_generic_alias() {
    let pipeline = include_str!("hub_pipeline.rs");
    for node in [
        "HubReq01Inbound",
        "HubReq02Process",
        "HubReq03Outbound",
        "HubResp04Inbound",
        "HubResp05Process",
        "HubResp06Outbound",
    ] {
        assert!(
            pipeline.contains(&format!("struct {node}")),
            "missing unique provider hub node {node}"
        );
    }
    let lib = include_str!("lib.rs");
    assert!(
        !lib.contains("struct PreparedRequestBuilder"),
        "no generic PreparedRequestBuilder alias allowed"
    );
    assert!(
        pipeline.contains("ProviderRequest") || pipeline.contains("PreparedRequest"),
        "Hub chain must keep PreparedRequest as wire body"
    );
}

#[test]
fn provider_hub_has_no_from_conversions_or_intermediate_numbering() {
    let pipeline = include_str!("hub_pipeline.rs");
    assert!(
        !pipeline.contains("impl From<"),
        "Hub chain must not have scattered From conversions"
    );
    for forbidden in ["HubReq02a", "HubReq02_", "HubReq02.", "HubRespV2"] {
        assert!(
            !pipeline.contains(forbidden),
            "forbidden provider hub numbering {forbidden}"
        );
    }
}
