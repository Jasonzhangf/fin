pub fn is_hidden_session_source(source: &str) -> bool {
    source.starts_with("framework.resume_checkpoint")
        || source.starts_with("framework.owner_loop.")
        || source.starts_with("framework.task_kickoff.")
        || source.starts_with("project.resume_checkpoint")
        || source == "project.assignment"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_sources_are_correctly_identified() {
        // framework resume_checkpoint
        assert!(is_hidden_session_source(
            "framework.resume_checkpoint.tool_wait"
        ));
        assert!(is_hidden_session_source("framework.resume_checkpoint"));

        // framework owner_loop
        assert!(is_hidden_session_source("framework.owner_loop.dispatch"));
        assert!(is_hidden_session_source("framework.owner_loop.review"));

        // framework task_kickoff
        assert!(is_hidden_session_source("framework.task_kickoff.formalize"));

        // project resume_checkpoint
        assert!(is_hidden_session_source(
            "project.resume_checkpoint.anything"
        ));

        // project assignment (exact match)
        assert!(is_hidden_session_source("project.assignment"));
    }

    #[test]
    fn normal_sources_are_not_hidden() {
        assert!(!is_hidden_session_source("cli.user"));
        assert!(!is_hidden_session_source("cli-session"));
        assert!(!is_hidden_session_source("qqbot.inbound"));
        assert!(!is_hidden_session_source("web.user"));
        assert!(!is_hidden_session_source("heartbeat_probe"));
        assert!(!is_hidden_session_source("supervisor_heartbeat_due"));
        assert!(!is_hidden_session_source("channel.peer"));
    }

    #[test]
    fn partial_prefix_matches_are_not_hidden() {
        // Must not accidentally match partial prefixes
        assert!(!is_hidden_session_source("framework.resume"));
        assert!(!is_hidden_session_source("framework.owner"));
        assert!(!is_hidden_session_source("framework.task"));
        assert!(!is_hidden_session_source("project.resume"));
        assert!(!is_hidden_session_source("project.assignment_extra"));
    }
}
