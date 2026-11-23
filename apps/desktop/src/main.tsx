import "@openscale/ui/theme.css";
import { initOpenScaleI18n } from "@openscale/i18n";
import { OpenScaleProvider, ToastProvider } from "@openscale/ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App.js";
import { TauriIpcTransport } from "./tauri-transport.js";

const savedLang = window.localStorage.getItem("openscale.lang");
const initialLang = savedLang === "en" || savedLang === "es" ? savedLang : "es";
initOpenScaleI18n(initialLang);

const savedTheme = window.localStorage.getItem("openscale.theme");
if (savedTheme === "dark") {
  document.documentElement.classList.add("dark");
}

const transport = new TauriIpcTransport();
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <OpenScaleProvider transport={transport}>
        <ToastProvider>
          <App />
        </ToastProvider>
      </OpenScaleProvider>
    </StrictMode>,
  );
}
