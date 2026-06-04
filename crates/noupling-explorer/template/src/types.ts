// Mirror of the Rust `noupling_explorer::data_contract::DataContract`
// emitted into the `<script id="noupling-data">` block. See PRD §6.
// Kept hand-authored (not codegen-from-Rust) — the contract is intended
// to be human-readable and the Rust + TS sides are version-locked.

export interface DataContract {
  format_version: 1;
  noupling_version: string;
  generated_at: string;
  report_options: ReportOptions;
  codebase: Codebase;
  health_score: number;
  summary_counts: SummaryCounts;
  layers: LayerEntry[];
  dependency_rules: DependencyRule[];
  effective_rules: EffectiveRule[];
  nodes: NodeEntry[];
  edges: EdgeEntry[];
  cycles: CycleEntry[];
  violations: ViolationEntry[];
  history: HistoryEntry[];
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
  minimum_cut: Array<{ from: string; to: string }>;
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
