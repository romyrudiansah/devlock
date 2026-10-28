import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { api } from "./api";
import { setLang } from "./i18n";
import "./styles.css";

// `npm run dev` di browser biasa: pakai data contoh karena backend Tauri tidak ada.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  (await import("./devMock")).install();
}

// Bahasa UI dari `[gui] language`; bila gagal dibaca, tetap Indonesia.
setLang(await api.getSettings().then((s) => s.language).catch(() => "id"));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
