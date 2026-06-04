//! HTML template assembly for the Explorer reporter.
//!
//! The template is the pre-built single-file HTML artifact emitted by
//! `crates/noupling-explorer/template/` (Vite + React + Tailwind, single-file
//! plugin). The frontend subproject runs its own build pipeline; the Rust
//! crate consumes the committed artifact via `include_str!`. CI drift-checks
//! that `dist/explorer.html` matches what `pnpm build` would produce from
//! `template/src/`.
//!
//! The placeholder `<script id="noupling-data" type="application/json"></script>`
//! element in the template is filled at emit time with the serialized
//! Data Contract.

const TEMPLATE: &str = include_str!("../template/dist/explorer.html");
const PLACEHOLDER: &str = "<script id=\"noupling-data\" type=\"application/json\"></script>";

/// Embed the serialized Data Contract into the template's placeholder script tag.
pub(crate) fn embed_data(json: &str) -> String {
    let filled = format!(
        "<script id=\"noupling-data\" type=\"application/json\">{}</script>",
        json
    );
    TEMPLATE.replacen(PLACEHOLDER, &filled, 1)
}
