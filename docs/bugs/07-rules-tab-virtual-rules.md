# Bug 7: Rules tab: virtual (local-only) rules — what-if exploration

**Area:** Explorer — left pane, Rules tab

**Status:** The tab itself is fine as-is.

**Request:** Allow adding a *virtual rule* that applies only locally inside the current Explorer view — in-memory, never written to `.noupling/settings.json` (consistent with PRD NG1/NG8). The user could sketch "what if this dependency direction were forbidden?" and see which edges would light up as violations.

**Caveat:** Violation evaluation currently happens in Rust at generation time; a purely client-side rule engine would mean re-implementing rule matching in the web view, or shipping precomputed edge data rich enough to evaluate globs in JS. Possibly large.

**Priority: VERY LOW.** Explicitly deprioritized — do not pick up before items 1–6. Aligns with PRD G6 (v2 what-if exploration), so park it there.

---

## Validation (Claude)

Author already deprioritized this to v2. No questions to raise — leaving as parked.

Two technical notes for whoever picks it up later:
- The "caveat" is correct: violation evaluation is in `noupling-core` (Rust) at generation time. Either we (a) ship every edge's `from`/`to` path strings rich enough to evaluate globs client-side, or (b) ship a compact compiled-rule format + a small JS glob matcher (e.g. `picomatch` is ~3 KB). (b) is the lower-bandwidth path.
- Aligns with PRD §9 v2 Sandbox. The action-log primitive (§9.5) from v2 would be the natural home: a virtual rule is conceptually one entry in the action log ("temporarily forbid edges matching X→Y").

## Open questions

_Parked — no questions until v1 work is complete._
