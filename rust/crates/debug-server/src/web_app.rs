pub fn javascript_for_path(path: &str) -> Option<&'static str> {
    match path {
        "/app.js" => Some(include_str!("../webui/dist/app.js")),
        "/chat.js" => Some(include_str!("../webui/dist/chat.js")),
        "/inspector.js" => Some(include_str!("../webui/dist/inspector.js")),
        "/tree.js" => Some(include_str!("../webui/dist/tree.js")),
        "/types.js" => Some(include_str!("../webui/dist/types.js")),
        _ => None,
    }
}