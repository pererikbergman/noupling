//! The `Issue` view over an [`AuditResult`] (#340, epic #338).
//!
//! See `CONTEXT.md` § Findings. Every report format that lists Findings
//! renders the same Issue cards from [`AuditResult::issues`]; the per-kind
//! lists on the result stay as they are so scoring and Metrics keep working.
//! This module is the one place that decides, for every kind, the severity
//! band, the subject, and the reason / recommendation wording.

use std::cmp::Ordering;
use std::fmt;

use serde::Serialize;

use super::{
    AuditResult, CohesionMetrics, CouplingViolation, DependencyDirection, DistanceMetric,
    GravityWell, LayerViolation, RedFlag, RedFlagType, RuleViolation, StabilityViolation, Zone,
};

/// The closed set of Issue kinds, in canonical order. Adding a variant is a
/// deliberate decision (`CONTEXT.md`); every listing format matches on
/// [`Issue`] exhaustively, so a new kind fails to compile until handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IssueKind {
    CouplingViolation,
    Cycle,
    RuleViolation,
    LayerViolation,
    GravityWell,
    RedFlag,
    StabilityViolation,
    ZoneFlag,
    LowCohesion,
}

impl IssueKind {
    /// Every kind, in canonical order.
    pub const ALL: [IssueKind; 9] = [
        IssueKind::CouplingViolation,
        IssueKind::Cycle,
        IssueKind::RuleViolation,
        IssueKind::LayerViolation,
        IssueKind::GravityWell,
        IssueKind::RedFlag,
        IssueKind::StabilityViolation,
        IssueKind::ZoneFlag,
        IssueKind::LowCohesion,
    ];

    /// The glossary name of the kind, e.g. `"Coupling Violation"`.
    pub fn name(self) -> &'static str {
        match self {
            IssueKind::CouplingViolation => "Coupling Violation",
            IssueKind::Cycle => "Cycle",
            IssueKind::RuleViolation => "Rule Violation",
            IssueKind::LayerViolation => "Layer Violation",
            IssueKind::GravityWell => "Gravity Well",
            IssueKind::RedFlag => "Red Flag",
            IssueKind::StabilityViolation => "Stability Violation",
            IssueKind::ZoneFlag => "Zone Flag",
            IssueKind::LowCohesion => "Low Cohesion",
        }
    }

    /// The accent colour every visual format uses for this kind, so the
    /// dashboard tiles, strategy series, graph edges and bundle legend
    /// agree. Exhaustive so a new kind must pick one.
    pub fn accent_color(self) -> &'static str {
        match self {
            IssueKind::CouplingViolation => "#eab308",
            IssueKind::Cycle => "#ef4444",
            IssueKind::RuleViolation => "#dc2626",
            IssueKind::LayerViolation => "#b91c1c",
            IssueKind::GravityWell => "#8b5cf6",
            IssueKind::RedFlag => "#db2777",
            IssueKind::StabilityViolation => "#0ea5e9",
            IssueKind::ZoneFlag => "#14b8a6",
            IssueKind::LowCohesion => "#64748b",
        }
    }

    /// Stable machine identifier, e.g. `"coupling_violation"`.
    pub fn id(self) -> &'static str {
        match self {
            IssueKind::CouplingViolation => "coupling_violation",
            IssueKind::Cycle => "cycle",
            IssueKind::RuleViolation => "rule_violation",
            IssueKind::LayerViolation => "layer_violation",
            IssueKind::GravityWell => "gravity_well",
            IssueKind::RedFlag => "red_flag",
            IssueKind::StabilityViolation => "stability_violation",
            IssueKind::ZoneFlag => "zone_flag",
            IssueKind::LowCohesion => "low_cohesion",
        }
    }
}

impl fmt::Display for IssueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The importance of an Issue on a four-step scale. Assigned once here;
/// every report shows the same band for the same Issue. Ordered so that
/// `Critical > High > Medium > Low`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SeverityBand {
    Low,
    Medium,
    High,
    Critical,
}

impl SeverityBand {
    pub fn name(self) -> &'static str {
        match self {
            SeverityBand::Critical => "critical",
            SeverityBand::High => "high",
            SeverityBand::Medium => "medium",
            SeverityBand::Low => "low",
        }
    }
}

impl fmt::Display for SeverityBand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What an Issue is about (`CONTEXT.md` § Subject).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// One module or directory (node-shaped Issues).
    Module(String),
    /// One `from → to` edge (edge-shaped Issues).
    Edge { from: String, to: String },
    /// An ordered ring of directories; the last entry closes back to the first.
    Ring(Vec<String>),
}

impl Subject {
    /// The path used to order Issues within a kind and band: the module,
    /// the edge's source, or the ring's first member.
    pub fn sort_path(&self) -> &str {
        match self {
            Subject::Module(p) => p,
            Subject::Edge { from, .. } => from,
            Subject::Ring(members) => members.first().map(String::as_str).unwrap_or(""),
        }
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Subject::Module(p) => f.write_str(p),
            Subject::Edge { from, to } => write!(f, "{} -> {}", from, to),
            Subject::Ring(members) => f.write_str(&members.join(" -> ")),
        }
    }
}

/// One Finding that names modules or edges and asks for a change.
///
/// The Issue-card header — kind, band, subject, reason, recommendation,
/// score impact — is computed by the accessor methods from `detail`, so
/// the wording lives in exactly one place. `baselined` is the one header
/// field that depends on something outside the audit (the baseline file).
#[derive(Debug, Clone)]
pub struct Issue {
    /// True when the project's baseline accepts this Issue: still reported,
    /// never counted as new (`CONTEXT.md` § Baseline).
    pub baselined: bool,
    /// The analyzer record this Issue was derived from, one variant per kind.
    pub detail: IssueDetail,
}

/// The per-kind payload of an [`Issue`]. One variant per Issue kind; every
/// listing format matches on it exhaustively, so a new kind fails to
/// compile until handled.
#[derive(Debug, Clone)]
pub enum IssueDetail {
    /// A sibling or upward edge the audit disallows. Never a circular pair.
    CouplingViolation(CouplingViolation),
    /// A ring of directories depending on each other back to the start.
    Cycle(CouplingViolation),
    RuleViolation(RuleViolation),
    LayerViolation(LayerViolation),
    GravityWell(GravityWell),
    RedFlag(RedFlag),
    StabilityViolation(StabilityViolation),
    /// A directory in the Zone of Pain or the Zone of Uselessness.
    ZoneFlag(DistanceMetric),
    LowCohesion(CohesionMetrics),
}

// ── Band thresholds ─────────────────────────────────────────────────────
//
// Coupling Violations and Cycles carry the audit's `severity`; the critical
// step is the `critical_severity` threshold reports already use (0.5).
// Gravity Wells and Red Flags carry only an RRI (direction weight × import
// density, default weights 2–10), so they map through an RRI ladder.

const SEVERITY_CRITICAL: f64 = 0.5;
const SEVERITY_HIGH: f64 = 0.2;
const SEVERITY_MEDIUM: f64 = 0.1;

const RRI_CRITICAL: f64 = 80.0;
const RRI_HIGH: f64 = 40.0;
const RRI_MEDIUM: f64 = 20.0;

/// Low Cohesion thresholds: the same cutoffs the text report has always
/// used (`min_cohesion` default 0.1; fewer than three children is noise).
const LOW_COHESION_MAX: f64 = 0.1;
const LOW_COHESION_MIN_CHILDREN: usize = 3;

fn band_for_severity(severity: f64) -> SeverityBand {
    if severity >= SEVERITY_CRITICAL {
        SeverityBand::Critical
    } else if severity >= SEVERITY_HIGH {
        SeverityBand::High
    } else if severity >= SEVERITY_MEDIUM {
        SeverityBand::Medium
    } else {
        SeverityBand::Low
    }
}

fn band_for_rri(rri: f64) -> SeverityBand {
    if rri >= RRI_CRITICAL {
        SeverityBand::Critical
    } else if rri >= RRI_HIGH {
        SeverityBand::High
    } else if rri >= RRI_MEDIUM {
        SeverityBand::Medium
    } else {
        SeverityBand::Low
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{} {}", n, word)
    } else {
        format!("{} {}s", n, word)
    }
}

impl Issue {
    pub fn kind(&self) -> IssueKind {
        match &self.detail {
            IssueDetail::CouplingViolation(_) => IssueKind::CouplingViolation,
            IssueDetail::Cycle(_) => IssueKind::Cycle,
            IssueDetail::RuleViolation(_) => IssueKind::RuleViolation,
            IssueDetail::LayerViolation(_) => IssueKind::LayerViolation,
            IssueDetail::GravityWell(_) => IssueKind::GravityWell,
            IssueDetail::RedFlag(_) => IssueKind::RedFlag,
            IssueDetail::StabilityViolation(_) => IssueKind::StabilityViolation,
            IssueDetail::ZoneFlag(_) => IssueKind::ZoneFlag,
            IssueDetail::LowCohesion(_) => IssueKind::LowCohesion,
        }
    }

    /// The severity band. RRI-bearing kinds map through the critical
    /// thresholds; Rule and Layer Violations break an explicit policy and
    /// are high; Stability Violations and Zone Flags are medium; Low
    /// Cohesion is low.
    pub fn severity(&self) -> SeverityBand {
        match &self.detail {
            IssueDetail::CouplingViolation(v) | IssueDetail::Cycle(v) => {
                band_for_severity(v.severity)
            }
            IssueDetail::RuleViolation(_) | IssueDetail::LayerViolation(_) => SeverityBand::High,
            IssueDetail::GravityWell(g) => band_for_rri(g.total_rri),
            IssueDetail::RedFlag(f) => band_for_rri(f.rri),
            IssueDetail::StabilityViolation(_) | IssueDetail::ZoneFlag(_) => SeverityBand::Medium,
            IssueDetail::LowCohesion(_) => SeverityBand::Low,
        }
    }

    pub fn subject(&self) -> Subject {
        match &self.detail {
            IssueDetail::CouplingViolation(v) => Subject::Edge {
                from: v.from_module.clone(),
                to: v.to_module.clone(),
            },
            IssueDetail::Cycle(v) => Subject::Ring(v.cycle_path.clone()),
            IssueDetail::RuleViolation(r) => Subject::Edge {
                from: r.from_module.clone(),
                to: r.to_module.clone(),
            },
            IssueDetail::LayerViolation(l) => Subject::Edge {
                from: l.from_module.clone(),
                to: l.to_module.clone(),
            },
            IssueDetail::GravityWell(g) => Subject::Module(g.module_path.clone()),
            IssueDetail::RedFlag(f) => match f.modules.as_slice() {
                [from, to, ..] => Subject::Edge {
                    from: from.clone(),
                    to: to.clone(),
                },
                [one] => Subject::Module(one.clone()),
                [] => Subject::Module(String::new()),
            },
            IssueDetail::StabilityViolation(s) => Subject::Edge {
                from: s.from_dir.clone(),
                to: s.to_dir.clone(),
            },
            IssueDetail::ZoneFlag(d) => Subject::Module(d.dir.clone()),
            IssueDetail::LowCohesion(c) => Subject::Module(c.dir.clone()),
        }
    }

    /// One sentence saying why this particular Issue exists, with its numbers.
    pub fn reason(&self) -> String {
        match &self.detail {
            IssueDetail::CouplingViolation(v) => {
                let rri = if v.rri > 0.0 {
                    format!(", RRI {:.0}", v.rri)
                } else {
                    String::new()
                };
                match v.direction {
                    DependencyDirection::Upward => format!(
                        "{} imports its parent directory {} ({}{}), so the child cannot be reused without it.",
                        v.from_module,
                        v.to_module,
                        plural(v.weight.max(1), "import"),
                        rri
                    ),
                    _ => format!(
                        "{} imports across sibling directories {} and {} ({}, severity {:.2}{}).",
                        v.from_module,
                        v.dir_a,
                        v.dir_b,
                        plural(v.weight.max(1), "import"),
                        v.severity,
                        rri
                    ),
                }
            }
            IssueDetail::Cycle(v) => {
                let ring = v.cycle_path.join(" -> ");
                let total: usize = v.cycle_hop_counts.iter().sum();
                match &v.weakest_link {
                    Some(link) => format!(
                        "{} form a cycle of {} ({} in the ring); the cheapest break is {}.",
                        ring,
                        plural(v.cycle_order, "directory").replace("directorys", "directories"),
                        plural(total, "import"),
                        link
                    ),
                    None => format!(
                        "{} form a cycle of {} ({} in the ring).",
                        ring,
                        plural(v.cycle_order, "directory").replace("directorys", "directories"),
                        plural(total, "import")
                    ),
                }
            }
            IssueDetail::RuleViolation(r) => format!(
                "{} imports {} at line {}, which a dependency rule forbids: {}.",
                r.from_module,
                r.to_module,
                r.line_number,
                r.message.trim().trim_end_matches('.')
            ),
            IssueDetail::LayerViolation(l) => {
                if l.to_layer == "<unlayered>" {
                    format!(
                        "{} in layer {} imports {} at line {}, which belongs to no layer.",
                        l.from_module, l.from_layer, l.to_module, l.line_number
                    )
                } else {
                    format!(
                        "{} in layer {} imports {} in the higher layer {} at line {}.",
                        l.from_module, l.from_layer, l.to_module, l.to_layer, l.line_number
                    )
                }
            }
            IssueDetail::GravityWell(g) => format!(
                "{} carries a total RRI of {:.0} across {} — disproportionate to its neighbours, so everything nearby bends toward it.",
                g.module_path,
                g.total_rri,
                plural(g.relationship_count, "relationship")
            ),
            IssueDetail::RedFlag(f) => {
                let (a, b) = (
                    f.modules.first().map(String::as_str).unwrap_or(""),
                    f.modules.get(1).map(String::as_str).unwrap_or(""),
                );
                match f.flag_type {
                    RedFlagType::FusedSibling => format!(
                        "Fused sibling: {} and {} have {} between their directories (median {:.0}, RRI {:.0}), far tighter than their peers.",
                        a,
                        b,
                        plural(f.imports, "import"),
                        f.median_density,
                        f.rri
                    ),
                    RedFlagType::TrappedChild => format!(
                        "Trapped child: {} imports its parent {} and cannot be reused without it (RRI {:.0}).",
                        a, b, f.rri
                    ),
                }
            }
            IssueDetail::StabilityViolation(s) => format!(
                "{} (I={:.2}) depends on the less stable {} (I={:.2}), against the Stable Dependencies Principle.",
                s.from_dir, s.from_instability, s.to_dir, s.to_instability
            ),
            IssueDetail::ZoneFlag(d) => match d.zone {
                Zone::Pain => format!(
                    "{} is in the Zone of Pain: concrete (A={:.2}) yet stable (I={:.2}), D={:.2}.",
                    d.dir, d.abstractness, d.instability, d.distance
                ),
                Zone::Uselessness => format!(
                    "{} is in the Zone of Uselessness: abstract (A={:.2}) yet unstable (I={:.2}), D={:.2}.",
                    d.dir, d.abstractness, d.instability, d.distance
                ),
                Zone::MainSequence => format!(
                    "{} sits on the main sequence (A={:.2}, I={:.2}).",
                    d.dir, d.abstractness, d.instability
                ),
            },
            IssueDetail::LowCohesion(c) => format!(
                "{} has cohesion {:.2}: its {} share only {}.",
                c.dir,
                c.cohesion.unwrap_or(0.0),
                plural(c.n_children, "child").replace("childs", "children"),
                plural(c.internal_deps, "internal dependency").replace("dependencys", "dependencies")
            ),
        }
    }

    /// One sentence saying what to do about this Issue.
    pub fn recommendation(&self) -> String {
        match &self.detail {
            IssueDetail::CouplingViolation(v) => match v.direction {
                DependencyDirection::Upward => {
                    "Invert the dependency or introduce an interface the child can depend on."
                        .to_string()
                }
                _ => format!(
                    "Move the shared code into a common parent of {} and {}, or put it behind an interface both siblings depend on.",
                    v.dir_a, v.dir_b
                ),
            },
            IssueDetail::Cycle(v) => match &v.weakest_link {
                Some(link) => format!(
                    "Cut the cycle at {} by removing {} or moving that code behind an interface.",
                    link.split(" (").next().unwrap_or(link),
                    plural(v.break_cost, "import")
                ),
                None => "Cut the cycle at its weakest hop and move that code behind an interface."
                    .to_string(),
            },
            IssueDetail::RuleViolation(_) => {
                "Remove the import or route it through a module the rule allows.".to_string()
            }
            IssueDetail::LayerViolation(l) => {
                if l.to_layer == "<unlayered>" {
                    format!(
                        "Add {} to a layer below {}, or record the exception in dependency_rules.",
                        l.to_module, l.from_layer
                    )
                } else {
                    format!(
                        "Remove the import or move the shared code into {} or a layer below it.",
                        l.from_layer
                    )
                }
            }
            IssueDetail::GravityWell(g) => format!(
                "Split {} by responsibility so no single module anchors this much coupling.",
                g.module_path
            ),
            IssueDetail::RedFlag(f) => match f.flag_type {
                RedFlagType::FusedSibling => {
                    "Merge the two modules or extract the shared part into an abstraction both depend on."
                        .to_string()
                }
                RedFlagType::TrappedChild => {
                    "Invert the dependency or introduce an interface the child can depend on."
                        .to_string()
                }
            },
            IssueDetail::StabilityViolation(s) => format!(
                "Depend on an abstraction instead, or make {} at least as stable as {}.",
                s.to_dir, s.from_dir
            ),
            IssueDetail::ZoneFlag(d) => match d.zone {
                Zone::Pain => format!(
                    "Introduce abstractions in {} so its dependants can vary without editing it.",
                    d.dir
                ),
                Zone::Uselessness => format!(
                    "Remove the unused abstractions in {} or give them dependants.",
                    d.dir
                ),
                Zone::MainSequence => "No change needed.".to_string(),
            },
            IssueDetail::LowCohesion(c) => format!(
                "Split {} into packages whose files actually use each other, or fold it into a neighbour.",
                c.dir
            ),
        }
    }

    /// The points this Issue takes off the project score. Zero for kinds
    /// that do not score (Rule / Layer / Stability Violation, Gravity Well,
    /// Red Flag, Zone Flag, Low Cohesion). A Cycle's impact includes the
    /// ring hops folded into it, so the sum over all Issues equals
    /// `100 − score`.
    pub fn score_impact(&self) -> f64 {
        match &self.detail {
            IssueDetail::CouplingViolation(v) | IssueDetail::Cycle(v) => v.score_impact,
            IssueDetail::RuleViolation(_)
            | IssueDetail::LayerViolation(_)
            | IssueDetail::GravityWell(_)
            | IssueDetail::RedFlag(_)
            | IssueDetail::StabilityViolation(_)
            | IssueDetail::ZoneFlag(_)
            | IssueDetail::LowCohesion(_) => 0.0,
        }
    }

    /// The deepest directory that contains the whole subject: where a
    /// directory-tree report (html, md) files this Issue. Directory-shaped
    /// subjects anchor at themselves; file edges at the common parent of
    /// the two files; rings at the common parent of every member.
    pub fn anchor_dir(&self) -> String {
        match &self.detail {
            IssueDetail::CouplingViolation(v) => common_parent_dir(&[&v.dir_a, &v.dir_b]),
            IssueDetail::Cycle(v) => {
                let members: Vec<&str> = v.cycle_path.iter().map(String::as_str).collect();
                common_parent_dir(&members)
            }
            IssueDetail::RuleViolation(r) => {
                common_parent_dir(&[parent_dir(&r.from_module), parent_dir(&r.to_module)])
            }
            IssueDetail::LayerViolation(l) => {
                common_parent_dir(&[parent_dir(&l.from_module), parent_dir(&l.to_module)])
            }
            IssueDetail::GravityWell(g) => parent_dir(&g.module_path).to_string(),
            IssueDetail::RedFlag(f) => {
                let parents: Vec<&str> = f.modules.iter().map(|m| parent_dir(m)).collect();
                common_parent_dir(&parents)
            }
            IssueDetail::StabilityViolation(s) => common_parent_dir(&[&s.from_dir, &s.to_dir]),
            IssueDetail::ZoneFlag(d) => d.dir.clone(),
            IssueDetail::LowCohesion(c) => c.dir.clone(),
        }
    }

    /// Stable identity for the baseline: `<kind id>:<identity>`. Two Issues
    /// with the same fingerprint are the same Issue across audits. The
    /// identity is the subject, except for a Coupling Violation and a Red
    /// Flag, whose subjects are one representative import of an aggregated
    /// directory pair: there the identity is the pair (`dir_a -> dir_b`),
    /// so adding or removing an import between the same directories does
    /// not make the Issue look new.
    pub fn fingerprint(&self) -> String {
        match &self.detail {
            IssueDetail::CouplingViolation(v) => format!(
                "{}:{}",
                self.kind().id(),
                Subject::Edge {
                    from: v.dir_a.clone(),
                    to: v.dir_b.clone()
                }
            ),
            IssueDetail::RedFlag(f) => format!(
                "{}:{}:{}",
                self.kind().id(),
                f.flag_type.id(),
                Subject::Edge {
                    from: f.dir_a.clone(),
                    to: f.dir_b.clone()
                }
            ),
            IssueDetail::Cycle(_)
            | IssueDetail::RuleViolation(_)
            | IssueDetail::LayerViolation(_)
            | IssueDetail::GravityWell(_)
            | IssueDetail::StabilityViolation(_)
            | IssueDetail::ZoneFlag(_)
            | IssueDetail::LowCohesion(_) => format!("{}:{}", self.kind().id(), self.subject()),
        }
    }

    /// Canonical order: band descending, then kind, then subject path.
    fn canonical_cmp(&self, other: &Issue) -> Ordering {
        other
            .severity()
            .cmp(&self.severity())
            .then_with(|| self.kind().cmp(&other.kind()))
            .then_with(|| self.subject().sort_path().cmp(other.subject().sort_path()))
            .then_with(|| self.subject().to_string().cmp(&other.subject().to_string()))
    }
}

// ── Serialised Issue card (ADR 0002) ────────────────────────────────────
//
// One JSON shape for every consumer: the JSON / XML / Sonar reports and
// the Explorer's Data Contract all embed `IssueCard` as produced here, so
// they cannot drift. Documented in docs/docs.html § Report Formats.

/// The serialised form of one Issue: the card header plus a per-kind
/// `detail` payload. Field names are the public contract.
#[derive(Debug, Clone, Serialize)]
pub struct IssueCard {
    /// Machine id of the kind, e.g. `"coupling_violation"` ([`IssueKind::id`]).
    pub kind: &'static str,
    /// Glossary name of the kind, e.g. `"Coupling Violation"`.
    pub kind_name: &'static str,
    /// `"critical" | "high" | "medium" | "low"`.
    pub severity: &'static str,
    pub subject: SubjectCard,
    pub reason: String,
    pub recommendation: String,
    /// Points this Issue takes off the score; 0 for non-scoring kinds.
    pub score_impact: f64,
    pub baselined: bool,
    /// The baseline identity: `<kind>:<subject>`, except Coupling Violation
    /// and Red Flag which key on their directory pair (see
    /// [`Issue::fingerprint`]).
    pub fingerprint: String,
    /// Per-kind numbers, keyed by snake_case field names (see docs).
    pub detail: serde_json::Value,
}

/// Serialised [`Subject`], tagged by shape.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubjectCard {
    Module { path: String },
    Edge { from: String, to: String },
    Ring { members: Vec<String> },
}

impl From<Subject> for SubjectCard {
    fn from(s: Subject) -> Self {
        match s {
            Subject::Module(path) => SubjectCard::Module { path },
            Subject::Edge { from, to } => SubjectCard::Edge { from, to },
            Subject::Ring(members) => SubjectCard::Ring { members },
        }
    }
}

impl Issue {
    /// The serialised Issue card. Every JSON-bearing format embeds this
    /// verbatim (ADR 0002).
    pub fn to_card(&self) -> IssueCard {
        IssueCard {
            kind: self.kind().id(),
            kind_name: self.kind().name(),
            severity: self.severity().name(),
            subject: self.subject().into(),
            reason: self.reason(),
            recommendation: self.recommendation(),
            score_impact: self.score_impact(),
            baselined: self.baselined,
            fingerprint: self.fingerprint(),
            detail: self.detail_json(),
        }
    }

    /// Per-kind payload. Exhaustive so a new kind must declare its numbers.
    fn detail_json(&self) -> serde_json::Value {
        use serde_json::json;
        match &self.detail {
            IssueDetail::CouplingViolation(v) => json!({
                "dir_a": v.dir_a,
                "dir_b": v.dir_b,
                "line_number": v.line_number,
                "depth": v.depth,
                "imports": v.weight,
                "raw_severity": v.severity,
                "rri": v.rri,
                "direction": v.direction,
            }),
            IssueDetail::Cycle(v) => json!({
                "order": v.cycle_order,
                "depth": v.depth,
                "raw_severity": v.severity,
                "rri": v.rri,
                "hop_import_counts": v.cycle_hop_counts,
                "hops": v.cycle_hop_files.iter().map(|(from, to, line)| json!({
                    "from_file": from,
                    "to_file": to,
                    "line_number": line,
                })).collect::<Vec<_>>(),
                "weakest_link": v.weakest_link,
                "break_cost": v.break_cost,
            }),
            IssueDetail::RuleViolation(r) => json!({
                "line_number": r.line_number,
                "message": r.message,
            }),
            IssueDetail::LayerViolation(l) => json!({
                "line_number": l.line_number,
                "from_layer": l.from_layer,
                "to_layer": l.to_layer,
            }),
            IssueDetail::GravityWell(g) => json!({
                "total_rri": g.total_rri,
                "relationship_count": g.relationship_count,
                "direction_count": g.direction_count,
                "downward_rri": g.downward_rri,
                "sibling_rri": g.sibling_rri,
                "upward_rri": g.upward_rri,
                "circular_rri": g.circular_rri,
            }),
            IssueDetail::RedFlag(f) => json!({
                "flag_type": f.flag_type.id(),
                "modules": f.modules,
                "dir_a": f.dir_a,
                "dir_b": f.dir_b,
                "rri": f.rri,
                "imports": f.imports,
                "median_density": f.median_density,
            }),
            IssueDetail::StabilityViolation(s) => json!({
                "from_instability": s.from_instability,
                "to_instability": s.to_instability,
            }),
            IssueDetail::ZoneFlag(d) => json!({
                "zone": match d.zone {
                    Zone::Pain => "pain",
                    Zone::Uselessness => "uselessness",
                    Zone::MainSequence => "main_sequence",
                },
                "abstractness": d.abstractness,
                "instability": d.instability,
                "distance": d.distance,
            }),
            IssueDetail::LowCohesion(c) => json!({
                "cohesion": c.cohesion,
                "n_children": c.n_children,
                "internal_deps": c.internal_deps,
            }),
        }
    }
}

impl AuditResult {
    /// Every Issue in this result, in canonical order: severity band
    /// descending (critical first), then kind in [`IssueKind::ALL`] order,
    /// then subject path ascending. Deterministic for a given result.
    ///
    /// A derived view: the per-kind lists on the result are untouched.
    /// Circular pairs surface once, as a [`Issue::Cycle`], and their hop
    /// edges are never also emitted as Coupling Violations.
    pub fn issues(&self) -> Vec<Issue> {
        let mut cycles: Vec<CouplingViolation> = self
            .violations
            .iter()
            .filter(|v| v.is_circular)
            .cloned()
            .collect();

        let mut details: Vec<IssueDetail> = Vec::new();

        for v in self.violations.iter().filter(|v| !v.is_circular) {
            match hop_of_cycle(v, &cycles) {
                // A ring hop belongs to its Cycle, points included.
                Some(idx) => cycles[idx].score_impact += v.score_impact,
                None => details.push(IssueDetail::CouplingViolation(v.clone())),
            }
        }
        details.extend(cycles.into_iter().map(IssueDetail::Cycle));
        details.extend(
            self.rule_violations
                .iter()
                .cloned()
                .map(IssueDetail::RuleViolation),
        );
        details.extend(
            self.layer_violations
                .iter()
                .cloned()
                .map(IssueDetail::LayerViolation),
        );
        details.extend(
            self.gravity_wells
                .iter()
                .cloned()
                .map(IssueDetail::GravityWell),
        );
        details.extend(self.red_flags.iter().cloned().map(IssueDetail::RedFlag));
        details.extend(
            self.stability_violations
                .iter()
                .cloned()
                .map(IssueDetail::StabilityViolation),
        );
        details.extend(
            self.distance
                .iter()
                .filter(|d| d.zone != Zone::MainSequence)
                .cloned()
                .map(IssueDetail::ZoneFlag),
        );
        details.extend(
            self.cohesion
                .iter()
                .filter(|c| {
                    c.n_children >= LOW_COHESION_MIN_CHILDREN
                        && c.cohesion.is_some_and(|val| val < LOW_COHESION_MAX)
                })
                .cloned()
                .map(IssueDetail::LowCohesion),
        );

        let mut issues: Vec<Issue> = details
            .into_iter()
            .map(|detail| Issue {
                baselined: false,
                detail,
            })
            .collect();
        if let Some(baseline) = &self.baseline {
            for issue in &mut issues {
                issue.baselined = baseline.fingerprints.contains(&issue.fingerprint());
            }
        }
        issues.sort_by(Issue::canonical_cmp);
        issues
    }
}

/// The directory a file path lives in (`""` for a bare file name).
pub fn parent_dir(path: &str) -> &str {
    match path.rfind('/') {
        Some(idx) => &path[..idx],
        None => "",
    }
}

/// The deepest directory that is each of `dirs` or an ancestor of it,
/// respecting path-segment boundaries (`src/ab` is not an ancestor of
/// `src/abc`). `""` when they share no prefix. Directory-tree reports use
/// it to file violations and Issues under the same directory.
pub fn common_parent_dir(dirs: &[&str]) -> String {
    let Some(first) = dirs.first() else {
        return String::new();
    };
    let mut common = first.to_string();
    for dir in &dirs[1..] {
        while !(common.is_empty() || *dir == common || dir.starts_with(&format!("{}/", common))) {
            common = parent_dir(&common).to_string();
        }
    }
    common
}

/// The index of the detected cycle that `v` is a hop of (in either
/// direction), if any. Such an edge belongs to the Cycle.
fn hop_of_cycle(v: &CouplingViolation, cycles: &[CouplingViolation]) -> Option<usize> {
    cycles.iter().position(|c| {
        c.cycle_path.windows(2).any(|hop| {
            (hop[0] == v.dir_a && hop[1] == v.dir_b) || (hop[0] == v.dir_b && hop[1] == v.dir_a)
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::analyzer::audit_with_settings;
    use crate::analyzer::{Issue, IssueKind, SeverityBand};
    use crate::core::{Dependency, Module, ModuleType};
    use crate::settings::Settings;

    fn file(id: &str, path: &str) -> Module {
        Module {
            id: id.to_string(),
            snapshot_id: "snap".to_string(),
            parent_id: None,
            name: path.rsplit('/').next().unwrap().to_string(),
            path: path.to_string(),
            module_type: ModuleType::File,
            depth: path.matches('/').count() as i32,
        }
    }

    fn dep(from: &str, to: &str, line: i32) -> Dependency {
        Dependency {
            from_module_id: from.into(),
            to_module_id: to.into(),
            line_number: line,
        }
    }

    fn kinds(issues: &[Issue]) -> Vec<IssueKind> {
        issues.iter().map(|i| i.kind()).collect()
    }

    #[test]
    fn a_ring_is_one_cycle_and_never_its_constituent_coupling_violations() {
        // alpha → beta → gamma → alpha, siblings under src/ring.
        let modules = vec![
            file("a", "src/ring/alpha/a.rs"),
            file("b", "src/ring/beta/b.rs"),
            file("c", "src/ring/gamma/c.rs"),
        ];
        let deps = vec![dep("a", "b", 1), dep("b", "c", 1), dep("c", "a", 1)];

        let result = audit_with_settings(&modules, &deps, &[], &Settings::default());
        let issues = result.issues();

        let cycles = issues
            .iter()
            .filter(|i| i.kind() == IssueKind::Cycle)
            .count();
        let couplings = issues
            .iter()
            .filter(|i| i.kind() == IssueKind::CouplingViolation)
            .count();
        assert_eq!(cycles, 1, "one Cycle for the ring: {:?}", kinds(&issues));
        assert_eq!(
            couplings,
            0,
            "ring hops must not also surface as Coupling Violations: {:?}",
            kinds(&issues)
        );
    }

    #[test]
    fn a_mutual_pair_is_one_cycle_even_though_the_detector_emits_both_edges() {
        let modules = vec![
            file("a", "src/ring/alpha/a.rs"),
            file("b", "src/ring/beta/b.rs"),
        ];
        let deps = vec![dep("a", "b", 1), dep("b", "a", 1)];

        let result = audit_with_settings(&modules, &deps, &[], &Settings::default());
        assert!(
            result.violations.iter().filter(|v| !v.is_circular).count() >= 1,
            "precondition: the detector still lists the pair's edges as sibling violations"
        );

        assert_eq!(kinds(&result.issues()), vec![IssueKind::Cycle]);
    }

    #[test]
    fn a_sibling_pair_is_one_coupling_violation() {
        let modules = vec![
            file("a", "src/slices/scanner/mod.rs"),
            file("b", "src/slices/storage/mod.rs"),
        ];
        let deps = vec![dep("a", "b", 10)];

        let result = audit_with_settings(&modules, &deps, &[], &Settings::default());
        let issues = result.issues();

        assert_eq!(kinds(&issues), vec![IssueKind::CouplingViolation]);
    }

    // ── The every_issue_kind fixture (#339) end to end through core ──

    /// Scan + audit the CLI's `every_issue_kind` fixture without the CLI:
    /// the same scanner and pipeline, so every kind is present.
    fn fixture_audit() -> crate::analyzer::AuditResult {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../noupling-cli/tests/fixtures/every_issue_kind");
        let settings = Settings::load(&root).expect("fixture settings");
        let scan = crate::scanner::scan_project(&root, "snap", settings.allow_inline_suppression)
            .expect("scan fixture");
        let type_counts = crate::scanner::recompute_type_counts(&root, &scan.modules);
        audit_with_settings(&scan.modules, &scan.dependencies, &type_counts, &settings)
    }

    fn fixture_issues() -> Vec<Issue> {
        fixture_audit().issues()
    }

    #[test]
    fn fixture_yields_every_kind() {
        let issues = fixture_issues();
        for kind in IssueKind::ALL {
            assert!(
                issues.iter().any(|i| i.kind() == kind),
                "fixture is missing {kind}: {:?}",
                kinds(&issues)
            );
        }
    }

    #[test]
    fn every_fixture_issue_has_a_reason_and_a_recommendation() {
        let issues = fixture_issues();
        assert!(!issues.is_empty());
        for issue in &issues {
            let reason = issue.reason();
            let recommendation = issue.recommendation();
            assert!(
                !reason.trim().is_empty(),
                "{} {} has an empty reason",
                issue.kind(),
                issue.subject()
            );
            assert!(
                !recommendation.trim().is_empty(),
                "{} {} has an empty recommendation",
                issue.kind(),
                issue.subject()
            );
            assert!(
                reason.contains(issue.subject().sort_path()),
                "{} reason must name its subject: {reason}",
                issue.kind()
            );
        }
    }

    #[test]
    fn issues_are_ordered_by_band_then_kind_then_subject() {
        let result = fixture_audit();
        let issues = result.issues();
        let keys: Vec<(std::cmp::Reverse<SeverityBand>, IssueKind, String)> = issues
            .iter()
            .map(|i| {
                (
                    std::cmp::Reverse(i.severity()),
                    i.kind(),
                    i.subject().sort_path().to_string(),
                )
            })
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "issues() must come out in canonical order");
        // Two calls on the same result agree exactly.
        let again: Vec<String> = result
            .issues()
            .iter()
            .map(|i| i.subject().to_string())
            .collect();
        let first: Vec<String> = issues.iter().map(|i| i.subject().to_string()).collect();
        assert_eq!(first, again);
    }

    #[test]
    fn fused_sibling_reason_carries_the_import_count_and_median() {
        // fused/left ↔ fused/right: 6 imports, median 1 (fixture README).
        let issues = fixture_issues();
        let flag = issues
            .iter()
            .find(|i| i.kind() == IssueKind::RedFlag)
            .expect("fixture has a Red Flag");
        let reason = flag.reason();
        assert!(reason.contains("6 imports"), "{reason}");
        assert!(reason.contains("median 1"), "{reason}");
    }

    #[test]
    fn severity_bands_follow_the_documented_ladders() {
        let issues = fixture_issues();
        let of = |kind: IssueKind| issues.iter().find(|i| i.kind() == kind).unwrap();
        // fused/left ↔ fused/right: severity 1.50 ≥ 0.5 → critical.
        assert_eq!(
            of(IssueKind::CouplingViolation).severity(),
            SeverityBand::Critical
        );
        assert_eq!(of(IssueKind::RuleViolation).severity(), SeverityBand::High);
        assert_eq!(of(IssueKind::LayerViolation).severity(), SeverityBand::High);
        assert_eq!(
            of(IssueKind::StabilityViolation).severity(),
            SeverityBand::Medium
        );
        assert_eq!(of(IssueKind::ZoneFlag).severity(), SeverityBand::Medium);
        assert_eq!(of(IssueKind::LowCohesion).severity(), SeverityBand::Low);
    }

    // ── Score impact (#342) ──

    #[test]
    fn score_impacts_sum_to_the_points_lost_on_the_fixture() {
        let result = fixture_audit();
        let issues = result.issues();
        let lost = 100.0 - result.score;
        let sum: f64 = issues.iter().map(|i| i.score_impact()).sum();
        assert!(lost > 1.0, "fixture must actually lose points: {lost}");
        assert!(
            (sum - lost).abs() < 1e-6,
            "sum of score impacts {sum} != points lost {lost}"
        );
    }

    #[test]
    fn non_scoring_kinds_report_exactly_zero_impact() {
        let issues = fixture_issues();
        for issue in &issues {
            match issue.kind() {
                IssueKind::CouplingViolation | IssueKind::Cycle => {
                    assert!(issue.score_impact() > 0.0, "{} must score", issue.kind())
                }
                _ => assert_eq!(issue.score_impact(), 0.0, "{} must not score", issue.kind()),
            }
        }
    }

    #[test]
    fn a_folded_ring_hop_charges_its_points_to_the_cycle() {
        // alpha ↔ beta: the detector emits a circular violation plus the two
        // sibling edges; all three carry points, but issues() shows one Cycle.
        let modules = vec![
            file("a", "src/ring/alpha/a.rs"),
            file("b", "src/ring/beta/b.rs"),
        ];
        let deps = vec![dep("a", "b", 1), dep("b", "a", 1)];
        let result = audit_with_settings(&modules, &deps, &[], &Settings::default());

        let issues = result.issues();
        assert_eq!(kinds(&issues), vec![IssueKind::Cycle]);
        let lost = 100.0 - result.score;
        assert!(
            (issues[0].score_impact() - lost).abs() < 1e-6,
            "cycle impact {} must equal points lost {}",
            issues[0].score_impact(),
            lost
        );
    }

    #[test]
    fn impacts_still_add_up_when_the_score_is_clamped_at_zero() {
        // One module, many heavy sibling imports: raw loss far exceeds 100.
        let modules = vec![file("a", "src/x/a.rs"), file("b", "src/y/b.rs")];
        let deps: Vec<Dependency> = (1..=200).map(|i| dep("a", "b", i)).collect();
        let result = audit_with_settings(&modules, &deps, &[], &Settings::default());
        assert_eq!(result.score, 0.0, "precondition: clamped");

        let sum: f64 = result.issues().iter().map(|i| i.score_impact()).sum();
        assert!(
            (sum - 100.0).abs() < 1e-6,
            "sum {sum} must equal the 100 points lost"
        );
    }

    // ── anchor_dir (#346) ──

    #[test]
    fn anchor_dir_is_the_deepest_directory_containing_the_whole_subject() {
        let issues = fixture_issues();
        let anchor_of = |kind: IssueKind, needle: &str| {
            issues
                .iter()
                .find(|i| i.kind() == kind && i.subject().to_string().contains(needle))
                .unwrap_or_else(|| panic!("no {kind} about {needle}"))
                .anchor_dir()
        };
        // Edge between src/loose/x and src/loose/y lives under src/loose.
        assert_eq!(
            anchor_of(IssueKind::CouplingViolation, "src/loose/x"),
            "src/loose"
        );
        // The ring alpha ↔ beta lives under src/ring.
        assert_eq!(anchor_of(IssueKind::Cycle, "src/ring/alpha"), "src/ring");
        // A file-level rule violation across top-level dirs anchors at src.
        assert_eq!(anchor_of(IssueKind::RuleViolation, "src/plugins"), "src");
        // Directory-shaped Issues anchor at the directory itself.
        assert_eq!(anchor_of(IssueKind::LowCohesion, "src/bag"), "src/bag");
        assert_eq!(
            anchor_of(IssueKind::ZoneFlag, "src/concrete"),
            "src/concrete"
        );
        // A gravity well is a file: its parent directory.
        assert_eq!(anchor_of(IssueKind::GravityWell, "l1.rs"), "src/fused/left");
        // Stability violation between src/stable and src/volatile: src.
        assert_eq!(
            anchor_of(IssueKind::StabilityViolation, "src/stable"),
            "src"
        );
    }
}
