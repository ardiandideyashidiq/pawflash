import { memo, useEffect, useRef, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { Progress } from "@/components/ui/progress";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { formatBytes, formatSpeed } from "@/lib/format";

const SPINNER_FRAMES = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

function useElapsed(active: boolean): string {
  const startRef = useRef<number | null>(null);
  const [elapsed, setElapsed] = useState("00:00");

  useEffect(() => {
    if (!active) {
      startRef.current = null;
      return;
    }
    if (startRef.current === null) {
      startRef.current = Date.now();
    }
    const interval = setInterval(() => {
      const start = startRef.current ?? Date.now();
      const s = Math.floor((Date.now() - start) / 1000);
      const m = Math.floor(s / 60);
      const sec = s % 60;
      setElapsed(`${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`);
    }, 1000);
    return () => clearInterval(interval);
  }, [active]);

  return elapsed;
}

export const ProgressWidget = memo(function ProgressWidget() {
  const flash = useFlashProgress();
  const force = useForceFastboot();
  const [spinnerIndex, setSpinnerIndex] = useState(0);

  const flashActive = flash.phase === "waiting" || flash.phase === "flashing";
  const forceActive = force.phase === "waiting";
  const active = flashActive || forceActive;
  const elapsed = useElapsed(active);

  useEffect(() => {
    if (!active) return;
    const interval = setInterval(() => {
      setSpinnerIndex((prev) => (prev + 1) % SPINNER_FRAMES.length);
    }, 80);
    return () => clearInterval(interval);
  }, [active]);

  if (!active) return null;

  const overallPct =
    flash.overallTotal > 0
      ? Math.min(100, Math.round((flash.overallBytes / flash.overallTotal) * 100))
      : 0;

  return (
    <div className="border-b border-border/80 bg-card/50 backdrop-blur-sm px-3.5 py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          {forceActive ? (
            <p className="truncate text-xs font-semibold text-foreground">Forcing fastboot mode…</p>
          ) : (
            <p className="truncate text-xs font-semibold text-foreground">
              Writing <span className="font-mono text-trace-copper font-bold">'{flash.partition || "…"}'</span>
              {flash.total > 0 && (
                <span className="ml-1.5 text-[11px] font-normal text-muted-foreground">
                  ({formatBytes(flash.total)})
                </span>
              )}
            </p>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {flashActive && flash.total > 0 && (
            <span className="rounded border border-trace-copper/30 bg-trace-copper/10 px-1.5 py-0.5 font-mono text-[11px] font-bold text-trace-copper">
              {overallPct}%
            </span>
          )}
          <span className="font-mono text-base text-trace-copper animate-pulse">
            {SPINNER_FRAMES[spinnerIndex]}
          </span>
        </div>
      </div>

      {flashActive && flash.total > 0 && (
        <div className="mt-2.5">
          <Progress
            value={overallPct}
            indicatorClassName="progress-gradient"
            className="h-1.5 gap-0"
          />
          <div className="mt-1.5 flex items-center justify-between gap-3 font-mono text-[11px]">
            <span className="text-muted-foreground tabular-nums">
              {formatBytes(flash.bytes)} / {formatBytes(flash.total)}
            </span>
            <span className="text-trace-copper font-semibold tabular-nums">
              {formatSpeed(flash.speedBps)}
            </span>
          </div>
        </div>
      )}

      {forceActive && (
        <p className="mt-1.5 text-[11px] text-muted-foreground">{force.message || "Waiting for preloader…"}</p>
      )}

      <div className="mt-2 flex items-center gap-1.5 text-[11px]">
        <span className="relative flex h-2 w-2">
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success opacity-75" />
          <span className="relative inline-flex h-2 w-2 rounded-full bg-success" />
        </span>
        <span className="text-muted-foreground font-medium">Elapsed:</span>
        <span className="font-mono text-foreground font-bold">{elapsed}</span>
        {flashActive && <LoaderCircle className="ml-auto h-3.5 w-3.5 animate-spin text-trace-copper" />}
      </div>
    </div>
  );
});
