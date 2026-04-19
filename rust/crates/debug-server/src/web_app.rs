pub fn javascript_for_path(path: &str) -> Option<&'static str> {
    match path {
        "/app.js" => Some(include_str!("../webui/dist/app.js")),
        "/app_ui.js" => Some(include_str!("../webui/dist/app_ui.js")),
        "/chat.js" => Some(include_str!("../webui/dist/chat.js")),
        "/event_ledger.js" => Some(include_str!("../webui/dist/event_ledger.js")),
        "/event_ledger_links.js" => Some(include_str!("../webui/dist/event_ledger_links.js")),
        "/event_ledger_state.js" => Some(include_str!("../webui/dist/event_ledger_state.js")),
        "/event_ledger_summary.js" => Some(include_str!("../webui/dist/event_ledger_summary.js")),
        "/event_ledger_view_state.js" => {
            Some(include_str!("../webui/dist/event_ledger_view_state.js"))
        }
        "/focus.js" => Some(include_str!("../webui/dist/focus.js")),
        "/inspector.js" => Some(include_str!("../webui/dist/inspector.js")),
        "/section_renderers.js" => Some(include_str!("../webui/dist/section_renderers.js")),
        "/sidebar.js" => Some(include_str!("../webui/dist/sidebar.js")),
        "/time.js" => Some(include_str!("../webui/dist/time.js")),
        "/tree.js" => Some(include_str!("../webui/dist/tree.js")),
        "/types.js" => Some(include_str!("../webui/dist/types.js")),
        _ => None,
    }
}
