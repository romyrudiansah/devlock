import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

// `npm run dev` di browser biasa: pakai data contoh karena backend Tauri tidak ada.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  (await import("./devMock")).install();
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
