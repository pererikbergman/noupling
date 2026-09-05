//! XML format adapter. Reads the canonical `JsonReport` and serialises
//! it as a tabular `<noupling-report>` document: the shared `<issues>`
//! list (every Issue kind, ADR 0002) plus the legacy circular, coupling,
//! directory-tree, gravity-well, and red-flag children.

use noupling_core::analyzer::{AuditResult, SubjectCard};
use noupling_core::core::Module;

use super::data::JsonReport;
use super::VERSION;

pub fn format_xml(modules: &[Module], result: &AuditResult, snapshot_id: &str) -> String {
    let report = JsonReport::from_audit(modules, result, snapshot_id);
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<noupling-report generator=\"{}\" snapshot=\"{}\" score=\"{:.2}\" tri=\"{:.1}\" totalModules=\"{}\" totalXs=\"{}\" maxDepth=\"{}\" suppressedCount=\"{}\" violationAgeNew=\"{}\" violationAgeRecent=\"{}\" violationAgeChronic=\"{}\" criticalViolations=\"{}\" totalCircular=\"{}\" totalCoupling=\"{}\">\n",
        xml_escape(VERSION), xml_escape(&report.snapshot_id), report.score, report.tri, report.total_modules,
        report.total_xs, report.max_depth, report.suppressed_count,
        report.violation_age.new_count, report.violation_age.recent_count, report.violation_age.chronic_count,
        report.critical_violations, report.total_circular, report.total_coupling,
    ));

    // Issues — every kind, from the shared Issue cards (ADR 0002). The
    // per-kind sections below stay until #350 removes them.
    xml.push_str(&format!("  <issues count=\"{}\">\n", report.issues.len()));
    for card in &report.issues {
        xml.push_str(&format!(
            "    <issue kind=\"{}\" severity=\"{}\" baselined=\"{}\" scoreImpact=\"{:.1}\" fingerprint=\"{}\">\n",
            xml_escape(card.kind),
            xml_escape(card.severity),
            card.baselined,
            card.score_impact,
            xml_escape(&card.fingerprint),
        ));
        match &card.subject {
            SubjectCard::Module { path } => xml.push_str(&format!(
                "      <subject type=\"module\" path=\"{}\"/>\n",
                xml_escape(path)
            )),
            SubjectCard::Edge { from, to } => xml.push_str(&format!(
                "      <subject type=\"edge\" from=\"{}\" to=\"{}\"/>\n",
                xml_escape(from),
                xml_escape(to)
            )),
            SubjectCard::Ring { members } => {
                xml.push_str("      <subject type=\"ring\">\n");
                for m in members {
                    xml.push_str(&format!("        <member>{}</member>\n", xml_escape(m)));
                }
                xml.push_str("      </subject>\n");
            }
        }
        xml.push_str(&format!(
            "      <reason>{}</reason>\n      <recommendation>{}</recommendation>\n",
            xml_escape(&card.reason),
            xml_escape(&card.recommendation),
        ));
        // Scalar detail fields become attributes; nested ones (cycle hops)
        // are the JSON report's job.
        let mut attrs = String::new();
        if let Some(obj) = card.detail.as_object() {
            for (k, v) in obj {
                let text = match v {
                    serde_json::Value::String(t) => t.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_) => continue,
                };
                attrs.push_str(&format!(" {}=\"{}\"", xml_escape(k), xml_escape(&text)));
            }
        }
        xml.push_str(&format!("      <detail{}/>\n", attrs));
        xml.push_str("    </issue>\n");
    }
    xml.push_str("  </issues>\n");

    // Circular dependencies
    if !report.circular_dependencies.is_empty() {
        xml.push_str("  <circular-dependencies>\n");
        for (label, cycles) in &report.circular_dependencies {
            xml.push_str(&format!(
                "    <group label=\"{}\" count=\"{}\">\n",
                xml_escape(label),
                cycles.len()
            ));
            for cycle in cycles {
                let wl_attr = cycle
                    .weakest_link
                    .as_ref()
                    .map(|wl| {
                        format!(
                            " weakestLink=\"{}\" breakCost=\"{}\"",
                            xml_escape(wl),
                            cycle.break_cost
                        )
                    })
                    .unwrap_or_default();
                xml.push_str(&format!(
                    "      <cycle order=\"{}\" severity=\"{:.2}\"{}>\n",
                    cycle.cycle_order, cycle.severity, wl_attr
                ));
                xml.push_str("        <path>\n");
                for dir in &cycle.cycle_path {
                    xml.push_str(&format!("          <dir>{}</dir>\n", xml_escape(dir)));
                }
                xml.push_str("        </path>\n");
                xml.push_str("        <short-path>\n");
                for dir in &cycle.cycle_short_path {
                    xml.push_str(&format!("          <dir>{}</dir>\n", xml_escape(dir)));
                }
                xml.push_str("        </short-path>\n");
                xml.push_str("        <hops>\n");
                for hop in &cycle.hop_files {
                    xml.push_str(&format!(
                        "          <hop fromDir=\"{}\" fromFile=\"{}\" toFile=\"{}\"/>\n",
                        xml_escape(&hop.from_dir),
                        xml_escape(&hop.from_file),
                        xml_escape(&hop.to_file),
                    ));
                }
                xml.push_str("        </hops>\n");
                xml.push_str("      </cycle>\n");
            }
            xml.push_str("    </group>\n");
        }
        xml.push_str("  </circular-dependencies>\n");
    }

    // Coupling violations
    if !report.coupling_violations.is_empty() {
        xml.push_str("  <coupling-violations>\n");
        for v in &report.coupling_violations {
            xml.push_str(&format!(
                "    <violation severity=\"{:.2}\" rri=\"{:.1}\" direction=\"{}\" depth=\"{}\" fromModule=\"{}\" toModule=\"{}\" dirA=\"{}\" dirB=\"{}\"/>\n",
                v.severity, v.rri, xml_escape(&v.direction), v.depth, xml_escape(&v.from_module), xml_escape(&v.to_module),
                xml_escape(&v.dir_a), xml_escape(&v.dir_b),
            ));
        }
        xml.push_str("  </coupling-violations>\n");
    }

    // Directory tree
    xml.push_str("  <directory-tree>\n");
    for dir in &report.directory_tree {
        xml.push_str(&format!(
            "    <directory path=\"{}\" name=\"{}\" modules=\"{}\" score=\"{:.2}\" violations=\"{}\" circular=\"{}\" hasViolations=\"{}\">\n",
            xml_escape(&dir.path), xml_escape(&dir.name), dir.module_count,
            dir.score, dir.violations_count, dir.circular_count, dir.has_violations,
        ));
        for child in &dir.children {
            xml.push_str(&format!("      <child>{}</child>\n", xml_escape(child)));
        }
        for file in &dir.files {
            xml.push_str(&format!("      <file>{}</file>\n", xml_escape(file)));
        }
        xml.push_str("    </directory>\n");
    }
    xml.push_str("  </directory-tree>\n");

    // Gravity wells
    if !report.gravity_wells.is_empty() {
        xml.push_str("  <gravity-wells>\n");
        for g in &report.gravity_wells {
            xml.push_str(&format!(
                "    <well module=\"{}\" totalRri=\"{:.1}\" relationships=\"{}\" directions=\"{}\"/>\n",
                xml_escape(&g.module_path), g.total_rri, g.relationship_count, g.direction_count,
            ));
        }
        xml.push_str("  </gravity-wells>\n");
    }

    // Red flags
    if !report.red_flags.is_empty() {
        xml.push_str("  <red-flags>\n");
        for f in &report.red_flags {
            xml.push_str(&format!(
                "    <flag type=\"{}\" rri=\"{:.1}\">{}</flag>\n",
                xml_escape(&f.flag_type),
                f.rri,
                xml_escape(&f.recommendation),
            ));
        }
        xml.push_str("  </red-flags>\n");
    }

    xml.push_str("</noupling-report>\n");
    xml
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
