import { useCallback, useEffect, useRef, useState } from "react";
import { Copy, Terminal, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { useUI } from "@/hooks/useUI";
import { useConsole } from "@/hooks/useConsole";
import { useFlashPhase } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { useMountAnimation } from "@/hooks/useMountAnimation";
import { ProgressWidget } from "@/components/console/ProgressWidget";
import { cn } from "@/lib/utils";
import type { ConsoleLevel } from "@/types/progress";

const MIN_WIDTH = 300;
const MAX_WIDTH_FACTOR = 0.9;

const SCROLL_PIN_TOLERANCE = 120;

const SLIDE_DURATION_MS = 300;

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
  // Latest pointer X + pending frame id for coalescing resize updates.
  const dragXRef = useRef(0);
  const resizeRafRef = useRef<number | null>(null);
  const { mounted, shown } = useMountAnimation(logPanelOpen, SLIDE_DURATION_MS);

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

    const clampWidth = (clientX: number) => {
      const min = MIN_WIDTH;
      const max = Math.max(window.innerWidth * MAX_WIDTH_FACTOR, min);
      return Math.min(Math.max(window.innerWidth - clientX, min), max);
    };

    const applyWidth = (clientX: number) => {
      const clamped = clampWidth(clientX);
      panelWidthRef.current = clamped;
      // Live width lives in a CSS variable React does not own, so incoming
      // log entries re-rendering this component can never overwrite it.
      panelRef.current?.style.setProperty("--panel-width", `${clamped}px`);
    };

    const handlePointerMove = (e: PointerEvent) => {
      // Coalesce to one style write per animation frame. Every width change
      // forces a synchronous layout of the whole panel subtree (the log
      // list), so applying it at pointer rate (often >60Hz) causes multiple
      // reflows per frame and feels laggy.
      dragXRef.current = e.clientX;
      if (resizeRafRef.current != null) return;
      resizeRafRef.current = requestAnimationFrame(() => {
        resizeRafRef.current = null;
        applyWidth(dragXRef.current);
      });
    };

    const endResize = () => {
      const container = logsContainerRef.current;
      if (container) {
        container.style.width = "";
        container.style.overflowX = "";
      }
      panelWidthRef.current = clampWidth(dragXRef.current);
      setLogPanelWidth(panelWidthRef.current);
      setIsDragging(false);
    };

    // Freeze the log list at its starting width so text doesn't re-wrap on
    // every frame while resizing — only the panel chrome moves.
    const container = logsContainerRef.current;
    if (container) {
      container.style.width = `${container.offsetWidth}px`;
      container.style.overflowX = "hidden";
    }

    // Suppress text selection and cursor flicker while dragging.
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    document.addEventListener("pointermove", handlePointerMove);
    document.addEventListener("pointerup", endResize);
    document.addEventListener("pointercancel", endResize);
    return () => {
      document.removeEventListener("pointermove", handlePointerMove);
      document.removeEventListener("pointerup", endResize);
      document.removeEventListener("pointercancel", endResize);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      if (resizeRafRef.current != null) {
        cancelAnimationFrame(resizeRafRef.current);
        resizeRafRef.current = null;
      }
    };
  }, [isDragging, setLogPanelWidth]);

  if (!mounted) return null;

  return (
    <>
      <button
        type="button"
        aria-label="Close logs"
        className={cn(
          "fixed inset-0 z-40 cursor-default bg-black/50 backdrop-blur-md transition-opacity duration-300",
          shown ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onClick={closeLogPanel}
      />

      <div
        ref={panelRef}
        className={cn(
          "fixed top-0 right-0 z-50 flex h-full flex-col border-l border-border bg-card shadow-2xl",
          shown ? "translate-x-0 opacity-100" : "pointer-events-none translate-x-full opacity-0",
          !isDragging && "transition-all duration-300 ease-out",
          isDragging && "overflow-hidden",
        )}
        style={{ width: `var(--panel-width, ${logPanelWidth}px)` }}
        role="dialog"
        aria-label="Operation logs"
      >
        <button
          type="button"
          aria-label="Resize logs panel"
          className="absolute top-1/2 left-0 flex h-16 w-3 -translate-y-1/2 touch-none cursor-col-resize items-center justify-center rounded-r-md bg-muted transition-colors hover:bg-trace-copper"
          onPointerDown={(e) => {
            e.preventDefault();
            dragXRef.current = e.clientX;
            e.currentTarget.setPointerCapture(e.pointerId);
            setIsDragging(true);
          }}
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
