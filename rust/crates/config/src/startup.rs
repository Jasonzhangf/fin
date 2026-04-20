use crate::ConfigError;
use fin_shared::require_non_empty;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

fn default_system_local_worker_budget() -> usize {
    startup_defaults().system_agent.local_worker_budget
}

fn default_project_worker_budget() -> usize {
    startup_defaults().project_agent_defaults.worker_budget
}

fn default_auto_resume() -> bool {
    startup_defaults().system_agent.auto_resume
}

fn default_auto_connect() -> bool {
    startup_defaults().project_agent_defaults.auto_connect
}

fn default_project_auto_resume() -> bool {
    startup_defaults().project_agent_defaults.auto_resume
}

fn startup_defaults() -> &'static RuntimeStartupDefaultsFile {
    static STARTUP_DEFAULTS: OnceLock<RuntimeStartupDefaultsFile> = OnceLock::new();
    STARTUP_DEFAULTS.get_or_init(|| {
        toml::from_str(include_str!("../defaults/runtime-startup.toml"))
            .expect("embedded runtime-startup defaults must parse")
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectAgentMode {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RuntimeStartupDefaultsFile {
    system_agent: SystemAgentStartupDefaults,
    project_agent_defaults: ProjectAgentStartupDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SystemAgentStartupDefaults {
    local_worker_budget: usize,
    auto_resume: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ProjectAgentStartupDefaults {
    worker_budget: usize,
    auto_resume: bool,
    auto_connect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemAgentStartupConfig {
    #[serde(default = "default_system_local_worker_budget")]
    pub local_worker_budget: usize,
    #[serde(default = "default_auto_resume")]
    pub auto_resume: bool,
}

impl Default for SystemAgentStartupConfig {
    fn default() -> Self {
        Self {
            local_worker_budget: default_system_local_worker_budget(),
            auto_resume: default_auto_resume(),
        }
    }
}

impl SystemAgentStartupConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.local_worker_budget == 0 {
            return Err(ConfigError::Validation {
                message: "runtime.startup.system_agent.local_worker_budget must be greater than 0"
                    .into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentStartupConfig {
    pub project_id: String,
    pub mode: ProjectAgentMode,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default = "default_project_worker_budget")]
    pub worker_budget: usize,
    #[serde(default)]
    pub always_on: bool,
    #[serde(default = "default_project_auto_resume")]
    pub auto_resume: bool,
    #[serde(default = "default_auto_connect")]
    pub auto_connect: bool,
}

impl ProjectAgentStartupConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        require_non_empty(
            "runtime.startup.project_agents.project_id",
            &self.project_id,
        )?;
        if let Some(project_root) = &self.project_root {
            require_non_empty("runtime.startup.project_agents.project_root", project_root)?;
        }
        if let Some(endpoint) = &self.endpoint {
            require_non_empty("runtime.startup.project_agents.endpoint", endpoint)?;
        }
        if let Some(agent_name) = &self.agent_name {
            require_non_empty("runtime.startup.project_agents.agent_name", agent_name)?;
        }
        if self.worker_budget == 0 {
            return Err(ConfigError::Validation {
                message: format!(
                    "runtime.startup.project_agents.{}.worker_budget must be greater than 0",
                    self.project_id
                ),
            });
        }
        match self.mode {
            ProjectAgentMode::Local => {
                if self.project_root.is_none() {
                    return Err(ConfigError::Validation {
                        message: format!(
                            "runtime.startup.project_agents.{} local mode requires project_root",
                            self.project_id
                        ),
                    });
                }
            }
            ProjectAgentMode::Remote => {
                if self.endpoint.is_none() {
                    return Err(ConfigError::Validation {
                        message: format!(
                            "runtime.startup.project_agents.{} remote mode requires endpoint",
                            self.project_id
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStartupConfig {
    #[serde(default)]
    pub system_agent: SystemAgentStartupConfig,
    #[serde(default)]
    pub project_agents: Vec<ProjectAgentStartupConfig>,
}

impl RuntimeStartupConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.system_agent.validate()?;
        for project in &self.project_agents {
            project.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_defaults_are_loaded_from_embedded_config() {
        let defaults = startup_defaults();
        assert_eq!(defaults.system_agent.local_worker_budget, 4);
        assert!(defaults.system_agent.auto_resume);
        assert_eq!(defaults.project_agent_defaults.worker_budget, 4);
        assert!(defaults.project_agent_defaults.auto_resume);
        assert!(defaults.project_agent_defaults.auto_connect);
    }

    #[test]
    fn validate_accepts_local_and_remote_project_agents() {
        RuntimeStartupConfig {
            system_agent: SystemAgentStartupConfig::default(),
            project_agents: vec![
                ProjectAgentStartupConfig {
                    project_id: "fin".into(),
                    mode: ProjectAgentMode::Local,
                    project_root: Some("/tmp/fin".into()),
                    endpoint: None,
                    agent_name: Some("builder".into()),
                    worker_budget: 2,
                    always_on: true,
                    auto_resume: true,
                    auto_connect: true,
                },
                ProjectAgentStartupConfig {
                    project_id: "remote".into(),
                    mode: ProjectAgentMode::Remote,
                    project_root: None,
                    endpoint: Some("tcp://10.0.0.8:4711".into()),
                    agent_name: None,
                    worker_budget: 1,
                    always_on: false,
                    auto_resume: true,
                    auto_connect: true,
                },
            ],
        }
        .validate()
        .expect("startup config should validate");
    }

    #[test]
    fn validate_rejects_local_project_without_root() {
        let err = RuntimeStartupConfig {
            system_agent: SystemAgentStartupConfig::default(),
            project_agents: vec![ProjectAgentStartupConfig {
                project_id: "fin".into(),
                mode: ProjectAgentMode::Local,
                project_root: None,
                endpoint: None,
                agent_name: None,
                worker_budget: 1,
                always_on: false,
                auto_resume: true,
                auto_connect: true,
            }],
        }
        .validate()
        .expect_err("local project without root must fail");
        assert!(err.to_string().contains("local mode requires project_root"));
    }
}
