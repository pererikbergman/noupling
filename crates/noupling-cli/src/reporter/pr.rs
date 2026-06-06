//! Pull-request comment format. Tight Markdown summary suitable
//! for posting as a PR comment; length-bounded regardless of
//! project size.

use noupling_core::analyzer::AuditResult;

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
    out.push('\n');

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

    if !result.red_flags.is_empty() {
        out.push_str(&format!("### Red Flags ({})\n\n", result.red_flags.len()));
        for f in result.red_flags.iter().take(5) {
            out.push_str(&format!(
                "- **{:?}** (RRI: {:.0}) {}\n",
                f.flag_type, f.rri, f.recommendation
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!("---\n_{}_\n", VERSION));
    out
}
