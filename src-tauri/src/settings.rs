//! `settings.json` — the small amount of durable configuration.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Standing instructions, sent with every Chat message and Recipe run.
    /// Never mixed with retrieved documents.
    #[serde(alias = "house_rules")]
    pub house_rules: String,
    /// Root folder where managed Shelf folders are created.
    /// `None` → `<app-data>/library`.
    #[serde(alias = "shelf_root")]
    pub shelf_root: Option<PathBuf>,
    /// The AI Chat uses. The previous file stays until a new process is Ready.
    #[serde(alias = "active_model")]
    pub active_model: Option<ActiveModel>,
    /// Measured prompt-processing budget: how many characters of local
    /// context this machine can comfortably feed to the model.
    #[serde(alias = "context_budget_chars")]
    pub context_budget_chars: Option<usize>,
    /// Result of the installation benchmark (kept for diagnostics).
    pub benchmark: Option<BenchmarkResult>,
    /// First-run onboarding finished.
    #[serde(alias = "onboarding_done")]
    pub onboarding_done: bool,
    /// When true, Chat may look up public web pages from this computer.
    #[serde(alias = "allow_online_research")]
    pub allow_online_research: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveModel {
    /// GGUF file name inside `<app-data>/models/`.
    pub file: String,
    /// User-facing model name, e.g. "Gemma 4 12B".
    pub name: String,
    /// Where it came from ("huggingface" | "ollama").
    pub source: String,
    /// Repo or library reference, for Settings display.
    pub reference: String,
    /// License identifier shown before download.
    pub license: Option<String>,
    /// Approximate download size in bytes.
    #[serde(alias = "size_bytes")]
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    /// Prompt tokens processed per second, as measured on this machine.
    #[serde(alias = "prompt_tokens_per_second")]
    pub prompt_tokens_per_second: f64,
    /// Generation tokens per second.
    #[serde(alias = "generation_tokens_per_second")]
    pub generation_tokens_per_second: f64,
    /// When the benchmark ran (RFC 3339).
    #[serde(alias = "measured_at")]
    pub measured_at: String,
    /// Model file that was measured.
    #[serde(alias = "model_file")]
    pub model_file: String,
}

impl Settings {
    /// Load `settings.json`, or defaults when the file is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(mut settings) => {
                    settings.house_rules = crate::limits::clip_chars(
                        &settings.house_rules,
                        crate::limits::HOUSE_RULES_MAX_CHARS,
                    );
                    settings
                }
                Err(error) => {
                    log::warn!("settings.json is unreadable ({error}); using defaults");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("could not read settings.json ({error}); using defaults");
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::paths::atomic_write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn shelf_root(&self, paths: &crate::paths::Paths) -> PathBuf {
        self.shelf_root
            .clone()
            .unwrap_or_else(|| paths.library_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_json_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        let settings = Settings::load(&path);
        assert!(!settings.onboarding_done);
        assert!(settings.house_rules.is_empty());
        assert!(!settings.allow_online_research);
    }

    #[test]
    fn roundtrip_and_snake_case_alias() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"house_rules":"Answer in Catalan.","onboarding_done":true}"#,
        )
        .unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.house_rules, "Answer in Catalan.");
        assert!(loaded.onboarding_done);
        loaded.save(&path).unwrap();
        let again = Settings::load(&path);
        assert_eq!(again.house_rules, "Answer in Catalan.");
        assert!(!again.allow_online_research);
    }

    #[test]
    fn online_research_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            allow_online_research: true,
            ..Default::default()
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert!(loaded.allow_online_research);
    }

    #[test]
    fn house_rules_are_clipped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let long = "x".repeat(crate::limits::HOUSE_RULES_MAX_CHARS + 50);
        std::fs::write(&path, serde_json::json!({ "houseRules": long }).to_string()).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(
            loaded.house_rules.chars().count(),
            crate::limits::HOUSE_RULES_MAX_CHARS
        );
    }

    #[test]
    fn default_shelf_root_is_the_app_data_library() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("appdata"));
        assert_eq!(Settings::default().shelf_root(&paths), paths.library_dir());
    }

    #[test]
    fn explicit_shelf_root_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("appdata"));
        let custom = dir.path().join("custom-root");
        let settings = Settings {
            shelf_root: Some(custom.clone()),
            ..Default::default()
        };
        assert_eq!(settings.shelf_root(&paths), custom);
    }
}
