//! Architectural analysis engine.
//!
//! Computes coupling violations and circular dependencies using
//! bottom-up D_acc aggregation and top-down BFS sibling analysis.

use crate::core::{Dependency, DependencyDirection, Module};

mod actions;
mod cohesion;
mod coupling;
mod critical_path;
mod cycles;
mod gravity_wells;
mod independence;
mod layers;
mod metrics;
mod monorepo;
mod red_flags;
mod rules;
mod violation_age;

pub use actions::compute_top_actions;
#[allow(unused_imports)] // Public API surface: kept reachable as analyzer::TopAction
pub use actions::TopAction;
pub use cohesion::{compute_cohesion, CohesionMetrics};
pub use coupling::CouplingViolation;
pub use critical_path::compute_critical_path;
pub use gravity_wells::{compute_gravity_wells, GravityWell};
pub use independence::{compute_independence, ModuleIndependence};
pub use layers::{check_layer_rules, LayerViolation};
pub use metrics::{compute_hotspots, ExternalDepMetric, ModuleMetrics};
pub use monorepo::{audit_modules, MonorepoResult};
#[allow(unused_imports)] // Public API surface: kept reachable as analyzer::CrossModuleViolation
pub use monorepo::CrossModuleViolation;
pub use red_flags::{compute_red_flags, RedFlag, RedFlagType};
pub use rules::{check_dependency_rules, RuleViolation};
pub use violation_age::{compute_violation_age, ViolationAgeSummary};

/// The result of running an architectural audit on a project snapshot.
#[derive(Debug)]
pub struct AuditResult {
    /// All detected violations, sorted by severity descending.
    pub violations: Vec<CouplingViolation>,
    /// Overall health score (0-100). Higher is better.
    pub score: f64,
    /// Total Risk Index: sum of all violation RRIs. Lower is better.
    pub tri: f64,
    /// Total number of source modules analyzed.
    pub total_modules: usize,
    /// Per-module fan-in/fan-out metrics, sorted by fan_in descending.
    pub hotspots: Vec<ModuleMetrics>,
    /// Violations of custom dependency rules from settings.json.
    pub rule_violations: Vec<RuleViolation>,
    /// Violations of architectural layer ordering.
    pub layer_violations: Vec<LayerViolation>,
    /// Per-directory cohesion metrics.
    pub cohesion: Vec<CohesionMetrics>,
    /// Total excess: sum of all imports that need removal to fix all violations.
    pub total_xs: usize,
    /// Per-module independence scores (internal vs external dependency ratio).
    pub independence: Vec<ModuleIndependence>,
    /// Maximum dependency chain depth and the critical path.
    pub max_depth: usize,
    /// The longest dependency chain (file paths from root to deepest leaf).
    pub critical_path: Vec<String>,
    /// Violation age summary: new, recent, chronic counts.
    pub violation_age: ViolationAgeSummary,
    /// Sibling coupling pairs tracked as metrics (not violations) in actionable mode.
    pub coupling_metrics_count: usize,
    /// The actual sibling coupling pairs (kept for display in actionable mode,
    /// where they are informational rather than violations).
    pub coupling_metrics: Vec<CouplingViolation>,
    /// Number of imports suppressed by `noupling:ignore` comments.
    pub suppressed_count: usize,
    /// Modules with disproportionately high aggregate RRI — architectural "God Objects".
    pub gravity_wells: Vec<GravityWell>,
    /// Architectural red flags detected from the dependency analysis.
    pub red_flags: Vec<RedFlag>,
    /// Per-module count of external (third-party) imports.
    pub external_deps: Vec<ExternalDepMetric>,
    /// Total external import count across all modules.
    pub total_external_imports: usize,
}

impl AuditResult {
    /// Keep only violations involving at least one changed file and recalculate the score.
    pub fn filter_by_changed_files(&mut self, changed_files: &[String]) {
        self.violations.retain(|v| {
            // Coupling: check if from_module or to_module is a changed file
            if !v.is_circular {
                return changed_files
                    .iter()
                    .any(|f| v.from_module.ends_with(f) || f.ends_with(&v.from_module))
                    || changed_files
                        .iter()
                        .any(|f| v.to_module.ends_with(f) || f.ends_with(&v.to_module));
            }
            // Circular: check if any hop file in the cycle is a changed file
            for (from_file, to_file, _) in &v.cycle_hop_files {
                if changed_files
                    .iter()
                    .any(|f| from_file.ends_with(f) || f.ends_with(from_file))
                {
                    return true;
                }
                if changed_files
                    .iter()
                    .any(|f| to_file.ends_with(f) || f.ends_with(to_file))
                {
                    return true;
                }
            }
            false
        });
        self.recalculate_score();
    }

    /// Remove violations below the given severity threshold and recalculate the score.
    pub fn filter_by_severity(&mut self, minimum_severity: f64) {
        // Circular violations are always kept regardless of severity
        self.violations
            .retain(|v| v.is_circular || v.severity >= minimum_severity);
        self.recalculate_score();
    }

    /// In "actionable" coupling mode, sibling coupling violations are not
    /// counted as violations — only circular dependencies remain in the
    /// `violations` list. Layer/rule/cross-module violations are tracked
    /// separately and unaffected.
    ///
    /// Sibling coupling is still measured (cohesion, hotspots, weights) but
    /// no longer treated as a violation that drags down the score.
    pub fn apply_coupling_mode(&mut self, mode: &str) {
        if mode == "actionable" {
            // Move non-circular (sibling coupling) entries from violations into
            // coupling_metrics — they remain available for display but no longer
            // count as violations or affect the score.
            let (cycles, coupling): (Vec<_>, Vec<_>) = std::mem::take(&mut self.violations)
                .into_iter()
                .partition(|v| v.is_circular);
            self.violations = cycles;
            self.coupling_metrics_count = coupling.len();
            self.coupling_metrics = coupling;
            self.total_xs = self
                .violations
                .iter()
                .map(|v| {
                    if v.is_circular {
                        v.break_cost
                    } else {
                        v.weight
                    }
                })
                .sum();
            self.recalculate_score();
        }
    }

    /// Compute Relationship Risk Index (RRI) for each violation using
    /// the configured direction weights. RRI = direction_weight × density.
    ///
    /// For coupling violations, density = weight (import count between the pair).
    /// For circular violations, density = sum of all hop import counts.
    pub fn apply_risk_weights(&mut self, weights: &crate::settings::RiskWeights) {
        for v in &mut self.violations {
            let direction_weight = match v.direction {
                DependencyDirection::Downward => weights.downward,
                DependencyDirection::Sibling => weights.sibling,
                DependencyDirection::Upward => weights.upward,
                DependencyDirection::External => weights.external,
                DependencyDirection::Transitive => weights.transitive,
                DependencyDirection::Circular => weights.circular,
            };
            let density = if v.is_circular {
                let total: usize = v.cycle_hop_counts.iter().sum();
                total.max(1) as f64
            } else {
                v.weight.max(1) as f64
            };
            v.rri = direction_weight * density;
        }
        // Also compute RRI for coupling_metrics (informational, not violations)
        for v in &mut self.coupling_metrics {
            let direction_weight = match v.direction {
                DependencyDirection::Downward => weights.downward,
                DependencyDirection::Sibling => weights.sibling,
                DependencyDirection::Upward => weights.upward,
                DependencyDirection::External => weights.external,
                DependencyDirection::Transitive => weights.transitive,
                DependencyDirection::Circular => weights.circular,
            };
            v.rri = direction_weight * v.weight.max(1) as f64;
        }

        // Compute TRI (Total Risk Index) and derive health score.
        // TRI = sum of all violation RRIs.
        // Score = 100 * (1 - TRI / (total_modules * max_weight)), clamped to 0-100.
        // max_weight is the highest configured weight (typically circular=10),
        // so a project where every module averages 1 worst-case violation scores 0.
        self.tri = self.violations.iter().map(|v| v.rri).sum();
        let max_weight = weights
            .downward
            .max(weights.sibling)
            .max(weights.upward)
            .max(weights.external)
            .max(weights.transitive)
            .max(weights.circular);
        self.score = if self.total_modules > 0 && max_weight > 0.0 {
            (100.0 * (1.0 - self.tri / (self.total_modules as f64 * max_weight))).clamp(0.0, 100.0)
        } else {
            100.0
        };

        // Detect Gravity Wells: modules with disproportionately high aggregate RRI.
        self.gravity_wells = compute_gravity_wells(&self.violations, &self.coupling_metrics);
        self.red_flags = compute_red_flags(&self.violations, &self.coupling_metrics);
    }

    pub fn recalculate_score(&mut self) {
        let sum_severity: f64 = self.violations.iter().map(|v| v.severity).sum();
        self.score = if self.total_modules > 0 {
            (100.0 * (1.0 - sum_severity / self.total_modules as f64)).max(0.0)
        } else {
            100.0
        };
    }
}

/// Run the full audit: D_acc aggregation, BFS coupling detection, severity, and health score.
pub fn audit(modules: &[Module], dependencies: &[Dependency]) -> AuditResult {
    if modules.is_empty() {
        return AuditResult {
            violations: Vec::new(),
            score: 100.0,
            tri: 0.0,
            total_modules: 0,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            coupling_metrics: Vec::new(),
            suppressed_count: 0,
            gravity_wells: Vec::new(),
            red_flags: Vec::new(),
            external_deps: Vec::new(),
            total_external_imports: 0,
        };
    }

    let violations = coupling::compute_coupling_violations(modules, dependencies);

    // Health score
    let sum_severity: f64 = violations.iter().map(|v| v.severity).sum();
    let total_modules = modules.len();
    let score = (100.0 * (1.0 - sum_severity / total_modules as f64)).max(0.0);

    let hotspots = compute_hotspots(modules, dependencies);

    let cohesion = compute_cohesion(modules, dependencies);

    let independence = compute_independence(modules, dependencies);

    // Calculate total XS: sum of weights for coupling + break_cost for circular
    let total_xs: usize = violations
        .iter()
        .map(|v| {
            if v.is_circular {
                v.break_cost
            } else {
                v.weight
            }
        })
        .sum();

    let (max_depth, critical_path) = compute_critical_path(modules, dependencies);

    AuditResult {
        violations,
        score,
        tri: 0.0,
        total_modules,
        hotspots,
        rule_violations: Vec::new(),
        layer_violations: Vec::new(),
        cohesion,
        total_xs,
        independence,
        max_depth,
        critical_path,
        violation_age: ViolationAgeSummary::default(),
        coupling_metrics_count: 0,
        coupling_metrics: Vec::new(),
        suppressed_count: 0,
        gravity_wells: Vec::new(),
        red_flags: Vec::new(),
        external_deps: Vec::new(),
        total_external_imports: 0,
    }
}

/// Audit a snapshot and apply all settings-driven transformations in one call.
///
/// Wraps [`audit`] with the deterministic 5-step pipeline that every command
/// previously had to spell out: severity filtering, coupling-mode adjustment,
/// risk-weight RRI computation, layer-weight reductions, and layer filtering.
/// Call order matters and is fixed here so callers can't get it wrong.
///
/// Command-specific augmentations (violation age, rule violations, layer
/// violations, sidecar metadata, diff filtering) are intentionally left out
/// — they vary per command and stay as separate post-hoc operations.
pub fn audit_with_settings(
    modules: &[Module],
    dependencies: &[Dependency],
    settings: &crate::settings::Settings,
) -> AuditResult {
    let mut result = audit(modules, dependencies);
    result.filter_by_severity(settings.thresholds.minimum_severity);
    result.apply_coupling_mode(settings.effective_coupling_mode());
    result.apply_risk_weights(&settings.risk_weights);
    result.apply_layer_weights(&settings.layers);
    result.filter_by_layers(&settings.layers);
    result
}

#[cfg(test)]
mod tests;
