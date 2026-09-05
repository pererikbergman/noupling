//! Sprint-planning briefing: Markdown report ranking the top 10 Issues
//! by score impact (then band), with effort estimates and a projected
//! score after the top three. The approach on each item is the Issue's
//! recommendation from core, verbatim.

use noupling_core::analyzer::{AuditResult, Issue, IssueDetail, IssueKind};

use super::VERSION;

pub fn format_briefing(result: &AuditResult) -> String {
    let mut out = String::new();

    let date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0);
    let years = 1970 + date / 365;
    let day_of_year = date % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    out.push_str(&format!(
        "# Architecture Briefing — {:04}-{:02}-{:02}\n\n",
        years, month, day
    ));

    // issues() is band-ordered; rank by score impact first so the sprint
    // list leads with what moves the score, then falls back to band.
    let mut ranked: Vec<Issue> = result.issues();
    ranked.sort_by(|a, b| {
        b.score_impact()
            .partial_cmp(&a.score_impact())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.severity().cmp(&a.severity()))
    });
    let top: Vec<&Issue> = ranked.iter().take(10).collect();

    // Projected score after fixing the top 3: their score impact comes back.
    let recovered: f64 = top.iter().take(3).map(|i| i.score_impact()).sum();
    let projected_score = (result.score + recovered).min(100.0);
    let delta = projected_score - result.score;

    out.push_str(&format!("**Current score:** {:.1}/100  \n", result.score));
    if result.tri > 0.0 {
        out.push_str(&format!("**Total Risk Index:** {:.1}  \n", result.tri));
    }
    if delta > 0.1 {
        out.push_str(&format!(
            "**If you fix the top 3 below, projected score:** {:.1} (+{:.1})\n\n",
            projected_score, delta
        ));
    } else {
        out.push('\n');
    }

    // Summary metrics
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n");
    out.push_str("| :--- | :--- |\n");
    out.push_str(&format!("| Total modules | {} |\n", result.total_modules));
    out.push_str(&format!("| Issues | {} |\n", ranked.len()));
    for kind in IssueKind::ALL {
        let n = ranked.iter().filter(|i| i.kind() == kind).count();
        if n > 0 {
            out.push_str(&format!("| &nbsp;&nbsp;{} | {} |\n", kind, n));
        }
    }
    out.push_str(&format!(
        "| Active violations | {} |\n",
        result.violations.len()
    ));
    out.push_str(&format!(
        "| Total XS (imports to remove) | {} |\n",
        result.total_xs
    ));
    if result.max_depth > 0 {
        out.push_str(&format!(
            "| Max dependency depth | {} |\n",
            result.max_depth
        ));
    }
    if result.coupling_metrics_count > 0 {
        out.push_str(&format!(
            "| Sibling coupling pairs (informational) | {} |\n",
            result.coupling_metrics_count
        ));
    }
    out.push('\n');

    if top.is_empty() {
        out.push_str("## No Actionable Items\n\n");
        out.push_str("Architecture is healthy. \u{1F389}\n\n");
        out.push_str(&format!("---\n_{}_\n", VERSION));
        return out;
    }

    out.push_str("## Top Refactoring Opportunities\n\n");
    out.push_str(
        "Ranked by score impact, then severity band. Each item says why it matters and what to do.\n\n",
    );

    for (i, issue) in top.iter().enumerate() {
        out.push_str(&format!(
            "### {}. [{}] {}: `{}`{}\n\n",
            i + 1,
            issue.severity().name().to_uppercase(),
            issue.kind(),
            issue.subject(),
            if issue.baselined {
                " _(baselined)_"
            } else {
                ""
            }
        ));
        match effort_imports(issue) {
            Some(cost) => out.push_str(&format!(
                "- **Effort:** {} import{} to remove ({})\n",
                cost,
                if cost == 1 { "" } else { "s" },
                effort_estimate(cost)
            )),
            None => out.push_str("- **Effort:** structural change (no single import to remove)\n"),
        }
        let impact = issue.score_impact();
        if impact > 0.0 {
            out.push_str(&format!("- **Impact:** score +{:.1}\n", impact));
        } else {
            out.push_str("- **Impact:** does not score; reduces structural risk\n");
        }
        out.push_str(&format!("- **Why:** {}\n", issue.reason()));
        out.push_str(&format!("- **Approach:** {}\n\n", issue.recommendation()));
    }

    out.push_str(&format!("---\n_{}_\n", VERSION));
    out
}

/// Imports to remove to resolve the Issue, for kinds where that is the
/// fix. Exhaustive so a new kind must say how it is paid for.
fn effort_imports(issue: &Issue) -> Option<usize> {
    match &issue.detail {
        IssueDetail::CouplingViolation(v) => Some(v.weight.max(1)),
        IssueDetail::Cycle(v) => Some(v.break_cost.max(1)),
        IssueDetail::RuleViolation(_) | IssueDetail::LayerViolation(_) => Some(1),
        IssueDetail::StabilityViolation(_) => Some(1),
        IssueDetail::GravityWell(_)
        | IssueDetail::RedFlag(_)
        | IssueDetail::ZoneFlag(_)
        | IssueDetail::LowCohesion(_) => None,
    }
}

fn effort_estimate(cost: usize) -> &'static str {
    match cost {
        0..=1 => "5 minutes",
        2..=5 => "1-2 hours",
        6..=20 => "half a day",
        21..=50 => "1-2 days",
        _ => "1+ week",
    }
}
