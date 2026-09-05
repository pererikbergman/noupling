import type { DataContract } from "./shared/types";

/**
 * Read the Data Contract from the inlined `<script id="noupling-data">` tag.
 *
 * Two modes:
 *  - **Production** (Rust-emitted HTML): the script tag is filled with the
 *    serialized contract at report-emission time.
 *  - **Dev** (`pnpm dev`): the script tag is empty; we fall back to fetching
 *    `/samples/<name>.json` based on the `?sample=<name>` URL param (default
 *    `acme-payments`).
 */
export async function loadDataContract(): Promise<DataContract> {
  const inline = document.getElementById("noupling-data");
  const raw = inline?.textContent?.trim();
  if (raw && raw.length > 0) {
    return JSON.parse(raw) as DataContract;
  }
  const params = new URLSearchParams(location.search);
  const sample = params.get("sample") ?? "acme-payments";
  const res = await fetch(`/samples/${sample}.json`);
  if (!res.ok) {
    throw new Error(
      `noupling-data block is empty and the dev sample "/samples/${sample}.json" returned ${res.status}.`,
    );
  }
  return (await res.json()) as DataContract;
}
