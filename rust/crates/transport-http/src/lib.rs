#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpTransportConfig {
    pub bind_addr: String,
    pub ws_path: String,
}

impl Default for HttpTransportConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".into(),
            ws_path: "/events".into(),
        }
    }
}
