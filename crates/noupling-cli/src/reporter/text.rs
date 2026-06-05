//! Plain-text format adapter — what the `audit` command prints to
//! stdout. Also owns the monorepo-aware variant used when audit
//! discovers per-module configs.

use noupling_core::analyzer::AuditResult;

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

    if !result.violations.is_empty() {
        output.push('\n');
        for v in &result.violations {
            let dir_label = match v.direction {
                noupling_core::analyzer::DependencyDirection::Downward => "\u{2193}",
                noupling_core::analyzer::DependencyDirection::Sibling => "\u{2194}",
                noupling_core::analyzer::DependencyDirection::Upward => "\u{2191}",
                noupling_core::analyzer::DependencyDirection::External => "\u{2197}",
                noupling_core::analyzer::DependencyDirection::Transitive => "\u{21dd}",
                noupling_core::analyzer::DependencyDirection::Circular => "\u{21bb}",
            };
            let rri_label = if v.rri > 0.0 {
                format!(" RRI:{:.0}", v.rri)
            } else {
                String::new()
            };
            let weight_label = if v.is_circular {
                " CIRCULAR".to_string()
            } else if v.weight > 1 {
                format!(" x{}", v.weight)
            } else {
                String::new()
            };
            output.push_str(&format!(
                "  [{:.2}]{}{} {} {} -> {} (depth {})\n",
                v.severity, weight_label, rri_label, dir_label, v.from_module, v.to_module, v.depth
            ));
            output.push_str(&format!("         {} <> {}\n", v.dir_a, v.dir_b));
            if let Some(ref wl) = v.weakest_link {
                output.push_str(&format!("         Weakest link: {}\n", wl));
            }
        }
    }

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

    // Rule violations
    if !result.rule_violations.is_empty() {
        output.push_str(&format!(
            "\nRule Violations ({}):\n",
            result.rule_violations.len()
        ));
        for rv in &result.rule_violations {
            output.push_str(&format!(
                "  {} -> {} (line {})\n    {}\n",
                rv.from_module, rv.to_module, rv.line_number, rv.message
            ));
        }
    }

    // Layer violations
    if !result.layer_violations.is_empty() {
        output.push_str(&format!(
            "\nLayer Violations ({}):\n",
            result.layer_violations.len()
        ));
        for lv in &result.layer_violations {
            output.push_str(&format!(
                "  {} ({}) -> {} ({}) (line {})\n",
                lv.from_module, lv.from_layer, lv.to_module, lv.to_layer, lv.line_number
            ));
        }
    }

    // Cohesion (low cohesion directories — Packages only; Containers have
    // undefined cohesion by design, so they never appear in this section).
    let low_cohesion: Vec<_> = result
        .cohesion
        .iter()
        .filter_map(|c| c.cohesion.map(|val| (c, val)))
        .filter(|(c, val)| *val < 0.1 && c.n_children >= 3)
        .take(10)
        .collect();
    if !low_cohesion.is_empty() {
        output.push_str("\nLow Cohesion:\n");
        for (c, val) in &low_cohesion {
            output.push_str(&format!(
                "  {:.2} {} ({} files, {} internal deps)\n",
                val, c.dir, c.n_children, c.internal_deps
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

    // Stable Dependencies Principle violations
    if !result.stability_violations.is_empty() {
        output.push_str("\nStability Violations:\n");
        output
            .push_str("  (a more-stable directory depends on a less-stable one — Martin's SDP)\n");
        for v in result.stability_violations.iter().take(10) {
            output.push_str(&format!(
                "  {} (I={:.2}) -> {} (I={:.2})\n",
                v.from_dir, v.from_instability, v.to_dir, v.to_instability
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

    // Gravity Wells
    if !result.gravity_wells.is_empty() {
        output.push_str(&format!(
            "\nGravity Wells ({}):\n",
            result.gravity_wells.len()
        ));
        for g in result.gravity_wells.iter().take(10) {
            output.push_str(&format!(
                "  [RRI:{:.0}] {} ({} relationships)\n",
                g.total_rri, g.module_path, g.relationship_count
            ));
        }
    }

    // Red Flags
    if !result.red_flags.is_empty() {
        output.push_str(&format!("\nRed Flags ({}):\n", result.red_flags.len()));
        for f in result.red_flags.iter().take(10) {
            let flag_icon = match f.flag_type {
                noupling_core::analyzer::RedFlagType::FusedSibling => "\u{26a0}",
                noupling_core::analyzer::RedFlagType::TrappedChild => "\u{26d4}",
            };
            output.push_str(&format!("  {} {}\n", flag_icon, f.recommendation));
        }
    }

    output.push_str(&format!("\n{}\n", VERSION));

    output
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
