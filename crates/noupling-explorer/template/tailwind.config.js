/** @type {import("tailwindcss").Config} */
export default {
  darkMode: ["class", '[data-theme="dark"]'],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Tokens from docs/noupling-explorer-design.md (dark + light).
        // Re-exposed as Tailwind names so components stay readable.
        canvas: "rgb(var(--bg) / <alpha-value>)",
        card: "rgb(var(--card) / <alpha-value>)",
        "card-header": "rgb(var(--card-header) / <alpha-value>)",
        border: "rgb(var(--border) / <alpha-value>)",
        text: "rgb(var(--text) / <alpha-value>)",
        muted: "rgb(var(--text-muted) / <alpha-value>)",
        pill: "rgb(var(--pill-active-bg) / <alpha-value>)",
        "pill-text": "rgb(var(--pill-active-text) / <alpha-value>)",
        action: "rgb(var(--action-bg) / <alpha-value>)",
        "action-text": "rgb(var(--action-text) / <alpha-value>)",
        success: "rgb(var(--success) / <alpha-value>)",
        "accent-ui": "rgb(var(--accent-ui) / <alpha-value>)",
        "accent-domain": "rgb(var(--accent-domain) / <alpha-value>)",
        "accent-infra": "rgb(var(--accent-infra) / <alpha-value>)",
      },
      borderRadius: { sm: "8px", md: "12px", lg: "16px" },
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Text",
          "Inter",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          "ui-monospace",
          "SFMono-Regular",
          "SF Mono",
          "Menlo",
          "monospace",
        ],
      },
    },
  },
};
