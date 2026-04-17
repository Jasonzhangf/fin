use fin_contracts::TaskStatus;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("invalid task transition from {from:?} to {to:?}")]
    Invalid { from: TaskStatus, to: TaskStatus },
}

pub fn advance_task(from: TaskStatus, to: TaskStatus) -> Result<TaskStatus, TransitionError> {
    let valid = matches!(
        (&from, &to),
        (TaskStatus::Created, TaskStatus::Dispatched)
            | (TaskStatus::Dispatched, TaskStatus::Accepted)
            | (TaskStatus::Accepted, TaskStatus::Running)
            | (TaskStatus::Running, TaskStatus::Claimed)
            | (TaskStatus::Claimed, TaskStatus::Verified)
            | (TaskStatus::Verified, TaskStatus::Closed)
    );

    if valid {
        Ok(to)
    } else {
        Err(TransitionError::Invalid { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transition_advances() {
        let result = advance_task(TaskStatus::Created, TaskStatus::Dispatched);
        assert_eq!(result, Ok(TaskStatus::Dispatched));
    }

    #[test]
    fn invalid_transition_fails() {
        let result = advance_task(TaskStatus::Created, TaskStatus::Closed);
        assert_eq!(
            result,
            Err(TransitionError::Invalid {
                from: TaskStatus::Created,
                to: TaskStatus::Closed,
            })
        );
    }
}