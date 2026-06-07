pub fn is_hidden_session_source(source: &str) -> bool {
    source.starts_with("framework.resume_checkpoint")
        || source.starts_with("framework.owner_loop.")
        || source.starts_with("framework.task_kickoff.")
        || source.starts_with("framework.assignment_runtime")
        || source.starts_with("project.resume_checkpoint")
        || source == "project.resume"
        || source == "project.assignment"
        || source == "daemon_headless"
        || source.starts_with("daemon_headless_")
}

pub fn suppresses_session_result_history(source: &str) -> bool {
    source.starts_with("framework.resume_checkpoint") && !source.contains("wait_reminder_resume")
}

#[cfg(test)]
mod tests {
    use super::{is_hidden_session_source, suppresses_session_result_history};

    #[test]
    fn control_plane_sources_use_ephemeral_session_persistence() {
        for source in [
            "framework.resume_checkpoint.wait",
            "framework.owner_loop.dispatch",
            "framework.task_kickoff.plan",
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
        }
    }

    #[test]
    fn only_plain_resume_checkpoint_suppresses_result_history() {
        assert!(suppresses_session_result_history(
            "framework.resume_checkpoint.wait"
        ));
        assert!(!suppresses_session_result_history(
            "framework.resume_checkpoint.wait_reminder_resume"
        ));
        assert!(!suppresses_session_result_history(
            "framework.owner_loop.review_submitted_task"
        ));
        assert!(!suppresses_session_result_history(
            "framework.task_kickoff.plan"
        ));
    }
}
