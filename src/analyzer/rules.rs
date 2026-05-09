//! Custom dependency rule checking.
//!
//! Validates the project's dependency graph against forbidden-edge rules
//! declared in `settings.json` (e.g. "data must not import ui").

use fxhash::FxHashMap;

use crate::core::{Dependency, Module};

/// A violation of a custom dependency rule.
#[derive(Debug, Clone)]
pub struct RuleViolation {
    /// Source file path.
    pub from_module: String,
    /// Target file path.
    pub to_module: String,
    /// Line number of the import.
    pub line_number: i32,
    /// Custom message from the rule definition.
    pub message: String,
}

/// Check dependencies against custom rules from settings.json.
pub fn check_dependency_rules(
    modules: &[Module],
    dependencies: &[Dependency],
    rules: &[crate::settings::DependencyRule],
) -> Vec<RuleViolation> {
    if rules.is_empty() {
        return Vec::new();
    }

    let id_to_path: FxHashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    let mut violations = Vec::new();

    for rule in rules {
        if rule.allow {
            continue; // Only check forbidden rules
        }

        let from_glob = match globset::Glob::new(&rule.from) {
            Ok(g) => g.compile_matcher(),
            Err(_) => continue,
        };
        let to_glob = match globset::Glob::new(&rule.to) {
            Ok(g) => g.compile_matcher(),
            Err(_) => continue,
        };

        for dep in dependencies {
            let from_path = match id_to_path.get(dep.from_module_id.as_str()) {
                Some(p) => *p,
                None => continue,
            };
            let to_path = match id_to_path.get(dep.to_module_id.as_str()) {
                Some(p) => *p,
                None => continue,
            };

            if from_glob.is_match(from_path) && to_glob.is_match(to_path) {
                violations.push(RuleViolation {
                    from_module: from_path.to_string(),
                    to_module: to_path.to_string(),
                    line_number: dep.line_number,
                    message: if rule.message.is_empty() {
                        format!("Forbidden dependency: {} -> {}", rule.from, rule.to)
                    } else {
                        rule.message.clone()
                    },
                });
            }
        }
    }

    violations
}
