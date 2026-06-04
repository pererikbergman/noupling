import { useState } from "react";
import type { DataContract } from "./types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";

export interface AppProps {
  data: DataContract;
}

export function App({ data }: AppProps) {
  const [theme, setTheme] = useState<"dark" | "light">(
    (document.documentElement.getAttribute("data-theme") as "dark" | "light") ?? "dark",
  );

  function toggleTheme() {
    const next = theme === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    setTheme(next);
  }

  return (
    <div className="grid h-screen w-screen grid-cols-[380px_1fr] grid-rows-[auto_auto_1fr]">
      <TopBar data={data} theme={theme} onToggleTheme={toggleTheme} />
      <SearchRow data={data} />
      <SidePanel data={data} />
      <CanvasArea data={data} />
    </div>
  );
}
