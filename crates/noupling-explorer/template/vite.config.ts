import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";

// Produces a single self-contained HTML at dist/explorer.html.
// The Rust `noupling-explorer` crate embeds this file via include_str!
// and injects the Data Contract JSON into the placeholder
// <script id="noupling-data"> element at report-emission time.
export default defineConfig({
  plugins: [react(), viteSingleFile()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: "index.html",
      output: { entryFileNames: "explorer.js" },
    },
    assetsInlineLimit: 100_000_000,
    cssCodeSplit: false,
  },
  // Allow `?sample=<name>` to pick a fixture for dev hot-reload.
  server: { port: 5174, open: true },
});
