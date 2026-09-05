//! Reporter crate root. Owns the shared `Report` shape (`data`
//! module) and the per-format adapters as siblings — xml, sonar,
//! text, pr, briefing, plus the pre-existing html / dashboard /
//! bundle / graph / md / strategy reporters.
//!
//! Issue #319: this module used to hold `JsonReport` plus three
//! formats plus the test suite (~2000 LOC). The shape moved to
//! `data.rs`, each format to its own sibling; mod.rs is now a thin
//! re-export shell that callers (`report_formatter`, `commands/audit`,
//! `commands/report`, `md`, …) keep using through unchanged paths.

mod briefing;
mod bundle;
mod dashboard;
mod data;
mod graph;
mod html;
mod md;
mod pr;
mod sonar;
mod strategy;
mod text;
mod xml;

/// The version string used across all report outputs.
pub const VERSION: &str = concat!("noupling v", env!("CARGO_PKG_VERSION"));

pub use briefing::format_briefing;
pub use bundle::generate_bundle_report;
pub use dashboard::generate_dashboard;
// Only the types consumed by sibling reporters cross the `mod.rs`
// surface — md.rs reaches `super::JsonReport` + `super::JsonDirectory`.
// The other Json* sub-structs stay scoped to `data::` so the public
// surface of the reporter crate doesn't carry every wire shape.
pub use data::{JsonDirectory, JsonReport};
pub use graph::{format_dot, format_mermaid};
pub use html::generate_html_report;
pub use md::generate_markdown_report;

/// Remove the pages a previous multi-file report left in `output_dir`:
/// every subdirectory (the per-directory pages mirror the source tree, so
/// a renamed or deleted source directory would otherwise keep serving its
/// old page) plus the root page file. Anything else at the top level was
/// not written by noupling and is left alone (#383).
pub(crate) fn clear_generated_pages(
    output_dir: &std::path::Path,
    root_page: &str,
) -> std::io::Result<()> {
    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return Ok(()); // nothing generated yet
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else if entry.file_name() == root_page {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}
pub use pr::format_pr;
pub use sonar::format_sonar;
pub use strategy::generate_strategy_report;
pub use text::{format_monorepo_text, format_text};
pub use xml::format_xml;

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::analyzer::{AuditResultBuilder, CouplingViolation};
    use noupling_core::core::Module;

    fn make_violation(from: &str, to: &str, severity: f64, depth: i32) -> CouplingViolation {
        CouplingViolation {
            dir_a: "dir_a".to_string(),
            dir_b: "dir_b".to_string(),
            from_module: from.to_string(),
            to_module: to.to_string(),
            depth,
            severity,
            direction: noupling_core::analyzer::DependencyDirection::Sibling,
            rri: 0.0,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
            score_impact: 0.0,
            line_number: 0,
            weight: 0,
        }
    }

    #[test]
    fn json_report_has_required_fields() {
        let modules = vec![];
        let result = AuditResultBuilder::new()
            .with_violations(vec![make_violation("a.rs", "b.rs", 1.0, 0)])
            .with_score(50.0)
            .with_total_modules(2)
            .build();

        let report = JsonReport::from_audit(&modules, &result, "snap-1");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["snapshot_id"], "snap-1");
        assert_eq!(parsed["score"], 50.0);
        assert_eq!(parsed["total_modules"], 2);
        assert_eq!(parsed["critical_violations"], 1);
        assert_eq!(parsed["total_coupling"], 1);
        assert_eq!(parsed["total_circular"], 0);
    }

    #[test]
    fn json_report_valid_json() {
        let modules = vec![];
        let result = AuditResultBuilder::new().with_total_modules(5).build();

        let report = JsonReport::from_audit(&modules, &result, "snap-2");
        let json = report.to_json().unwrap();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn critical_violations_counts_high_severity() {
        let modules = vec![];
        let result = AuditResultBuilder::new()
            .with_violations(vec![
                make_violation("a.rs", "b.rs", 1.0, 0),
                make_violation("c.rs", "d.rs", 0.5, 1),
                make_violation("e.rs", "f.rs", 0.25, 2),
            ])
            .with_score(42.0)
            .with_total_modules(6)
            .build();

        let report = JsonReport::from_audit(&modules, &result, "snap-3");
        assert_eq!(report.critical_violations, 2);
    }

    #[test]
    fn text_format_shows_score_and_violations() {
        let result = AuditResultBuilder::new()
            .with_violations(vec![make_violation(
                "scanner/mod.rs",
                "storage/mod.rs",
                0.5,
                1,
            )])
            .with_score(75.0)
            .with_total_modules(4)
            .build();

        let text = format_text(&result);
        assert!(text.contains("Health Score: 75.0/100"));
        assert!(text.contains("Violations: 1"));
        assert!(text.contains("scanner/mod.rs"));
    }

    #[test]
    fn text_format_clean_when_no_violations() {
        let result = AuditResultBuilder::new().with_total_modules(4).build();

        let text = format_text(&result);
        assert!(text.contains("Health Score: 100.0/100"));
        assert!(text.contains("Violations: 0"));
    }

    #[test]
    fn json_report_stability_violations_live_in_issues_not_a_per_kind_array() {
        use noupling_core::analyzer::StabilityViolation;
        let modules = vec![];
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_stability_violations(vec![StabilityViolation {
                from_dir: "src/stable".into(),
                to_dir: "src/unstable".into(),
                from_instability: 0.17,
                to_instability: 0.83,
            }])
            .build();
        let report = JsonReport::from_audit(&modules, &result, "snap-s");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // ADR 0002 / #350: the per-kind array is gone; filter `issues` by kind.
        assert!(parsed.get("stability_violations").is_none());
        let stability: Vec<&serde_json::Value> = parsed["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "stability_violation")
            .collect();
        assert_eq!(stability.len(), 1);
        assert_eq!(stability[0]["subject"]["from"], "src/stable");
        assert_eq!(stability[0]["subject"]["to"], "src/unstable");
        assert_eq!(stability[0]["detail"]["from_instability"], 0.17);
    }

    #[test]
    fn json_report_includes_instability() {
        use noupling_core::analyzer::InstabilityMetric;
        let modules = vec![];
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_instability(vec![InstabilityMetric {
                dir: "src/core".into(),
                ca: 5,
                ce: 1,
                instability: 1.0 / 6.0,
            }])
            .build();

        let report = JsonReport::from_audit(&modules, &result, "snap-z");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed["instability"].as_array().expect("instability array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["dir"], "src/core");
        assert_eq!(arr[0]["ca"], 5);
        assert_eq!(arr[0]["ce"], 1);
        let i = arr[0]["instability"].as_f64().unwrap();
        assert!((i - 1.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn json_report_includes_abstractness() {
        use noupling_core::analyzer::AbstractnessMetric;
        let modules = vec![];
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_abstractness(vec![AbstractnessMetric {
                dir: "src/api".into(),
                abstract_count: 1,
                concrete_count: 4,
                abstractness: 0.2,
            }])
            .build();

        let report = JsonReport::from_audit(&modules, &result, "snap-z");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed["abstractness"]
            .as_array()
            .expect("abstractness array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["dir"], "src/api");
        assert_eq!(arr[0]["abstract_count"], 1);
        assert_eq!(arr[0]["concrete_count"], 4);
        let a = arr[0]["abstractness"].as_f64().unwrap();
        assert!((a - 0.2).abs() < 1e-9);
    }

    #[test]
    fn text_format_notes_inferred_layers_once_and_names_them() {
        use noupling_core::settings::Layer;
        let layer = |name: &str, pattern: &str| Layer {
            name: name.into(),
            pattern: pattern.into(),
            allow_sibling: false,
            max_sibling_density: None,
            reduced_sibling_weight: 2.5,
        };
        let result = AuditResultBuilder::new()
            .with_total_modules(9)
            .with_layers(
                vec![
                    layer("presentation", "**/ui/**"),
                    layer("data", "**/data/**"),
                ],
                true,
            )
            .build();

        let text = format_text(&result);

        let note = text
            .lines()
            .find(|l| l.starts_with("Layers: inferred"))
            .unwrap_or_else(|| panic!("missing inferred-layers note:\n{text}"));
        assert!(note.contains("presentation, data"), "{note}");
        assert!(
            note.contains("settings.json"),
            "must say how to opt out: {note}"
        );
        assert_eq!(
            text.matches("Layers: inferred").count(),
            1,
            "one line, not a section"
        );
    }

    #[test]
    fn text_format_omits_layer_note_when_layers_are_configured() {
        let result = AuditResultBuilder::new().with_total_modules(3).build();
        let text = format_text(&result);
        assert!(!text.contains("Layers:"), "{text}");
    }

    #[test]
    fn text_format_renders_stability_violation_as_an_issue_card() {
        use noupling_core::analyzer::StabilityViolation;
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_stability_violations(vec![StabilityViolation {
                from_dir: "src/stable".into(),
                to_dir: "src/unstable".into(),
                from_instability: 0.17,
                to_instability: 0.83,
            }])
            .build();
        let text = format_text(&result);
        assert!(
            text.contains("[MEDIUM] Stability Violation: src/stable -> src/unstable"),
            "missing card header: {}",
            text
        );
        assert!(text.contains("0.17"), "missing from_i");
        assert!(text.contains("0.83"), "missing to_i");
    }

    /// The Issue cards are the text report's Issue sections: every card
    /// carries kind, band, subject, reason and recommendation, in
    /// `issues()` order, under one "Issues (N)" heading.
    #[test]
    fn text_format_renders_issue_cards_from_issues_in_canonical_order() {
        use noupling_core::analyzer::{CouplingViolation, DependencyDirection};
        let sibling = CouplingViolation {
            dir_a: "src/a".into(),
            dir_b: "src/b".into(),
            from_module: "src/a/x.rs".into(),
            to_module: "src/b/y.rs".into(),
            line_number: 3,
            depth: 1,
            weight: 2,
            severity: 1.0,
            direction: DependencyDirection::Sibling,
            rri: 8.0,
            is_circular: false,
            cycle_path: vec![],
            cycle_hop_files: vec![],
            cycle_order: 0,
            cycle_hop_counts: vec![],
            weakest_link: None,
            break_cost: 0,
            score_impact: 0.0,
        };
        let cycle = CouplingViolation {
            dir_a: "src/p".into(),
            dir_b: "src/q".into(),
            from_module: "src/p".into(),
            to_module: "src/q".into(),
            line_number: 0,
            depth: 1,
            weight: 0,
            severity: 0.1,
            direction: DependencyDirection::Circular,
            rri: 20.0,
            is_circular: true,
            cycle_path: vec!["src/p".into(), "src/q".into(), "src/p".into()],
            cycle_hop_files: vec![],
            cycle_order: 2,
            cycle_hop_counts: vec![1, 3],
            weakest_link: Some("src/p -> src/q (1 import)".into()),
            break_cost: 1,
            score_impact: 0.0,
        };
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_violations(vec![cycle, sibling])
            .build();

        let text = format_text(&result);

        assert!(text.contains("Issues (2)"), "{text}");
        let coupling_at = text
            .find("[CRITICAL] Coupling Violation: src/a/x.rs -> src/b/y.rs")
            .unwrap_or_else(|| panic!("coupling card missing:\n{text}"));
        let cycle_at = text
            .find("[MEDIUM] Cycle: src/p -> src/q -> src/p")
            .unwrap_or_else(|| panic!("cycle card missing:\n{text}"));
        assert!(
            coupling_at < cycle_at,
            "critical must come before medium:\n{text}"
        );
        assert!(
            text.contains("Reason: src/a/x.rs imports across sibling"),
            "{text}"
        );
        assert!(
            text.contains("Recommendation: Move the shared code"),
            "{text}"
        );
        assert!(
            text.contains("Recommendation: Cut the cycle at src/p -> src/q"),
            "{text}"
        );
        // The old per-kind sections are gone.
        assert!(!text.contains("Stability Violations:"));
        assert!(!text.contains("Red Flags ("));
    }

    /// The breakdown line sums to the headline points lost and each card
    /// shows its own score impact.
    #[test]
    fn text_format_points_lost_breakdown_adds_up_to_the_headline() {
        use noupling_core::analyzer::{CouplingViolation, DependencyDirection};
        let mut sibling = CouplingViolation {
            dir_a: "src/a".into(),
            dir_b: "src/b".into(),
            from_module: "src/a/x.rs".into(),
            to_module: "src/b/y.rs".into(),
            line_number: 3,
            depth: 1,
            weight: 2,
            severity: 1.0,
            direction: DependencyDirection::Sibling,
            rri: 8.0,
            is_circular: false,
            cycle_path: vec![],
            cycle_hop_files: vec![],
            cycle_order: 0,
            cycle_hop_counts: vec![],
            weakest_link: None,
            break_cost: 0,
            score_impact: 0.0,
        };
        let mut cycle = sibling.clone();
        cycle.dir_a = "src/p".into();
        cycle.dir_b = "src/q".into();
        cycle.from_module = "src/p".into();
        cycle.to_module = "src/q".into();
        cycle.severity = 0.4;
        cycle.direction = DependencyDirection::Circular;
        cycle.is_circular = true;
        cycle.cycle_path = vec!["src/p".into(), "src/q".into(), "src/p".into()];
        cycle.cycle_order = 2;
        sibling.score_impact = 20.0;
        cycle.score_impact = 8.0;
        let result = AuditResultBuilder::new()
            .with_total_modules(5)
            .with_score(72.0)
            .with_violations(vec![cycle, sibling])
            .build();

        let text = format_text(&result);

        assert!(
            text.contains("Points lost: 28.0 (Coupling Violation 20.0, Cycle 8.0)"),
            "{text}"
        );
        assert!(text.contains("Score impact: 20.0"), "{text}");
        assert!(text.contains("Score impact: 8.0"), "{text}");
    }

    #[test]
    fn text_format_marks_baselined_cards_and_prints_new_vs_baselined_counts() {
        use noupling_core::analyzer::{CouplingViolation, DependencyDirection};
        use noupling_core::baseline::Baseline;
        // Two different directory pairs: the fingerprint is the pair.
        let edge = |from: &str, to: &str| CouplingViolation {
            dir_a: from.rsplit_once('/').unwrap().0.into(),
            dir_b: to.rsplit_once('/').unwrap().0.into(),
            from_module: from.into(),
            to_module: to.into(),
            line_number: 1,
            depth: 1,
            weight: 1,
            severity: 0.5,
            direction: DependencyDirection::Sibling,
            rri: 4.0,
            is_circular: false,
            cycle_path: vec![],
            cycle_hop_files: vec![],
            cycle_order: 0,
            cycle_hop_counts: vec![],
            weakest_link: None,
            break_cost: 0,
            score_impact: 1.0,
        };
        let mut result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_violations(vec![
                edge("src/a/old.rs", "src/b/y.rs"),
                edge("src/c/new.rs", "src/b/y.rs"),
            ])
            .build();
        let accepted = Baseline {
            fingerprints: ["coupling_violation:src/a -> src/b".to_string()]
                .into_iter()
                .collect(),
            legacy_format: false,
        };
        result.apply_baseline(&accepted);

        let text = format_text(&result);

        assert!(text.contains("Issues (2): 1 new, 1 baselined"), "{text}");
        assert!(
            text.contains("[CRITICAL] Coupling Violation: src/a/old.rs -> src/b/y.rs (baselined)"),
            "{text}"
        );
        assert!(
            text.contains("[CRITICAL] Coupling Violation: src/c/new.rs -> src/b/y.rs\n"),
            "new issue must not be marked: {text}"
        );
    }

    #[test]
    fn text_format_warns_once_about_an_old_format_baseline() {
        use noupling_core::baseline::Baseline;
        let mut result = AuditResultBuilder::new().with_total_modules(4).build();
        result.apply_baseline(&Baseline {
            fingerprints: Default::default(),
            legacy_format: true,
        });
        let text = format_text(&result);
        let warnings: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with("Baseline:"))
            .collect();
        assert_eq!(warnings.len(), 1, "{text}");
        assert!(warnings[0].contains("baseline save"), "{}", warnings[0]);
    }

    /// The headline `Violations:` agrees with the cards: a ring's hop edges
    /// count once, as their Cycle (#358).
    #[test]
    fn text_headline_violations_count_matches_the_issue_cards() {
        use noupling_core::analyzer::{audit_with_settings, IssueKind};
        use noupling_core::core::{Dependency, ModuleType};
        use noupling_core::settings::Settings;
        let file = |id: &str, path: &str| Module {
            id: id.into(),
            snapshot_id: "s".into(),
            parent_id: None,
            name: path.rsplit('/').next().unwrap().into(),
            path: path.into(),
            module_type: ModuleType::File,
            depth: 3,
        };
        let dep = |from: &str, to: &str| Dependency {
            from_module_id: from.into(),
            to_module_id: to.into(),
            line_number: 1,
        };
        let modules = vec![
            file("a", "src/ring/alpha/a.rs"),
            file("b", "src/ring/beta/b.rs"),
            file("x", "src/loose/x/x1.rs"),
            file("y", "src/loose/y/y1.rs"),
        ];
        let deps = vec![dep("a", "b"), dep("b", "a"), dep("x", "y")];
        let result = audit_with_settings(&modules, &deps, &[], &Settings::default());
        assert_eq!(result.violations.len(), 4, "raw list keeps the hops");

        let text = format_text(&result);
        let cards = result
            .issues()
            .iter()
            .filter(|i| matches!(i.kind(), IssueKind::Cycle | IssueKind::CouplingViolation))
            .count();
        assert_eq!(cards, 2);
        assert!(text.contains("Violations: 2\n"), "{text}");
    }

    #[test]
    fn text_format_has_no_issues_heading_when_there_are_none() {
        let result = AuditResultBuilder::new().with_total_modules(4).build();
        let text = format_text(&result);
        assert!(!text.contains("Issues ("), "{text}");
    }

    /// The shared `issues` array (ADR 0002): one Issue card per Issue,
    /// header fields plus a per-kind `detail` payload.
    #[test]
    fn json_report_issues_array_carries_the_card_header_and_a_detail_payload() {
        use noupling_core::analyzer::{CouplingViolation, DependencyDirection, StabilityViolation};
        let cycle = CouplingViolation {
            dir_a: "src/p".into(),
            dir_b: "src/q".into(),
            from_module: "src/p".into(),
            to_module: "src/q".into(),
            line_number: 0,
            depth: 1,
            weight: 0,
            severity: 0.6,
            direction: DependencyDirection::Circular,
            rri: 20.0,
            is_circular: true,
            cycle_path: vec!["src/p".into(), "src/q".into(), "src/p".into()],
            cycle_hop_files: vec![("src/p/a.rs".into(), "src/q/b.rs".into(), 3)],
            cycle_order: 2,
            cycle_hop_counts: vec![1, 3],
            weakest_link: Some("src/p -> src/q (1 import)".into()),
            break_cost: 1,
            score_impact: 12.0,
        };
        let result = AuditResultBuilder::new()
            .with_total_modules(5)
            .with_violations(vec![cycle])
            .with_stability_violations(vec![StabilityViolation {
                from_dir: "src/stable".into(),
                to_dir: "src/unstable".into(),
                from_instability: 0.17,
                to_instability: 0.83,
            }])
            .build();

        let json = JsonReport::from_audit(&[], &result, "snap-i")
            .to_json()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let issues = parsed["issues"].as_array().expect("issues array");
        assert_eq!(issues.len(), 2);

        let cycle = &issues[0];
        assert_eq!(cycle["kind"], "cycle");
        assert_eq!(cycle["kind_name"], "Cycle");
        assert_eq!(cycle["severity"], "critical");
        assert_eq!(cycle["subject"]["type"], "ring");
        assert_eq!(cycle["subject"]["members"][1], "src/q");
        assert!(cycle["reason"].as_str().unwrap().contains("cheapest break"));
        assert!(cycle["recommendation"]
            .as_str()
            .unwrap()
            .starts_with("Cut the cycle"));
        assert_eq!(cycle["score_impact"], 12.0);
        assert_eq!(cycle["baselined"], false);
        assert_eq!(cycle["fingerprint"], "cycle:src/p -> src/q -> src/p");
        assert_eq!(cycle["detail"]["break_cost"], 1);
        assert_eq!(cycle["detail"]["hops"][0]["from_file"], "src/p/a.rs");

        let stability = &issues[1];
        assert_eq!(stability["kind"], "stability_violation");
        assert_eq!(stability["severity"], "medium");
        assert_eq!(stability["subject"]["type"], "edge");
        assert_eq!(stability["subject"]["from"], "src/stable");
        assert_eq!(stability["score_impact"], 0.0);
        assert_eq!(stability["detail"]["from_instability"], 0.17);
    }

    fn result_with_a_zone_flag_and_a_rule_violation() -> noupling_core::analyzer::AuditResult {
        use noupling_core::analyzer::{DistanceMetric, RuleViolation, Zone};
        AuditResultBuilder::new()
            .with_total_modules(5)
            .with_rule_violations(vec![RuleViolation {
                from_module: "src/plugins/x.rs".into(),
                to_module: "src/legacy/y.rs".into(),
                line_number: 7,
                message: "plugins must not reach into legacy".into(),
            }])
            .with_distance(vec![DistanceMetric {
                dir: "src/concrete".into(),
                abstractness: 0.0,
                instability: 0.0,
                distance: 1.0,
                zone: Zone::Pain,
            }])
            .build()
    }

    /// XML derives its `<issues>` from the same cards as JSON.
    #[test]
    fn xml_report_lists_every_issue_from_the_shared_array() {
        let xml = format_xml(
            &[],
            &result_with_a_zone_flag_and_a_rule_violation(),
            "snap-x",
        );
        assert!(xml.contains("<issues count=\"2\">"), "{xml}");
        assert!(
            xml.contains("<issue kind=\"rule_violation\" severity=\"high\" baselined=\"false\" scoreImpact=\"0.0\""),
            "{xml}"
        );
        assert!(
            xml.contains(
                "<subject type=\"edge\" from=\"src/plugins/x.rs\" to=\"src/legacy/y.rs\"/>"
            ),
            "{xml}"
        );
        assert!(xml.contains("<issue kind=\"zone_flag\""), "{xml}");
        assert!(
            xml.contains("<subject type=\"module\" path=\"src/concrete\"/>"),
            "{xml}"
        );
        assert!(
            xml.contains("<reason>src/concrete is in the Zone of Pain"),
            "{xml}"
        );
        assert!(xml.contains("<recommendation>"), "{xml}");
        assert!(
            xml.contains(
                "<detail abstractness=\"0.0\" distance=\"1.0\" instability=\"0.0\" zone=\"pain\"/>"
            ),
            "detail attributes: {xml}"
        );
    }

    /// Nested detail payloads (a Cycle's hops) survive into XML as child
    /// elements, so the removed `<circular-dependencies>` loses nothing.
    #[test]
    fn xml_detail_carries_nested_payloads_as_child_elements() {
        use noupling_core::analyzer::{CouplingViolation, DependencyDirection};
        let cycle = CouplingViolation {
            dir_a: "src/p".into(),
            dir_b: "src/q".into(),
            from_module: "src/p".into(),
            to_module: "src/q".into(),
            line_number: 0,
            depth: 1,
            weight: 0,
            severity: 0.6,
            direction: DependencyDirection::Circular,
            rri: 20.0,
            is_circular: true,
            cycle_path: vec!["src/p".into(), "src/q".into(), "src/p".into()],
            cycle_hop_files: vec![
                ("src/p/a.rs".into(), "src/q/b.rs".into(), 3),
                ("src/q/b.rs".into(), "src/p/a.rs".into(), 9),
            ],
            cycle_order: 2,
            cycle_hop_counts: vec![1, 3],
            weakest_link: Some("src/p -> src/q (1 import)".into()),
            break_cost: 1,
            score_impact: 0.0,
        };
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_violations(vec![cycle])
            .build();
        let xml = format_xml(&[], &result, "snap-x");
        assert!(xml.contains("<detail break_cost=\"1\""), "{xml}");
        assert!(
            xml.contains(
                "<item from_file=\"src/p/a.rs\" line_number=\"3\" to_file=\"src/q/b.rs\"/>"
            ),
            "hop files as child items: {xml}"
        );
        assert!(xml.contains("<hop_import_counts>"), "{xml}");
        assert!(xml.contains("<item>3</item>"), "{xml}");
        assert!(xml.contains("</detail>"), "{xml}");
    }

    /// Sonar emits one generic issue per Issue, for every kind, with the
    /// band mapped to a Sonar severity.
    #[test]
    fn sonar_report_emits_one_issue_per_issue_for_every_kind() {
        let file = |id: &str, path: &str| Module {
            id: id.into(),
            snapshot_id: "s".into(),
            parent_id: None,
            name: path.rsplit('/').next().unwrap().into(),
            path: path.into(),
            module_type: noupling_core::core::ModuleType::File,
            depth: 2,
        };
        // A child package that sorts first must not steal the pin: direct
        // children of the directory win.
        let modules = vec![
            file("z", "src/concrete/zz.rs"),
            file("t", "src/concrete/types.rs"),
            file("n", "src/concrete/aaa/nested.rs"),
        ];
        let sonar = format_sonar(&modules, &result_with_a_zone_flag_and_a_rule_violation());
        let parsed: serde_json::Value = serde_json::from_str(&sonar).unwrap();
        let issues = parsed["issues"].as_array().unwrap();
        assert_eq!(issues.len(), 2);
        let rule = issues
            .iter()
            .find(|i| i["ruleId"] == "noupling:rule_violation")
            .expect("rule violation issue");
        assert_eq!(rule["severity"], "MAJOR");
        assert_eq!(rule["primaryLocation"]["filePath"], "src/plugins/x.rs");
        assert_eq!(rule["primaryLocation"]["textRange"]["startLine"], 7);
        assert!(rule["primaryLocation"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Rule Violation: "));
        let zone = issues
            .iter()
            .find(|i| i["ruleId"] == "noupling:zone_flag")
            .expect("zone flag issue");
        assert_eq!(zone["severity"], "MINOR");
        // Sonar drops locations that are not indexed files, so a
        // directory-shaped Issue pins to the first file in that directory.
        assert_eq!(zone["primaryLocation"]["filePath"], "src/concrete/types.rs");
    }

    /// A result with a Cycle (scoring), a Coupling Violation (scoring), a
    /// Rule Violation and a Zone Flag (non-scoring).
    fn mixed_kind_result() -> noupling_core::analyzer::AuditResult {
        use noupling_core::analyzer::{
            CouplingViolation, DependencyDirection, DistanceMetric, RuleViolation, Zone,
        };
        let sibling = CouplingViolation {
            dir_a: "src/a".into(),
            dir_b: "src/b".into(),
            from_module: "src/a/x.rs".into(),
            to_module: "src/b/y.rs".into(),
            line_number: 3,
            depth: 1,
            weight: 2,
            severity: 0.4,
            direction: DependencyDirection::Sibling,
            rri: 8.0,
            is_circular: false,
            cycle_path: vec![],
            cycle_hop_files: vec![],
            cycle_order: 0,
            cycle_hop_counts: vec![],
            weakest_link: None,
            break_cost: 0,
            score_impact: 2.0,
        };
        let mut cycle = sibling.clone();
        cycle.dir_a = "src/p".into();
        cycle.dir_b = "src/q".into();
        cycle.from_module = "src/p".into();
        cycle.to_module = "src/q".into();
        cycle.severity = 0.6;
        cycle.direction = DependencyDirection::Circular;
        cycle.is_circular = true;
        cycle.cycle_path = vec!["src/p".into(), "src/q".into(), "src/p".into()];
        cycle.cycle_order = 2;
        cycle.cycle_hop_counts = vec![1, 3];
        cycle.weakest_link = Some("src/p -> src/q (1 import)".into());
        cycle.break_cost = 1;
        cycle.score_impact = 6.0;
        AuditResultBuilder::new()
            .with_total_modules(10)
            .with_score(92.0)
            .with_violations(vec![cycle, sibling])
            .with_rule_violations(vec![RuleViolation {
                from_module: "src/plugins/x.rs".into(),
                to_module: "src/legacy/y.rs".into(),
                line_number: 7,
                message: "plugins must not reach into legacy".into(),
            }])
            .with_distance(vec![DistanceMetric {
                dir: "src/concrete".into(),
                abstractness: 0.0,
                instability: 0.0,
                distance: 1.0,
                zone: Zone::Pain,
            }])
            .build()
    }

    /// The PR comment names every kind with a count (zero included) and
    /// lists the top Issues by band with their recommendation.
    #[test]
    fn pr_names_every_kind_with_a_count_and_lists_top_issues() {
        let pr = format_pr(&mixed_kind_result(), None, None, None, None);
        assert!(pr.contains("| Issues | 4 |"), "{pr}");
        let kinds_line = pr
            .lines()
            .find(|l| l.starts_with("**By kind:**"))
            .unwrap_or_else(|| panic!("no by-kind line:\n{pr}"));
        for kind in noupling_core::analyzer::IssueKind::ALL {
            assert!(
                kinds_line.contains(kind.name()),
                "{kind} missing from {kinds_line}"
            );
        }
        assert!(kinds_line.contains("Cycle 1"), "{kinds_line}");
        assert!(kinds_line.contains("Gravity Well 0"), "{kinds_line}");
        assert!(pr.contains("### Top issues"), "{pr}");
        let top_at = pr.find("### Top issues").unwrap();
        let top = &pr[top_at..];
        assert!(
            top.contains("**[CRITICAL] Cycle** `src/p -> src/q -> src/p`"),
            "{top}"
        );
        assert!(top.contains("Cut the cycle at src/p -> src/q"), "{top}");
        assert!(
            !pr.contains("### Red Flags"),
            "old section must be gone: {pr}"
        );
    }

    /// The PR comment stays short: at most five Issue entries however many exist.
    #[test]
    fn pr_top_issues_are_capped() {
        use noupling_core::analyzer::CohesionMetrics;
        use noupling_core::analyzer::DirectoryKind;
        let cohesion: Vec<CohesionMetrics> = (0..20)
            .map(|i| CohesionMetrics {
                dir: format!("src/d{i}"),
                kind: DirectoryKind::Package,
                n_children: 3,
                internal_deps: 0,
                cohesion: Some(0.0),
            })
            .collect();
        let result = AuditResultBuilder::new()
            .with_total_modules(60)
            .with_cohesion(cohesion)
            .build();
        let pr = format_pr(&result, None, None, None, None);
        assert_eq!(pr.matches("**[LOW] Low Cohesion**").count(), 5, "{pr}");
        assert!(pr.contains("… and 15 more"), "{pr}");
    }

    /// Briefing items are Issues ranked by score impact, and the approach
    /// text is the Issue's recommendation verbatim.
    #[test]
    fn briefing_ranks_by_score_impact_and_uses_core_recommendations() {
        let result = mixed_kind_result();
        let issues = result.issues();
        let briefing = format_briefing(&result);
        let first = briefing.find("### 1. ").unwrap();
        let second = briefing.find("### 2. ").unwrap();
        let third = briefing.find("### 3. ").unwrap();
        assert!(briefing[first..second].contains("Cycle"), "{briefing}");
        assert!(
            briefing[second..third].contains("Coupling Violation"),
            "{briefing}"
        );
        for issue in &issues {
            assert!(
                briefing.contains(&format!("- **Approach:** {}", issue.recommendation())),
                "recommendation for {} must appear verbatim:\n{briefing}",
                issue.kind()
            );
        }
        assert!(
            briefing.contains("projected score:** 100.0 (+8.0)"),
            "{briefing}"
        );
    }

    #[test]
    fn json_report_low_cohesion_lives_in_issues_and_containers_never_flag() {
        use noupling_core::analyzer::{CohesionMetrics, DirectoryKind};
        let modules = vec![];
        let result = AuditResultBuilder::new()
            .with_total_modules(2)
            .with_cohesion(vec![
                CohesionMetrics {
                    dir: "src/features".into(),
                    kind: DirectoryKind::Container,
                    n_children: 0,
                    internal_deps: 0,
                    cohesion: None,
                },
                CohesionMetrics {
                    dir: "src/scanner".into(),
                    kind: DirectoryKind::Package,
                    n_children: 3,
                    internal_deps: 0,
                    cohesion: Some(0.0),
                },
            ])
            .build();

        let report = JsonReport::from_audit(&modules, &result, "snap-c");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // ADR 0002 / #350: no `cohesion` array; Low Cohesion is an Issue.
        assert!(parsed.get("cohesion").is_none());
        let low: Vec<&serde_json::Value> = parsed["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "low_cohesion")
            .collect();
        assert_eq!(low.len(), 1, "the Container (cohesion null) never flags");
        assert_eq!(low[0]["subject"]["path"], "src/scanner");
        assert_eq!(low[0]["detail"]["n_children"], 3);
        assert_eq!(low[0]["detail"]["cohesion"], 0.0);
    }

    #[test]
    fn text_format_low_cohesion_section_omits_containers() {
        use noupling_core::analyzer::{CohesionMetrics, DirectoryKind};
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_cohesion(vec![
                CohesionMetrics {
                    dir: "src/features".into(),
                    kind: DirectoryKind::Container,
                    n_children: 0,
                    internal_deps: 0,
                    cohesion: None,
                },
                CohesionMetrics {
                    dir: "src/scanner".into(),
                    kind: DirectoryKind::Package,
                    n_children: 5,
                    internal_deps: 0,
                    cohesion: Some(0.00),
                },
            ])
            .build();

        let text = format_text(&result);

        assert!(
            text.contains("[LOW] Low Cohesion: src/scanner"),
            "package with low cohesion must appear as a card: {text}"
        );
        assert!(
            !text.contains("src/features"),
            "container must not appear in Low Cohesion section: {}",
            text
        );
    }

    #[test]
    fn text_format_includes_instability_section() {
        use noupling_core::analyzer::InstabilityMetric;
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_instability(vec![
                InstabilityMetric {
                    dir: "src/app".into(),
                    ca: 0,
                    ce: 3,
                    instability: 1.0,
                },
                InstabilityMetric {
                    dir: "src/core".into(),
                    ca: 5,
                    ce: 0,
                    instability: 0.0,
                },
            ])
            .build();

        let text = format_text(&result);
        assert!(text.contains("Instability:"), "missing header: {}", text);
        assert!(text.contains("src/app"), "missing dir: {}", text);
        assert!(text.contains("I=1.00"), "missing high-I value: {}", text);
        assert!(text.contains("I=0.00"), "missing low-I value: {}", text);
        assert!(text.contains("Ca=5"), "missing afferent count: {}", text);
        assert!(text.contains("Ce=3"), "missing efferent count: {}", text);
    }

    #[test]
    fn text_format_includes_abstractness_section() {
        use noupling_core::analyzer::AbstractnessMetric;
        let result = AuditResultBuilder::new()
            .with_total_modules(4)
            .with_abstractness(vec![AbstractnessMetric {
                dir: "src/api".into(),
                abstract_count: 2,
                concrete_count: 3,
                abstractness: 0.4,
            }])
            .build();

        let text = format_text(&result);
        assert!(
            text.contains("Abstractness:"),
            "missing section header: {}",
            text
        );
        assert!(text.contains("src/api"), "missing dir: {}", text);
        assert!(
            text.contains("0.40"),
            "missing abstractness value: {}",
            text
        );
        assert!(
            text.contains("2 abstract"),
            "missing abstract count: {}",
            text
        );
        assert!(
            text.contains("3 concrete"),
            "missing concrete count: {}",
            text
        );
    }
}
