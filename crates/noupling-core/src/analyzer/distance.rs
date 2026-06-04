//! Per-directory Distance from the Main Sequence: Martin's `D = |A + I - 1|`.
//!
//! - `A` is per-directory abstractness from [`crate::analyzer::abstractness`].
//! - `I` is per-directory instability from [`crate::analyzer::instability`].
//! - `D` ranges 0.0 (on the main sequence — well-balanced) to 1.0 (worst).
//!
//! Two "danger zones" are recognized when `D` is high:
//!
//! - **Zone of Pain** (low A, low I): stable and concrete. Rigid — lots of code
//!   depends on it but it has no abstractions, so every change ripples widely.
//! - **Zone of Uselessness** (high A, high I): abstract and unstable.
//!   Abstractions nobody depends on — speculative architecture that pays no rent.
//!
//! The main sequence (`A + I = 1`) is the locus where stability is justified by
//! abstraction: stable modules are abstract (so they can be implemented many
//! ways), unstable modules are concrete (so they can change without breaking
//! anyone).

use fxhash::FxHashMap;

use crate::analyzer::abstractness::AbstractnessMetric;
use crate::analyzer::instability::InstabilityMetric;

/// Which danger zone a directory falls into when `D` is high, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    MainSequence,
    Pain,
    Uselessness,
}

/// Distance from main sequence for one directory.
#[derive(Debug, Clone)]
pub struct DistanceMetric {
    pub dir: String,
    pub abstractness: f64,
    pub instability: f64,
    /// `D = |A + I - 1|`. Range 0.0 (main sequence) to 1.0 (corner).
    pub distance: f64,
    pub zone: Zone,
}

/// Compute per-directory distance from main sequence by joining abstractness
/// and instability on directory path. Returns only directories that appear in
/// **both** inputs (so a directory must have *some* type declarations and
/// *some* boundary-crossing edges to be classified). Sorted by directory path
/// for stable output.
///
/// `pain_threshold` is the value of `D` above which a low-I directory is
/// flagged as Zone of Pain (and a high-I one as Zone of Uselessness). 0.5 is
/// the conventional cutoff: anything more than halfway off the main sequence.
pub fn compute_distance(
    abstractness: &[AbstractnessMetric],
    instability: &[InstabilityMetric],
    pain_threshold: f64,
) -> Vec<DistanceMetric> {
    let i_by_dir: FxHashMap<&str, f64> = instability
        .iter()
        .map(|m| (m.dir.as_str(), m.instability))
        .collect();

    let mut result: Vec<DistanceMetric> = abstractness
        .iter()
        .filter_map(|a| {
            let i = *i_by_dir.get(a.dir.as_str())?;
            let d = (a.abstractness + i - 1.0).abs();
            let zone = if d <= pain_threshold {
                Zone::MainSequence
            } else if i >= 0.5 {
                Zone::Uselessness
            } else {
                Zone::Pain
            };
            Some(DistanceMetric {
                dir: a.dir.clone(),
                abstractness: a.abstractness,
                instability: i,
                distance: d,
                zone,
            })
        })
        .collect();
    result.sort_by(|a, b| a.dir.cmp(&b.dir));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::abstractness::AbstractnessMetric;
    use crate::analyzer::instability::InstabilityMetric;

    fn a(dir: &str, abstractness: f64) -> AbstractnessMetric {
        AbstractnessMetric {
            dir: dir.into(),
            abstract_count: 0,
            concrete_count: 0,
            abstractness,
        }
    }

    fn i(dir: &str, instability: f64) -> InstabilityMetric {
        InstabilityMetric {
            dir: dir.into(),
            ca: 0,
            ce: 0,
            instability,
        }
    }

    #[test]
    fn classifies_low_a_low_i_as_zone_of_pain() {
        let abstractness = vec![a("src/schema", 0.0)];
        let instability = vec![i("src/schema", 0.0)];
        let r = compute_distance(&abstractness, &instability, 0.5);
        assert_eq!(r.len(), 1);
        assert!((r[0].distance - 1.0).abs() < 1e-9);
        assert_eq!(r[0].zone, Zone::Pain);
    }
}
