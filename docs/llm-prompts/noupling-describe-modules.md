# Skill: noupling-describe-modules

**Status:** v1 spec — first focused enrichment skill (#280).

**Purpose:** Generate a one-line natural-language **purpose** for each container/package in a noupling-scanned codebase, plus an optional multi-sentence **responsibility** description and a small set of **tags**. The output lands at `.noupling/enrichment/modules.json`; the Explorer's Composition view (PRD §10.8, #279) picks it up automatically on next `noupling report --format explorer`.

## When the user runs this

```
$ claude code skill run noupling-describe-modules
```

(Or invoke directly inside Claude Code by referring to the prompt below; the runtime is intentionally up to the user.)

The skill is **provider-neutral in spec**, **Claude-only for v1** (per #280 resolution). The prompt + JSON output schema below is documented so future runners (GPT, local model) can port.

## Prompt (executed per container)

> You are summarising one folder/package in a software codebase noupling has scanned.
>
> **Folder path:** `{module_path}`
> **Files in this folder:** `{file_count}` (`{dominant_language}`)
> **A handful of representative file names:** `{up to 8 basenames}`
> **Layer noupling classified this in:** `{layer_or_unlayered}`
>
> Return a JSON object with the following shape:
>
> ```json
> {
>   "summary": "A 3–7 word noun phrase describing what this folder *is*. Examples: 'Payment processing core', 'HTTP request decoders', 'Test fixtures for billing'. Avoid generic phrases like 'utility code' — be specific.",
>   "responsibility": "Two or three sentences describing what this folder *does*, written for a developer who has never seen the codebase. Mention specific responsibilities; avoid restating the folder name.",
>   "tags": ["domain", "test", "infra", "ui", "platform-adapter", …]
> }
> ```
>
> Constraints:
> - If you cannot confidently classify this folder, emit `null` for `summary` and `responsibility` and leave `tags` empty.
> - Do not invent dependencies or relationships. Describe identity only.

## Sidecar file shape

The skill writes per-container entries to `.noupling/enrichment/modules.json`:

```json
{
  "schema_version": 1,
  "entries": [
    {
      "module_path": "src/payments",
      "summary": "Payment processing core",
      "responsibility": "Drives Stripe and Adyen integrations, normalising webhooks into the domain Order events.",
      "tags": ["domain"],
      "generated_at": "2026-06-05T10:32:01Z",
      "model": "claude-opus-4-7"
    }
  ]
}
```

- `module_path` must match a `NodeEntry.id` (a container path) in the Data Contract — typically the folder path relative to the codebase root.
- `schema_version: 1` lets the Rust loader warn-and-ignore newer versions.
- The Rust loader (`crates/noupling-cli/src/commands/report.rs`) merges these into each node's `metrics.llm` block; the Composition view reads `metrics.llm.summary` as the labeled card text.

## Commit policy

`.noupling/enrichment/` is meant to be **committed to the repo** so CI checkouts don't have to re-run an LLM. The repo's `.gitignore` should carve it out of any blanket `.noupling/*` exclusion:

```gitignore
.noupling/*
!.noupling/enrichment/
```

The skill should verify or offer this gitignore edit when it first writes to the directory.

## Deferred from v1 (per #280)

- **Other focused skills**: `noupling-narrate-architecture`, `noupling-explain-cycles`.
- **Umbrella skill** that runs all enrichment skills together.
- **Provider-neutral runner** (GPT, local model).
- **Content-hash staleness detection**: v1 doesn't compare current file content to the hash that produced the entry — `generated_at` is the only freshness signal. The schema field for hashes is reserved for v2.
- **Migration tooling** (`noupling enrichment migrate`).
