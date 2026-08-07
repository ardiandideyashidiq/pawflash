import { memo, useCallback, useEffect, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  CheckCircle2,
  Construction,
  Download,
  FolderOpen,
  HardDrive,
  Loader2,
  RefreshCw,
  Stethoscope,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useConsole } from "@/hooks/useConsole";
import { useSimulation } from "@/hooks/useSimulation";
import { errorMessage } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface MtkStatusPayload {
  version: string | null;
  path: string | null;
  installed: boolean;
  device_visible: boolean;
  platform: string;
}

const PARTTYPES = [
  { id: "user", label: "user", desc: "Main UFS/eMMC Flash storage" },
  { id: "boot1", label: "boot1", desc: "Hardware Boot Partition 1" },
  { id: "boot2", label: "boot2", desc: "Hardware Boot Partition 2" },
  { id: "rpmb", label: "rpmb", desc: "Replay Protected Memory Block" },
] as const;

const PARTITION_PRESETS = [
  "boot",
  "init_boot",
  "vbmeta",
  "vendor_boot",
  "recovery",
  "super",
  "userdata",
  "preloader",
];

export default memo(function MtkTab() {
  const { addEntry, addProgressEvent } = useConsole();
  const { simulate } = useSimulation();
  const [status, setStatus] = useState<MtkStatusPayload | null>(null);
  const [busy, setBusy] = useState(false);
  const [opBytes, setOpBytes] = useState<{ bytes: number; total: number } | null>(null);

  // Form State
  const [partitionName, setPartitionName] = useState("");
  const [filePath, setFilePath] = useState("");
  const [selectedParttype, setSelectedParttype] = useState<string>("user");

  // Download Dialog State
  const [downloadModalOpen, setDownloadModalOpen] = useState(false);
  const [downloadPhase, setDownloadPhase] = useState<string>("Preparing download…");
  const [downloadBytes, setDownloadBytes] = useState<{ bytes: number; total: number } | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);

  // Erase Confirmation State
  const [eraseConfirmOpen, setEraseConfirmOpen] = useState(false);

  const refreshStatus = useCallback(async () => {
    try {
      const info = await invoke<MtkStatusPayload>("mtk_status", { simulate });
      setStatus(info);
    } catch (error) {
      toast.error(errorMessage(error));
    }
  }, [simulate]);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const runChannelOp = useCallback(
    async (
      command: string,
      args: Record<string, unknown>,
      onProgress?: (bytes: number, total: number) => void,
      onPhase?: (phase: string, message: string) => void,
    ): Promise<unknown> => {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = (event) => {
        addProgressEvent(event);
        if (event.event === "MtkPhase" && onPhase) {
          onPhase(event.data.phase, event.data.message);
        }
        if (event.event === "MtkProgress") {
          const next = { bytes: event.data.bytes, total: event.data.total };
          if (onProgress) {
            onProgress(next.bytes, next.total);
          } else {
            setOpBytes(next);
          }
        }
        if (event.event === "MtkDone" || event.event === "Error") {
          setBusy(false);
        }
      };
      setBusy(true);
      setOpBytes(null);
      try {
        const result = await invoke(command, { ...args, onEvent: channel, simulate });
        return result;
      } catch (error) {
        toast.error(errorMessage(error));
        return null;
      } finally {
        setBusy(false);
      }
    },
    [addProgressEvent, simulate],
  );

  const download = useCallback(async () => {
    setDownloadModalOpen(true);
    setIsDownloading(true);
    setDownloadPhase("Initializing download...");
    setDownloadBytes(null);

    const ok = await runChannelOp(
      "mtk_download",
      {},
      (bytes, total) => setDownloadBytes({ bytes, total }),
      (_phase, message) => setDownloadPhase(message),
    );

    setIsDownloading(false);
    if (ok !== null) {
      addEntry({ text: "mtk bridge downloaded successfully", level: "success" });
      toast.success("MTK Bridge downloaded");
      void refreshStatus();
      setTimeout(() => {
        setDownloadModalOpen(false);
      }, 1200);
    }
  }, [runChannelOp, refreshStatus, addEntry]);

  const doctor = useCallback(async () => {
    await runChannelOp("mtk_doctor", {});
  }, [runChannelOp]);

  const remove = useCallback(async () => {
    await runChannelOp("mtk_remove", {});
    void refreshStatus();
    toast.info("MTK Bridge uninstalled");
  }, [runChannelOp, refreshStatus]);

  const handleBrowseFile = useCallback(async () => {
    try {
      // Dynamic import to support desktop environment dialog safely
      const dialog = await import("@tauri-apps/plugin-dialog");
      const selected = await dialog.open({
        multiple: false,
        filters: [{ name: "Image Files", extensions: ["img", "bin", "iso"] }],
      });
      if (selected && typeof selected === "string") {
        setFilePath(selected);
      }
    } catch {
      // Fallback for missing dialog plugin context
      const fileInput = document.createElement("input");
      fileInput.type = "file";
      fileInput.onchange = (e) => {
        const target = e.target as HTMLInputElement;
        if (target.files && target.files[0]) {
          setFilePath(target.files[0].name);
        }
      };
      fileInput.click();
    }
  }, []);

  const op = useCallback(
    async (
      command: "mtk_read" | "mtk_write" | "mtk_erase",
      partition: string,
      file: string,
      parttype: string,
    ) => {
      const args: Record<string, unknown> = { partition, parttype };
      if (command !== "mtk_erase") args.file = file;
      const result = await runChannelOp(command, args);
      if (result !== null) {
        addEntry({ text: `${command} complete for ${partition}`, level: "success" });
        toast.success(`Operation ${command.replace("mtk_", "")} completed successfully`);
      }
    },
    [runChannelOp, addEntry],
  );

  const executeOp = useCallback(
    (action: "read" | "write" | "erase") => {
      if (!partitionName.trim()) {
        toast.error("Partition name is required");
        return;
      }
      if (action !== "erase" && !filePath.trim()) {
        toast.error("File path is required");
        return;
      }
      if (action === "erase") {
        setEraseConfirmOpen(true);
        return;
      }
      const command = action === "read" ? "mtk_read" : "mtk_write";
      void op(command, partitionName.trim(), filePath.trim(), selectedParttype);
    },
    [partitionName, filePath, selectedParttype, op],
  );

  const confirmErase = useCallback(() => {
    setEraseConfirmOpen(false);
    void op("mtk_erase", partitionName.trim(), "", selectedParttype);
  }, [op, partitionName, selectedParttype]);

  const isBusy = busy || isDownloading;
  const downloadPercent =
    downloadBytes && downloadBytes.total > 0
      ? Math.round((downloadBytes.bytes / downloadBytes.total) * 100)
      : null;

  const opPercent =
    opBytes && opBytes.total > 0 ? Math.round((opBytes.bytes / opBytes.total) * 100) : null;

  return (
    <div className="relative min-h-full">
      {/* Blurred & Disabled Background Content */}
      <div className="flex min-h-full flex-col gap-5 pointer-events-none select-none blur-[3px] opacity-60 lg:grid lg:grid-cols-2 lg:gap-6">
        {/* LEFT COLUMN: Status & Bridge Controls */}
        <div className="flex flex-col gap-4">
          <SectionCard
            title="MTK Bridge Status"
            description="DA-mode partition operations via the frozen mtkclient bridge."
            headerActions={
              <Button
                variant="ghost"
                size="icon"
                title="Refresh status"
                disabled={isBusy}
                onClick={() => void refreshStatus()}
              >
                <RefreshCw className={`h-4 w-4 ${isBusy ? "animate-spin" : ""}`} />
              </Button>
            }
            contentClassName="space-y-4"
          >
            {/* Status Indicators Grid */}
            <div className="grid grid-cols-2 gap-3 rounded-lg border border-border/50 bg-background/40 p-3.5 text-xs">
              <div className="flex flex-col gap-1">
                <span className="text-muted-foreground font-medium">Bridge Installation</span>
                <div className="flex items-center gap-2 font-mono">
                  <span
                    className={`inline-block h-2 w-2 rounded-full ${
                      status?.installed ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]" : "bg-amber-500"
                    }`}
                  />
                  <span className="font-semibold">
                    {status?.installed ? status.version : "Not Installed"}
                  </span>
                </div>
              </div>

              <div className="flex flex-col gap-1">
                <span className="text-muted-foreground font-medium">DA Device</span>
                <div className="flex items-center gap-2">
                  <span
                    className={`inline-block h-2 w-2 rounded-full ${
                      status?.device_visible
                        ? "bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.5)]"
                        : "bg-muted-foreground/40"
                    }`}
                  />
                  <span className="font-semibold">
                    {status?.device_visible ? "Connected (DA)" : "Not Detected"}
                  </span>
                </div>
              </div>

              <div className="flex flex-col gap-1">
                <span className="text-muted-foreground font-medium">Host Platform</span>
                <span className="font-mono text-foreground/90">{status?.platform ?? "…"}</span>
              </div>

              {status?.path && (
                <div className="col-span-2 flex flex-col gap-1 border-t border-border/30 pt-2">
                  <span className="text-muted-foreground font-medium">Bridge Executable Path</span>
                  <span className="truncate font-mono text-[11px] text-muted-foreground" title={status.path}>
                    {status.path}
                  </span>
                </div>
              )}
            </div>

            {/* Action Bar */}
            <div className="flex flex-wrap items-center gap-2 pt-1">
              <Button
                className="gap-2"
                disabled={isBusy || status?.installed}
                onClick={() => void download()}
              >
                <Download className="h-4 w-4" />
                {status?.installed ? "Installed" : "Download Bridge"}
              </Button>
              <Button variant="outline" className="gap-2" disabled={isBusy} onClick={() => void doctor()}>
                <Stethoscope className="h-4 w-4" />
                Doctor
              </Button>
              <Button
                variant="outline"
                className="gap-2 text-destructive hover:text-destructive"
                disabled={isBusy || !status?.installed}
                onClick={() => void remove()}
              >
                <Trash2 className="h-4 w-4" />
                Remove
              </Button>
            </div>

            {simulate && (
              <div className="flex items-center gap-2 rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-500">
                <AlertTriangle className="h-4 w-4 shrink-0" />
                <span>SIMULATED MODE — real device I/O is bypassed.</span>
              </div>
            )}
          </SectionCard>
        </div>

        {/* RIGHT COLUMN: DA Operations */}
        <div className="flex flex-col gap-4">
          <SectionCard
            title="Direct DA Partition Operations"
            description="Read, write, or erase specific partition blocks via Download Agent."
            contentClassName="space-y-4"
          >
            {/* Target Partition */}
            <div className="space-y-2">
              <label htmlFor="mtk-partition-input" className="text-xs font-medium text-muted-foreground">
                Target Partition Name
              </label>
              <Input
                id="mtk-partition-input"
                value={partitionName}
                onChange={(e) => setPartitionName(e.target.value)}
                placeholder="e.g. boot, vbmeta, recovery"
                aria-label="MTK partition"
                disabled={isBusy}
              />

              {/* Quick Presets */}
              <div className="flex flex-wrap gap-1.5 pt-1">
                {PARTITION_PRESETS.map((preset) => (
                  <button
                    key={preset}
                    type="button"
                    disabled={isBusy}
                    onClick={() => setPartitionName(preset)}
                    className={`rounded-md border px-2 py-0.5 text-[11px] font-mono transition-colors ${
                      partitionName === preset
                        ? "border-primary bg-primary/10 text-primary font-semibold"
                        : "border-border/60 bg-muted/30 text-muted-foreground hover:bg-muted hover:text-foreground"
                    }`}
                  >
                    {preset}
                  </button>
                ))}
              </div>
            </div>

            {/* File Path Selection */}
            <div className="space-y-2">
              <label htmlFor="mtk-file-input" className="text-xs font-medium text-muted-foreground">
                Image File Path (Read output / Write input)
              </label>
              <div className="flex gap-2">
                <Input
                  id="mtk-file-input"
                  value={filePath}
                  onChange={(e) => setFilePath(e.target.value)}
                  placeholder="/path/to/image.img"
                  aria-label="MTK file path"
                  disabled={isBusy}
                  className="font-mono text-xs"
                />
                <Button
                  variant="outline"
                  size="icon"
                  disabled={isBusy}
                  onClick={() => void handleBrowseFile()}
                  title="Browse Image File"
                >
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
            </div>

            {/* Hardware Target Partition Type */}
            <div className="space-y-2">
              <label htmlFor="mtk-parttype-select" className="text-xs font-medium text-muted-foreground">
                Hardware Partition Target (`parttype`)
              </label>
              <select
                id="mtk-parttype-select"
                value={selectedParttype}
                onChange={(e) => setSelectedParttype(e.target.value)}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                aria-label="MTK parttype"
                disabled={isBusy}
              >
                {PARTTYPES.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label} — {p.desc}
                  </option>
                ))}
              </select>
            </div>

            {/* Operation Progress Indicator */}
            {opPercent !== null && (
              <div className="space-y-1.5 rounded-lg border border-primary/20 bg-primary/5 p-3">
                <div className="flex items-center justify-between text-xs font-medium">
                  <span className="flex items-center gap-2 text-primary">
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    Executing Operation…
                  </span>
                  <span className="font-mono">{opPercent}%</span>
                </div>
                <Progress value={opPercent} className="h-2" />
              </div>
            )}

            {/* Action Trigger Buttons */}
            <div className="grid grid-cols-3 gap-2.5 pt-2">
              <Button
                className="gap-2"
                disabled={isBusy || !partitionName || !filePath}
                onClick={() => executeOp("read")}
              >
                <HardDrive className="h-4 w-4" />
                Read
              </Button>
              <Button
                className="gap-2"
                disabled={isBusy || !partitionName || !filePath}
                onClick={() => executeOp("write")}
              >
                <HardDrive className="h-4 w-4" />
                Write
              </Button>
              <Button
                variant="outline"
                className="gap-2 border-destructive/40 text-destructive hover:bg-destructive/10 hover:text-destructive"
                disabled={isBusy || !partitionName}
                onClick={() => executeOp("erase")}
              >
                <Trash2 className="h-4 w-4" />
                Erase
              </Button>
            </div>
          </SectionCard>
        </div>
      </div>

      {/* Work In Progress Overlay */}
      <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 text-center">
        <div className="flex flex-col items-center gap-3 rounded-lg border border-primary/30 bg-background/80 px-8 py-7 shadow-2xl backdrop-blur-md">
          <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
            <Construction className="h-6 w-6" />
          </div>
          <div className="space-y-1">
            <h3 className="text-lg font-bold tracking-tight text-foreground">
              Work In Progress
            </h3>
            <p className="max-w-xs text-xs text-muted-foreground">
              The MTK Client bridge tab is currently under active development.
            </p>
          </div>
        </div>
      </div>

      {/* DOWNLOAD PROGRESS MODAL */}
      <Dialog open={downloadModalOpen} onOpenChange={isDownloading ? undefined : setDownloadModalOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-lg">
              {isDownloading ? (
                <Loader2 className="h-5 w-5 animate-spin text-primary" />
              ) : (
                <CheckCircle2 className="h-5 w-5 text-emerald-500" />
              )}
              {isDownloading ? "Downloading MTK Bridge" : "Download Complete"}
            </DialogTitle>
            <DialogDescription className="text-xs">
              Fetching official frozen bridge binary release for MediaTek DA-mode operations.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-2">
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-xs">
                <span className="font-medium text-foreground">{downloadPhase}</span>
                {downloadPercent !== null && (
                  <span className="font-mono text-muted-foreground">{downloadPercent}%</span>
                )}
              </div>
              <Progress
                value={downloadPercent ?? 0}
                className="h-2.5"
              />
            </div>

            {downloadBytes && downloadBytes.total > 0 && (
              <div className="flex justify-between font-mono text-[11px] text-muted-foreground">
                <span>Downloaded: {(downloadBytes.bytes / (1024 * 1024)).toFixed(1)} MiB</span>
                <span>Total: {(downloadBytes.total / (1024 * 1024)).toFixed(1)} MiB</span>
              </div>
            )}
          </div>

          <DialogFooter className="sm:justify-end">
            <Button
              variant="outline"
              disabled={isDownloading}
              onClick={() => setDownloadModalOpen(false)}
            >
              {isDownloading ? "Downloading..." : "Close"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ERASE CONFIRMATION DIALOG */}
      <Dialog open={eraseConfirmOpen} onOpenChange={setEraseConfirmOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <AlertTriangle className="h-5 w-5" />
              Confirm Partition Erase
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground">
              Are you sure you want to permanently erase the partition{" "}
              <strong className="font-mono text-foreground">{partitionName}</strong> on target{" "}
              <strong className="font-mono text-foreground">{selectedParttype}</strong>? This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2 sm:justify-end">
            <Button variant="ghost" onClick={() => setEraseConfirmOpen(false)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={confirmErase}>
              Erase Partition
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
});

