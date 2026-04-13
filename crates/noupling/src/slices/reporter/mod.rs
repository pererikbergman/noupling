mod html;

use serde::Serialize;

use crate::slices::analyzer::{AuditResult, CouplingViolation};

pub use html::generate_html_report;

#[derive(Serialize)]
pub struct JsonReport {
    pub snapshot_id: String,
    pub score: f64,
    pub total_modules: usize,
    pub critical_violations: usize,
    pub violations: Vec<JsonViolation>,
}

#[derive(Serialize)]
pub struct JsonViolation {
    pub from_module: String,
    pub to_module: String,
    pub dir_a: String,
    pub dir_b: String,
    pub depth: i32,
    pub severity: f64,
    pub is_circular: bool,
}

impl JsonReport {
    pub fn from_audit(result: &AuditResult, snapshot_id: &str) -> Self {
        let critical_violations = result
            .violations
            .iter()
            .filter(|v| v.severity >= 0.5)
            .count();

        let violations: Vec<JsonViolation> = result
            .violations
            .iter()
            .map(|v| JsonViolation {
                from_module: v.from_module.clone(),
                to_module: v.to_module.clone(),
                dir_a: v.dir_a.clone(),
                dir_b: v.dir_b.clone(),
                depth: v.depth,
                severity: v.severity,
                is_circular: v.is_circular,
            })
            .collect();

        JsonReport {
            snapshot_id: snapshot_id.to_string(),
            score: result.score,
            total_modules: result.total_modules,
            critical_violations,
            violations,
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

pub fn format_text(result: &AuditResult) -> String {
    let mut output = String::new();

    output.push_str(&format!("Health Score: {:.1}/100\n", result.score));
    output.push_str(&format!("Total Modules: {}\n", result.total_modules));
    output.push_str(&format!("Violations: {}\n", result.violations.len()));

    if !result.violations.is_empty() {
        output.push('\n');
        for v in &result.violations {
            let label = if v.is_circular { " CIRCULAR" } else { "" };
            output.push_str(&format!(
                "  [{:.2}]{} {} -> {} (depth {})\n",
                v.severity, label, v.from_module, v.to_module, v.depth
            ));
            output.push_str(&format!(
                "         {} <> {}\n",
                v.dir_a, v.dir_b
            ));
        }
    }

    output
}

pub fn format_markdown(result: &AuditResult, snapshot_id: &str) -> String {
    let mut md = String::new();

    md.push_str("# noupling Audit Report\n\n");
    md.push_str(&format!("**Snapshot:** `{}`\n\n", snapshot_id));

    md.push_str("## Summary\n\n");
    md.push_str(&format!("| Metric | Value |\n"));
    md.push_str(&format!("| :--- | :--- |\n"));
    md.push_str(&format!("| Health Score | {:.1}/100 |\n", result.score));
    md.push_str(&format!("| Total Modules | {} |\n", result.total_modules));
    md.push_str(&format!("| Violations | {} |\n", result.violations.len()));

    let critical = result.violations.iter().filter(|v| v.severity >= 0.5).count();
    md.push_str(&format!("| Critical (severity >= 0.5) | {} |\n", critical));

    let circular = result.violations.iter().filter(|v| v.is_circular).count();
    if circular > 0 {
        md.push_str(&format!("| Structural Loops | {} |\n", circular));
    }

    if !result.violations.is_empty() {
        md.push_str("\n## Violations\n\n");
        md.push_str("| Severity | From | To | Depth | Type |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
        for v in &result.violations {
            let vtype = if v.is_circular { "Structural Loop" } else { "Coupling" };
            md.push_str(&format!(
                "| {:.2} | `{}` | `{}` | {} | {} |\n",
                v.severity, v.from_module, v.to_module, v.depth, vtype
            ));
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_violation(from: &str, to: &str, severity: f64, depth: i32) -> CouplingViolation {
        CouplingViolation {
            dir_a: "dir_a".to_string(),
            dir_b: "dir_b".to_string(),
            from_module: from.to_string(),
            to_module: to.to_string(),
            depth,
            severity,
            is_circular: false,
        }
    }

    #[test]
    fn json_report_has_required_fields() {
        let result = AuditResult {
            violations: vec![make_violation("a.rs", "b.rs", 1.0, 0)],
            score: 50.0,
            total_modules: 2,
        };

        let report = JsonReport::from_audit(&result, "snap-1");
        let json = report.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["snapshot_id"], "snap-1");
        assert_eq!(parsed["score"], 50.0);
        assert_eq!(parsed["total_modules"], 2);
        assert_eq!(parsed["critical_violations"], 1);
        assert_eq!(parsed["violations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn json_report_valid_json() {
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 5,
        };

        let report = JsonReport::from_audit(&result, "snap-2");
        let json = report.to_json().unwrap();
        // Should parse without error
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn critical_violations_counts_high_severity() {
        let result = AuditResult {
            violations: vec![
                make_violation("a.rs", "b.rs", 1.0, 0),   // critical (>= 0.5)
                make_violation("c.rs", "d.rs", 0.5, 1),   // critical (>= 0.5)
                make_violation("e.rs", "f.rs", 0.25, 2),  // not critical
            ],
            score: 42.0,
            total_modules: 6,
        };

        let report = JsonReport::from_audit(&result, "snap-3");
        assert_eq!(report.critical_violations, 2);
    }

    #[test]
    fn json_violations_preserve_order() {
        let result = AuditResult {
            violations: vec![
                make_violation("a.rs", "b.rs", 1.0, 0),
                make_violation("c.rs", "d.rs", 0.5, 1),
            ],
            score: 75.0,
            total_modules: 4,
        };

        let report = JsonReport::from_audit(&result, "snap-4");
        assert_eq!(report.violations[0].severity, 1.0);
        assert_eq!(report.violations[1].severity, 0.5);
    }

    #[test]
    fn text_format_shows_score_and_violations() {
        let result = AuditResult {
            violations: vec![make_violation("scanner/mod.rs", "storage/mod.rs", 0.5, 1)],
            score: 75.0,
            total_modules: 4,
        };

        let text = format_text(&result);
        assert!(text.contains("Health Score: 75.0/100"));
        assert!(text.contains("Violations: 1"));
        assert!(text.contains("scanner/mod.rs"));
    }

    #[test]
    fn text_format_clean_when_no_violations() {
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 4,
        };

        let text = format_text(&result);
        assert!(text.contains("Health Score: 100.0/100"));
        assert!(text.contains("Violations: 0"));
    }

    // ── Markdown reporter ──

    #[test]
    fn markdown_has_heading_and_summary_table() {
        let result = AuditResult {
            violations: vec![],
            score: 100.0,
            total_modules: 5,
        };

        let md = format_markdown(&result, "snap-1");
        assert!(md.contains("# noupling Audit Report"));
        assert!(md.contains("**Snapshot:** `snap-1`"));
        assert!(md.contains("| Health Score | 100.0/100 |"));
        assert!(md.contains("| Total Modules | 5 |"));
        assert!(md.contains("| Violations | 0 |"));
    }

    #[test]
    fn markdown_has_violation_table() {
        let result = AuditResult {
            violations: vec![make_violation("a.rs", "b.rs", 1.0, 0)],
            score: 50.0,
            total_modules: 2,
        };

        let md = format_markdown(&result, "snap-2");
        assert!(md.contains("## Violations"));
        assert!(md.contains("| Severity |"));
        assert!(md.contains("| 1.00 | `a.rs` | `b.rs` | 0 | Coupling |"));
    }

    #[test]
    fn markdown_shows_structural_loop() {
        let mut v = make_violation("a.rs", "b.rs", 1.0, 0);
        v.is_circular = true;
        let result = AuditResult {
            violations: vec![v],
            score: 50.0,
            total_modules: 2,
        };

        let md = format_markdown(&result, "snap-3");
        assert!(md.contains("Structural Loop"));
        assert!(md.contains("| Structural Loops | 1 |"));
    }
}
