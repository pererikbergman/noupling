//! HTML template assembly for the Explorer reporter.
//!
//! The template is a pre-built single-file HTML artifact (placeholder for
//! now; #232 replaces it with the real shadcn/Vite output). The placeholder
//! `<script id="noupling-data" type="application/json"></script>` element
//! is filled at emit time with the serialized Data Contract.

const TEMPLATE: &str = include_str!("../assets/placeholder.html");
const PLACEHOLDER: &str = "<script id=\"noupling-data\" type=\"application/json\"></script>";

/// Embed the serialized Data Contract into the template's placeholder script tag.
pub(crate) fn embed_data(json: &str) -> String {
    let filled = format!(
        "<script id=\"noupling-data\" type=\"application/json\">{}</script>",
        json
    );
    TEMPLATE.replacen(PLACEHOLDER, &filled, 1)
}
