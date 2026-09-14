import { memo, useEffect, useState } from "react";
import { Progress } from "@/components/ui/progress";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { formatBytes, formatSpeed } from "@/lib/format";

function useElapsed(active: boolean): string {
  const [elapsed, setElapsed] = useState("00:00");

  useEffect(() => {
    if (!active) return;
    const start = Date.now();
    const interval = setInterval(() => {
      const s = Math.floor((Date.now() - start) / 1000);
      const m = Math.floor(s / 60);
      const sec = s % 60;
      setElapsed(`${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`);
    }, 1000);
    return () => clearInterval(interval);
  }, [active]);

  return active ? elapsed : "00:00";
}

export const ProgressWidget = memo(function ProgressWidget() {
  const flash = useFlashProgress();
  const force = useForceFastboot();

  const flashActive = flash.phase === "waiting" || flash.phase === "flashing";
  const forceActive = force.phase === "waiting";
  const active = flashActive || forceActive;
  const elapsed = useElapsed(active);

  if (!active) return null;

  const overallPct =
    flash.overallTotal > 0
      ? Math.min(100, Math.round((flash.overallBytes / flash.overallTotal) * 100))
      : 0;

  const operationLabel = flash.operation === "erase" ? "Erasing" : "Flashing";

  return (
    <div className="border-b border-border/60 bg-muted/20 px-4 py-2.5">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-1.5 text-xs">
          {forceActive ? (
            <span className="font-medium text-foreground truncate">Forcing fastboot mode…</span>
          ) : (
            <>
              <span className="text-muted-foreground">{operationLabel}</span>
              <span className="font-mono font-medium text-foreground truncate">
                {flash.partition || "…"}
              </span>
              {flash.speedBps > 0 && (
                <span className="text-[11px] font-mono text-muted-foreground/70 shrink-0">
                  • {formatSpeed(flash.speedBps)}
                </span>
              )}
            </>
          )}
        </div>

        <div className="flex items-center gap-2 shrink-0 font-mono text-[11px]">
          {flashActive && flash.total > 0 && (
            <span className="font-medium text-foreground tabular-nums">{overallPct}%</span>
          )}
          <span className="text-muted-foreground/70 tabular-nums">{elapsed}</span>
        </div>
      </div>

      {flashActive && flash.total > 0 && (
        <div className="mt-2 space-y-1">
          <Progress
            value={overallPct}
            indicatorClassName="bg-trace-copper"
            className="h-1 bg-muted/60"
          />
          <div className="flex items-center justify-between font-mono text-[10px] text-muted-foreground tabular-nums">
            <span>
              {formatBytes(flash.bytes)} / {formatBytes(flash.total)}
            </span>
            {flash.overallTotal > 0 && flash.total !== flash.overallTotal && (
              <span className="text-muted-foreground/60">
                total {formatBytes(flash.overallBytes)} / {formatBytes(flash.overallTotal)}
              </span>
            )}
          </div>
        </div>
      )}

      {forceActive && force.message && (
        <p className="mt-1 text-[11px] text-muted-foreground truncate">{force.message}</p>
      )}
    </div>
  );
});
