import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { initTheme } from "./hooks/use-theme";
import "./index.css";

initTheme();

const PopoverApp = lazy(() =>
  import("./PopoverApp").then((module) => ({ default: module.PopoverApp })),
);

const label = getCurrentWindow().label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {label === "popover" ? (
      <Suspense fallback={null}>
        <PopoverApp />
      </Suspense>
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
