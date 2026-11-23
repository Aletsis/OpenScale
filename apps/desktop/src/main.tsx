import "@openscale/ui/theme.css";
import { initOpenScaleI18n } from "@openscale/i18n";
import { OpenScaleProvider } from "@openscale/ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App.js";
import { TauriIpcTransport } from "./tauri-transport.js";

initOpenScaleI18n("es");

const transport = new TauriIpcTransport();
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <OpenScaleProvider transport={transport}>
        <App />
      </OpenScaleProvider>
    </StrictMode>,
  );
}
