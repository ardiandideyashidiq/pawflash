import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { dirname, resolve as pathResolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": pathResolve(__dirname, "./src"),
    },
  },
  build: {
    // Split vendor libraries out of the app chunk so the webview parses less
    // per-frame and unchanged vendor code stays cache-friendly across rebuilds.
    rollupOptions: {
      output: {
        manualChunks: {
          react: ["react", "react-dom"],
          "base-ui": ["@base-ui/react"],
          lucide: ["lucide-react"],
        },
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
