//! Architectural analysis engine.
//!
//! Computes coupling violations and circular dependencies using
//! bottom-up D_acc aggregation and top-down BFS sibling analysis.

use crate::core::{Dependency, Module};

mod abstractness;
mod actions;
mod auto_layers;
mod cohesion;
mod coupling;
mod critical_path;
mod cycles;
mod direction;
mod distance;
mod gravity_wells;
mod independence;
mod instability;
mod issue;
mod layers;
mod metrics;
mod monorepo;
mod red_flags;
mod rules;
mod violation_age;

pub use direction::DependencyDirection;

pub use abstractness::{compute_abstractness, AbstractnessMetric};
pub use actions::compute_top_actions;
#[allow(unused_imports)] // Public API surface: kept reachable as analyzer::TopAction
pub use actions::TopAction;
pub use auto_layers::detect_layers;
pub use cohesion::{compute_cohesion, CohesionMetrics, DirectoryKind};
pub use coupling::CouplingViolation;
pub use critical_path::compute_critical_path;
pub use distance::{compute_distance, DistanceMetric, Zone};
pub use gravity_wells::{compute_gravity_wells, GravityWell};
pub use independence::{compute_independence, ModuleIndependence};
pub use instability::{
    compute_directory_instability, compute_stability_violations, InstabilityMetric,
    StabilityViolation,
};
pub use issue::{Issue, IssueKind, SeverityBand, Subject};
pub use layers::{check_layer_rules, LayerViolation};
pub use metrics::{compute_hotspots, ExternalDepMetric, ModuleMetrics};
#[allow(unused_imports)]
// Public API surface: kept reachable as analyzer::CrossModuleViolation
pub use monorepo::CrossModuleViolation;
pub use monorepo::{audit_modules, MonorepoResult};
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
    /// Per-directory abstractness metric (Martin's A).
    pub abstractness: Vec<AbstractnessMetric>,
    /// Per-directory instability metric (Martin's I).
    pub instability: Vec<InstabilityMetric>,
    /// Stable Dependencies Principle violations: directory pairs where a
    /// more-stable directory depends on a less-stable one.
    pub stability_violations: Vec<StabilityViolation>,
    /// Per-directory Distance from Main Sequence (Martin's D = |A + I − 1|).
    pub distance: Vec<DistanceMetric>,
    /// The layers this audit ran with: the configured `layers`, or the
    /// inferred set when none were configured (ADR 0001). Every format
    /// draws layers from here, never from settings directly.
    pub layers: Vec<crate::settings::Layer>,
    /// True when `layers` were inferred from path names rather than read
    /// from settings. Formats use it to say "these layers were inferred".
    pub layers_auto_detected: bool,
}

/// Test-only builder for `AuditResult` with sensible defaults.
///
/// Existed because the production type has ~25 fields and tests historically had
/// to spell out every single one even when asserting on one or two. Use `with_*`
/// to override just what the test cares about.
#[cfg(any(test, feature = "test-utils"))]
pub struct AuditResultBuilder {
    inner: AuditResult,
}

#[cfg(any(test, feature = "test-utils"))]
impl Default for AuditResultBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-utils"))]
#[allow(dead_code)] // setters exercised across crate + downstream test crates
impl AuditResultBuilder {
    pub fn new() -> Self {
        Self {
            inner: AuditResult {
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
                abstractness: Vec::new(),
                instability: Vec::new(),
                stability_violations: Vec::new(),
                distance: Vec::new(),
                layers: Vec::new(),
                layers_auto_detected: false,
            },
        }
    }

    pub fn with_distance(mut self, v: Vec<DistanceMetric>) -> Self {
        self.inner.distance = v;
        self
    }
    pub fn with_score(mut self, score: f64) -> Self {
        self.inner.score = score;
        self
    }
    pub fn with_tri(mut self, tri: f64) -> Self {
        self.inner.tri = tri;
        self
    }
    pub fn with_total_modules(mut self, n: usize) -> Self {
        self.inner.total_modules = n;
        self
    }
    pub fn with_total_xs(mut self, n: usize) -> Self {
        self.inner.total_xs = n;
        self
    }
    pub fn with_max_depth(mut self, n: usize) -> Self {
        self.inner.max_depth = n;
        self
    }
    pub fn with_suppressed_count(mut self, n: usize) -> Self {
        self.inner.suppressed_count = n;
        self
    }
    pub fn with_total_external_imports(mut self, n: usize) -> Self {
        self.inner.total_external_imports = n;
        self
    }
    pub fn with_violations(mut self, v: Vec<CouplingViolation>) -> Self {
        self.inner.violations = v;
        self
    }
    pub fn with_hotspots(mut self, v: Vec<ModuleMetrics>) -> Self {
        self.inner.hotspots = v;
        self
    }
    pub fn with_rule_violations(mut self, v: Vec<RuleViolation>) -> Self {
        self.inner.rule_violations = v;
        self
    }
    pub fn with_layer_violations(mut self, v: Vec<LayerViolation>) -> Self {
        self.inner.layer_violations = v;
        self
    }
    pub fn with_cohesion(mut self, v: Vec<CohesionMetrics>) -> Self {
        self.inner.cohesion = v;
        self
    }
    pub fn with_independence(mut self, v: Vec<ModuleIndependence>) -> Self {
        self.inner.independence = v;
        self
    }
    pub fn with_critical_path(mut self, v: Vec<String>) -> Self {
        self.inner.critical_path = v;
        self
    }
    pub fn with_coupling_metrics(mut self, v: Vec<CouplingViolation>) -> Self {
        self.inner.coupling_metrics_count = v.len();
        self.inner.coupling_metrics = v;
        self
    }
    pub fn with_gravity_wells(mut self, v: Vec<GravityWell>) -> Self {
        self.inner.gravity_wells = v;
        self
    }
    pub fn with_red_flags(mut self, v: Vec<RedFlag>) -> Self {
        self.inner.red_flags = v;
        self
    }
    pub fn with_external_deps(mut self, v: Vec<ExternalDepMetric>) -> Self {
        self.inner.external_deps = v;
        self
    }
    pub fn with_abstractness(mut self, v: Vec<AbstractnessMetric>) -> Self {
        self.inner.abstractness = v;
        self
    }
    pub fn with_instability(mut self, v: Vec<InstabilityMetric>) -> Self {
        self.inner.instability = v;
        self
    }
    pub fn with_stability_violations(mut self, v: Vec<StabilityViolation>) -> Self {
        self.inner.stability_violations = v;
        self
    }
    pub fn with_layers(mut self, v: Vec<crate::settings::Layer>, auto_detected: bool) -> Self {
        self.inner.layers = v;
        self.inner.layers_auto_detected = auto_detected;
        self
    }
    pub fn build(self) -> AuditResult {
        self.inner
    }
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
        if self.total_modules > 0 && max_weight > 0.0 {
            let denominator = self.total_modules as f64 * max_weight;
            self.assign_score(|v| 100.0 * v.rri / denominator);
        } else {
            self.assign_score(|_| 0.0);
        }

        // Detect Gravity Wells: modules with disproportionately high aggregate RRI.
        self.gravity_wells = compute_gravity_wells(&self.violations, &self.coupling_metrics);
        self.red_flags = compute_red_flags(&self.violations, &self.coupling_metrics);
    }

    pub fn recalculate_score(&mut self) {
        let total_modules = self.total_modules as f64;
        if self.total_modules > 0 {
            self.assign_score(|v| 100.0 * v.severity / total_modules);
        } else {
            self.assign_score(|_| 0.0);
        }
    }

    /// The one place the score and each violation's score impact are
    /// written. `impact_of` gives a violation's raw points; the score is
    /// `100 − Σ impact`, clamped to `0..=100`. When the raw loss exceeds
    /// 100 the impacts are scaled down proportionally so they still sum to
    /// the points actually lost (`CONTEXT.md` § Score impact).
    fn assign_score(&mut self, impact_of: impl Fn(&CouplingViolation) -> f64) {
        let raw_loss: f64 = self.violations.iter().map(&impact_of).sum();
        let scale = if raw_loss > 100.0 {
            100.0 / raw_loss
        } else {
            1.0
        };
        for v in &mut self.violations {
            v.score_impact = impact_of(v) * scale;
        }
        for v in &mut self.coupling_metrics {
            v.score_impact = 0.0;
        }
        self.score = (100.0 - raw_loss).clamp(0.0, 100.0);
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
            abstractness: Vec::new(),
            instability: Vec::new(),
            stability_violations: Vec::new(),
            distance: Vec::new(),
            layers: Vec::new(),
            layers_auto_detected: false,
        };
    }

    let violations = coupling::compute_coupling_violations(modules, dependencies);
    let total_modules = modules.len();

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

    let instability = compute_directory_instability(modules, dependencies);
    let stability_violations = compute_stability_violations(modules, dependencies, &instability);

    let mut result = AuditResult {
        violations,
        score: 100.0,
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
        abstractness: Vec::new(),
        instability,
        stability_violations,
        // Distance is computed in audit_with_settings once abstractness is populated.
        // audit() leaves it empty because it has no type-counts input.
        distance: Vec::new(),
        layers: Vec::new(),
        layers_auto_detected: false,
    };
    // Health score (severity-based until risk weights are applied).
    result.recalculate_score();
    result
}

/// Audit a snapshot and apply all settings-driven transformations in one call.
///
/// Wraps [`audit`] with the deterministic pipeline that every command
/// previously had to spell out: layer resolution, severity filtering,
/// coupling-mode adjustment, risk-weight RRI computation, layer-weight
/// reductions, layer filtering, and the rule / layer violation checks.
/// Call order matters and is fixed here so callers can't get it wrong.
///
/// **One audit per snapshot (ADR 0001).** When `settings.layers` is empty
/// the layers are inferred from path names ([`detect_layers`]); if that
/// produces anything, the audit switches to `actionable` coupling mode
/// unless the settings name a `coupling_mode` explicitly. The result records
/// the effective `layers` and `layers_auto_detected` so every format sees
/// the same Issues and score without re-auditing.
///
/// Command-specific augmentations (violation age, sidecar metadata, diff
/// filtering) are intentionally left out — they vary per command and stay
/// as separate post-hoc operations.
pub fn audit_with_settings(
    modules: &[Module],
    dependencies: &[Dependency],
    type_counts: &[crate::core::ModuleTypeCounts],
    settings: &crate::settings::Settings,
) -> AuditResult {
    let (layers, layers_auto_detected) = resolve_layers(modules, settings);
    // Auto-detected layers are by definition coarse (`**/ui/**` etc.), so
    // every sibling coupling inside one of them would count as a strict-mode
    // violation and the score would collapse with no actionable signal.
    // Switch to "actionable" so only cycles count; siblings stay
    // informational. Only the top-level `coupling_mode` alias counts as an
    // explicit choice: `thresholds.coupling_mode` is always present in the
    // file `noupling init` writes, so it cannot tell "user chose strict"
    // from "default".
    let coupling_mode = if layers_auto_detected && settings.coupling_mode.is_none() {
        "actionable"
    } else {
        settings.effective_coupling_mode()
    };

    let mut result = audit(modules, dependencies);
    result.abstractness = compute_abstractness(modules, type_counts);
    // Distance composes A (just populated) with I (populated in audit()).
    // 0.5 is the conventional cutoff: anything more than halfway off the main sequence.
    result.distance = compute_distance(&result.abstractness, &result.instability, 0.5);
    result.filter_by_severity(settings.thresholds.minimum_severity);
    result.apply_coupling_mode(coupling_mode);
    result.apply_risk_weights(&settings.risk_weights);
    result.apply_layer_weights(&layers);
    result.filter_by_layers(&layers);
    result.rule_violations =
        check_dependency_rules(modules, dependencies, &settings.dependency_rules);
    result.layer_violations = check_layer_rules(modules, dependencies, &layers);
    if layers_auto_detected {
        // Inferred layers are coarse and may cover only 30% of files; an
        // import into an unlayered file is ordinary there, not a violation
        // (it is one for hand-written layers, #220).
        result
            .layer_violations
            .retain(|l| l.to_layer != "<unlayered>");
    }
    result.layers = layers;
    result.layers_auto_detected = layers_auto_detected;
    result
}

/// The layers an audit runs with: the configured ones, or an inferred set
/// when none are configured. Returns `(layers, auto_detected)`.
fn resolve_layers(
    modules: &[Module],
    settings: &crate::settings::Settings,
) -> (Vec<crate::settings::Layer>, bool) {
    if !settings.layers.is_empty() {
        return (settings.layers.clone(), false);
    }
    let inferred = detect_layers(modules);
    let auto = !inferred.is_empty();
    (inferred, auto)
}

#[cfg(test)]
mod tests;
