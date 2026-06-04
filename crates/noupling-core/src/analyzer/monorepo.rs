//! Monorepo analysis: per-module audits + cross-module dependency detection
//! based on the declared `depends_on` graph in settings.json.

use fxhash::{FxHashMap, FxHashSet};

use super::{audit, AuditResult};
use crate::core::{Dependency, Module};

/// A cross-module dependency that violates the declared `depends_on` graph.
#[derive(Debug, Clone)]
pub struct CrossModuleViolation {
    /// Source module config name (e.g., "app").
    pub from_config: String,
    /// Target module config name (e.g., "lib-network").
    pub to_config: String,
    /// Source file path.
    pub from_file: String,
    /// Target file path.
    pub to_file: String,
    /// Line number of the import.
    pub line_number: i32,
}

/// Result of analyzing a monorepo with multiple configured modules.
#[derive(Debug)]
pub struct MonorepoResult {
    /// Per-module audit results: (module_name, audit_result).
    pub module_results: Vec<(String, AuditResult)>,
    /// Cross-module violations (imports not declared in depends_on).
    pub cross_module_violations: Vec<CrossModuleViolation>,
    /// Weighted average score across all modules.
    pub overall_score: f64,
    /// Total source files across all modules.
    pub total_modules: usize,
}

/// Run independent audits per configured module and detect cross-module violations.
pub fn audit_modules(
    all_modules: &[Module],
    all_dependencies: &[Dependency],
    module_configs: &[crate::settings::ModuleConfig],
) -> MonorepoResult {
    let id_to_path: FxHashMap<&str, &str> = all_modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    // Map each file to which module config it belongs to (first match wins)
    let mut file_to_config: FxHashMap<&str, usize> = FxHashMap::default();
    for module in all_modules {
        for (i, cfg) in module_configs.iter().enumerate() {
            let prefix = format!("{}/", cfg.path);
            if module.path.starts_with(&prefix) || module.path == cfg.path {
                file_to_config.insert(module.id.as_str(), i);
                break;
            }
        }
    }

    // Run independent audit per module
    let mut module_results: Vec<(String, AuditResult)> = Vec::new();
    let mut total_files = 0usize;
    let mut weighted_score_sum = 0.0f64;

    for (i, cfg) in module_configs.iter().enumerate() {
        let module_ids: FxHashSet<&str> = file_to_config
            .iter()
            .filter(|(_, &config_idx)| config_idx == i)
            .map(|(&id, _)| id)
            .collect();

        let filtered_modules: Vec<Module> = all_modules
            .iter()
            .filter(|m| module_ids.contains(m.id.as_str()))
            .cloned()
            .collect();

        let filtered_deps: Vec<Dependency> = all_dependencies
            .iter()
            .filter(|d| {
                module_ids.contains(d.from_module_id.as_str())
                    && module_ids.contains(d.to_module_id.as_str())
            })
            .cloned()
            .collect();

        let result = audit(&filtered_modules, &filtered_deps);
        let file_count = filtered_modules.len();
        weighted_score_sum += result.score * file_count as f64;
        total_files += file_count;
        module_results.push((cfg.name.clone(), result));
    }

    // Detect cross-module violations
    let mut cross_module_violations = Vec::new();
    for dep in all_dependencies {
        let from_cfg = file_to_config.get(dep.from_module_id.as_str()).copied();
        let to_cfg = file_to_config.get(dep.to_module_id.as_str()).copied();

        if let (Some(from_idx), Some(to_idx)) = (from_cfg, to_cfg) {
            if from_idx != to_idx {
                let from_config = &module_configs[from_idx];
                let to_config = &module_configs[to_idx];
                if !from_config.depends_on.contains(&to_config.name) {
                    cross_module_violations.push(CrossModuleViolation {
                        from_config: from_config.name.clone(),
                        to_config: to_config.name.clone(),
                        from_file: id_to_path
                            .get(dep.from_module_id.as_str())
                            .unwrap_or(&"")
                            .to_string(),
                        to_file: id_to_path
                            .get(dep.to_module_id.as_str())
                            .unwrap_or(&"")
                            .to_string(),
                        line_number: dep.line_number,
                    });
                }
            }
        }
    }

    // Apply cross-module penalty to overall score
    let cross_penalty = if total_files > 0 {
        cross_module_violations.len() as f64 / total_files as f64 * 100.0
    } else {
        0.0
    };

    let overall_score = if total_files > 0 {
        (weighted_score_sum / total_files as f64 - cross_penalty).max(0.0)
    } else {
        100.0
    };

    MonorepoResult {
        module_results,
        cross_module_violations,
        overall_score,
        total_modules: total_files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ModuleType;

    fn make_module(id: &str, path: &str) -> Module {
        Module {
            id: id.to_string(),
            snapshot_id: "snap".to_string(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            path: path.to_string(),
            module_type: ModuleType::File,
            depth: std::path::Path::new(path).components().count() as i32,
        }
    }

    #[test]
    fn audit_modules_independent_analysis() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("a2", "app/src/util.rs"),
            make_module("l1", "lib/src/core.rs"),
            make_module("l2", "lib/src/helper.rs"),
        ];
        // Coupling within app module only
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "a2".to_string(),
            line_number: 1,
        }];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec![],
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        assert_eq!(result.module_results.len(), 2);
        assert_eq!(result.module_results[0].0, "app");
        assert_eq!(result.module_results[1].0, "lib");
        // lib has no violations
        assert_eq!(result.module_results[1].1.violations.len(), 0);
        assert!(result.cross_module_violations.is_empty());
    }

    #[test]
    fn audit_modules_cross_module_violation() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("l1", "lib/src/core.rs"),
        ];
        // app imports lib without declaring depends_on
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 5,
        }];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec![], // does NOT list "lib"
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        assert_eq!(result.cross_module_violations.len(), 1);
        assert_eq!(result.cross_module_violations[0].from_config, "app");
        assert_eq!(result.cross_module_violations[0].to_config, "lib");
    }

    #[test]
    fn audit_modules_allowed_cross_dep() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("l1", "lib/src/core.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 5,
        }];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec!["lib".to_string()], // allowed
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        assert!(result.cross_module_violations.is_empty());
    }

    #[test]
    fn audit_modules_weighted_score() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("a2", "app/src/util.rs"),
            make_module("a3", "app/src/helper.rs"),
            make_module("l1", "lib/src/core.rs"),
        ];
        // No deps = perfect scores
        let deps = vec![];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec![],
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        // Both modules have score 100, weighted average is 100
        assert!((result.overall_score - 100.0).abs() < 0.01);
        assert_eq!(result.total_modules, 4);
    }

    #[test]
    fn audit_modules_empty_config_not_used() {
        // This test verifies the fallback path works by ensuring
        // audit_modules with empty config returns empty results
        let modules = vec![make_module("a1", "src/main.rs")];
        let deps = vec![];
        let configs = vec![];

        let result = audit_modules(&modules, &deps, &configs);
        assert!(result.module_results.is_empty());
        assert_eq!(result.overall_score, 100.0);
    }
}
