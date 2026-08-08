import { memo } from "react";
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { formatBytes, formatSpeed } from "@/lib/format";
import { useFlashProgress, type FlashPhase } from "@/hooks/useFlashProgress";
import {
  createDismissibleDialogRootHandler,
  type DialogChangeReason,
} from "@/components/shared/dialogBehavior";

interface FlashDialogProps {
  open: boolean;
  onOpenChange: (open: boolean, reason?: DialogChangeReason) => void;
  onCancel: () => void | Promise<void>;
  canCancel: boolean;
}

export const FlashDialog = memo(function FlashDialog({
  open,
  onOpenChange,
  onCancel,
  canCancel,
}: FlashDialogProps) {
  const {
    phase,
    partition,
    bytes,
    total,
    speedBps,
    overallBytes,
    overallTotal,
    summary,
    errorMessage,
    statusText,
  } = useFlashProgress();

  const imagePct = total > 0 ? Math.round((bytes / total) * 100) : 0;
  const overallPct = overallTotal > 0 ? Math.min(100, Math.round((overallBytes / overallTotal) * 100)) : 0;
  const tone = phaseTone(phase);
  const isFinished = phase === "complete" || phase === "cancelled" || phase === "error";

  return (
    <DialogPrimitive.Root
      open={open}
      onOpenChange={createDismissibleDialogRootHandler(onOpenChange)}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Backdrop className="fixed inset-0 z-50 bg-stone-950/18 backdrop-blur-sm transition-opacity duration-150 data-closed:opacity-0 data-open:opacity-100" />
        <DialogPrimitive.Popup
          data-slot="flash-dialog"
          className="fixed top-1/2 left-1/2 z-50 flex w-[min(38rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-md border border-border bg-background shadow-[var(--overlay-shadow)] pointer-events-auto outline-none transition-[height,opacity,transform] duration-200 ease-out data-closed:scale-[0.99] data-closed:opacity-0 data-open:scale-100 data-open:opacity-100"
        >
          <div className="grid gap-3 border-b border-border px-4 py-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="min-w-0">
              <DialogPrimitive.Title className={cn("truncate text-base font-semibold", tone.title)}>
                {compactTitle(phase)}
              </DialogPrimitive.Title>
            </div>
            <div className="relative z-10 flex shrink-0 flex-wrap items-center justify-end gap-2">
              {canCancel && (
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full rounded-sm whitespace-nowrap sm:w-auto"
                  onClick={onCancel}
                >
                  <X className="h-3.5 w-3.5" />
                  Cancel
                </Button>
              )}
              {!canCancel && isFinished && (
                <Button
                  variant="outline"
                  size="sm"
                  className="w-full rounded-sm whitespace-nowrap sm:w-auto"
                  onClick={() => onOpenChange(false)}
                >
                  <X className="h-4 w-4" />
                  Close
                </Button>
              )}
            </div>
          </div>

          <div className="space-y-3 px-4 py-3">
            {errorMessage && (phase === "error" || phase === "cancelled") && (
              <p
                className={cn(
                  "rounded-sm border px-3 py-2 text-xs break-words leading-5",
                  phase === "cancelled"
                    ? "border-warning/20 bg-warning/8 text-warning"
                    : "border-error/20 bg-error/8 text-error",
                )}
              >
                {errorMessage}
              </p>
            )}

            {statusText && !errorMessage && (
              <p className="text-center text-xs font-medium uppercase tracking-[0.12em] text-muted-foreground">
                {statusText}
              </p>
            )}

            <div className="grid gap-3 sm:grid-cols-2">
              <section className="status-shell space-y-3 px-3 py-3">
                <ProgressBlock
                  label={currentProgressLabel(phase)}
                  value={phase === "complete" ? 100 : phase === "waiting" ? 0 : imagePct}
                  toneClass={tone.bar}
                  caption={currentProgressCaption(phase, partition, statusText)}
                  amount={phase === "complete" ? "Finished" : phase === "waiting" ? "" : formatBytesProgress(bytes, total)}
                />
                <div className="flex items-center justify-between border-t border-border/50 pt-2 text-xs">
                  <span className="text-muted-foreground">Speed</span>
                  <span className="font-semibold tabular-nums">
                    {phase === "flashing" && speedBps > 0 ? formatSpeed(speedBps) : "—"}
                  </span>
                </div>
              </section>

              <section className="status-shell space-y-3 px-3 py-3">
                <ProgressBlock
                  label="Overall progress"
                  value={overallPct}
                  toneClass={tone.bar}
                  caption={overallCaption(phase, overallBytes, overallTotal)}
                  amount={overallTotal > 0 ? formatBytesProgress(overallBytes, overallTotal) : ""}
                />
                {summary ? (
                  <div className="flex items-center justify-around border-t border-border/50 pt-2 text-xs">
                    <div>
                      <span className="text-muted-foreground">Success: </span>
                      <span className="font-semibold text-success">{summary.flashed}</span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Skip: </span>
                      <span className="font-semibold text-warning">{summary.skipped}</span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Fail: </span>
                      <span className="font-semibold text-error">{summary.failed}</span>
                    </div>
                  </div>
                ) : (
                  <div className="border-t border-border/50 pt-2 text-xs text-transparent select-none">
                    —
                  </div>
                )}
              </section>
            </div>
          </div>
        </DialogPrimitive.Popup>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
});

function ProgressBlock({
  label,
  value,
  toneClass,
  caption,
  amount,
}: {
  label: string;
  value: number;
  toneClass: string;
  caption: string;
  amount: string;
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between gap-3 text-xs font-medium uppercase tracking-[0.12em] text-muted-foreground">
        <span className="min-w-0 truncate">{label}</span>
        <span className="tabular-nums">{value}%</span>
      </div>
      <Progress value={value} indicatorClassName={toneClass} className="gap-0" />
      {(caption || amount) && (
        <div className="flex min-w-0 items-center justify-between gap-3 text-sm">
          <span className="min-w-0 flex-1 truncate text-muted-foreground">{caption}</span>
          <span className="shrink-0 tabular-nums text-muted-foreground">{amount}</span>
        </div>
      )}
    </div>
  );
}



function phaseTone(phase: FlashPhase) {
  switch (phase) {
    case "error":
      return { title: "text-error", bar: "bg-error" };
    case "cancelled":
      return { title: "text-warning", bar: "bg-warning" };
    case "complete":
      return { title: "text-success", bar: "bg-success" };
    case "waiting":
      return { title: "text-foreground", bar: "bg-info" };
    default:
      return { title: "text-foreground", bar: "progress-gradient" };
  }
}

function compactTitle(phase: FlashPhase) {
  switch (phase) {
    case "waiting":
      return "Waiting for device...";
    case "flashing":
      return "Flash progress";
    case "complete":
      return "Flash completed";
    case "cancelled":
      return "Cancelled";
    case "error":
      return "Flash failed";
    default:
      return "Preparing...";
  }
}

function currentProgressLabel(phase: FlashPhase) {
  if (phase === "waiting") return "Current step";
  return "Current partition";
}

function currentProgressCaption(
  phase: FlashPhase,
  partition: string,
  statusText = "",
) {
  if (phase === "waiting") return statusText || "No device connected";
  if (partition) return partition;
  return "Preparing partition";
}

function overallCaption(
  phase: FlashPhase,
  overallBytes: number,
  overallTotal: number,
) {
  if (phase === "waiting") return "Waiting for device";
  if (phase === "complete") return "";
  if (phase === "cancelled") return "Stopped before finishing all actions";
  if (phase === "error") return "Stopped due to an error";
  if (overallTotal <= 0 && overallBytes <= 0) return "Preparing progress";
  return "Cumulative transfer";
}

function formatBytesProgress(bytes: number, total: number) {
  if (total <= 0) return "";
  return `${formatBytes(bytes)} / ${formatBytes(total)}`;
}
