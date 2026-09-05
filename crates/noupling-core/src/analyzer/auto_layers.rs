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

/// Minimum share of files the previous snapshot's inferred layers must still
/// match to be kept instead of re-inferred (#355). Half the entry threshold:
/// inference switches on at 30% and off below 15%, so a project sitting near
/// 30% does not flip between layered and unlayered with every commit.
const KEEP_COVERAGE_PERCENT: f64 = MIN_COVERAGE_PERCENT / 2.0;

/// [`detect_layers`] with hysteresis: when the previous snapshot inferred
/// `prior` layers and they still cover at least [`KEEP_COVERAGE_PERCENT`]
/// of the current files, return them unchanged. Otherwise (no prior, an
/// empty prior, or a prior that no longer fits) run a fresh detection.
pub fn detect_layers_with_prior(modules: &[Module], prior: Option<&[Layer]>) -> Vec<Layer> {
    if let Some(prior) = prior.filter(|p| !p.is_empty()) {
        let files: Vec<&Module> = modules
            .iter()
            .filter(|m| matches!(m.module_type, ModuleType::File))
            .collect();
        let matchers: Vec<globset::GlobMatcher> = prior
            .iter()
            .filter_map(|l| globset::Glob::new(&l.pattern).ok())
            .map(|g| g.compile_matcher())
            .collect();
        if !files.is_empty() && !matchers.is_empty() {
            let matched = files
                .iter()
                .filter(|f| matchers.iter().any(|m| m.is_match(&f.path)))
                .count();
            let coverage = matched as f64 / files.len() as f64 * 100.0;
            if coverage >= KEEP_COVERAGE_PERCENT {
                return prior.to_vec();
            }
        }
    }
    detect_layers(modules)
}

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
    // remember the directory segments (original case) each layer was seen
    // through so the emitted glob matches the real paths (`ui`, `Screens`),
    // not the catalogue name (`presentation`).
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut seen_segments: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    let mut total_matched = 0usize;
    for f in &files {
        if let Some((layer_name, segment)) = classify(&f.path) {
            *counts.entry(layer_name).or_insert(0) += 1;
            seen_segments
                .entry(layer_name)
                .or_default()
                .insert(segment.to_string());
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
            let segments = seen_segments.get(*name)?;
            let pattern = if segments.len() == 1 {
                format!("**/{}/**", segments.iter().next()?)
            } else {
                let alternatives: Vec<&str> = segments.iter().map(String::as_str).collect();
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

/// Return the catalogue layer name a path falls under, plus the directory
/// segment (as written) that placed it there, or `None` when no directory
/// segment matches. The file name is never considered: the emitted glob
/// `**/<segment>/**` can only match a directory.
fn classify(path: &str) -> Option<(&'static str, &str)> {
    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segments.pop(); // the file name
    for (layer_name, keywords) in KEYWORDS {
        for seg in &segments {
            let lower = seg.to_ascii_lowercase();
            if keywords.contains(&lower.as_str()) {
                return Some((*layer_name, seg));
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
    fn file_names_never_count_as_layer_segments() {
        // `src/db.rs`, `src/api.rs`, `src/storage.rs` are files, not
        // directories; the emitted glob `**/db/**` could never match them,
        // so they must not trigger inference.
        let modules = vec![
            file("src/main.rs"),
            file("src/db.rs"),
            file("src/api.rs"),
            file("src/storage.rs"),
        ];
        assert!(detect_layers(&modules).is_empty());
    }

    #[test]
    fn emitted_globs_keep_the_directory_case() {
        let modules = vec![
            file("src/Controllers/A.kt"),
            file("src/Controllers/B.kt"),
            file("src/Views/C.kt"),
        ];
        let layers = detect_layers(&modules);
        let presentation = layers.iter().find(|l| l.name == "presentation").unwrap();
        let matcher = globset::Glob::new(&presentation.pattern)
            .unwrap()
            .compile_matcher();
        assert!(
            matcher.is_match("src/Controllers/A.kt"),
            "{}",
            presentation.pattern
        );
        assert!(
            matcher.is_match("src/Views/C.kt"),
            "{}",
            presentation.pattern
        );
    }

    /// Layers inferred for the previous snapshot stay in force while they
    /// still cover a meaningful share of the files, even when a fresh
    /// detection would fall under the 30% threshold — so one unrelated file
    /// cannot flip the audit between layered and unlayered (#355).
    #[test]
    fn prior_layers_are_kept_across_the_coverage_threshold() {
        // 3 ui files + 6 others = 33% → inferred on the first snapshot.
        let mut modules: Vec<Module> = (0..3).map(|i| file(&format!("app/ui/s{i}.kt"))).collect();
        modules.extend((0..6).map(|i| file(&format!("app/misc/m{i}.kt"))));
        let first = detect_layers(&modules);
        assert_eq!(first.len(), 1, "precondition: ui inferred");

        // One more unrelated file: 3/10 = 30% is still fine, 3/11 = 27% is not.
        modules.push(file("app/misc/m6.kt"));
        modules.push(file("app/misc/m7.kt"));
        assert!(
            detect_layers(&modules).is_empty(),
            "fresh detection drops below 30%"
        );
        let kept = detect_layers_with_prior(&modules, Some(&first));
        assert_eq!(
            kept, first,
            "the prior layers are kept while coverage stays above 15%"
        );
    }

    #[test]
    fn prior_layers_are_dropped_once_they_cover_almost_nothing() {
        let prior = detect_layers(
            &(0..3)
                .map(|i| file(&format!("app/ui/s{i}.kt")))
                .collect::<Vec<_>>(),
        );
        assert_eq!(prior.len(), 1);
        // The ui directory is gone; 1 of 12 files matches (8%).
        let mut modules: Vec<Module> = (0..11)
            .map(|i| file(&format!("app/misc/m{i}.kt")))
            .collect();
        modules.push(file("app/ui/leftover.kt"));
        assert!(detect_layers_with_prior(&modules, Some(&prior)).is_empty());
    }

    #[test]
    fn a_prior_of_no_layers_is_just_a_fresh_detection() {
        let mut modules: Vec<Module> = (0..3).map(|i| file(&format!("app/ui/s{i}.kt"))).collect();
        modules.extend((0..3).map(|i| file(&format!("app/misc/m{i}.kt"))));
        assert_eq!(
            detect_layers_with_prior(&modules, Some(&[])),
            detect_layers(&modules)
        );
        assert_eq!(
            detect_layers_with_prior(&modules, None),
            detect_layers(&modules)
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
