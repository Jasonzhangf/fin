pub fn is_hidden_session_source(source: &str) -> bool {
    source.starts_with("framework.resume_checkpoint")
        || source.starts_with("framework.owner_loop.")
        || source.starts_with("framework.assignment_runtime")
        || source.starts_with("project.resume_checkpoint")
        || source == "project.resume"
        || source == "project.assignment"
        || source == "daemon_headless"
        || source.starts_with("daemon_headless_")
}

pub fn uses_ephemeral_session_persistence(source: &str) -> bool {
    is_hidden_session_source(source)
}

#[cfg(test)]
mod tests {
    use super::{is_hidden_session_source, uses_ephemeral_session_persistence};

    #[test]
    fn control_plane_sources_use_ephemeral_session_persistence() {
        for source in [
            "framework.resume_checkpoint.wait",
            "framework.owner_loop.dispatch",
            "framework.assignment_runtime",
            "project.resume_checkpoint.user",
            "project.resume",
            "project.assignment",
            "daemon_headless",
            "daemon_headless_project_resume",
        ] {
            assert!(
                is_hidden_session_source(source),
                "{source} should be hidden"
            );
            assert!(
                uses_ephemeral_session_persistence(source),
                "{source} should use ephemeral persistence"
            );
        }
    }
}
