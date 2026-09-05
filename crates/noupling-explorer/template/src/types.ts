// Mirror of the Rust `noupling_explorer::data_contract::DataContract`
// emitted into the `<script id="noupling-data">` block. See PRD §6.
// Kept hand-authored (not codegen-from-Rust) — the contract is intended
// to be human-readable and the Rust + TS sides are version-locked.

export interface DataContract {
  format_version: 2;
  noupling_version: string;
  generated_at: string;
  report_options: ReportOptions;
  layers_auto_detected: boolean;
  codebase: Codebase;
  health_score: number;
  score_breakdown: ScoreBreakdown;
  summary_counts: SummaryCounts;
  layers: LayerEntry[];
  dependency_rules: DependencyRule[];
  effective_rules: EffectiveRule[];
  nodes: NodeEntry[];
  edges: EdgeEntry[];
  /** Ring geometry for the canvas; the Cycle *Issue* lives in `issues`. */
  cycles: CycleEntry[];
  /** Edge geometry for the canvas; the violation *Issues* live in `issues`. */
  violations: ViolationEntry[];
  /** Every Issue, as the same Issue cards the JSON report emits
   *  (ADR 0002, #345), plus the participant node ids focus mode uses. */
  issues: IssueEntry[];
  history: HistoryEntry[];
  /** Pre-computed Force-view module clusters (#278 follow-up).
   *  Tightly coupled groups detected in Rust via label propagation. */
  clusters: ClusterEntry[];
}

export interface ClusterEntry {
  id: string;
  members: string[];
}

export interface ScoreBreakdown {
  total_modules: number;
  points_lost: number;
  /** One row per kind that scores; sums to `points_lost`. */
  by_kind: KindPoints[];
  /** Top Issues by score impact, at most 5. */
  top_contributors: ScoreContributor[];
}

export interface KindPoints {
  kind: IssueKindId;
  kind_name: string;
  points: number;
}

export interface ScoreContributor {
  kind: IssueKindId;
  kind_name: string;
  subject: string;
  focus_id: string;
  points: number;
  fingerprint: string;
}

/** Machine ids of the nine Issue kinds (`IssueKind::id` in core). */
export type IssueKindId =
  | "coupling_violation"
  | "cycle"
  | "rule_violation"
  | "layer_violation"
  | "gravity_well"
  | "red_flag"
  | "stability_violation"
  | "zone_flag"
  | "low_cohesion";

export type SeverityBand = "critical" | "high" | "medium" | "low";

export type IssueSubject =
  | { type: "module"; path: string }
  | { type: "edge"; from: string; to: string }
  | { type: "ring"; members: string[] };

/**
 * One Issue card — the shape `noupling_core::analyzer::IssueCard`
 * serialises, identical to an entry of `report.json`'s `issues` array
 * (documented in the docs site under Report Formats) — plus the node
 * ids that participate in it.
 */
export interface IssueEntry {
  kind: IssueKindId;
  kind_name: string;
  severity: SeverityBand;
  subject: IssueSubject;
  reason: string;
  recommendation: string;
  score_impact: number;
  baselined: boolean;
  fingerprint: string;
  /** Per-kind numbers; keys documented per kind in the docs site. */
  detail: Record<string, unknown>;
  /** Node ids focus mode expands, highlights and scopes to. */
  participants: string[];
}

export interface ReportOptions {
  editor: string | null;
  title: string | null;
}

export interface Codebase {
  path: string;
  module_count: number;
  file_count: number;
  edge_count: number;
  language_distribution: Array<{ language: string; file_count: number }>;
}

export interface SummaryCounts {
  violations: number;
  cycles: number;
  gravity_wells: number;
  red_flags: number;
  issues: number;
  new_issues: number;
  baselined_issues: number;
  /** All nine kinds, zero included, canonical order. */
  by_kind: KindCount[];
}

export interface KindCount {
  kind: IssueKindId;
  kind_name: string;
  count: number;
}

export interface LayerEntry {
  name: string;
  pattern: string;
  allow_sibling: boolean;
  index: number;
  file_count: number;
  afferent: number;
  efferent: number;
  instability: number | null;
}

export interface DependencyRule {
  from: string;
  to: string;
  allow: boolean;
  message: string;
}

export interface EffectiveRule extends DependencyRule {
  source: "dependency_rule" | "layer_order";
  current_violation_count: number;
}

export interface NodeEntry {
  id: string;
  kind: "file" | "package" | "container";
  parent: string | null;
  layer: string | null;
  metrics: Record<string, unknown>;
}

export interface EdgeEntry {
  from: string;
  to: string;
  weight: number;
  violates_rule: string | null;
}

export interface CycleEntry {
  id: string;
  size: number;
  members: string[];
  minimum_cut: CutEdge[];
}

export interface CutEdge {
  from: string;
  to: string;
  weight: number;
  vs_weight: number;
}

export interface ViolationEntry {
  rule: { from: string; to: string };
  edge: { from: string; to: string };
  severity: "low" | "medium" | "high";
  introduced_in: string | null;
}

export interface HistoryEntry {
  snapshot_id: string;
  taken_at: string;
  health_score: number;
}
