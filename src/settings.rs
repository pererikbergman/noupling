//! Configuration loaded from `.noupling/settings.json`.

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Project settings loaded from `.noupling/settings.json`.
///
/// Auto-created with defaults on first run. All fields have defaults,
/// so partial JSON files are supported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Score and severity thresholds.
    #[serde(default = "default_thresholds")]
    pub thresholds: Thresholds,
    /// Glob patterns for directories/files to ignore (gitignore-style).
    #[serde(default = "default_ignore_patterns")]
    pub ignore_patterns: Vec<String>,
    /// File extensions to include in the scan.
    #[serde(default = "default_source_extensions")]
    pub source_extensions: Vec<String>,
    /// Custom dependency rules: allow or forbid specific import patterns.
    #[serde(default)]
    pub dependency_rules: Vec<DependencyRule>,
    /// Architectural layers (ordered top to bottom). Dependencies may only flow downward.
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Whether `noupling:ignore` inline comments are allowed. Default: true.
    #[serde(default = "default_allow_inline_suppression")]
    pub allow_inline_suppression: bool,
    /// Monorepo modules. Each gets independent analysis. If empty, whole project is one module.
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
    /// Severity weights per dependency direction for RRI calculation.
    #[serde(default = "default_risk_weights")]
    pub risk_weights: RiskWeights,
}

fn default_allow_inline_suppression() -> bool {
    true
}

/// Severity weights per dependency direction for RRI (Relationship Risk Index).
///
/// RRI = weight × density. Higher weight = more architectural risk.
/// See: Software Dependency Risk Framework (#167).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskWeights {
    /// Parent imports child. Essential for layering, low risk.
    #[serde(default = "default_weight_downward")]
    pub downward: f64,
    /// Same-level directories import each other. Moderate risk.
    #[serde(default = "default_weight_sibling")]
    pub sibling: f64,
    /// Child imports parent. Violates architectural flow, high risk.
    #[serde(default = "default_weight_upward")]
    pub upward: f64,
    /// Circular dependency between directories. Lethal.
    #[serde(default = "default_weight_circular")]
    pub circular: f64,
}

fn default_risk_weights() -> RiskWeights {
    RiskWeights {
        downward: default_weight_downward(),
        sibling: default_weight_sibling(),
        upward: default_weight_upward(),
        circular: default_weight_circular(),
    }
}

fn default_weight_downward() -> f64 {
    2.0
}
fn default_weight_sibling() -> f64 {
    4.0
}
fn default_weight_upward() -> f64 {
    6.0
}
fn default_weight_circular() -> f64 {
    10.0
}

/// A module within a monorepo. Each module is analyzed independently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    /// Human-readable name (e.g., "app", "lib-core").
    pub name: String,
    /// Relative path from project root (e.g., "app/src").
    pub path: String,
    /// Module names this module may import from. Unlisted cross-module imports are violations.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// An architectural layer. Dependencies may only flow downward (higher index = lower layer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Human-readable name (e.g., "presentation", "domain", "data").
    pub name: String,
    /// Glob pattern matching module paths in this layer.
    pub pattern: String,
}

/// A custom rule that allows or forbids dependencies matching glob patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRule {
    /// Glob pattern for the source module path.
    pub from: String,
    /// Glob pattern for the target module path.
    pub to: String,
    /// If false, this dependency is forbidden. If true, explicitly allowed.
    pub allow: bool,
    /// Custom message shown when the rule is violated.
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    #[serde(default = "default_score_green")]
    pub score_green: f64,
    #[serde(default = "default_score_yellow")]
    pub score_yellow: f64,
    #[serde(default = "default_critical_severity")]
    pub critical_severity: f64,
    #[serde(default = "default_minimum_severity")]
    pub minimum_severity: f64,
    /// Modules with fan-in (incoming imports) above this are flagged as hotspots.
    #[serde(default = "default_hotspot_fan_in")]
    pub hotspot_fan_in: usize,
    /// Directories with cohesion below this are flagged as low cohesion.
    #[serde(default = "default_min_cohesion")]
    pub min_cohesion: f64,
    /// Coupling detection mode: "actionable" (default) or "strict".
    /// - actionable: only flag circular deps, layer/rule/cross-module violations.
    ///   Sibling coupling is tracked as a metric but not a violation.
    /// - strict: every sibling dependency is a coupling violation (legacy behavior).
    #[serde(default = "default_coupling_mode")]
    pub coupling_mode: String,
}

fn default_thresholds() -> Thresholds {
    Thresholds {
        score_green: default_score_green(),
        score_yellow: default_score_yellow(),
        critical_severity: default_critical_severity(),
        minimum_severity: default_minimum_severity(),
        hotspot_fan_in: default_hotspot_fan_in(),
        min_cohesion: default_min_cohesion(),
        coupling_mode: default_coupling_mode(),
    }
}

fn default_coupling_mode() -> String {
    "actionable".to_string()
}

fn default_score_green() -> f64 {
    90.0
}
fn default_score_yellow() -> f64 {
    70.0
}
fn default_critical_severity() -> f64 {
    0.5
}
fn default_minimum_severity() -> f64 {
    0.2
}
fn default_hotspot_fan_in() -> usize {
    10
}
fn default_min_cohesion() -> f64 {
    0.1
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        "**/.git/**".to_string(),
        "**/target/**".to_string(),
        "**/node_modules/**".to_string(),
        "**/.noupling/**".to_string(),
        "**/.agent/**".to_string(),
        "**/build/**".to_string(),
        "**/dist/**".to_string(),
        "**/.gradle/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/.venv/**".to_string(),
        "**/zig-cache/**".to_string(),
        "**/zig-out/**".to_string(),
        "**/generated/**".to_string(),
        "**/.idea/**".to_string(),
        "**/.vscode/**".to_string(),
    ]
}

fn default_source_extensions() -> Vec<String> {
    vec![
        "rs".to_string(),
        "kt".to_string(),
        "kts".to_string(),
        "ts".to_string(),
        "tsx".to_string(),
        "swift".to_string(),
        "cs".to_string(),
        "go".to_string(),
        "hs".to_string(),
        "java".to_string(),
        "js".to_string(),
        "jsx".to_string(),
        "py".to_string(),
        "dart".to_string(),
        "php".to_string(),
        "rb".to_string(),
        "zig".to_string(),
    ]
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            thresholds: default_thresholds(),
            ignore_patterns: default_ignore_patterns(),
            source_extensions: default_source_extensions(),
            dependency_rules: Vec::new(),
            layers: Vec::new(),
            allow_inline_suppression: default_allow_inline_suppression(),
            modules: Vec::new(),
            risk_weights: default_risk_weights(),
        }
    }
}

impl Settings {
    /// Load settings from `.noupling/settings.json` under the given project path.
    /// Falls back to defaults if the file doesn't exist.
    pub fn load(project_path: &Path) -> Result<Self> {
        let settings_path = project_path.join(".noupling").join("settings.json");
        if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)?;
            let settings: Settings = serde_json::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(Settings::default())
        }
    }

    /// Write default settings to `.noupling/settings.json`.
    pub fn write_defaults(project_path: &Path) -> Result<()> {
        let noupling_dir = project_path.join(".noupling");
        std::fs::create_dir_all(&noupling_dir)?;
        let settings = Settings::default();
        let content = serde_json::to_string_pretty(&settings)?;
        std::fs::write(noupling_dir.join("settings.json"), content)?;
        Ok(())
    }

    /// Build a GlobSet from the ignore_patterns for matching paths.
    pub fn build_ignore_set(&self) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &self.ignore_patterns {
            builder.add(Glob::new(pattern)?);
        }
        Ok(builder.build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_valid() {
        let settings = Settings::default();
        assert_eq!(settings.thresholds.score_green, 90.0);
        assert_eq!(settings.thresholds.score_yellow, 70.0);
        assert_eq!(settings.thresholds.critical_severity, 0.5);
        assert!(!settings.ignore_patterns.is_empty());
        assert!(!settings.source_extensions.is_empty());
    }

    #[test]
    fn loads_defaults_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.thresholds.score_green, 90.0);
    }

    #[test]
    fn loads_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let noupling_dir = dir.path().join(".noupling");
        std::fs::create_dir_all(&noupling_dir).unwrap();
        std::fs::write(
            noupling_dir.join("settings.json"),
            r#"{"thresholds": {"score_green": 95.0, "score_yellow": 80.0, "critical_severity": 0.3}}"#,
        ).unwrap();

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.thresholds.score_green, 95.0);
        assert_eq!(settings.thresholds.score_yellow, 80.0);
        assert_eq!(settings.thresholds.critical_severity, 0.3);
        assert!(!settings.ignore_patterns.is_empty());
    }

    #[test]
    fn partial_settings_use_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let noupling_dir = dir.path().join(".noupling");
        std::fs::create_dir_all(&noupling_dir).unwrap();
        std::fs::write(
            noupling_dir.join("settings.json"),
            r#"{"thresholds": {"score_green": 85.0}}"#,
        )
        .unwrap();

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.thresholds.score_green, 85.0);
        assert_eq!(settings.thresholds.score_yellow, 70.0);
        assert_eq!(settings.thresholds.critical_severity, 0.5);
    }

    #[test]
    fn write_defaults_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        Settings::write_defaults(dir.path()).unwrap();

        let path = dir.path().join(".noupling").join("settings.json");
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        let settings: Settings = serde_json::from_str(&content).unwrap();
        assert_eq!(settings.thresholds.score_green, 90.0);
    }

    #[test]
    fn serializes_to_pretty_json() {
        let settings = Settings::default();
        let json = serde_json::to_string_pretty(&settings).unwrap();
        assert!(json.contains("score_green"));
        assert!(json.contains("ignore_patterns"));
        assert!(json.contains("source_extensions"));
    }

    #[test]
    fn ignore_set_matches_patterns() {
        let settings = Settings::default();
        let ignore_set = settings.build_ignore_set().unwrap();
        assert!(ignore_set.is_match("project/.git/HEAD"));
        assert!(ignore_set.is_match("project/build/output.jar"));
        assert!(ignore_set.is_match("app/src/generated/MyClass.kt"));
        assert!(!ignore_set.is_match("app/src/main/MyClass.kt"));
    }

    #[test]
    fn custom_ignore_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let noupling_dir = dir.path().join(".noupling");
        std::fs::create_dir_all(&noupling_dir).unwrap();
        std::fs::write(
            noupling_dir.join("settings.json"),
            r#"{"ignore_patterns": ["**/test/**", "**/vendor/**"]}"#,
        )
        .unwrap();

        let settings = Settings::load(dir.path()).unwrap();
        let ignore_set = settings.build_ignore_set().unwrap();
        assert!(ignore_set.is_match("src/test/MyTest.kt"));
        assert!(ignore_set.is_match("vendor/lib/util.go"));
        assert!(!ignore_set.is_match("src/main/App.kt"));
    }

    #[test]
    fn risk_weights_default_values() {
        let settings = Settings::default();
        assert_eq!(settings.risk_weights.downward, 2.0);
        assert_eq!(settings.risk_weights.sibling, 4.0);
        assert_eq!(settings.risk_weights.upward, 6.0);
        assert_eq!(settings.risk_weights.circular, 10.0);
    }

    #[test]
    fn risk_weights_configurable() {
        let dir = tempfile::tempdir().unwrap();
        let noupling_dir = dir.path().join(".noupling");
        std::fs::create_dir_all(&noupling_dir).unwrap();
        std::fs::write(
            noupling_dir.join("settings.json"),
            r#"{"risk_weights": {"circular": 15.0, "sibling": 3.0}}"#,
        )
        .unwrap();

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.risk_weights.circular, 15.0);
        assert_eq!(settings.risk_weights.sibling, 3.0);
        // Unset fields use defaults
        assert_eq!(settings.risk_weights.downward, 2.0);
        assert_eq!(settings.risk_weights.upward, 6.0);
    }

    #[test]
    fn old_settings_without_risk_weights_get_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let noupling_dir = dir.path().join(".noupling");
        std::fs::create_dir_all(&noupling_dir).unwrap();
        std::fs::write(
            noupling_dir.join("settings.json"),
            r#"{"thresholds": {"score_green": 90.0}}"#,
        )
        .unwrap();

        let settings = Settings::load(dir.path()).unwrap();
        assert_eq!(settings.risk_weights.downward, 2.0);
        assert_eq!(settings.risk_weights.sibling, 4.0);
        assert_eq!(settings.risk_weights.upward, 6.0);
        assert_eq!(settings.risk_weights.circular, 10.0);
    }
}
