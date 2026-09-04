//! Heuristic layer detection for codebases whose `.noupling/settings.json`
//! declares no `layers` array (ADR 0001, #341).
//!
//! Walks every file path looking for well-known directory-name keywords and
//! buckets files by the first keyword that matches. Returns a `Vec<Layer>`
//! that [`super::audit_with_settings`] uses in place of the empty configured
//! set, so `audit`, every report format, and the CI gate see the same
//! layers. The layers themselves are marker-only (a single `**/<keyword>/**`
//! glob each) — good enough to read as tiered, not a substitute for
//! hand-written architecture rules. The user's settings.json is never
//! touched.
//!
//! The detection is intentionally conservative: a keyword has to claim at
//! least `MIN_FILES_PER_LAYER` files to count, and the synthesized layer
//! set must cover at least `MIN_COVERAGE_PERCENT` of all source files
//! before it's emitted. If nothing reasonable is detected we return an
//! empty `Vec` and the audit runs unlayered.

use crate::core::{Module, ModuleType};
use crate::settings::Layer;
use std::collections::{BTreeMap, BTreeSet};

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

    // Bucket files by the first matching keyword in priority order, and
    // remember which keywords each layer was seen through so the emitted
    // glob matches the real path segments (`ui`, `screens`), not the
    // catalogue name (`presentation`).
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut seen_keywords: BTreeMap<&'static str, BTreeSet<&'static str>> = BTreeMap::new();
    let mut total_matched = 0usize;
    for f in &files {
        if let Some((layer_name, keyword)) = classify(&f.path) {
            *counts.entry(layer_name).or_insert(0) += 1;
            seen_keywords.entry(layer_name).or_default().insert(keyword);
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
            let keywords = seen_keywords.get(*name)?;
            let pattern = if keywords.len() == 1 {
                format!("**/{}/**", keywords.iter().next()?)
            } else {
                let alternatives: Vec<&str> = keywords.iter().copied().collect();
                format!("**/{{{}}}/**", alternatives.join(","))
            };
            Some(Layer {
                name: (*name).to_string(),
                pattern,
                allow_sibling: false,
                max_sibling_density: None,
                reduced_sibling_weight: 2.5,
            })
        })
        .collect()
}

/// Return the catalogue layer name a path falls under, plus the keyword
/// (path segment) that placed it there, or `None` when no segment matches.
fn classify(path: &str) -> Option<(&'static str, &'static str)> {
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
                    return Some((*layer_name, *kw));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> Module {
        Module {
            id: format!("id-{path}"),
            snapshot_id: "snap".into(),
            parent_id: None,
            name: path.split('/').next_back().unwrap_or(path).to_string(),
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
    fn inferred_patterns_match_the_paths_that_produced_them() {
        // The catalogue name ("presentation") is not a path segment; the
        // pattern has to be built from the keywords actually seen ("ui",
        // "screens") or the layer matches nothing downstream.
        let modules = vec![
            file("app/ui/Home.kt"),
            file("app/ui/Login.kt"),
            file("app/screens/Settings.kt"),
            file("app/domain/CartUseCase.kt"),
            file("app/domain/OrderUseCase.kt"),
            file("app/domain/UserUseCase.kt"),
        ];
        let layers = detect_layers(&modules);
        let presentation = layers
            .iter()
            .find(|l| l.name == "presentation")
            .expect("presentation layer");
        let matcher = globset::Glob::new(&presentation.pattern)
            .unwrap()
            .compile_matcher();
        assert!(
            matcher.is_match("app/ui/Home.kt"),
            "{}",
            presentation.pattern
        );
        assert!(
            matcher.is_match("app/screens/Settings.kt"),
            "{}",
            presentation.pattern
        );
        assert!(!matcher.is_match("app/domain/CartUseCase.kt"));
        assert!(
            !matcher.is_match("app/view/Other.kt"),
            "keywords nobody uses stay out of the pattern: {}",
            presentation.pattern
        );
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
