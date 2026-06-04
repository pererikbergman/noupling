import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";
import { loadDataContract } from "./data";
import { App } from "./App";

async function bootstrap() {
  const data = await loadDataContract();
  const root = createRoot(document.getElementById("root")!);
  root.render(
    <StrictMode>
      <App data={data} />
    </StrictMode>,
  );
}

bootstrap().catch((err: unknown) => {
  document.getElementById("root")!.innerHTML =
    `<div style="padding:24px;font-family:ui-monospace,monospace;color:#ff453a">Failed to load Explorer data: ${String(err)}</div>`;
});
