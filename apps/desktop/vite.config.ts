import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Port tetap agar cocok dengan `build.devUrl` di tauri.conf.json.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
