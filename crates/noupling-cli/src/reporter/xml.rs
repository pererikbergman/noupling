//! XML format adapter. Reads the canonical `JsonReport` and serialises
//! it as a tabular `<noupling-report>` document: the shared `<issues>`
//! list (every Issue kind, ADR 0002) plus the directory tree. The
//! per-kind `<circular-dependencies>`, `<coupling-violations>`,
//! `<gravity-wells>` and `<red-flags>` children were removed in 0.9.0
//! (#350); read `<issues>` and filter on `kind`.

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

    // Issues — every kind, from the shared Issue cards (ADR 0002).
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
        // Scalar detail fields become attributes of <detail/>; nested ones
        // (a Cycle's hops and hop counts, a Red Flag's modules) become child
        // elements so XML consumers lose nothing the JSON card carries.
        xml.push_str(&detail_element(&card.detail, "      "));
        xml.push_str("    </issue>\n");
    }
    xml.push_str("  </issues>\n");

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

    xml.push_str("</noupling-report>\n");
    xml
}

/// `<detail a="1" b="x">` with one child element per nested value:
/// arrays of objects → `<name><item k="v" …/></name>`, arrays of scalars →
/// `<name><item>v</item></name>`, objects → `<name k="v" …/>`.
fn detail_element(detail: &serde_json::Value, indent: &str) -> String {
    use serde_json::Value;
    let scalar = |v: &Value| -> Option<String> {
        match v {
            Value::String(t) => Some(t.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
        }
    };
    let attrs_of = |obj: &serde_json::Map<String, Value>| -> String {
        obj.iter()
            .filter_map(|(k, v)| {
                scalar(v).map(|t| format!(" {}=\"{}\"", xml_escape(k), xml_escape(&t)))
            })
            .collect()
    };
    let Some(obj) = detail.as_object() else {
        return format!("{indent}<detail/>\n");
    };
    let nested: Vec<(&String, &Value)> = obj
        .iter()
        .filter(|(_, v)| matches!(v, Value::Array(_) | Value::Object(_)))
        .collect();
    if nested.is_empty() {
        return format!("{indent}<detail{}/>\n", attrs_of(obj));
    }
    let mut out = format!("{indent}<detail{}>\n", attrs_of(obj));
    for (name, value) in nested {
        let name = xml_escape(name);
        match value {
            Value::Object(o) => out.push_str(&format!("{indent}  <{name}{}/>\n", attrs_of(o))),
            Value::Array(items) => {
                out.push_str(&format!("{indent}  <{name}>\n"));
                for item in items {
                    match item {
                        Value::Object(o) => {
                            out.push_str(&format!("{indent}    <item{}/>\n", attrs_of(o)))
                        }
                        other => out.push_str(&format!(
                            "{indent}    <item>{}</item>\n",
                            xml_escape(&scalar(other).unwrap_or_default())
                        )),
                    }
                }
                out.push_str(&format!("{indent}  </{name}>\n"));
            }
            _ => {}
        }
    }
    out.push_str(&format!("{indent}</detail>\n"));
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
