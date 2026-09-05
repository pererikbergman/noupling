//! Violation age tracking — classify violations as new / recent / chronic
//! based on their presence in historical snapshots.

use super::CouplingViolation;

/// Summary of violation ages across snapshots.
#[derive(Debug, Clone, Default)]
pub struct ViolationAgeSummary {
    /// Violations that first appeared in the latest snapshot.
    pub new_count: usize,
    /// Violations that have existed for 2-4 snapshots.
    pub recent_count: usize,
    /// Violations that have existed for 5+ snapshots.
    pub chronic_count: usize,
}

/// Compute violation ages by comparing current violations against historical snapshots.
/// Returns an updated ViolationAgeSummary.
/// The identity a violation keeps across snapshots: the directory pair
/// (the same identity the baseline uses), not whichever import happens
/// to represent it this scan. Cycles key on their ring.
pub fn age_key(v: &CouplingViolation) -> (String, String) {
    if v.is_circular {
        (v.cycle_path.join(" -> "), String::new())
    } else {
        (v.dir_a.clone(), v.dir_b.clone())
    }
}

pub fn compute_violation_age(
    current_violations: &[CouplingViolation],
    historical_violation_sets: &[Vec<(String, String)>], // Vec of `age_key`s per snapshot
) -> ViolationAgeSummary {
    let mut new_count = 0;
    let mut recent_count = 0;
    let mut chronic_count = 0;

    for v in current_violations {
        let fingerprint = age_key(v);
        let age = historical_violation_sets
            .iter()
            .filter(|snap_violations| snap_violations.contains(&fingerprint))
            .count();

        if age == 0 {
            new_count += 1;
        } else if age < 5 {
            recent_count += 1;
        } else {
            chronic_count += 1;
        }
    }

    ViolationAgeSummary {
        new_count,
        recent_count,
        chronic_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::DependencyDirection;

    #[test]
    fn violation_age_all_new() {
        let violations = vec![CouplingViolation {
            dir_a: "src/a".to_string(),
            dir_b: "src/b".to_string(),
            from_module: "src/a/main.rs".to_string(),
            to_module: "src/b/lib.rs".to_string(),
            line_number: 1,
            depth: 1,
            weight: 1,
            severity: 0.5,
            direction: DependencyDirection::Sibling,
            rri: 0.0,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
            score_impact: 0.0,
        }];
        // No historical snapshots
        let age = compute_violation_age(&violations, &[]);
        assert_eq!(age.new_count, 1);
        assert_eq!(age.recent_count, 0);
        assert_eq!(age.chronic_count, 0);
    }

    #[test]
    fn violation_age_chronic() {
        let violations = vec![CouplingViolation {
            dir_a: "src/a".to_string(),
            dir_b: "src/b".to_string(),
            from_module: "src/a/main.rs".to_string(),
            to_module: "src/b/lib.rs".to_string(),
            line_number: 1,
            depth: 1,
            weight: 1,
            severity: 0.5,
            direction: DependencyDirection::Sibling,
            rri: 0.0,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
            score_impact: 0.0,
        }];
        // Same directory pair in 6 historical snapshots -> chronic, even
        // though the representative import differed back then (the pair is
        // the violation's identity, like the baseline's).
        let mut earlier = violations[0].clone();
        earlier.from_module = "src/a/other.rs".to_string();
        let fp = vec![age_key(&earlier)];
        assert_eq!(fp[0], ("src/a".to_string(), "src/b".to_string()));
        let historical: Vec<Vec<(String, String)>> = vec![
            fp.clone(),
            fp.clone(),
            fp.clone(),
            fp.clone(),
            fp.clone(),
            fp,
        ];
        let age = compute_violation_age(&violations, &historical);
        assert_eq!(age.new_count, 0);
        assert_eq!(age.chronic_count, 1);
    }
}
