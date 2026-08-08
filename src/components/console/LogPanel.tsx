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

interface LevelStyle {
  text: string;
  bg: string;
  border: string;
}

function levelStyle(level: ConsoleLevel): LevelStyle {
  switch (level) {
    case "success":
      return {
        text: "text-signal-green font-semibold",
        bg: "bg-success/15",
        border: "border-success/30",
      };
    case "error":
      return {
        text: "text-signal-red font-semibold",
        bg: "bg-error/15",
        border: "border-error/30",
      };
    case "warning":
      return {
        text: "text-signal-amber font-semibold",
        bg: "bg-warning/15",
        border: "border-warning/30",
      };
    case "command":
      return {
        text: "text-trace-copper font-semibold",
        bg: "bg-trace-copper/15",
        border: "border-trace-copper/30",
      };
    case "response":
      return {
        text: "text-foreground/90 font-medium",
        bg: "bg-muted/60",
        border: "border-border/50",
      };
    default:
      return {
        text: "text-muted-foreground font-medium",
        bg: "bg-muted/40",
        border: "border-border/40",
      };
  }
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
      .map((e) => `[${e.time}] [${e.level.toUpperCase()}] ${e.text}`)
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
      panelRef.current?.style.setProperty("--panel-width", `${clamped}px`);
    };

    const handlePointerMove = (e: PointerEvent) => {
      dragXRef.current = e.clientX;
      if (resizeRafRef.current != null) return;
      resizeRafRef.current = requestAnimationFrame(() => {
        resizeRafRef.current = null;
        applyWidth(dragXRef.current);
      });
    };

    const endResize = () => {
      panelWidthRef.current = clampWidth(dragXRef.current);
      setLogPanelWidth(panelWidthRef.current);
      setIsDragging(false);
    };

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
          "fixed inset-0 z-40 cursor-default bg-stone-950/25 backdrop-blur-sm transition-opacity duration-300",
          shown ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onClick={closeLogPanel}
      />

      <div
        ref={panelRef}
        className={cn(
          "fixed top-0 right-0 z-50 flex h-full flex-col border-l border-border bg-card/95 backdrop-blur-md shadow-2xl",
          shown ? "translate-x-0 opacity-100" : "pointer-events-none translate-x-full opacity-0",
          isDragging ? "transition-none select-none" : "transition-all duration-300 ease-out",
        )}
        style={{ width: `var(--panel-width, ${logPanelWidth}px)` }}
        role="dialog"
        aria-label="Operation logs"
      >
        <button
          type="button"
          aria-label="Resize logs panel"
          className="absolute top-1/2 left-0 flex h-16 w-3 -translate-y-1/2 touch-none cursor-col-resize items-center justify-center rounded-r-md bg-muted/80 transition-colors hover:bg-trace-copper group"
          onPointerDown={(e) => {
            e.preventDefault();
            dragXRef.current = e.clientX;
            e.currentTarget.setPointerCapture(e.pointerId);
            setIsDragging(true);
          }}
          title="Drag to resize"
        >
          <span className="h-8 w-0.5 rounded-full bg-muted-foreground/60 group-hover:bg-white transition-colors" />
        </button>

        <div className="ml-2 flex items-center justify-between border-b border-border/80 p-3.5">
          <div className="flex items-center gap-2.5">
            <h2 className="text-sm font-semibold text-foreground tracking-tight">Operation Logs</h2>
            {isLive && (
              <span className="flex items-center gap-1.5 rounded-full border border-success/40 bg-success/15 px-2.5 py-0.5">
                <span className="relative flex h-2 w-2">
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success opacity-75" />
                  <span className="relative inline-flex h-2 w-2 rounded-full bg-success" />
                </span>
                <span className="text-[11px] font-bold text-signal-green tracking-wide uppercase">Live</span>
              </span>
            )}
          </div>
          <div className="flex items-center gap-1">
            {entries.length > 0 && (
              <button
                onClick={clearConsole}
                className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent-soft hover:text-foreground"
                title="Clear logs"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            )}
            <button
              onClick={() => void copyLogs()}
              disabled={entries.length === 0}
              className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent-soft hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
              title="Copy logs"
            >
              <Copy className="h-4 w-4" />
            </button>
            <button
              onClick={closeLogPanel}
              className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent-soft hover:text-foreground"
              title="Close"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </div>

        <ProgressWidget />

        <div
          ref={logsContainerRef}
          className="ml-2 flex-1 space-y-1 overflow-y-auto p-3.5 font-mono text-xs"
        >
          {entries.length === 0 ? (
            <div className="flex h-64 flex-col items-center justify-center gap-2 text-center text-muted-foreground select-none">
              <Terminal className="h-8 w-8 text-muted-foreground/30" />
              <p className="text-xs font-medium">No operation logs recorded yet</p>
              <p className="text-[11px] text-muted-foreground/60">Flashing and device logs will stream here</p>
            </div>
          ) : (
            entries.map((entry) => {
              const style = levelStyle(entry.level);
              return (
                <div
                  key={entry.id}
                  className="group flex items-start gap-2.5 rounded px-2 py-1 transition-colors hover:bg-accent-soft/40 leading-relaxed"
                >
                  <span className="shrink-0 font-mono text-[11px] text-muted-foreground tabular-nums select-none pt-0.5">
                    {entry.time}
                  </span>
                  <span
                    className={cn(
                      "inline-flex shrink-0 items-center rounded border px-1.5 py-0.2 text-[10px] tracking-wider select-none uppercase",
                      style.text,
                      style.bg,
                      style.border,
                    )}
                  >
                    {entry.level}
                  </span>
                  <span className="min-w-0 flex-1 break-all text-foreground/95 font-mono text-xs leading-relaxed">
                    {entry.text}
                  </span>
                </div>
              );
            })
          )}
          <div ref={logsEndRef} />
        </div>
      </div>
    </>
  );
}

