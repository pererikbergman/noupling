//! Plain-text format adapter — what the `audit` command prints to
//! stdout. Also owns the monorepo-aware variant used when audit
//! discovers per-module configs.

use noupling_core::analyzer::{AuditResult, Issue, SeverityBand};

use super::VERSION;

pub fn format_text(result: &AuditResult) -> String {
    let mut output = String::new();

    output.push_str(&format!("Health Score: {:.1}/100\n", result.score));
    if result.tri > 0.0 {
        output.push_str(&format!("Total Risk Index (TRI): {:.1}\n", result.tri));
    }
    output.push_str(&format!("Total Modules: {}\n", result.total_modules));
    output.push_str(&format!("Violations: {}\n", result.violations.len()));
    if result.total_xs > 0 {
        output.push_str(&format!(
            "Total XS: {} import{} to remove\n",
            result.total_xs,
            if result.total_xs == 1 { "" } else { "s" }
        ));
    }
    if result.suppressed_count > 0 {
        output.push_str(&format!(
            "Suppressed: {} import{} via noupling:ignore\n",
            result.suppressed_count,
            if result.suppressed_count == 1 {
                ""
            } else {
                "s"
            }
        ));
    }

    if result.layers_auto_detected {
        let names: Vec<&str> = result.layers.iter().map(|l| l.name.as_str()).collect();
        output.push_str(&format!(
            "Layers: inferred from path names ({}) — set `layers` in .noupling/settings.json to override\n",
            names.join(", ")
        ));
    }

    if result.total_external_imports > 0 {
        output.push_str(&format!(
            "External Imports: {} across {} modules\n",
            result.total_external_imports,
            result.external_deps.len()
        ));
        let mut sorted_ext = result.external_deps.clone();
        sorted_ext.sort_by_key(|e| std::cmp::Reverse(e.count));
        for e in sorted_ext.iter().take(5) {
            output.push_str(&format!("  [{} imports] {}\n", e.count, e.module_path));
        }
    }

    // Top Actions — what to do
    let top_actions = noupling_core::analyzer::compute_top_actions(result, 5);
    if !top_actions.is_empty() {
        output.push_str("\nTop Actions:\n");
        for (i, action) in top_actions.iter().enumerate() {
            output.push_str(&format!(
                "  {}. {} [{}]\n",
                i + 1,
                action.title,
                action.category
            ));
            output.push_str(&format!("     {}\n", action.detail));
            output.push_str(&format!(
                "     \u{2192} {} (cost: {} import{})\n",
                action.action,
                action.cost,
                if action.cost == 1 { "" } else { "s" }
            ));
        }
    }

    // Issues — every kind, as Issue cards, in issues() order (#340).
    output.push_str(&format_issue_cards(result));

    // Hotspots (top 10 most-imported modules)
    let top_hotspots: Vec<_> = result
        .hotspots
        .iter()
        .filter(|h| h.fan_in > 0)
        .take(10)
        .collect();
    if !top_hotspots.is_empty() {
        output.push_str("\nHotspots (most imported):\n");
        for h in &top_hotspots {
            output.push_str(&format!(
                "  [{} in, {} out] {}\n",
                h.fan_in, h.fan_out, h.path
            ));
        }
    }

    // Zone of pain: stable modules (low instability) with high fan-in
    let zone_of_pain: Vec<_> = result
        .hotspots
        .iter()
        .filter(|h| h.instability < 0.3 && h.fan_in >= 5)
        .take(10)
        .collect();
    if !zone_of_pain.is_empty() {
        output.push_str("\nZone of Pain (stable, high fan-in):\n");
        for h in &zone_of_pain {
            output.push_str(&format!(
                "  I={:.2} [{} in, {} out] {}\n",
                h.instability, h.fan_in, h.fan_out, h.path
            ));
        }
    }

    // Highest blast radius
    let mut by_blast: Vec<_> = result
        .hotspots
        .iter()
        .filter(|h| h.blast_radius > 0)
        .collect();
    by_blast.sort_by_key(|h| std::cmp::Reverse(h.blast_radius));
    let top_blast: Vec<_> = by_blast.into_iter().take(10).collect();
    if !top_blast.is_empty() {
        output.push_str("\nHighest Blast Radius:\n");
        for h in &top_blast {
            output.push_str(&format!(
                "  [{}] {} ({} in, {} out)\n",
                h.blast_radius, h.path, h.fan_in, h.fan_out
            ));
        }
    }

    // Abstractness per directory
    if !result.abstractness.is_empty() {
        output.push_str("\nAbstractness:\n");
        for a in result.abstractness.iter().take(10) {
            output.push_str(&format!(
                "  {:.2} {} ({} abstract, {} concrete)\n",
                a.abstractness, a.dir, a.abstract_count, a.concrete_count
            ));
        }
    }

    // Instability per directory (Martin's I)
    if !result.instability.is_empty() {
        output.push_str("\nInstability:\n");
        for i in result.instability.iter().take(10) {
            output.push_str(&format!(
                "  I={:.2} {} (Ca={}, Ce={})\n",
                i.instability, i.dir, i.ca, i.ce
            ));
        }
    }

    // Distance from main sequence per directory
    if !result.distance.is_empty() {
        use noupling_core::analyzer::Zone;
        output.push_str("\nDistance from Main Sequence:\n");
        for d in result.distance.iter().take(10) {
            let zone_tag = match d.zone {
                Zone::MainSequence => "",
                Zone::Pain => "  [Zone of Pain]",
                Zone::Uselessness => "  [Zone of Uselessness]",
            };
            output.push_str(&format!(
                "  D={:.2} {} (A={:.2}, I={:.2}){}\n",
                d.distance, d.dir, d.abstractness, d.instability, zone_tag
            ));
        }
    }

    // Module independence
    let low_independence: Vec<_> = result
        .independence
        .iter()
        .filter(|m| m.independence < 0.7)
        .take(10)
        .collect();
    if !low_independence.is_empty() {
        output.push_str("\nLow Independence:\n");
        for m in &low_independence {
            output.push_str(&format!(
                "  {:.0}% {} ({} files, {} internal, {} external)\n",
                m.independence * 100.0,
                m.dir,
                m.file_count,
                m.internal_deps,
                m.external_deps
            ));
        }
    }

    // Dependency depth
    if result.max_depth > 0 {
        output.push_str(&format!(
            "\nDependency Depth: {} (longest chain)\n",
            result.max_depth
        ));
        if !result.critical_path.is_empty() {
            output.push_str("  Critical path: ");
            for (i, p) in result.critical_path.iter().enumerate() {
                if i > 0 {
                    output.push_str(" -> ");
                }
                let short = std::path::Path::new(p)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(p);
                output.push_str(short);
            }
            output.push('\n');
        }
    }

    // Violation age summary
    let age = &result.violation_age;
    if age.new_count > 0 || age.recent_count > 0 || age.chronic_count > 0 {
        output.push_str(&format!(
            "\nViolation Age: {} new, {} recent, {} chronic\n",
            age.new_count, age.recent_count, age.chronic_count
        ));
    }

    output.push_str(&format!("\n{}\n", VERSION));

    output
}

/// Render every Issue as an Issue card (`CONTEXT.md` § Issue card):
/// kind, severity band, subject, reason, recommendation. The match is
/// exhaustive on purpose — a new kind must be handled here to compile.
fn format_issue_cards(result: &AuditResult) -> String {
    let issues = result.issues();
    if issues.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let band_count = |band: SeverityBand| issues.iter().filter(|i| i.severity() == band).count();
    out.push_str(&format!(
        "\nIssues ({}): {} critical, {} high, {} medium, {} low\n",
        issues.len(),
        band_count(SeverityBand::Critical),
        band_count(SeverityBand::High),
        band_count(SeverityBand::Medium),
        band_count(SeverityBand::Low),
    ));
    for issue in &issues {
        // Per-kind extra line: the number formats have always shown next
        // to the subject and that the card's one-sentence reason omits.
        let extra: Option<String> = match issue {
            Issue::CouplingViolation(v) => Some(format!(
                "{} <> {} (depth {}, line {})",
                v.dir_a, v.dir_b, v.depth, v.line_number
            )),
            Issue::Cycle(v) => v
                .weakest_link
                .as_ref()
                .map(|wl| format!("Weakest link: {}", wl)),
            Issue::RuleViolation(r) => Some(format!("line {}", r.line_number)),
            Issue::LayerViolation(l) => Some(format!(
                "{} -> {} (line {})",
                l.from_layer, l.to_layer, l.line_number
            )),
            Issue::GravityWell(g) => Some(format!(
                "RRI {:.0} across {} relationships",
                g.total_rri, g.relationship_count
            )),
            Issue::RedFlag(f) => Some(format!("RRI {:.0}", f.rri)),
            Issue::StabilityViolation(s) => Some(format!(
                "I={:.2} -> I={:.2}",
                s.from_instability, s.to_instability
            )),
            Issue::ZoneFlag(d) => Some(format!(
                "D={:.2} (A={:.2}, I={:.2})",
                d.distance, d.abstractness, d.instability
            )),
            Issue::LowCohesion(c) => Some(format!(
                "{} children, {} internal deps",
                c.n_children, c.internal_deps
            )),
        };
        out.push_str(&format!(
            "\n  [{}] {}: {}\n",
            issue.severity().name().to_uppercase(),
            issue.kind(),
            issue.subject()
        ));
        if let Some(extra) = extra {
            out.push_str(&format!("      {}\n", extra));
        }
        out.push_str(&format!("      Reason: {}\n", issue.reason()));
        out.push_str(&format!(
            "      Recommendation: {}\n",
            issue.recommendation()
        ));
    }
    out
}

pub fn format_monorepo_text(monorepo: &noupling_core::analyzer::MonorepoResult) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "Overall Score: {:.1}/100\n",
        monorepo.overall_score
    ));
    output.push_str(&format!("Total Modules: {}\n\n", monorepo.total_modules));
    output.push_str(&format!(
        "{:<20} {:>8} {:>10} {:>12}\n",
        "MODULE", "SCORE", "MODULES", "VIOLATIONS"
    ));
    output.push_str(&format!("{}\n", "-".repeat(52)));

    for (name, result) in &monorepo.module_results {
        output.push_str(&format!(
            "{:<20} {:>7.1} {:>10} {:>12}\n",
            name,
            result.score,
            result.total_modules,
            result.violations.len(),
        ));
    }

    if !monorepo.cross_module_violations.is_empty() {
        output.push_str(&format!(
            "\nCross-Module Violations ({}):\n",
            monorepo.cross_module_violations.len()
        ));
        for v in &monorepo.cross_module_violations {
            output.push_str(&format!(
                "  {} -> {} (not in depends_on)\n",
                v.from_config, v.to_config
            ));
            output.push_str(&format!(
                "    {} -> {} (line {})\n",
                v.from_file, v.to_file, v.line_number
            ));
        }
    }

    output.push_str(&format!("\n{}\n", VERSION));
    output
}
