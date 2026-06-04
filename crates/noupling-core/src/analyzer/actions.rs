//! Prioritized actions ranked by ROI: circular > layer > rule > hotspot.

use super::{AuditResult, ModuleMetrics};

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
