use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoadedSkill {
    pub(crate) skill_id: String,
    pub(crate) title: String,
    pub(crate) summary: String,
}

pub(crate) fn load_global_skills(runtime_home: Option<&str>) -> Vec<LoadedSkill> {
    let Some(runtime_home) = runtime_home.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };
    let skills_root = Path::new(runtime_home).join("skills");
    let Ok(entries) = fs::read_dir(&skills_root) else {
        return Vec::new();
    };

    let mut skills = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| load_skill(&path))
        .collect::<Vec<_>>();
    skills.sort_by(|left, right| left.skill_id.cmp(&right.skill_id));
    skills
}

pub(crate) fn summarize_loaded_skills(skills: &[LoadedSkill], limit: usize) -> String {
    if skills.is_empty() {
        return "0 global skills loaded".into();
    }
    let names = skills
        .iter()
        .take(limit)
        .map(|skill| skill.skill_id.as_str())
        .collect::<Vec<_>>();
    if skills.len() > limit {
        format!(
            "{} global skills loaded: {}, +{} more",
            skills.len(),
            names.join(", "),
            skills.len() - limit
        )
    } else {
        format!(
            "{} global skills loaded: {}",
            skills.len(),
            names.join(", ")
        )
    }
}

fn load_skill(skill_dir: &Path) -> Option<LoadedSkill> {
    let skill_path = skill_dir.join("SKILL.md");
    if !skill_path.is_file() {
        return None;
    }
    let content = fs::read_to_string(&skill_path).ok()?;
    let skill_id = skill_dir.file_name()?.to_str()?.to_string();
    let title = frontmatter_value(&content, "name")
        .or_else(|| first_heading(&content))
        .unwrap_or_else(|| skill_id.clone());
    let summary = frontmatter_value(&content, "description")
        .or_else(|| first_meaningful_body_line(&content))
        .unwrap_or_else(|| format!("loaded global skill {skill_id}"));

    Some(LoadedSkill {
        skill_id,
        title,
        summary,
    })
}

fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line == "---" {
            return None;
        }
        let (name, value) = line.split_once(':')?;
        if name.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn first_heading(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn first_meaningful_body_line(content: &str) -> Option<String> {
    let body = if content.starts_with(
        "---
",
    ) {
        content
            .splitn(
                3, "---
",
            )
            .nth(2)
            .unwrap_or(content)
    } else {
        content
    };
    body.lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with('-')
                && !line.starts_with("```")
        })
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fin-skill-loader-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ))
    }

    #[test]
    fn load_global_skills_reads_skill_markdown_from_runtime_home() {
        let home = temp_runtime_home();
        let skill_dir = home.join("skills/research-helper");
        fs::create_dir_all(&skill_dir).expect("skill dir should create");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---
name: research-helper
description: Search and summarize project facts.
---

# Research Helper
",
        )
        .expect("skill file should write");

        let skills = load_global_skills(home.to_str());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill_id, "research-helper");
        assert_eq!(skills[0].title, "research-helper");
        assert_eq!(skills[0].summary, "Search and summarize project facts.");
        assert!(summarize_loaded_skills(&skills, 6).contains("research-helper"));
    }
}
