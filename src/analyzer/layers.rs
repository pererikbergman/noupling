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

        if let (Some((from_idx, _from_name)), Some((to_idx, to_name))) = (from_layer, to_layer) {
            // Violation: importing from a higher layer (lower index = higher layer)
            if to_idx < from_idx {
                let from_path = id_to_path.get(dep.from_module_id.as_str()).unwrap_or(&"");
                let to_path = id_to_path.get(dep.to_module_id.as_str()).unwrap_or(&"");
                let from_name = module_layer
                    .get(dep.from_module_id.as_str())
                    .map(|(_, n)| *n)
                    .unwrap_or("");
                violations.push(LayerViolation {
                    from_module: from_path.to_string(),
                    to_module: to_path.to_string(),
                    line_number: dep.line_number,
                    from_layer: from_name.to_string(),
                    to_layer: to_name.to_string(),
                });
            }
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
