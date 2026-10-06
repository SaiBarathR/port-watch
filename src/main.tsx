import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initTheme } from "./hooks/use-theme";
import { initSettings } from "./lib/settings-store";
import "./index.css";

initTheme();

// The first render already has the user's settings: the table is not drawn
// with the defaults and then redrawn.
void initSettings().finally(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
});
