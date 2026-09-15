import { memo } from "react";
import {
  CheckCircle2,
  Cpu,
  Download,
  Loader2,
  Shield,
  XCircle,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import type { PenumbraDaEntry } from "@/types/api";

export interface DaDownloadModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  entry: PenumbraDaEntry | null;
  deviceName: string | null;
  status: "idle" | "downloading" | "success" | "error";
  progressBytes: number;
  totalBytes: number;
  phaseMessage: string;
  errorMessage?: string;
  onDone: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export const DaDownloadModal = memo(function DaDownloadModal({
  open,
  onOpenChange,
  entry,
  deviceName,
  status,
  progressBytes,
  totalBytes,
  phaseMessage,
  errorMessage,
  onDone,
}: DaDownloadModalProps) {
  const percent = totalBytes > 0 ? Math.min(100, Math.round((progressBytes / totalBytes) * 100)) : 0;
  const hasAuth = Boolean(entry?.auth || entry?.auth_url);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={status === "success" || status === "error"}
        className="sm:max-w-md"
      >
        <DialogHeader>
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-trace-copper/10 text-trace-copper border border-trace-copper/30">
              {status === "downloading" && <Loader2 className="h-5 w-5 animate-spin" />}
              {status === "success" && <CheckCircle2 className="h-5 w-5 text-signal-green" />}
              {status === "error" && <XCircle className="h-5 w-5 text-error" />}
              {status === "idle" && <Download className="h-5 w-5" />}
            </div>
            <div>
              <DialogTitle className="text-base font-semibold">
                {status === "success"
                  ? "Download Complete"
                  : status === "error"
                  ? "Download Failed"
                  : hasAuth
                  ? "Downloading DA + Auth Combo"
                  : "Downloading Working DA"}
              </DialogTitle>
              <p className="text-xs text-muted-foreground mt-0.5">
                {deviceName ?? (entry ? `${entry.brand} ${entry.chipset}` : "MediaTek Device")}
              </p>
            </div>
          </div>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* Component Badges */}
          <div className="flex flex-wrap items-center gap-2 text-xs">
            {entry && (
              <>
                <Badge variant="outline" className="border-border/80 font-mono text-[11px] gap-1">
                  <Cpu className="h-3 w-3 text-trace-copper" />
                  {entry.chipset.toUpperCase()}
                </Badge>
                <Badge variant="outline" className="border-border/80 text-[11px] uppercase">
                  {entry.brand}
                </Badge>
                {hasAuth && (
                  <Badge variant="outline" className="border-signal-amber/60 bg-signal-amber/10 text-signal-amber text-[11px] gap-1 font-mono">
                    <Shield className="h-3 w-3" />
                    + SLA/DAA Auth
                  </Badge>
                )}
              </>
            )}
          </div>

          {/* Progress or Error */}
          {status === "error" ? (
            <div className="rounded-md border border-error/30 bg-error/10 p-3 text-xs text-error">
              {errorMessage || "An unexpected error occurred during download."}
            </div>
          ) : (
            <div className="space-y-2">
              <div className="flex items-center justify-between text-xs">
                <span className="font-medium text-foreground truncate max-w-[260px]">
                  {phaseMessage || "Connecting..."}
                </span>
                <span className="font-mono text-muted-foreground tabular-nums">
                  {totalBytes > 0 ? `${percent}%` : ""}
                </span>
              </div>

              <Progress value={totalBytes > 0 ? percent : null} className="h-2" />

              {totalBytes > 0 && (
                <div className="flex items-center justify-between text-[11px] text-muted-foreground font-mono tabular-nums">
                  <span>{formatBytes(progressBytes)}</span>
                  <span>{formatBytes(totalBytes)}</span>
                </div>
              )}
            </div>
          )}
        </div>

        <DialogFooter>
          {(status === "success" || status === "error") && (
            <Button
              size="sm"
              onClick={onDone}
              className={
                status === "success"
                  ? "bg-trace-copper font-semibold text-zinc-950 hover:bg-trace-gold"
                  : ""
              }
            >
              {status === "success" ? "Done" : "Close"}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
