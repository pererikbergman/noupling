//! Architectural layer checking — layer ordering rules, layer-based
//! violation filtering, and layer-aware sibling weight reductions.

use fxhash::FxHashMap;

use super::AuditResult;
use super::DependencyDirection;
use crate::core::{Dependency, Module};

/// Compiled layer-pattern index. Holds one `GlobMatcher` per layer (in their
/// configured order) and answers "which layer is this path in?" without
/// recompiling globs on every call.
///
/// Existed because `check_layer_rules`, `AuditResult::filter_by_layers`, and
/// `AuditResult::apply_layer_weights` previously each compiled their own
/// matcher vec and walked it independently. Now there is one place to fix a
/// layer-matching bug or change the resolution rule.
pub(super) struct LayerIndex<'a> {
    matchers: Vec<(usize, &'a str, globset::GlobMatcher)>,
}

impl<'a> LayerIndex<'a> {
    pub(super) fn new(layers: &'a [crate::settings::Layer]) -> Self {
        let matchers = layers
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                globset::Glob::new(&l.pattern)
                    .ok()
                    .map(|g| (i, l.name.as_str(), g.compile_matcher()))
            })
            .collect();
        LayerIndex { matchers }
    }

    /// First layer whose pattern matches `path`, or `None`.
    pub(super) fn layer_of(&self, path: &str) -> Option<(usize, &'a str)> {
        for (idx, name, matcher) in &self.matchers {
            if matcher.is_match(path) {
                return Some((*idx, name));
            }
        }
        None
    }
}

/// A violation of architectural layer ordering.
#[derive(Debug, Clone)]
pub struct LayerViolation {
    /// Source file path.
    pub from_module: String,
    /// Target file path.
    pub to_module: String,
    /// Line number of the import.
    pub line_number: i32,
    /// Layer of the source module.
    pub from_layer: String,
    /// Layer of the target module (higher layer being imported).
    pub to_layer: String,
}

/// Check dependencies against architectural layer ordering.
/// Dependencies may only flow downward (higher index = lower layer).
pub fn check_layer_rules(
    modules: &[Module],
    dependencies: &[Dependency],
    layers: &[crate::settings::Layer],
) -> Vec<LayerViolation> {
    if layers.is_empty() {
        return Vec::new();
    }

    let index = LayerIndex::new(layers);

    let id_to_path: FxHashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    let mut module_layer: FxHashMap<&str, (usize, &str)> = FxHashMap::default();
    for module in modules {
        if let Some(layer) = index.layer_of(&module.path) {
            module_layer.insert(module.id.as_str(), layer);
        }
    }

    let mut violations = Vec::new();

    for dep in dependencies {
        let from_layer = module_layer.get(dep.from_module_id.as_str());
        let to_layer = module_layer.get(dep.to_module_id.as_str());

        let from_path = id_to_path.get(dep.from_module_id.as_str()).unwrap_or(&"");
        let to_path = id_to_path.get(dep.to_module_id.as_str()).unwrap_or(&"");

        match (from_layer, to_layer) {
            // Both layered: violation if target is in a higher (lower-index) layer.
            (Some((from_idx, from_name)), Some((to_idx, to_name))) => {
                if to_idx < from_idx {
                    violations.push(LayerViolation {
                        from_module: from_path.to_string(),
                        to_module: to_path.to_string(),
                        line_number: dep.line_number,
                        from_layer: from_name.to_string(),
                        to_layer: to_name.to_string(),
                    });
                }
            }
            // Bug #220: layered source → unlayered target is a deliberate
            // cross-layer dependency the team should see. Surface it with
            // to_layer = "<unlayered>" so the team can either add the target
            // to a layer or record the exception in dependency_rules.
            (Some((_, from_name)), None) => {
                violations.push(LayerViolation {
                    from_module: from_path.to_string(),
                    to_module: to_path.to_string(),
                    line_number: dep.line_number,
                    from_layer: from_name.to_string(),
                    to_layer: "<unlayered>".to_string(),
                });
            }
            // Unlayered source → anywhere is fine. Sources outside layers are
            // typically entrypoints (main.rs, build scripts) and importing
            // layered code is the normal entry case.
            (None, _) => {}
        }
    }

    violations
}

impl AuditResult {
    /// Remove coupling violations that follow the defined layer direction (downward).
    /// Keeps circular violations and violations where modules have no layer assigned.
    pub fn filter_by_layers(&mut self, layers: &[crate::settings::Layer]) {
        if layers.is_empty() {
            return;
        }

        let index = LayerIndex::new(layers);

        self.violations.retain(|v| {
            // Always keep circular violations
            if v.is_circular {
                return true;
            }

            let from_idx = index.layer_of(&v.from_module).map(|(i, _)| i);
            let to_idx = index.layer_of(&v.to_module).map(|(i, _)| i);

            match (from_idx, to_idx) {
                // Both have layers: suppress downward deps (from_idx < to_idx)
                // Keep: same layer (from_idx == to_idx) or upward (from_idx > to_idx)
                (Some(from), Some(to)) => from >= to,
                // One or both unassigned: keep the violation
                _ => true,
            }
        });
        self.recalculate_score();
    }

    /// Apply layer-specific weight reductions for sanctioned sibling connections.
    /// If both modules in a sibling violation belong to a layer with `allow_sibling: true`,
    /// the direction weight is reduced to the layer's `reduced_sibling_weight`.
    pub fn apply_layer_weights(&mut self, layers: &[crate::settings::Layer]) {
        if layers.is_empty() {
            return;
        }

        let index = LayerIndex::new(layers);

        // Adjust sibling violations in allow_sibling layers
        for v in self
            .violations
            .iter_mut()
            .chain(self.coupling_metrics.iter_mut())
        {
            if v.direction != DependencyDirection::Sibling {
                continue;
            }
            let from_layer = index.layer_of(&v.from_module).map(|(i, _)| i);
            let to_layer = index.layer_of(&v.to_module).map(|(i, _)| i);

            // Both in the same layer that allows siblings → reduced weight
            if let (Some(fi), Some(ti)) = (from_layer, to_layer) {
                if fi == ti && layers[fi].allow_sibling {
                    v.rri = layers[fi].reduced_sibling_weight * v.weight.max(1) as f64;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Module, ModuleType};
    use crate::settings::Layer;

    fn layer(name: &str, pattern: &str) -> Layer {
        Layer {
            name: name.into(),
            pattern: pattern.into(),
            allow_sibling: false,
            max_sibling_density: None,
            reduced_sibling_weight: 2.5,
        }
    }

    fn file_module(id: &str, path: &str) -> Module {
        Module {
            id: id.into(),
            snapshot_id: "snap".into(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: path.into(),
            module_type: ModuleType::File,
            depth: 1,
        }
    }

    fn dep(from: &str, to: &str, line: i32) -> Dependency {
        Dependency {
            from_module_id: from.into(),
            to_module_id: to.into(),
            line_number: line,
        }
    }

    #[test]
    fn flags_layered_source_to_unlayered_target_as_violation() {
        // Bug #220: a file inside a layer importing a top-level / unlayered
        // file (e.g. scanner/discovery.rs → src/settings.rs) is a deliberate
        // cross-layer dependency the team should see and decide about. The
        // old code silently dropped it because the target wasn't in any layer.
        let layers = vec![layer("scanner", "**/scanner/**")];
        let modules = vec![
            file_module("s1", "src/scanner/discovery.rs"),
            file_module("set1", "src/settings.rs"), // unlayered
        ];
        let deps = vec![dep("s1", "set1", 7)];

        let violations = check_layer_rules(&modules, &deps, &layers);

        assert_eq!(violations.len(), 1, "expected one violation");
        assert_eq!(violations[0].from_module, "src/scanner/discovery.rs");
        assert_eq!(violations[0].to_module, "src/settings.rs");
        assert_eq!(violations[0].from_layer, "scanner");
        assert_eq!(violations[0].to_layer, "<unlayered>");
    }

    #[test]
    fn does_not_flag_unlayered_source_to_layered_target() {
        // Asymmetric: entrypoints like src/main.rs aren't layered and importing
        // INTO a layer is the normal entry case, not a violation.
        let layers = vec![layer("scanner", "**/scanner/**")];
        let modules = vec![
            file_module("m1", "src/main.rs"), // unlayered
            file_module("s1", "src/scanner/discovery.rs"),
        ];
        let deps = vec![dep("m1", "s1", 1)];

        let violations = check_layer_rules(&modules, &deps, &layers);

        assert!(
            violations.is_empty(),
            "unlayered source → layered target is the entrypoint case, not a violation"
        );
    }

    #[test]
    fn flags_upward_layered_to_layered_unchanged() {
        // Existing behaviour preserved: B in a lower layer importing from a
        // higher layer is still a violation with the higher layer named in
        // to_layer.
        let layers = vec![
            layer("reporter", "**/reporter/**"), // index 0 = top
            layer("core", "**/core/**"),         // index 1 = bottom
        ];
        let modules = vec![
            file_module("c1", "src/core/mod.rs"),
            file_module("r1", "src/reporter/html.rs"),
        ];
        let deps = vec![dep("c1", "r1", 3)];

        let violations = check_layer_rules(&modules, &deps, &layers);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].from_layer, "core");
        assert_eq!(violations[0].to_layer, "reporter");
    }

    #[test]
    fn does_not_flag_downward_layered_to_layered() {
        // Downward (reporter → core, the right direction) stays silent.
        let layers = vec![
            layer("reporter", "**/reporter/**"), // index 0
            layer("core", "**/core/**"),         // index 1
        ];
        let modules = vec![
            file_module("r1", "src/reporter/html.rs"),
            file_module("c1", "src/core/mod.rs"),
        ];
        let deps = vec![dep("r1", "c1", 2)];

        let violations = check_layer_rules(&modules, &deps, &layers);

        assert!(violations.is_empty());
    }

    #[test]
    fn does_not_flag_when_layers_config_is_empty() {
        // Empty layers config means "no layer rules"; nothing should be flagged.
        let layers: Vec<Layer> = vec![];
        let modules = vec![
            file_module("a", "src/scanner/foo.rs"),
            file_module("b", "src/settings.rs"),
        ];
        let deps = vec![dep("a", "b", 1)];

        let violations = check_layer_rules(&modules, &deps, &layers);

        assert!(violations.is_empty());
    }
}
