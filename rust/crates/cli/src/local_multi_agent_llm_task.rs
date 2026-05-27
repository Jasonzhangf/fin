use crate::{CliError, config::default_provider_facade, fs_utils::read_file};
use fin_provider::{InferenceProvider, ProviderRequest};
use serde_json::json;
use std::path::Path;

pub(crate) fn execute_project_llm_task(
    user_config_path: &Path,
    cwd: &Path,
    task_summary: &str,
    observed_files: &[String],
) -> Result<serde_json::Value, CliError> {
    if std::env::var("FIN_LOCAL_MULTI_AGENT_STATIC_LLM")
        .ok()
        .as_deref()
        == Some("1")
    {
        return Ok(json!({
            "provider_name": "static-test-provider",
            "model": "static-test-model",
            "status": 200,
            "output_chars": 92,
            "output_text": "static project agent LLM response: inspect cwd, compare multi-agent design, report fin optimization suggestions",
            "stop_reason": "test"
        }));
    }
    let user_toml = read_file(user_config_path)?;
    let system = crate::config::load_effective_system_config(&user_toml, None)?;
    let provider = default_provider_facade(&system)?;
    let prompt = format!(
        "你是 fin 的 project agent。请真实执行以下任务并给 system agent 返回简洁审计结果。\n\n任务：{task_summary}\n项目 cwd：{}\n已观察文件：{}\n\n要求：说明你基于该 cwd 可以给 fin 的 3 条优化建议，输出中文，120 字以内。",
        cwd.display(),
        observed_files.join(", ")
    );
    let prepared = provider.prepare_request(&ProviderRequest {
        input: prompt.clone(),
        rendered_input: Some(prompt),
        override_model: None,
        prompt_cache_key: Some("local-multi-agent-project-task".into()),
    });
    let response = provider.execute_prepared(&prepared)?;
    Ok(json!({
        "provider_name": response.provider_name,
        "model": response.model,
        "status": response.status,
        "output_chars": response.output_text.chars().count(),
        "output_text": response.output_text,
        "response_id": response.response_id,
        "stop_reason": response.stop_reason,
        "usage": response.usage
    }))
}
