import type { NodeEntry, EdgeEntry } from "../shared/types";
import { displayLabel } from "../shared/labels";

/**
 * Composition view (PRD §10.8). Answers "what *kind* of thing is
 * each module" by rendering containers as labeled cards. v1 ships the
 * derived-only fallback: name, file count, dominant language, layer
 * tag. LLM-generated purpose labels light up automatically once #280
 * (enrichment infrastructure) lands and the data contract carries an
 * `llm` block per node.
 *
 * Distinct from LSM (architecture) by focusing on identity, not
 * relationships.
 */
export interface CompositionViewProps {
  nodes: NodeEntry[];
  edges: EdgeEntry[];
  onNodeClick?: (id: string) => void;
  /** Under issue focus: the participants. Other cards stay, dimmed (#335, #401). */
  focusIds?: Set<string> | null;
}

export function CompositionView({ nodes, onNodeClick, focusIds }: CompositionViewProps) {
  // The same node set the LSM shows at this scope: directories, and the
  // files that stand in for an expanded container under issue focus.
  // Filtering files out here made a focused Issue show only the
  // *non*-participants (#401).
  if (nodes.length === 0) {
    return (
      <p className="m-6 max-w-md text-[12px] text-muted">
        Nothing at this level. Drill out, or clear the filter.
      </p>
    );
  }

  return (
    <div className="h-full overflow-auto p-4">
      <EnrichmentBanner />
      <ul className="m-0 mt-3 grid list-none grid-cols-1 gap-3 p-0 md:grid-cols-2 lg:grid-cols-3">
        {nodes.map((n) => (
          <ContainerCard
            key={n.id}
            node={n}
            dimmed={!!focusIds && focusIds.size > 0 && !focusIds.has(n.id)}
            onClick={() => onNodeClick?.(n.id)}
          />
        ))}
      </ul>
    </div>
  );
}

function ContainerCard({
  node,
  onClick,
  dimmed = false,
}: {
  node: NodeEntry;
  onClick: () => void;
  dimmed?: boolean;
}) {
  const fc =
    typeof node.metrics.file_count === "number" ? node.metrics.file_count : null;
  const lang = pickDominantLanguage(node);
  // Per the bug resolution: when `llm` is absent, fall back to derived
  // metadata. Once #280 ships an `llm.summary` per container, this is
  // a pure data upgrade — no view changes.
  const llmSummary =
    typeof node.metrics.llm === "object" &&
    node.metrics.llm !== null &&
    typeof (node.metrics.llm as { summary?: string }).summary === "string"
      ? (node.metrics.llm as { summary: string }).summary
      : null;

  return (
    <li>
      <button
        onClick={onClick}
        data-dimmed={dimmed ? "true" : "false"}
        className={
          "block w-full rounded-md border border-border bg-canvas p-3 text-left text-[12px] transition-colors hover:border-text/30 hover:bg-canvas/60" +
          (dimmed ? " opacity-40" : "")
        }
        title={`${node.id} — click to focus`}
      >
        <div className="mb-1 flex items-baseline justify-between gap-2">
          <span className="truncate font-mono text-[13px] font-semibold text-text">
            {displayLabel(node)}
          </span>
          {node.layer && (
            <span
              className={
                "rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider " +
                layerChipClass(node.layer)
              }
            >
              {node.layer}
            </span>
          )}
        </div>
        {llmSummary && (
          <p className="m-0 mb-1.5 text-[11px] leading-relaxed text-text">
            {llmSummary}
          </p>
        )}
        <p className="m-0 font-mono text-[10px] text-muted">
          {node.kind === "file"
            ? "file"
            : fc !== null
              ? `${fc} file${fc === 1 ? "" : "s"}`
              : "—"}
          {lang ? ` · ${lang}` : ""}
        </p>
      </button>
    </li>
  );
}

function EnrichmentBanner() {
  return (
    <div
      role="note"
      className="rounded-md border border-accent-ui/40 bg-accent-ui/10 p-3 text-[11px] leading-relaxed"
    >
      <strong className="text-accent-ui">Composition</strong> shows what
      each module <em>is</em>: name, file count, dominant language, layer.
      When <code className="font-mono">.noupling/enrichment/modules.json</code>{" "}
      carries a summary per module, it is shown on the card.
    </div>
  );
}

function pickDominantLanguage(node: NodeEntry): string | null {
  // Heuristic: read `language_distribution` if present (container metrics
  // can carry it post-#280), else attempt to infer from file extension
  // when this is a single-file leaf.
  const dist = (node.metrics as { language_distribution?: Array<{ language: string; file_count: number }> })
    .language_distribution;
  if (Array.isArray(dist) && dist.length > 0) {
    return dist[0].language;
  }
  // Fallback to extension of node id when leafy.
  const ext = node.id.split(".").pop();
  if (ext && ext.length <= 4) return ext;
  return null;
}

function layerChipClass(layer: string): string {
  if (layer.includes("ui")) return "bg-accent-ui/20 text-accent-ui";
  if (layer.includes("infra")) return "bg-accent-infra/20 text-accent-infra";
  if (layer.includes("domain")) return "bg-accent-domain/20 text-accent-domain";
  return "bg-muted/20 text-muted";
}
