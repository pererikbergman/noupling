//! SonarCloud format adapter. Emits a generic-issue JSON document
//! Sonar can ingest at `sonar.externalIssuesReportPaths`.
//!
//! One Sonar issue per Issue from `issues()`, every kind (ADR 0002):
//! `ruleId` is `noupling:<kind id>`, the severity band maps to a Sonar
//! severity, and the message is the Issue card's reason + recommendation.

use noupling_core::analyzer::{AuditResult, Issue, IssueDetail, SeverityBand};
use noupling_core::core::Module;

pub fn format_sonar(modules: &[Module], result: &AuditResult) -> String {
    let issues: Vec<serde_json::Value> = result
        .issues()
        .iter()
        .map(|i| sonar_issue(i, modules))
        .collect();
    let report = serde_json::json!({ "issues": issues });
    serde_json::to_string_pretty(&report).unwrap_or_default()
}

fn sonar_severity(band: SeverityBand) -> (&'static str, i32) {
    match band {
        SeverityBand::Critical => ("CRITICAL", 60),
        SeverityBand::High => ("MAJOR", 30),
        SeverityBand::Medium => ("MINOR", 20),
        SeverityBand::Low => ("INFO", 10),
    }
}

/// Where Sonar should pin the Issue: a file and line when the Issue has
/// one, otherwise the first file (sorted) inside the directory it is
/// about, at line 1. Sonar's generic-issue importer resolves `filePath`
/// against indexed files and silently drops anything else, so a bare
/// directory would lose the Issue.
fn primary_location(issue: &Issue, modules: &[Module]) -> (String, i32) {
    let file_in = |dir: &str| -> String {
        let prefix = format!("{}/", dir);
        modules
            .iter()
            .filter(|m| m.path.starts_with(&prefix))
            .map(|m| m.path.as_str())
            .min()
            .unwrap_or(dir)
            .to_string()
    };
    match &issue.detail {
        IssueDetail::CouplingViolation(v) => (v.from_module.clone(), v.line_number.max(1)),
        IssueDetail::Cycle(v) => match v.cycle_hop_files.first() {
            Some((from_file, _, line)) => (from_file.clone(), (*line).max(1)),
            None => (v.from_module.clone(), 1),
        },
        IssueDetail::RuleViolation(r) => (r.from_module.clone(), r.line_number.max(1)),
        IssueDetail::LayerViolation(l) => (l.from_module.clone(), l.line_number.max(1)),
        IssueDetail::GravityWell(g) => (g.module_path.clone(), 1),
        IssueDetail::RedFlag(f) => (f.modules.first().cloned().unwrap_or_default(), 1),
        IssueDetail::StabilityViolation(s) => (file_in(&s.from_dir), 1),
        IssueDetail::ZoneFlag(d) => (file_in(&d.dir), 1),
        IssueDetail::LowCohesion(c) => (file_in(&c.dir), 1),
    }
}

/// Extra locations that help a reader follow an edge-shaped Issue.
fn secondary_locations(issue: &Issue) -> Vec<serde_json::Value> {
    let range = |line: i32| serde_json::json!({ "startLine": line.max(1), "endLine": line.max(1) });
    match &issue.detail {
        IssueDetail::Cycle(v) => v
            .cycle_hop_files
            .iter()
            .skip(1)
            .map(|(from_file, _, line)| {
                serde_json::json!({
                    "message": "Part of the cycle",
                    "filePath": from_file,
                    "textRange": range(*line),
                })
            })
            .collect(),
        IssueDetail::CouplingViolation(v) => vec![serde_json::json!({
            "message": format!("Coupled target in {}", v.dir_b),
            "filePath": v.to_module,
            "textRange": range(v.line_number),
        })],
        IssueDetail::RuleViolation(r) => vec![serde_json::json!({
            "message": "Forbidden target",
            "filePath": r.to_module,
            "textRange": range(r.line_number),
        })],
        IssueDetail::LayerViolation(l) => vec![serde_json::json!({
            "message": format!("Target in layer {}", l.to_layer),
            "filePath": l.to_module,
            "textRange": range(l.line_number),
        })],
        IssueDetail::GravityWell(_)
        | IssueDetail::RedFlag(_)
        | IssueDetail::StabilityViolation(_)
        | IssueDetail::ZoneFlag(_)
        | IssueDetail::LowCohesion(_) => Vec::new(),
    }
}

fn sonar_issue(issue: &Issue, modules: &[Module]) -> serde_json::Value {
    let (severity, effort) = sonar_severity(issue.severity());
    let (file_path, line) = primary_location(issue, modules);
    let mut value = serde_json::json!({
        "engineId": "noupling",
        "ruleId": format!("noupling:{}", issue.kind().id()),
        "severity": severity,
        "type": "CODE_SMELL",
        "effortMinutes": effort,
        "primaryLocation": {
            "message": format!("{}: {} {}", issue.kind(), issue.reason(), issue.recommendation()),
            "filePath": file_path,
            "textRange": { "startLine": line, "endLine": line }
        }
    });
    let secondary = secondary_locations(issue);
    if !secondary.is_empty() {
        value["secondaryLocations"] = serde_json::json!(secondary);
    }
    value
}
