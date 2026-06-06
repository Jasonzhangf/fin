use crate::*;
use crate::model::parser::ModelToolCall;
use fin_contracts::ControlFeedback;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::model::parser::{parse_control_feedback, parse_tool_calls};
use crate::model::shapes::{extract_tag, strip_structured_blocks};

pub(crate) const USER_RESPONSE_TAG: &str = "fin_user_response";
pub(crate) const CONTROL_FEEDBACK_TAG: &str = "fin_control_feedback";
pub(crate) const TOOL_CALLS_TAG: &str = "fin_tool_calls";
pub(crate) const KNOWN_TAGS: &[&str] = &[
    USER_RESPONSE_TAG,
    CONTROL_FEEDBACK_TAG,
    TOOL_CALLS_TAG,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp01ModelRaw {
    pub raw_text: String,
    pub contract_detected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp02TaggedBlocks {
    pub raw: FeedbackResp01ModelRaw,
    pub user_response_block: Option<String>,
    pub user_response_repaired: bool,
    pub control_feedback_block: Option<String>,
    pub control_feedback_repaired: bool,
    pub tool_calls_block: Option<String>,
    pub tool_calls_repaired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp03UserVisible {
    pub tagged: FeedbackResp02TaggedBlocks,
    pub user_response_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp04ControlFeedback {
    pub tagged: FeedbackResp02TaggedBlocks,
    pub control_feedback: Option<ControlFeedback>,
    pub salvaged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp05ToolIntent {
    pub tagged: FeedbackResp02TaggedBlocks,
    pub tool_calls: Vec<ModelToolCall>,
    pub tool_calls_block_present: bool,
    pub tool_calls_parse_status: String,
    pub tool_calls_invalid_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp06SessionMaterialized {
    pub user_visible: FeedbackResp03UserVisible,
    pub control_feedback: FeedbackResp04ControlFeedback,
    pub tool_intent: FeedbackResp05ToolIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackResp07ChannelRender {
    pub material: FeedbackResp06SessionMaterialized,
    pub render_dto: Value,
}

#[derive(Debug, Default)]
pub struct FeedbackResp02TaggedBlocksParser;

impl FeedbackResp02TaggedBlocksParser {
    pub fn parse(&self, raw: FeedbackResp01ModelRaw) -> FeedbackResp02TaggedBlocks {
        let text = raw.raw_text.clone();
        let (user_resp, user_repaired) = extract_tag_full(&text, "fin_user_response");
        let (control_block, control_repaired) =
            extract_tag_full(&text, "fin_control_feedback");
        let (tool_block, tool_repaired) = extract_tag_full(&text, "fin_tool_calls");
        FeedbackResp02TaggedBlocks {
            raw,
            user_response_block: user_resp,
            user_response_repaired: user_repaired,
            control_feedback_block: control_block,
            control_feedback_repaired: control_repaired,
            tool_calls_block: tool_block,
            tool_calls_repaired: tool_repaired,
        }
    }
}

#[derive(Debug, Default)]
pub struct FeedbackResp03UserVisibleBuilder;

impl FeedbackResp03UserVisibleBuilder {
    pub fn build(
        &self,
        tagged: FeedbackResp02TaggedBlocks,
    ) -> FeedbackResp03UserVisible {
        let stripped = strip_known_blocks(&tagged.raw.raw_text);
        let text = tagged
            .user_response_block
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(stripped);
        FeedbackResp03UserVisible {
            tagged,
            user_response_text: text,
        }
    }
}

#[derive(Debug, Default)]
pub struct FeedbackResp04ControlFeedbackParser;

impl FeedbackResp04ControlFeedbackParser {
    pub fn parse(
        &self,
        tagged: FeedbackResp02TaggedBlocks,
    ) -> FeedbackResp04ControlFeedback {
        let parsed = tagged
            .control_feedback_block
            .as_deref()
            .and_then(parse_control_feedback_block);
        let (control_feedback, salvaged) = match parsed {
            Some(result) => (Some(result.feedback), result.salvaged),
            None => (None, false),
        };
        FeedbackResp04ControlFeedback {
            tagged,
            control_feedback,
            salvaged,
        }
    }
}

#[derive(Debug, Default)]
pub struct FeedbackResp05ToolIntentParser;

impl FeedbackResp05ToolIntentParser {
    pub fn parse(&self, tagged: FeedbackResp02TaggedBlocks) -> FeedbackResp05ToolIntent {
        let parsed = tagged
            .tool_calls_block
            .as_deref()
            .map(parse_tool_calls_block)
            .unwrap_or_else(default_tool_calls);
        FeedbackResp05ToolIntent {
            tagged,
            tool_calls: parsed.calls,
            tool_calls_block_present: parsed.block_present,
            tool_calls_parse_status: parsed.parse_status,
            tool_calls_invalid_reason: parsed.invalid_reason,
        }
    }
}

#[derive(Debug, Default)]
pub struct FeedbackResp06SessionMaterializedBuilder;

impl FeedbackResp06SessionMaterializedBuilder {
    pub fn build(
        &self,
        user_visible: FeedbackResp03UserVisible,
        control_feedback: FeedbackResp04ControlFeedback,
        tool_intent: FeedbackResp05ToolIntent,
    ) -> FeedbackResp06SessionMaterialized {
        FeedbackResp06SessionMaterialized {
            user_visible,
            control_feedback,
            tool_intent,
        }
    }
}

#[derive(Debug, Default)]
pub struct FeedbackResp07ChannelRenderBuilder;

impl FeedbackResp07ChannelRenderBuilder {
    pub fn build(
        &self,
        material: FeedbackResp06SessionMaterialized,
    ) -> FeedbackResp07ChannelRender {
        let render_dto = serde_json::json!({
            "user_response": material.user_visible.user_response_text,
            "contract_detected": material.user_visible.tagged.raw.contract_detected,
            "tool_calls_present": material.tool_intent.tool_calls_block_present,
            "tool_calls_status": material.tool_intent.tool_calls_parse_status,
            "control_feedback_origin": material
                .control_feedback
                .control_feedback
                .as_ref()
                .map(|cf| cf.origin.clone()),
        });
        FeedbackResp07ChannelRender {
            material,
            render_dto,
        }
    }
}

fn extract_tag_full(raw: &str, tag: &str) -> (Option<String>, bool) {
    extract_tag(raw, tag, KNOWN_TAGS)
        .map(|block| (Some(block.content), block.repaired))
        .unwrap_or((None, false))
}

fn strip_known_blocks(raw: &str) -> String {
    strip_structured_blocks(
        raw,
        &["fin_control_feedback", "fin_tool_calls"],
        KNOWN_TAGS,
    )
    .trim()
    .to_string()
}

#[derive(Debug, Clone)]
struct ParsedControlFeedbackBlock {
    feedback: ControlFeedback,
    salvaged: bool,
}

fn parse_control_feedback_block(content: &str) -> Option<ParsedControlFeedbackBlock> {
    parse_control_feedback(content)
        .map(|result| ParsedControlFeedbackBlock {
            feedback: result.feedback,
            salvaged: result.salvaged,
        })
}

#[derive(Debug, Clone)]
struct ParsedToolCallsBlock {
    calls: Vec<ModelToolCall>,
    block_present: bool,
    parse_status: String,
    invalid_reason: Option<String>,
}

fn parse_tool_calls_block(content: &str) -> ParsedToolCallsBlock {
    let result = parse_tool_calls(content, false);
    ParsedToolCallsBlock {
        calls: result.calls,
        block_present: result.block_present,
        parse_status: result.parse_status,
        invalid_reason: result.invalid_reason,
    }
}

fn default_tool_calls() -> ParsedToolCallsBlock {
    ParsedToolCallsBlock {
        calls: Vec::new(),
        block_present: false,
        parse_status: "absent".into(),
        invalid_reason: None,
    }
}
