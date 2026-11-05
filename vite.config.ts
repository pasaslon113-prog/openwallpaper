import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    hmr: { protocol: "ws", host: "127.0.0.1", port: 1421 },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13", rollupOptions: { input: { main: fileURLToPath(new URL("./index.html", import.meta.url)), wallpaper: fileURLToPath(new URL("./wallpaper.html", import.meta.url)) } } },
});
