//! Pull-request comment format. Tight Markdown summary suitable
//! for posting as a PR comment; length-bounded regardless of
//! project size.

use noupling_core::analyzer::{AuditResult, IssueDetail, IssueKind};

use super::VERSION;

/// `previous_score` and `previous_violation_count` come from the
/// previous snapshot (or baseline) and drive the delta indicators.
/// Both are optional — when absent, only current state is shown.
pub fn format_pr(
    result: &AuditResult,
    previous_score: Option<f64>,
    previous_violation_count: Option<usize>,
    new_violations: Option<usize>,
    resolved_violations: Option<usize>,
) -> String {
    let mut out = String::new();

    let score_emoji = if result.score >= 90.0 {
        "\u{2705}"
    } else if result.score >= 70.0 {
        "\u{26a0}\u{fe0f}"
    } else {
        "\u{274c}"
    };

    out.push_str("## Architecture Check\n\n");

    // Score line with optional delta
    let score_line = match previous_score {
        Some(prev) => {
            let delta = result.score - prev;
            let arrow = if delta > 0.05 {
                format!(" (+{:.1} since previous)", delta)
            } else if delta < -0.05 {
                format!(" ({:.1} since previous)", delta)
            } else {
                String::new()
            };
            format!(
                "**Score:** {:.1}/100{} {}\n",
                result.score, arrow, score_emoji
            )
        }
        None => format!("**Score:** {:.1}/100 {}\n", result.score, score_emoji),
    };
    out.push_str(&score_line);
    out.push('\n');

    // Summary table
    out.push_str("### Summary\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| :--- | :--- |\n");
    out.push_str(&format!(
        "| Violations | {}{} |\n",
        result.violations.len(),
        previous_violation_count
            .map(|p| {
                let d = result.violations.len() as i64 - p as i64;
                if d > 0 {
                    format!(" (+{})", d)
                } else if d < 0 {
                    format!(" ({})", d)
                } else {
                    String::new()
                }
            })
            .unwrap_or_default()
    ));
    if result.tri > 0.0 {
        out.push_str(&format!("| Total Risk Index | {:.1} |\n", result.tri));
    }
    out.push_str(&format!("| Total XS | {} imports |\n", result.total_xs));
    if let Some(n) = new_violations {
        out.push_str(&format!("| New violations | {} |\n", n));
    }
    if let Some(r) = resolved_violations {
        out.push_str(&format!("| Resolved violations | {} |\n", r));
    }
    // Every Issue kind, with a count — a kind is never silently absent.
    let issues = result.issues();
    out.push_str(&format!("| Issues | {} |\n", issues.len()));
    if result.baseline.is_some() {
        let baselined = issues.iter().filter(|i| i.baselined).count();
        out.push_str(&format!(
            "| New / baselined | {} / {} |\n",
            issues.len() - baselined,
            baselined
        ));
    }
    out.push('\n');
    let by_kind: Vec<String> = IssueKind::ALL
        .iter()
        .map(|kind| {
            format!(
                "{} {}",
                kind,
                issues.iter().filter(|i| i.kind() == *kind).count()
            )
        })
        .collect();
    out.push_str(&format!("**By kind:** {}\n\n", by_kind.join(" · ")));

    // Top actions
    let actions = noupling_core::analyzer::compute_top_actions(result, 3);
    if !actions.is_empty() {
        out.push_str("### Action items\n\n");
        for (i, a) in actions.iter().enumerate() {
            out.push_str(&format!(
                "{}. **{}** [{}]\n   - {}\n   - {} _(cost: {} import{})_\n\n",
                i + 1,
                a.title,
                a.category,
                a.detail,
                a.action,
                a.cost,
                if a.cost == 1 { "" } else { "s" }
            ));
        }
    } else {
        out.push_str("### Action items\n\nNo violations to fix \u{1f389}\n\n");
    }

    // Top Issues by band (issues() order), capped so the comment stays short.
    const TOP: usize = 5;
    if !issues.is_empty() {
        out.push_str("### Top issues\n\n");
        for issue in issues.iter().take(TOP) {
            // One compact number per kind; the reason carries the rest.
            let figure: String = match &issue.detail {
                IssueDetail::CouplingViolation(v) => format!(
                    "{} import{}",
                    v.weight.max(1),
                    if v.weight.max(1) == 1 { "" } else { "s" }
                ),
                IssueDetail::Cycle(v) => format!("break cost {}", v.break_cost),
                IssueDetail::RuleViolation(r) => format!("line {}", r.line_number),
                IssueDetail::LayerViolation(l) => format!("{} → {}", l.from_layer, l.to_layer),
                IssueDetail::GravityWell(g) => format!("RRI {:.0}", g.total_rri),
                IssueDetail::RedFlag(f) => format!("RRI {:.0}", f.rri),
                IssueDetail::StabilityViolation(s) => {
                    format!("I {:.2} → {:.2}", s.from_instability, s.to_instability)
                }
                IssueDetail::ZoneFlag(d) => format!("D {:.2}", d.distance),
                IssueDetail::LowCohesion(c) => format!("cohesion {:.2}", c.cohesion.unwrap_or(0.0)),
            };
            out.push_str(&format!(
                "- **[{}] {}** `{}` _({}{})_\n  {}\n",
                issue.severity().name().to_uppercase(),
                issue.kind(),
                issue.subject(),
                figure,
                if issue.baselined { ", baselined" } else { "" },
                issue.recommendation()
            ));
        }
        if issues.len() > TOP {
            out.push_str(&format!("- … and {} more\n", issues.len() - TOP));
        }
        out.push('\n');
    }

    out.push_str(&format!("---\n_{}_\n", VERSION));
    out
}
