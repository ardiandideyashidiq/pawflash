import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App.tsx";
import { ErrorBoundary } from "./components/ErrorBoundary.tsx";

function showFatal(message: string, detail?: string) {
  if (document.getElementById("fatal-error")) return;
  const el = document.createElement("pre");
  el.id = "fatal-error";
  el.style.cssText =
    "position:fixed;inset:0;z-index:99999;margin:0;padding:24px;overflow:auto;" +
    "background:#14100c;color:#e07a4c;font:12px/1.5 monospace;white-space:pre-wrap;";
  el.textContent = `pawflash fatal error\n\n${message}${detail ? `\n\n${detail}` : ""}`;
  document.body.appendChild(el);
}

window.addEventListener("error", (event) => {
  showFatal(event.message, event.error?.stack ?? "");
});
window.addEventListener("unhandledrejection", (event) => {
  showFatal(String(event.reason), event.reason?.stack ?? "");
});

const root = document.documentElement;
const media = window.matchMedia("(prefers-color-scheme: dark)");
const stored = window.localStorage.getItem("app-theme");
const initial = stored ?? (media.matches ? "dark" : "light");
root.classList.toggle("dark", initial === "dark");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
);
