//! Heuristic layer detection for codebases that ship the Explorer without a
//! configured `.noupling/settings.json` layers array.
//!
//! Walks every file path looking for well-known directory-name keywords and
//! buckets files by the first keyword that matches. Returns a `Vec<Layer>`
//! the cli can splice into a clone of the project settings before re-running
//! audit + rendering the Explorer. The layers themselves are
//! marker-only (a single `**/<keyword>/**` glob each) — good enough for
//! the LSM to read as tiered, not a substitute for hand-written architecture
//! rules.
//!
//! The detection is intentionally conservative: a keyword has to claim at
//! least `MIN_FILES_PER_LAYER` files to count, and the synthesized layer
//! set must cover at least `MIN_COVERAGE_PERCENT` of all source files
//! before it's emitted. If nothing reasonable is detected we return an
//! empty `Vec` and the cli leaves the settings alone.

use noupling_core::core::{Module, ModuleType};
use noupling_core::settings::Layer;
use std::collections::BTreeMap;

const MIN_FILES_PER_LAYER: usize = 3;
const MIN_COVERAGE_PERCENT: f64 = 30.0;

/// Layer keyword catalogue ordered by typical architectural depth: a file
/// that lives under `presentation/domain/foo.kt` belongs to `presentation`
/// (the first match wins, not the deepest).
const KEYWORDS: &[(&str, &[&str])] = &[
    (
        "presentation",
        &[
            "presentation",
            "ui",
            "view",
            "views",
            "screen",
            "screens",
            "viewmodel",
            "viewmodels",
            "fragment",
            "fragments",
            "activity",
            "activities",
            "controller",
            "controllers",
            "handler",
            "handlers",
        ],
    ),
    (
        "domain",
        &[
            "domain",
            "usecase",
            "usecases",
            "use_case",
            "use_cases",
            "interactor",
            "interactors",
            "entity",
            "entities",
            "service",
            "services",
        ],
    ),
    (
        "data",
        &[
            "data",
            "repository",
            "repositories",
            "dao",
            "datasource",
            "datasources",
            "network",
            "api",
            "remote",
            "db",
            "database",
            "storage",
            "store",
        ],
    ),
    (
        "model",
        &[
            "model",
            "models",
            "dto",
            "dtos",
            "response",
            "responses",
            "request",
            "requests",
        ],
    ),
    (
        "infra",
        &[
            "infra",
            "infrastructure",
            "platform",
            "common",
            "shared",
            "core",
            "util",
            "utils",
            "helper",
            "helpers",
            "di",
            "module",
            "modules",
            "injection",
        ],
    ),
];

/// Detect a sensible layer set from the given source modules.
///
/// Returns an empty vec when:
/// * no settings file existed and no keyword matches enough files
/// * the synthesized set would cover less than [`MIN_COVERAGE_PERCENT`] of
///   the codebase (we'd rather show "0 layers" than a bad guess)
pub fn detect_layers(modules: &[Module]) -> Vec<Layer> {
    let files: Vec<&Module> = modules
        .iter()
        .filter(|m| matches!(m.module_type, ModuleType::File))
        .collect();
    if files.is_empty() {
        return Vec::new();
    }

    // Bucket files by the first matching keyword in priority order.
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut total_matched = 0usize;
    for f in &files {
        if let Some(layer_name) = classify(&f.path) {
            *counts.entry(layer_name).or_insert(0) += 1;
            total_matched += 1;
        }
    }

    let coverage = (total_matched as f64 / files.len() as f64) * 100.0;
    if coverage < MIN_COVERAGE_PERCENT {
        return Vec::new();
    }

    // Emit layers in the catalogue order (presentation → infra) so the
    // resulting LSM reads top-down by architectural depth.
    KEYWORDS
        .iter()
        .filter_map(|(name, _)| {
            let count = counts.get(*name).copied().unwrap_or(0);
            if count < MIN_FILES_PER_LAYER {
                return None;
            }
            Some(Layer {
                name: (*name).to_string(),
                pattern: format!("**/{}/**", name),
                allow_sibling: false,
                max_sibling_density: None,
                reduced_sibling_weight: 2.5,
            })
        })
        .collect()
}

/// Return the catalogue layer name a path falls under, or `None` when no
/// path segment matches.
fn classify(path: &str) -> Option<&'static str> {
    let segments: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches(".kt").trim_end_matches(".rs"))
        .collect();
    for (layer_name, keywords) in KEYWORDS {
        for seg in &segments {
            let lower = seg.to_ascii_lowercase();
            for kw in *keywords {
                if &lower == kw {
                    return Some(*layer_name);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::core::ModuleType;

    fn file(path: &str) -> Module {
        Module {
            id: format!("id-{path}"),
            snapshot_id: "snap".into(),
            parent_id: None,
            name: path.split('/').last().unwrap_or(path).to_string(),
            path: path.into(),
            module_type: ModuleType::File,
            depth: path.matches('/').count() as i32,
        }
    }

    #[test]
    fn detects_android_presentation_domain_data() {
        let modules = vec![
            file("app/src/main/java/com/foo/ui/Home.kt"),
            file("app/src/main/java/com/foo/ui/Login.kt"),
            file("app/src/main/java/com/foo/ui/Settings.kt"),
            file("app/src/main/java/com/foo/domain/CartUseCase.kt"),
            file("app/src/main/java/com/foo/domain/OrderUseCase.kt"),
            file("app/src/main/java/com/foo/domain/UserUseCase.kt"),
            file("app/src/main/java/com/foo/data/CartRepository.kt"),
            file("app/src/main/java/com/foo/data/OrderRepository.kt"),
            file("app/src/main/java/com/foo/data/Api.kt"),
        ];
        let layers = detect_layers(&modules);
        let names: Vec<&str> = layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["presentation", "domain", "data"]);
    }

    #[test]
    fn returns_empty_when_no_layers_meet_threshold() {
        let modules = vec![file("src/foo.rs"), file("src/bar.rs"), file("src/baz.rs")];
        let layers = detect_layers(&modules);
        assert!(layers.is_empty(), "no recognised segments → no auto-layers");
    }

    #[test]
    fn returns_empty_when_coverage_below_threshold() {
        // Three matched + many unmatched → coverage well below 30%.
        let mut modules = vec![
            file("app/ui/A.kt"),
            file("app/ui/B.kt"),
            file("app/ui/C.kt"),
        ];
        for i in 0..50 {
            modules.push(file(&format!("misc/random_{i}.rs")));
        }
        let layers = detect_layers(&modules);
        assert!(layers.is_empty());
    }

    #[test]
    fn first_match_in_priority_order_wins() {
        // `presentation` keyword is checked before `data`, so a path that
        // has both segments lands in presentation.
        let modules = vec![
            file("app/ui/data/A.kt"),
            file("app/ui/data/B.kt"),
            file("app/ui/data/C.kt"),
        ];
        let layers = detect_layers(&modules);
        let names: Vec<&str> = layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["presentation"]);
    }
}
