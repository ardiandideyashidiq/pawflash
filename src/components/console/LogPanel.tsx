import { useCallback, useEffect, useRef, useState } from "react";
import { Copy, Terminal, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { useUI } from "@/hooks/useUI";
import { useConsole } from "@/hooks/useConsole";
import { useFlashPhase } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { ProgressWidget } from "@/components/console/ProgressWidget";
import type { ConsoleLevel } from "@/types/progress";

const MIN_WIDTH = 300;
const MAX_WIDTH_FACTOR = 0.9;

const SCROLL_PIN_TOLERANCE = 120;

function levelColor(level: ConsoleLevel): string {
  switch (level) {
    case "success":
      return "text-signal-green";
    case "error":
      return "text-signal-red";
    case "warning":
      return "text-signal-amber";
    case "command":
      return "text-trace-copper";
    case "response":
      return "text-foreground/70";
    default:
      return "text-muted-foreground";
  }
}

function levelTag(level: ConsoleLevel): string {
  return level.toUpperCase();
}

export function LogPanel() {
  const { logPanelOpen, closeLogPanel, logPanelWidth, setLogPanelWidth } = useUI();
  const { entries, clearConsole } = useConsole();
  const flash = useFlashPhase();
  const force = useForceFastboot();

  const logsEndRef = useRef<HTMLDivElement>(null);
  const logsContainerRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const panelWidthRef = useRef(logPanelWidth);

  const isLive =
    flash.phase === "waiting" || flash.phase === "flashing" || force.phase === "waiting";

  // Auto-scroll only while the user is already pinned near the bottom, and
  // scroll instantly — smooth scrolling on a high-frequency live log stutters.
  useEffect(() => {
    if (!logPanelOpen) return;
    const container = logsContainerRef.current;
    if (!container) return;
    const distanceFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight;
    if (distanceFromBottom > SCROLL_PIN_TOLERANCE) return;
    logsEndRef.current?.scrollIntoView({ behavior: "auto" });
  }, [entries, logPanelOpen]);

  const copyLogs = useCallback(async () => {
    if (entries.length === 0) {
      toast.error("No logs to copy");
      return;
    }
    const text = entries
      .map((e) => `[${e.time}] [${levelTag(e.level)}] ${e.text}`)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      toast.success("Logs copied to clipboard");
    } catch {
      toast.error("Copy failed");
    }
  }, [entries]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && logPanelOpen) {
        closeLogPanel();
      }
      if (
        e.ctrlKey &&
        e.key === "c" &&
        logPanelOpen &&
        !window.getSelection()?.toString()
      ) {
        e.preventDefault();
        void copyLogs();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [logPanelOpen, closeLogPanel, copyLogs]);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = window.innerWidth - e.clientX;
      const clamped = Math.min(Math.max(newWidth, MIN_WIDTH), window.innerWidth * MAX_WIDTH_FACTOR);
      if (panelRef.current) {
        panelRef.current.style.width = `${clamped}px`;
      }
      panelWidthRef.current = clamped;
    };

    const handleMouseUp = () => {
      setIsDragging(false);
      setLogPanelWidth(panelWidthRef.current);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging, setLogPanelWidth]);

  if (!logPanelOpen) return null;

  return (
    <>
      <button
        type="button"
        aria-label="Close logs"
        className="fixed inset-0 z-40 cursor-default bg-black/50 transition-opacity duration-200"
        onClick={closeLogPanel}
      />

      <div
        ref={panelRef}
        className={`fixed top-0 right-0 flex h-full flex-col border-l border-border bg-card shadow-2xl z-50 ${
          isDragging ? "" : "transition-all duration-300 ease-out"
        }`}
        style={{ width: `${logPanelWidth}px` }}
        role="dialog"
        aria-label="Operation logs"
      >
        <button
          type="button"
          aria-label="Resize logs panel"
          className="absolute top-1/2 left-0 flex h-16 w-3 -translate-y-1/2 cursor-col-resize items-center justify-center rounded-r-md bg-muted transition-colors hover:bg-trace-copper"
          onMouseDown={() => setIsDragging(true)}
          title="Drag to resize"
        >
          <span className="h-8 w-0.5 rounded-full bg-muted-foreground/60" />
        </button>

        <div className="ml-2 flex items-center justify-between border-b border-border p-4">
          <div className="flex items-center gap-2">
            <Terminal className="h-5 w-5 text-trace-copper" />
            <h2 className="text-base font-semibold text-foreground">Operation Logs</h2>
            {isLive && (
              <span className="flex items-center gap-1.5 rounded-full border border-success/40 bg-success/10 px-2 py-0.5">
                <span className="h-2 w-2 rounded-full bg-success animate-pulse" />
                <span className="text-xs font-medium text-signal-green">Live</span>
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            {entries.length > 0 && (
              <button
                onClick={clearConsole}
                className="rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                title="Clear logs"
              >
                <Trash2 className="h-5 w-5" />
              </button>
            )}
            <button
              onClick={() => void copyLogs()}
              disabled={entries.length === 0}
              className="rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
              title="Copy logs"
            >
              <Copy className="h-5 w-5" />
            </button>
            <button
              onClick={closeLogPanel}
              className="rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              title="Close"
            >
              <X className="h-5 w-5" />
            </button>
          </div>
        </div>

        <ProgressWidget />

        <div
          ref={logsContainerRef}
          className="ml-2 flex-1 space-y-1 overflow-y-auto p-4 font-mono text-xs"
        >
          {entries.length === 0 ? (
            <div className="mt-8 text-center text-muted-foreground/60">No logs yet</div>
          ) : (
            entries.map((entry) => (
              <div key={entry.id} className="flex gap-2 leading-relaxed">
                <span className="shrink-0 text-muted-foreground/50 tabular-nums">
                  {entry.time}
                </span>
                <span className={levelColor(entry.level)}>[{levelTag(entry.level)}]</span>
                <span className="min-w-0 break-all text-foreground/90">{entry.text}</span>
              </div>
            ))
          )}
          <div ref={logsEndRef} />
        </div>
      </div>
    </>
  );
}
