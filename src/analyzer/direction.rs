//! Dependency direction classification for the architectural risk framework.

use serde::{Deserialize, Serialize};

/// The architectural direction of a dependency between two modules.
///
/// Used to assign risk weights in the dependency risk framework:
/// downward (2) < sibling (4) < upward (6) < external (8) < transitive (9) < circular (10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyDirection {
    /// Parent directory imports from child directory. Low risk.
    #[serde(rename = "downward")]
    Downward,
    /// Same-level directories import each other. Moderate risk.
    #[serde(rename = "sibling")]
    Sibling,
    /// Child directory imports from parent directory. High risk.
    #[serde(rename = "upward")]
    Upward,
    /// Import of a third-party package not in the project source tree. Critical.
    #[serde(rename = "external")]
    External,
    /// Indirect dependency through another module (A depends on C only via B). Extreme.
    #[serde(rename = "transitive")]
    Transitive,
    /// Mutual or transitive cycle between directories. Lethal.
    #[serde(rename = "circular")]
    Circular,
}

#[cfg(test)]
mod tests {
    use super::DependencyDirection;

    #[test]
    fn dependency_direction_serde_roundtrip() {
        let variants = [
            (DependencyDirection::Downward, "\"downward\""),
            (DependencyDirection::Sibling, "\"sibling\""),
            (DependencyDirection::Upward, "\"upward\""),
            (DependencyDirection::External, "\"external\""),
            (DependencyDirection::Transitive, "\"transitive\""),
            (DependencyDirection::Circular, "\"circular\""),
        ];
        for (variant, expected_json) in &variants {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(&json, expected_json);
            let deserialized: DependencyDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(&deserialized, variant);
        }
    }
}
