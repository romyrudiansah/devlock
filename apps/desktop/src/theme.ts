/**
 * Tema GUI: ikut Windows (default), terang, atau gelap. Pilihan disimpan per perangkat
 * (localStorage) dan diterapkan ke CSS (`data-theme`) serta jendela Tauri, agar Mica dan title
 * bar ikut berubah.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemePref = "system" | "light" | "dark";

const KEY = "theme";
const dark = window.matchMedia("(prefers-color-scheme: dark)");
let current: ThemePref = "system";

export function loadTheme(): ThemePref {
  try {
    const saved = localStorage.getItem(KEY);
    return saved === "light" || saved === "dark" ? saved : "system";
  } catch {
    return "system";
  }
}

export function getTheme(): ThemePref {
  return current;
}

function paint() {
  const effective = current === "system" ? (dark.matches ? "dark" : "light") : current;
  document.documentElement.dataset.theme = effective;
}

export function applyTheme(pref: ThemePref) {
  current = pref;
  try {
    if (pref === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, pref);
  } catch {
    // localStorage tidak tersedia: pilihan berlaku sampai jendela ditutup.
  }
  paint();
  // Tema jendela (Mica, title bar). Saat `system`, WebView kembali mengikuti Windows dan media
  // query bisa berubah, jadi warnai ulang setelahnya.
  try {
    getCurrentWindow()
      .setTheme(pref === "system" ? null : pref)
      .catch(() => {})
      .finally(paint);
  } catch {
    // Bukan di Tauri (pratinjau browser): cukup CSS.
  }
}

dark.addEventListener("change", () => {
  if (current === "system") paint();
});
