use fin_contracts::ExecutionStateRecord;
use fin_debug_server::ChatSendRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChatDisposition {
    StatusProbe,
    InterruptRequest { message: String },
    Queue { reason: String },
    RunNow,
}

pub(crate) fn classify_request(
    request: &ChatSendRequest,
    state: Option<&ExecutionStateRecord>,
) -> ChatDisposition {
    if request.is_status_probe() {
        return ChatDisposition::StatusProbe;
    }

    if request
        .input_kind
        .as_deref()
        .is_some_and(|kind| kind == "interrupt_request")
    {
        let message = request.message.trim();
        return if message.is_empty() {
            ChatDisposition::Queue {
                reason: "interrupt_request_missing_message".into(),
            }
        } else {
            ChatDisposition::InterruptRequest {
                message: message.to_string(),
            }
        };
    }

    if let Some(message) = request.message.trim().strip_prefix("/interrupt") {
        let message = message.trim();
        return if message.is_empty() {
            ChatDisposition::Queue {
                reason: "interrupt_request_missing_message".into(),
            }
        } else {
            ChatDisposition::InterruptRequest {
                message: message.into(),
            }
        };
    }

    if let Some(state) = state {
        if matches!(
            state.status.as_str(),
            "paused" | "running" | "waiting_external"
        ) {
            return ChatDisposition::Queue {
                reason: state
                    .reason
                    .clone()
                    .unwrap_or_else(|| format!("state={}", state.status)),
            };
        }
    }

    ChatDisposition::RunNow
}
