/**
 * Click-to-source URL construction per PRD §8.10.
 *
 * The `editor` option chosen at generation time (`noupling report
 * --format explorer --editor <name>`) maps to a URL scheme. Both
 * named editors and custom templates (`scheme://x/{path}:{line}`)
 * are supported. When `editor` is `null` (no flag passed), falls
 * back to a plain `file://` URL the OS will open with its default
 * association.
 */

export interface SourceTarget {
  /** Relative path inside the codebase (`crates/noupling-cli/src/cli.rs`). */
  relPath: string;
  /** Absolute prefix from `data.codebase.path`. */
  codebaseRoot: string;
  /** Optional line number for in-file navigation. */
  line?: number;
}

// vscode/cursor want `vscode://file/<absolute-path>:<line>` — for POSIX
// paths whose absolute form starts with `/`, this works out to TWO slashes
// between the scheme host (`file`) and the path. The leading `/` on the
// abs path supplies the second.
//
// sublime wants `subl://open?url=file:///<absolute-path>:<line>` — the
// path inside the `file:` URL needs its own leading-slash run; templates
// here add the two extra so the result is `file:///<path>` after
// substitution.
const TEMPLATES: Record<string, string> = {
  vscode: "vscode://file/{path}:{line}",
  jetbrains:
    "jetbrains://idea/navigate/reference?project={projectName}&path={relPath}:{line}",
  sublime: "subl://open?url=file://{path}:{line}",
  cursor: "cursor://file/{path}:{line}",
};

export function buildSourceUrl(editor: string | null, target: SourceTarget): string {
  const absPath = joinPath(target.codebaseRoot, target.relPath);
  const line = target.line ?? 1;
  const projectName = basename(target.codebaseRoot);
  if (!editor) {
    return `file://${absPath}`;
  }
  const template = TEMPLATES[editor] ?? (editor.includes("{path}") ? editor : null);
  if (!template) {
    // Unknown editor name — degrade to file://.
    return `file://${absPath}`;
  }
  return template
    .replaceAll("{path}", absPath)
    .replaceAll("{relPath}", target.relPath)
    .replaceAll("{line}", String(line))
    .replaceAll("{projectName}", projectName);
}

function joinPath(root: string, rel: string): string {
  const r = root.endsWith("/") ? root.slice(0, -1) : root;
  if (rel.startsWith("/")) return rel;
  // Special-case `.` or `./` — `codebase.path` is often the literal `.`
  // when the user runs `noupling report .` from inside the repo. The CLI
  // resolves it on emit, but if it lands here unresolved, joining yields
  // `./crates/...` which is useless for an editor URL. Fall back to the
  // relative path alone; the OS or editor association will figure it out.
  if (r === "." || r === "" || r === "./") return rel;
  return `${r}/${rel}`;
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}
