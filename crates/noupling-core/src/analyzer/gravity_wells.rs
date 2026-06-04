//! Gravity Well detection — modules with disproportionately high aggregate
//! RRI across multiple dependency types. These "God Objects" pull the
//! system into their orbit, making extraction or replacement difficult.

use super::CouplingViolation;
use super::DependencyDirection;

/// A module with disproportionately high aggregate RRI across multiple
/// dependency types. These "God Objects" pull the system into their orbit,
/// making extraction or replacement extremely difficult.
#[derive(Debug, Clone)]
pub struct GravityWell {
    /// File path of the module (or directory).
    pub module_path: String,
    /// Total RRI across all violations involving this module.
    pub total_rri: f64,
    /// Number of distinct violations this module participates in.
    pub relationship_count: usize,
    /// RRI broken down by dependency direction.
    pub downward_rri: f64,
    pub sibling_rri: f64,
    pub upward_rri: f64,
    pub circular_rri: f64,
    /// Number of distinct direction types with non-zero RRI.
    pub direction_count: usize,
}

/// Identify modules with disproportionately high aggregate RRI.
/// A module is a Gravity Well when its total RRI exceeds 2× the median.
pub fn compute_gravity_wells(
    violations: &[CouplingViolation],
    coupling_metrics: &[CouplingViolation],
) -> Vec<GravityWell> {
    use std::collections::HashMap;

    // Aggregate RRI per module path, broken down by direction.
    struct ModuleRisk {
        total_rri: f64,
        count: usize,
        downward: f64,
        sibling: f64,
        upward: f64,
        circular: f64,
    }

    let mut risk_map: HashMap<String, ModuleRisk> = HashMap::new();

    let all_violations = violations.iter().chain(coupling_metrics.iter());
    for v in all_violations {
        // Each violation involves from_module and to_module
        for path in [&v.from_module, &v.to_module] {
            if path.is_empty() {
                continue;
            }
            let entry = risk_map.entry(path.clone()).or_insert(ModuleRisk {
                total_rri: 0.0,
                count: 0,
                downward: 0.0,
                sibling: 0.0,
                upward: 0.0,
                circular: 0.0,
            });
            entry.total_rri += v.rri;
            entry.count += 1;
            match v.direction {
                DependencyDirection::Downward => entry.downward += v.rri,
                DependencyDirection::Sibling => entry.sibling += v.rri,
                DependencyDirection::Upward => entry.upward += v.rri,
                DependencyDirection::External | DependencyDirection::Transitive => {
                    entry.sibling += v.rri
                }
                DependencyDirection::Circular => entry.circular += v.rri,
            }
        }
    }

    if risk_map.is_empty() {
        return Vec::new();
    }

    // Find median total_rri
    let mut rris: Vec<f64> = risk_map.values().map(|r| r.total_rri).collect();
    rris.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = rris[rris.len() / 2];

    // Gravity Well = total_rri > 2× median AND count >= 2
    let threshold = (median * 2.0).max(1.0);
    let mut wells: Vec<GravityWell> = risk_map
        .into_iter()
        .filter(|(_, r)| r.total_rri > threshold && r.count >= 2)
        .map(|(path, r)| {
            let direction_count = [r.downward, r.sibling, r.upward, r.circular]
                .iter()
                .filter(|&&v| v > 0.0)
                .count();
            GravityWell {
                module_path: path,
                total_rri: r.total_rri,
                relationship_count: r.count,
                downward_rri: r.downward,
                sibling_rri: r.sibling,
                upward_rri: r.upward,
                circular_rri: r.circular,
                direction_count,
            }
        })
        .collect();

    wells.sort_by(|a, b| b.total_rri.partial_cmp(&a.total_rri).unwrap());
    wells
}
