//! Architectural red flag detection — Fused Sibling and Trapped Child
//! anti-patterns derived from the violation set.

use super::CouplingViolation;
use super::DependencyDirection;

/// An architectural anti-pattern detected from the dependency analysis.
#[derive(Debug, Clone)]
pub struct RedFlag {
    /// The type of anti-pattern.
    pub flag_type: RedFlagType,
    /// Modules involved (file paths or directory paths).
    pub modules: Vec<String>,
    /// The RRI that triggered this flag.
    pub rri: f64,
    /// Actionable recommendation to fix the issue.
    pub recommendation: String,
}

/// Types of architectural red flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedFlagType {
    /// Two sibling modules with density higher than 2× the median sibling density.
    FusedSibling,
    /// A module with upward dependencies (child imports parent).
    TrappedChild,
}

/// Detect architectural red flags from violation data.
pub fn compute_red_flags(
    violations: &[CouplingViolation],
    coupling_metrics: &[CouplingViolation],
) -> Vec<RedFlag> {
    let mut flags = Vec::new();

    // Fused Sibling: sibling pairs with density > 2× median.
    let all_siblings: Vec<&CouplingViolation> = violations
        .iter()
        .chain(coupling_metrics.iter())
        .filter(|v| v.direction == DependencyDirection::Sibling)
        .collect();

    if all_siblings.len() >= 2 {
        let mut densities: Vec<usize> = all_siblings.iter().map(|v| v.weight.max(1)).collect();
        densities.sort();
        let median_density = densities[densities.len() / 2] as f64;
        let threshold = (median_density * 2.0).max(2.0);

        for v in &all_siblings {
            if v.weight as f64 > threshold {
                flags.push(RedFlag {
                    flag_type: RedFlagType::FusedSibling,
                    modules: vec![v.from_module.clone(), v.to_module.clone()],
                    rri: v.rri,
                    recommendation: format!(
                        "{} and {} have {} imports between them (median: {:.0}). \
                         Consider merging them or extracting a shared abstraction.",
                        v.dir_a, v.dir_b, v.weight, median_density
                    ),
                });
            }
        }
    }

    // Trapped Child: any module with upward dependencies.
    let upward: Vec<&CouplingViolation> = violations
        .iter()
        .chain(coupling_metrics.iter())
        .filter(|v| v.direction == DependencyDirection::Upward)
        .collect();

    for v in &upward {
        flags.push(RedFlag {
            flag_type: RedFlagType::TrappedChild,
            modules: vec![v.from_module.clone(), v.to_module.clone()],
            rri: v.rri,
            recommendation: format!(
                "{} imports from parent {}. This module cannot be reused \
                 without its parent. Invert the dependency or use an interface.",
                v.from_module, v.to_module
            ),
        });
    }

    flags.sort_by(|a, b| b.rri.partial_cmp(&a.rri).unwrap());
    flags
}
