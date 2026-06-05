//! SonarCloud format adapter. Emits a generic-issue JSON document
//! Sonar can ingest at `sonar.externalIssuesReportPaths`.

use noupling_core::analyzer::AuditResult;

pub fn format_sonar(result: &AuditResult) -> String {
    let mut issues = Vec::new();

    for v in &result.violations {
        if v.is_circular {
            let (file_path, first_line) = if !v.cycle_hop_files.is_empty() {
                (v.cycle_hop_files[0].0.clone(), v.cycle_hop_files[0].2)
            } else {
                (v.from_module.clone(), 1)
            };

            let short_dirs: Vec<String> = v
                .cycle_path
                .iter()
                .map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(p)
                        .to_string()
                })
                .collect();
            let cycle_desc = short_dirs.join(" -> ");

            let mut secondary = Vec::new();
            for (i, (from_file, _to_file, line)) in v.cycle_hop_files.iter().enumerate() {
                if i == 0 {
                    continue;
                }
                let dir_name = if i < v.cycle_path.len() {
                    std::path::Path::new(&v.cycle_path[i])
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("")
                } else {
                    ""
                };
                secondary.push(serde_json::json!({
                    "message": format!("Part of circular dependency chain ({})", dir_name),
                    "filePath": from_file,
                    "textRange": { "startLine": line, "endLine": line }
                }));
            }

            let effort = if v.break_cost > 0 {
                v.break_cost as i32 * 15
            } else {
                (v.cycle_order as i32) * 30
            };
            let mut issue = serde_json::json!({
                "engineId": "noupling",
                "ruleId": "noupling:circular-dependency",
                "severity": "CRITICAL",
                "type": "CODE_SMELL",
                "effortMinutes": effort,
                "primaryLocation": {
                    "message": format!("Circular dependency: {}", cycle_desc),
                    "filePath": file_path,
                    "textRange": { "startLine": first_line, "endLine": first_line }
                }
            });
            if !secondary.is_empty() {
                issue["secondaryLocations"] = serde_json::json!(secondary);
            }
            issues.push(issue);
        } else {
            // Map RRI to Sonar severity (fall back to old severity if RRI is 0)
            let risk = if v.rri > 0.0 {
                v.rri
            } else {
                v.severity * 10.0
            };
            let (sonar_severity, effort) = if risk >= 160.0 {
                ("BLOCKER", 60)
            } else if risk >= 80.0 {
                ("CRITICAL", 30)
            } else if risk >= 40.0 {
                ("MAJOR", 20)
            } else if risk >= 10.0 {
                ("MINOR", 10)
            } else {
                ("INFO", 5)
            };

            let dir_a_short = std::path::Path::new(&v.dir_a)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.dir_a);
            let dir_b_short = std::path::Path::new(&v.dir_b)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&v.dir_b);

            issues.push(serde_json::json!({
                "engineId": "noupling",
                "ruleId": "noupling:coupling",
                "severity": sonar_severity,
                "type": "CODE_SMELL",
                "effortMinutes": effort,
                "primaryLocation": {
                    "message": format!("Coupling violation: {} depends on {} (severity {:.2})", dir_a_short, dir_b_short, v.severity),
                    "filePath": v.from_module,
                    "textRange": { "startLine": v.line_number, "endLine": v.line_number }
                },
                "secondaryLocations": [{
                    "message": format!("Coupled target in {}", dir_b_short),
                    "filePath": v.to_module,
                    "textRange": { "startLine": v.line_number, "endLine": v.line_number }
                }]
            }));
        }
    }

    let report = serde_json::json!({
        "issues": issues,
    });

    serde_json::to_string_pretty(&report).unwrap_or_default()
}
