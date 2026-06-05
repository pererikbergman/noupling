//! Sprint-planning briefing: Markdown report ranking the top 10
//! refactoring opportunities by ROI, with effort estimates and a
//! projected score after the top three.

use noupling_core::analyzer::AuditResult;

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

    let actions = noupling_core::analyzer::compute_top_actions(result, 10);

    // Projected score after fixing top 3
    let actionable_severity: f64 = actions
        .iter()
        .take(3)
        .filter(|a| a.category != "hotspot")
        .map(|a| a.impact)
        .sum();
    let projected_score = if result.total_modules > 0 {
        let projected_sum =
            (100.0 - result.score) * result.total_modules as f64 / 100.0 - actionable_severity;
        (100.0 * (1.0 - projected_sum.max(0.0) / result.total_modules as f64))
            .clamp(result.score, 100.0)
    } else {
        result.score
    };
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

    if actions.is_empty() {
        out.push_str("## No Actionable Items\n\n");
        out.push_str("Architecture is healthy. \u{1F389}\n\n");
        out.push_str(&format!("---\n_{}_\n", VERSION));
        return out;
    }

    out.push_str("## Top Refactoring Opportunities\n\n");
    out.push_str(
        "Ranked by ROI (impact / effort). Each item includes effort, impact, and approach.\n\n",
    );

    for (i, action) in actions.iter().enumerate() {
        let effort_label = effort_estimate(action.cost);
        let impact_label = match action.category.as_str() {
            "circular" => "Resolves a cycle",
            "layer" => "Removes a layer violation",
            "rule" => "Resolves a rule violation",
            "cross-module" => "Removes a cross-module violation",
            "hotspot" => "Reduces blast radius",
            _ => "Improves architecture",
        };

        out.push_str(&format!("### {}. {}\n\n", i + 1, action.title));
        out.push_str(&format!(
            "- **Effort:** {} import{} to remove ({})\n",
            action.cost,
            if action.cost == 1 { "" } else { "s" },
            effort_label
        ));
        out.push_str(&format!(
            "- **Impact:** {} (score impact: ~{:.1})\n",
            impact_label, action.impact
        ));
        out.push_str(&format!("- **Detail:** `{}`\n", action.detail));
        out.push_str(&format!("- **Approach:** {}\n", action.action));
        out.push_str(&format!("- **Category:** {}\n\n", action.category));
    }

    out.push_str(&format!("---\n_{}_\n", VERSION));
    out
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
