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
    <div className="border-b border-border bg-muted/20 px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          {forceActive ? (
            <p className="truncate text-sm font-semibold text-foreground">Forcing fastboot…</p>
          ) : (
            <p className="truncate text-sm font-semibold text-foreground">
              Writing <span className="font-mono text-trace-copper">'{flash.partition || "…"}'</span>
              {flash.total > 0 && (
                <span className="ml-1.5 text-xs font-normal text-muted-foreground">
                  ({formatBytes(flash.total)})
                </span>
              )}
            </p>
          )}
        </div>
        <span className="text-xl text-trace-copper animate-pulse">
          {SPINNER_FRAMES[spinnerIndex]}
        </span>
      </div>

      {flashActive && flash.total > 0 && (
        <div className="mt-3">
          <Progress
            value={overallPct}
            indicatorClassName="progress-gradient"
            className="gap-0"
          />
          <div className="mt-2 flex items-center justify-between gap-3 text-xs">
            <span className="text-muted-foreground tabular-nums">
              {formatBytes(flash.bytes)} / {formatBytes(flash.total)}
            </span>
            <span className="text-muted-foreground tabular-nums">
              {formatSpeed(flash.speedBps)}
            </span>
          </div>
        </div>
      )}

      {forceActive && (
        <p className="mt-2 text-xs text-muted-foreground">{force.message || "Waiting for preloader…"}</p>
      )}

      <div className="mt-2 flex items-center gap-1.5 text-xs">
        <span className="inline-block size-1.5 rounded-full bg-success animate-pulse" />
        <span className="text-muted-foreground font-medium">Elapsed:</span>
        <span className="font-mono text-foreground font-semibold">{elapsed}</span>
        {flashActive && <LoaderCircle className="ml-auto h-3.5 w-3.5 animate-spin text-trace-copper" />}
      </div>
    </div>
  );
});
