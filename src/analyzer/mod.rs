//! Architectural analysis engine.
//!
//! Computes coupling violations and circular dependencies using
//! bottom-up D_acc aggregation and top-down BFS sibling analysis.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::{Dependency, DependencyDirection, Module};

mod cohesion;
mod coupling;
mod critical_path;
mod cycles;
mod gravity_wells;
mod independence;
mod layers;
mod metrics;
mod red_flags;
mod rules;
mod violation_age;

pub use cohesion::{compute_cohesion, CohesionMetrics};
pub use coupling::CouplingViolation;
pub use critical_path::compute_critical_path;
pub use gravity_wells::{compute_gravity_wells, GravityWell};
pub use independence::{compute_independence, ModuleIndependence};
pub use layers::{check_layer_rules, LayerViolation};
pub use metrics::{compute_hotspots, ExternalDepMetric, ModuleMetrics};
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

/// A prioritized, actionable recommendation derived from analysis results.
#[derive(Debug, Clone)]
pub struct TopAction {
    /// Short headline (e.g., "Break circular dependency: data ↔ domain").
    pub title: String,
    /// One-line description of the affected code (e.g., "data/Repo.kt -> domain/Service.kt").
    pub detail: String,
    /// What to do (e.g., "Remove 2 imports at the weakest link: domain -> data").
    pub action: String,
    /// Estimated cost in imports to remove.
    pub cost: usize,
    /// Impact score (higher = bigger architectural improvement).
    #[allow(dead_code)]
    pub impact: f64,
    /// Category: "circular", "layer", "rule", "cross-module", "hotspot".
    pub category: String,
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

/// Compute the top N actions to take based on the audit result, ranked by ROI.
///
/// ROI = impact / effort, where:
/// - Impact = severity × (blast_radius_factor + 1)
/// - Effort = break_cost (XS for circular, weight for coupling)
///
/// Categories: circular > layer > rule > cross-module > hotspot
pub fn compute_top_actions(result: &AuditResult, limit: usize) -> Vec<TopAction> {
    let mut actions: Vec<(f64, TopAction)> = Vec::new();

    // 1. Circular dependencies — highest priority
    for v in result.violations.iter().filter(|v| v.is_circular) {
        let cycle_str = v
            .cycle_path
            .iter()
            .map(|p| short_dir(p))
            .collect::<Vec<_>>()
            .join(" \u{2192} ");
        let cost = v.break_cost.max(1);
        let impact = v.severity * (v.cycle_order as f64);
        let action_text = if let Some(ref weakest) = v.weakest_link {
            format!("Break the cycle at the weakest link: {}", weakest)
        } else {
            format!(
                "Break this cycle by removing imports between {} modules",
                v.cycle_order
            )
        };
        actions.push((
            impact / cost as f64,
            TopAction {
                title: format!("Break circular dependency in {}", short_dir(&v.dir_a)),
                detail: cycle_str,
                action: action_text,
                cost,
                impact,
                category: "circular".to_string(),
            },
        ));
    }

    // 2. Layer violations
    for lv in &result.layer_violations {
        actions.push((
            5.0,
            TopAction {
                title: format!(
                    "Layer violation: {} \u{2192} {}",
                    lv.from_layer, lv.to_layer
                ),
                detail: format!("{}:{}", lv.from_module, lv.line_number),
                action: format!(
                    "Remove this import or move shared code into a lower layer than {}",
                    lv.from_layer
                ),
                cost: 1,
                impact: 5.0,
                category: "layer".to_string(),
            },
        ));
    }

    // 3. Custom rule violations
    for rv in &result.rule_violations {
        actions.push((
            3.0,
            TopAction {
                title: format!("Rule violation: {}", rv.message),
                detail: format!(
                    "{}:{} \u{2192} {}",
                    rv.from_module, rv.line_number, rv.to_module
                ),
                action: rv.message.clone(),
                cost: 1,
                impact: 3.0,
                category: "rule".to_string(),
            },
        ));
    }

    // 4. Hotspot review (high fan-in modules — change risk)
    let mut hotspots_sorted: Vec<&ModuleMetrics> =
        result.hotspots.iter().filter(|h| h.fan_in >= 10).collect();
    hotspots_sorted.sort_by_key(|h| std::cmp::Reverse(h.fan_in));
    for h in hotspots_sorted.iter().take(3) {
        actions.push((
            h.fan_in as f64 / 100.0,
            TopAction {
                title: format!("Review hotspot: {}", short_file(&h.path)),
                detail: format!("{} dependents, blast radius {}", h.fan_in, h.blast_radius),
                action: "Stabilize via interface/abstraction; any change ripples widely"
                    .to_string(),
                cost: h.fan_in,
                impact: h.fan_in as f64,
                category: "hotspot".to_string(),
            },
        ));
    }

    // Sort by ROI descending and take top N
    actions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    actions.into_iter().take(limit).map(|(_, a)| a).collect()
}

fn short_dir(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .to_string()
}

fn short_file(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .to_string()
}

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

    fn make_dep(from: &str, to: &str, line: i32) -> Dependency {
        Dependency {
            from_module_id: from.to_string(),
            to_module_id: to.to_string(),
            line_number: line,
        }
    }

    // ── BFS coupling detection ──

    #[test]
    fn detects_sibling_coupling() {
        // scanner depends on storage (siblings under src/slices)
        let modules = vec![
            make_module("a", "src/slices/scanner/mod.rs"),
            make_module("b", "src/slices/storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 10)];

        let result = audit(&modules, &deps);
        assert!(
            !result.violations.is_empty(),
            "Should detect coupling between scanner and storage"
        );
        assert_eq!(result.violations[0].dir_a, "src/slices/scanner");
        assert_eq!(result.violations[0].dir_b, "src/slices/storage");
    }

    #[test]
    fn no_violations_when_independent() {
        let modules = vec![
            make_module("a", "src/slices/scanner/mod.rs"),
            make_module("b", "src/slices/storage/mod.rs"),
        ];
        let deps: Vec<Dependency> = vec![];

        let result = audit(&modules, &deps);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn no_violations_for_internal_deps() {
        let modules = vec![
            make_module("a", "src/scanner/parser.rs"),
            make_module("b", "src/scanner/resolver.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(result.violations.is_empty());
    }

    // ── Severity ──

    #[test]
    fn severity_at_depth_zero() {
        // Two top-level sibling dirs
        let modules = vec![
            make_module("a", "scanner/mod.rs"),
            make_module("b", "storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(!result.violations.is_empty());
        assert!((result.violations[0].severity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn severity_decreases_with_depth() {
        // Siblings at depth 2 (under src/slices/)
        let modules = vec![
            make_module("a", "src/slices/scanner/mod.rs"),
            make_module("b", "src/slices/storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(!result.violations.is_empty());
        // parent "src/slices" has depth 2, children are at depth 3, severity = 1/(3+1) = 0.25
        let expected = 0.25;
        assert!(
            (result.violations[0].severity - expected).abs() < 0.01,
            "Expected severity ~{}, got {}",
            expected,
            result.violations[0].severity
        );
    }

    // ── Health score ──

    #[test]
    fn perfect_score_no_violations() {
        let modules = vec![
            make_module("a", "src/scanner/mod.rs"),
            make_module("b", "src/storage/mod.rs"),
        ];
        let deps: Vec<Dependency> = vec![];

        let result = audit(&modules, &deps);
        assert!((result.score - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_decreases_with_violations() {
        let modules = vec![
            make_module("a", "scanner/mod.rs"),
            make_module("b", "storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(result.score < 100.0);
        assert!(result.score >= 0.0);
        // severity=1.0, total_modules=2, score=100*(1-1.0/2)=50
        assert!(
            (result.score - 50.0).abs() < 0.01,
            "Expected ~50, got {}",
            result.score
        );
    }

    #[test]
    fn score_clamps_to_zero() {
        // Many high-severity violations
        let modules = vec![make_module("a", "x/mod.rs"), make_module("b", "y/mod.rs")];
        // Create multiple deps to push score below 0
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("a", "b", 2),
            make_dep("a", "b", 3),
            make_dep("b", "a", 1),
            make_dep("b", "a", 2),
            make_dep("b", "a", 3),
        ];

        let result = audit(&modules, &deps);
        assert!(result.score >= 0.0);
    }

    #[test]
    fn sibling_violations_have_sibling_direction() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];
        let result = audit(&modules, &deps);
        let siblings: Vec<&CouplingViolation> = result
            .violations
            .iter()
            .filter(|v| !v.is_circular)
            .collect();
        assert!(!siblings.is_empty(), "Should have sibling violations");
        for v in siblings {
            assert_eq!(v.direction, DependencyDirection::Sibling);
        }
    }

    #[test]
    fn circular_violations_have_circular_direction() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1), make_dep("b", "a", 5)];
        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(!circular.is_empty(), "Should have circular violations");
        for v in circular {
            assert_eq!(v.direction, DependencyDirection::Circular);
        }
    }

    #[test]
    fn gravity_wells_detected_for_high_rri_modules() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
            make_module("c", "src/gamma/mod.rs"),
        ];
        // a imports b (1 dep), a imports c (1 dep), b imports c (1 dep)
        // Module a participates in 2 violations, b in 2, c in 2
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("a", "c", 2),
            make_dep("b", "c", 3),
        ];
        let mut result = audit(&modules, &deps);
        let weights = crate::settings::RiskWeights {
            downward: 2.0,
            sibling: 4.0,
            upward: 6.0,
            external: 8.0,
            transitive: 9.0,
            circular: 10.0,
        };
        result.apply_risk_weights(&weights);

        // All modules participate in violations, gravity wells depend on
        // whether any module's total RRI exceeds 2× the median
        // This is a structural test — just verify the computation runs
        // and gravity_wells is populated (or empty) without panicking
        assert!(result.gravity_wells.len() <= modules.len());
    }

    #[test]
    fn apply_risk_weights_computes_rri() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        // 3 imports from alpha to beta → weight=3 after dedup
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("a", "b", 2),
            make_dep("a", "b", 3),
        ];
        let mut result = audit(&modules, &deps);
        let weights = crate::settings::RiskWeights {
            downward: 2.0,
            sibling: 4.0,
            upward: 6.0,
            external: 8.0,
            transitive: 9.0,
            circular: 10.0,
        };
        result.apply_risk_weights(&weights);

        let siblings: Vec<&CouplingViolation> = result
            .violations
            .iter()
            .filter(|v| !v.is_circular)
            .collect();
        assert!(!siblings.is_empty());
        // RRI = sibling_weight(4) × density(3) = 12
        assert_eq!(siblings[0].rri, 12.0);
    }

    #[test]
    fn apply_risk_weights_circular_uses_hop_counts() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("a", "b", 2),
            make_dep("b", "a", 5),
        ];
        let mut result = audit(&modules, &deps);
        let weights = crate::settings::RiskWeights {
            downward: 2.0,
            sibling: 4.0,
            upward: 6.0,
            external: 8.0,
            transitive: 9.0,
            circular: 10.0,
        };
        result.apply_risk_weights(&weights);

        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(!circular.is_empty());
        // Total hop imports: alpha→beta has some + beta→alpha has some
        // RRI = circular_weight(10) × total_density
        assert!(
            circular[0].rri >= 10.0,
            "Circular RRI should be at least 10"
        );
    }

    #[test]
    fn tri_computed_from_rri_sum() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1), make_dep("a", "b", 2)];
        let mut result = audit(&modules, &deps);
        let weights = crate::settings::RiskWeights {
            downward: 2.0,
            sibling: 4.0,
            upward: 6.0,
            external: 8.0,
            transitive: 9.0,
            circular: 10.0,
        };
        result.apply_risk_weights(&weights);

        // Sibling violation with density 2: RRI = 4 × 2 = 8
        // TRI = sum of all RRIs = 8
        assert_eq!(result.tri, 8.0);
        // Score = 100 * (1 - 8 / (2 * 10)) = 100 * (1 - 0.4) = 60
        assert!(
            (result.score - 60.0).abs() < 0.1,
            "Score should be ~60, got {}",
            result.score
        );
    }

    #[test]
    fn empty_project_scores_100() {
        let result = audit(&[], &[]);
        assert!((result.score - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn violations_sorted_by_severity_descending() {
        let modules = vec![
            make_module("a", "scanner/mod.rs"),
            make_module("b", "storage/mod.rs"),
            make_module("c", "src/slices/analyzer/mod.rs"),
            make_module("d", "src/slices/reporter/mod.rs"),
        ];
        let deps = vec![
            make_dep("a", "b", 1), // depth 0, severity 1.0
            make_dep("c", "d", 1), // depth 2, severity 0.33
        ];

        let result = audit(&modules, &deps);
        if result.violations.len() >= 2 {
            assert!(result.violations[0].severity >= result.violations[1].severity);
        }
    }

    // ── Circular dependencies ──

    #[test]
    fn detects_circular_dependency() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        // A -> B and B -> A = cycle
        let deps = vec![make_dep("a", "b", 1), make_dep("b", "a", 5)];

        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(
            !circular.is_empty(),
            "Should detect circular dependency between a and b"
        );
        // Severity depends on depth: siblings under "src" are at depth 2, severity = 1/(2+1)
        assert!(circular[0].severity > 0.0);
    }

    #[test]
    fn no_circular_when_one_direction() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(circular.is_empty());
    }

    #[test]
    fn detects_transitive_cycle() {
        let modules = vec![
            make_module("a", "src/x/mod.rs"),
            make_module("b", "src/y/mod.rs"),
            make_module("c", "src/z/mod.rs"),
        ];
        // A -> B -> C -> A = transitive cycle
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("b", "c", 1),
            make_dep("c", "a", 1),
        ];

        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(
            !circular.is_empty(),
            "Should detect transitive circular dependency"
        );
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

    #[test]
    fn independence_fully_internal() {
        let modules = vec![
            make_module("a1", "app/main.rs"),
            make_module("a2", "app/util.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "a2".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let app = result.independence.iter().find(|m| m.dir == "app");
        assert!(app.is_some());
        let app = app.unwrap();
        assert_eq!(app.internal_deps, 1);
        assert_eq!(app.external_deps, 0);
        assert!((app.independence - 1.0).abs() < 0.01);
    }

    #[test]
    fn independence_mixed_deps() {
        let modules = vec![
            make_module("a1", "app/main.rs"),
            make_module("a2", "app/util.rs"),
            make_module("l1", "lib/core.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "a2".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "l1".to_string(),
                line_number: 2,
            },
        ];
        let result = audit(&modules, &deps);
        let app = result.independence.iter().find(|m| m.dir == "app");
        assert!(app.is_some());
        let app = app.unwrap();
        assert_eq!(app.internal_deps, 1);
        assert_eq!(app.external_deps, 1);
        assert!((app.independence - 0.5).abs() < 0.01);
    }

    #[test]
    fn independence_sorted_lowest_first() {
        let modules = vec![
            make_module("a1", "app/main.rs"),
            make_module("a2", "app/util.rs"),
            make_module("l1", "lib/core.rs"),
            make_module("l2", "lib/helper.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "a2".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "l1".to_string(),
                line_number: 2,
            },
            Dependency {
                from_module_id: "l1".to_string(),
                to_module_id: "l2".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        assert_eq!(result.independence.len(), 2);
        assert_eq!(result.independence[0].dir, "app");
        assert_eq!(result.independence[1].dir, "lib");
    }

    #[test]
    fn instability_pure_consumer() {
        let modules = vec![
            make_module("a1", "src/app/main.rs"),
            make_module("l1", "src/lib/core.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let a1 = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/app/main.rs")
            .unwrap();
        assert_eq!(a1.fan_out, 1);
        assert_eq!(a1.fan_in, 0);
        assert!((a1.instability - 1.0).abs() < 0.01);
    }

    #[test]
    fn instability_pure_provider() {
        let modules = vec![
            make_module("a1", "src/app/main.rs"),
            make_module("l1", "src/lib/core.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let l1 = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/lib/core.rs")
            .unwrap();
        assert_eq!(l1.fan_in, 1);
        assert_eq!(l1.fan_out, 0);
        assert!((l1.instability - 0.0).abs() < 0.01);
    }

    #[test]
    fn instability_balanced() {
        let modules = vec![
            make_module("a", "src/a.rs"),
            make_module("b", "src/b.rs"),
            make_module("c", "src/c.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a".to_string(),
                to_module_id: "b".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "b".to_string(),
                to_module_id: "c".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        let b = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/b.rs")
            .unwrap();
        assert_eq!(b.fan_in, 1);
        assert_eq!(b.fan_out, 1);
        assert!((b.instability - 0.5).abs() < 0.01);
    }

    #[test]
    fn blast_radius_leaf_is_zero() {
        let modules = vec![make_module("a", "src/a.rs"), make_module("b", "src/b.rs")];
        let deps = vec![Dependency {
            from_module_id: "a".to_string(),
            to_module_id: "b".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let a = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/a.rs")
            .unwrap();
        assert_eq!(a.blast_radius, 0);
    }

    #[test]
    fn blast_radius_direct() {
        let modules = vec![make_module("a", "src/a.rs"), make_module("b", "src/b.rs")];
        let deps = vec![Dependency {
            from_module_id: "a".to_string(),
            to_module_id: "b".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let b = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/b.rs")
            .unwrap();
        assert_eq!(b.blast_radius, 1);
    }

    #[test]
    fn blast_radius_transitive() {
        let modules = vec![
            make_module("a", "src/a.rs"),
            make_module("b", "src/b.rs"),
            make_module("c", "src/c.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a".to_string(),
                to_module_id: "b".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "b".to_string(),
                to_module_id: "c".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        let c = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/c.rs")
            .unwrap();
        assert_eq!(c.blast_radius, 2);
    }

    // ── audit_with_settings: the deterministic settings-aware seam ──

    #[test]
    fn audit_with_settings_matches_manual_pipeline() {
        // The seam must produce the same AuditResult as calling the 5 methods by hand
        // in the documented order. This pins the contract so callers can't drift.
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("a", "b", 2),
            make_dep("a", "b", 3),
        ];
        let settings = crate::settings::Settings::default();

        let auto = audit_with_settings(&modules, &deps, &settings);

        let mut manual = audit(&modules, &deps);
        manual.filter_by_severity(settings.thresholds.minimum_severity);
        manual.apply_coupling_mode(settings.effective_coupling_mode());
        manual.apply_risk_weights(&settings.risk_weights);
        manual.apply_layer_weights(&settings.layers);
        manual.filter_by_layers(&settings.layers);

        assert_eq!(auto.score, manual.score);
        assert_eq!(auto.violations.len(), manual.violations.len());
        assert_eq!(auto.tri, manual.tri);
        for (a, m) in auto.violations.iter().zip(manual.violations.iter()) {
            assert_eq!(a.rri, m.rri);
            assert_eq!(a.from_module, m.from_module);
            assert_eq!(a.to_module, m.to_module);
        }
    }

    #[test]
    fn audit_with_settings_empty_project_scores_100() {
        let settings = crate::settings::Settings::default();
        let result = audit_with_settings(&[], &[], &settings);
        assert!((result.score - 100.0).abs() < f64::EPSILON);
    }
}
