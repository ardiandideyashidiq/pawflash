import { memo, useCallback, useEffect, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  CheckCircle2,
  Construction,
  Download,
  FolderOpen,
  HardDrive,
  ListTree,
  Loader2,
  Power,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
  Stethoscope,
  Trash2,
  X,
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

interface PenumbraStatusPayload {
  da_version: string | null;
  da_path: string | null;
  da_installed: boolean;
  device_visible: boolean;
  platform: string;
}

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

const DEVICE_MODEL_PRESETS = [
  "POCO X7 Pro",
  "Infinix NOTE 12",
  "Redmi Note 11T",
  "Realme 8 5G",
  "Tecno Pova 5",
];

export default memo(function PenumbraTab() {
  const { addEntry, addProgressEvent } = useConsole();
  const { simulate } = useSimulation();
  const [status, setStatus] = useState<PenumbraStatusPayload | null>(null);
  const [busy, setBusy] = useState(false);
  const [opBytes, setOpBytes] = useState<{ bytes: number; total: number } | null>(null);

  // Form State
  const [partitionName, setPartitionName] = useState("");
  const [filePath, setFilePath] = useState("");
  const [deviceModel, setDeviceModel] = useState("");

  // DA Download State
  const [downloadModalOpen, setDownloadModalOpen] = useState(false);
  const [downloadPhase, setDownloadPhase] = useState<string>("Preparing download…");
  const [downloadBytes, setDownloadBytes] = useState<{ bytes: number; total: number } | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);

  // Erase Confirmation State
  const [eraseConfirmOpen, setEraseConfirmOpen] = useState(false);

  const refreshStatus = useCallback(async () => {
    try {
      const info = await invoke<PenumbraStatusPayload>("penumbra_status", { simulate });
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
        if (event.event === "PenumbraPhase" && onPhase) {
          onPhase(event.data.phase, event.data.message);
        }
        if (event.event === "PenumbraProgress") {
          const next = { bytes: event.data.bytes, total: event.data.total };
          if (onProgress) {
            onProgress(next.bytes, next.total);
          } else {
            setOpBytes(next);
          }
        }
        if (event.event === "PenumbraDone" || event.event === "Error") {
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

  const download = useCallback(
    async (targetModel?: string) => {
      const modelToUse = targetModel ?? deviceModel;
      if (!modelToUse.trim()) {
        toast.error("Enter a device model (e.g. Infinix NOTE 12)");
        return;
      }
      setDownloadModalOpen(true);
      setIsDownloading(true);
      setDownloadPhase("Fetching DA manifest...");
      setDownloadBytes(null);

      const ok = await runChannelOp(
        "penumbra_da_download",
        { device: modelToUse.trim() },
        (bytes, total) => setDownloadBytes({ bytes, total }),
        (_phase, message) => setDownloadPhase(message),
      );

      setIsDownloading(false);
      if (ok !== null) {
        addEntry({ text: `DA downloaded & assigned for ${modelToUse}`, level: "success" });
        toast.success("DA installed successfully");
        void refreshStatus();
        setTimeout(() => {
          setDownloadModalOpen(false);
        }, 1200);
      }
    },
    [deviceModel, runChannelOp, refreshStatus, addEntry],
  );

  const doctor = useCallback(async () => {
    await runChannelOp("penumbra_doctor", {});
  }, [runChannelOp]);

  const remove = useCallback(async () => {
    await runChannelOp("penumbra_da_remove", {});
    void refreshStatus();
    toast.info("DA cache removed");
  }, [runChannelOp, refreshStatus]);

  const handleBrowseFile = useCallback(async () => {
    try {
      const dialog = await import("@tauri-apps/plugin-dialog");
      const selected = await dialog.open({
        multiple: false,
        filters: [{ name: "Image Files", extensions: ["img", "bin", "iso"] }],
      });
      if (selected && typeof selected === "string") {
        setFilePath(selected);
      }
    } catch {
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
    async (command: "penumbra_read" | "penumbra_write" | "penumbra_erase", partition: string, file: string) => {
      const args: Record<string, unknown> = { partition };
      if (command !== "penumbra_erase") args.file = file;
      const result = await runChannelOp(command, args);
      if (result !== null) {
        addEntry({ text: `${command} complete for ${partition}`, level: "success" });
        toast.success(`Operation ${command.replace("penumbra_", "")} completed successfully`);
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
      const command = action === "read" ? "penumbra_read" : "penumbra_write";
      void op(command, partitionName.trim(), filePath.trim());
    },
    [partitionName, filePath, op],
  );

  const confirmErase = useCallback(() => {
    setEraseConfirmOpen(false);
    void op("penumbra_erase", partitionName.trim(), "");
  }, [op, partitionName]);

  const seccfg = useCallback(
    (unlock: boolean) => {
      void runChannelOp("penumbra_seccfg", { unlock });
    },
    [runChannelOp],
  );

  const pgpt = useCallback(async () => {
    const result = await runChannelOp("penumbra_pgpt", {});
    if (result !== null) {
      const lines = Array.isArray(result) ? (result as string[]) : [];
      addEntry({ text: `partition table (${lines.length} partitions)`, level: "info" });
    }
  }, [runChannelOp, addEntry]);

  const reboot = useCallback(() => {
    void runChannelOp("penumbra_reboot", { mode: "fastboot" });
  }, [runChannelOp]);

  const shutdown = useCallback(() => {
    void runChannelOp("penumbra_shutdown", {});
  }, [runChannelOp]);

  const isBusy = busy || isDownloading;
  const downloadPercent =
    downloadBytes && downloadBytes.total > 0
      ? Math.round((downloadBytes.bytes / downloadBytes.total) * 100)
      : null;

  const opPercent =
    opBytes && opBytes.total > 0 ? Math.round((opBytes.bytes / opBytes.total) * 100) : null;

  return (
    <div className="relative min-h-full flex flex-col gap-3">
      {/* Blurred & Disabled Background Content */}
      <div className="flex flex-col gap-3.5 pointer-events-none select-none blur-[3px] opacity-60">
        {/* COMPACT HEADER STATUS STRIP */}
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border/60 bg-card/60 px-4 py-2.5 text-xs">
          <div className="flex flex-wrap items-center gap-4">
            <div className="flex items-center gap-2 font-mono">
              <span className="text-muted-foreground">DA Selection:</span>
              <span
                className={`inline-block h-2 w-2 rounded-full ${
                  status?.da_installed ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]" : "bg-amber-500"
                }`}
              />
              <span className="font-semibold text-foreground truncate max-w-[140px]" title={status?.da_version ?? "None Selected"}>
                {status?.da_installed ? status.da_version : "None Selected"}
              </span>
            </div>

            <div className="flex items-center gap-2">
              <span className="text-muted-foreground">DA Device:</span>
              <span
                className={`inline-block h-2 w-2 rounded-full ${
                  status?.device_visible
                    ? "bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.5)]"
                    : "bg-muted-foreground/40"
                }`}
              />
              <span className="font-semibold text-foreground">
                {status?.device_visible ? "Connected" : "Not Detected"}
              </span>
            </div>

            <div className="flex items-center gap-1.5 font-mono text-muted-foreground">
              <span>Platform:</span>
              <span className="text-foreground">{status?.platform ?? "…"}</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5">
            <Button size="sm" variant="outline" className="h-8 gap-1.5 text-xs" disabled={isBusy} onClick={() => void doctor()}>
              <Stethoscope className="h-3.5 w-3.5" />
              Doctor
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-8 gap-1.5 text-xs text-destructive hover:text-destructive"
              disabled={isBusy || !status?.da_installed}
              onClick={() => void remove()}
            >
              <Trash2 className="h-3.5 w-3.5" />
              Remove Cache
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              title="Refresh status"
              disabled={isBusy}
              onClick={() => void refreshStatus()}
            >
              <RefreshCw className={`h-3.5 w-3.5 ${isBusy ? "animate-spin" : ""}`} />
            </Button>
          </div>
        </div>

        {simulate && (
          <div className="flex items-center gap-2 rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-1.5 text-xs text-amber-500">
            <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
            <span>SIMULATED MODE — real device I/O is bypassed.</span>
          </div>
        )}

        {/* 2-COLUMN COMPACT CONTENT GRID */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3.5">
          {/* LEFT COLUMN: DA Search & Quick Controls */}
          <div className="flex flex-col gap-3.5">
            {/* DA Model Resolver */}
            <SectionCard
              title="DA Driver Resolver"
              description="Resolve Download Agent by device model name."
              contentClassName="space-y-3"
            >
              <div className="flex gap-2">
                <div className="relative flex-1">
                  <Input
                    id="penumbra-device-input"
                    value={deviceModel}
                    onChange={(e) => setDeviceModel(e.target.value)}
                    placeholder="e.g. Infinix NOTE 12, POCO X7 Pro"
                    aria-label="Device model"
                    disabled={isBusy}
                    className="h-9 font-mono text-xs pr-8"
                  />
                  {deviceModel && (
                    <button
                      type="button"
                      onClick={() => setDeviceModel("")}
                      className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
                {/* Preset Dropdown */}
                <select
                  aria-label="Preset Device Models"
                  onChange={(e) => {
                    if (e.target.value) {
                      setDeviceModel(e.target.value);
                      void download(e.target.value);
                    }
                  }}
                  className="h-9 rounded-md border border-input bg-background px-2 text-xs font-mono text-muted-foreground cursor-pointer focus:outline-none"
                  defaultValue=""
                >
                  <option value="" disabled>Presets…</option>
                  {DEVICE_MODEL_PRESETS.map((m) => (
                    <option key={m} value={m} className="bg-popover text-popover-foreground">
                      {m}
                    </option>
                  ))}
                </select>
                <Button
                  className="gap-1.5 h-9 text-xs shrink-0"
                  disabled={isBusy || !deviceModel.trim()}
                  onClick={() => void download()}
                >
                  <Download className="h-3.5 w-3.5" />
                  Fetch DA
                </Button>
              </div>
            </SectionCard>

            {/* Categorized Device Controls */}
            <SectionCard
              title="Device Commands"
              description="Bootloader lock state toggle, partition table, and power controls."
              contentClassName="space-y-2.5"
            >
              <div className="grid grid-cols-3 gap-2">
                <Button size="sm" variant="outline" className="gap-1.5 h-8 text-xs" disabled={isBusy} onClick={() => seccfg(true)}>
                  <ShieldAlert className="h-3.5 w-3.5 text-amber-500" />
                  Unlock BL
                </Button>
                <Button size="sm" variant="outline" className="gap-1.5 h-8 text-xs" disabled={isBusy} onClick={() => seccfg(false)}>
                  <ShieldCheck className="h-3.5 w-3.5 text-emerald-500" />
                  Lock BL
                </Button>
                <Button size="sm" variant="outline" className="gap-1.5 h-8 text-xs" disabled={isBusy} onClick={() => void pgpt()}>
                  <ListTree className="h-3.5 w-3.5 text-primary" />
                  PGPT
                </Button>
              </div>

              <div className="grid grid-cols-2 gap-2 pt-1 border-t border-border/40">
                <Button size="sm" variant="outline" className="gap-1.5 h-8 text-xs" disabled={isBusy} onClick={reboot}>
                  <RotateCcw className="h-3.5 w-3.5" />
                  Reboot Fastboot
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5 h-8 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
                  disabled={isBusy}
                  onClick={shutdown}
                >
                  <Power className="h-3.5 w-3.5" />
                  Shutdown
                </Button>
              </div>
            </SectionCard>
          </div>

          {/* RIGHT COLUMN: Partition Operations */}
          <div className="flex flex-col gap-4">
            <SectionCard
              title="DA Partition Operations"
              description="Read, write, or erase partition blocks via penumbra DA."
              contentClassName="space-y-3"
            >
              {/* Partition Name Input + Preset Dropdown */}
              <div className="space-y-1">
                <div className="flex items-center justify-between">
                  <label htmlFor="penumbra-partition-input" className="text-xs font-medium text-muted-foreground">
                    Target Partition Name
                  </label>
                  <select
                    aria-label="Preset Partition"
                    onChange={(e) => e.target.value && setPartitionName(e.target.value)}
                    className="bg-transparent text-[11px] font-mono text-primary cursor-pointer focus:outline-none"
                    defaultValue=""
                  >
                    <option value="" disabled>Select Preset…</option>
                    {PARTITION_PRESETS.map((p) => (
                      <option key={p} value={p} className="bg-popover text-popover-foreground">
                        {p}
                      </option>
                    ))}
                  </select>
                </div>
                <Input
                  id="penumbra-partition-input"
                  value={partitionName}
                  onChange={(e) => setPartitionName(e.target.value)}
                  placeholder="e.g. boot, vbmeta, recovery"
                  aria-label="penumbra partition"
                  disabled={isBusy}
                  className="h-9"
                />
              </div>

              {/* File Selection */}
              <div className="space-y-1">
                <label htmlFor="penumbra-file-input" className="text-xs font-medium text-muted-foreground">
                  Image File Path (Read output / Write input)
                </label>
                <div className="flex gap-2">
                  <Input
                    id="penumbra-file-input"
                    value={filePath}
                    onChange={(e) => setFilePath(e.target.value)}
                    placeholder="/path/to/image.img"
                    aria-label="penumbra file path"
                    disabled={isBusy}
                    className="h-9 font-mono text-xs"
                  />
                  <Button
                    variant="outline"
                    size="icon"
                    disabled={isBusy}
                    onClick={() => void handleBrowseFile()}
                    title="Browse Image File"
                    className="h-9 w-9 shrink-0"
                  >
                    <FolderOpen className="h-4 w-4" />
                  </Button>
                </div>
              </div>

              {/* Operation Progress Indicator */}
              {opPercent !== null && (
                <div className="space-y-1 rounded-lg border border-primary/20 bg-primary/5 p-2.5">
                  <div className="flex items-center justify-between text-xs font-medium">
                    <span className="flex items-center gap-2 text-primary">
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      Executing Operation…
                    </span>
                    <span className="font-mono">{opPercent}%</span>
                  </div>
                  <Progress value={opPercent} className="h-1.5" />
                </div>
              )}

              {/* Action Trigger Buttons */}
              <div className="grid grid-cols-3 gap-2 pt-1">
                <Button
                  className="gap-2 h-9 text-xs"
                  disabled={isBusy || !partitionName || !filePath}
                  onClick={() => executeOp("read")}
                >
                  <HardDrive className="h-4 w-4" />
                  Read
                </Button>
                <Button
                  className="gap-2 h-9 text-xs"
                  disabled={isBusy || !partitionName || !filePath}
                  onClick={() => executeOp("write")}
                >
                  <HardDrive className="h-4 w-4" />
                  Write
                </Button>
                <Button
                  variant="outline"
                  className="gap-2 h-9 text-xs border-destructive/40 text-destructive hover:bg-destructive/10 hover:text-destructive"
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
      </div>

      {/* Work In Progress Overlay */}
      <div className="absolute inset-0 z-20 flex items-center justify-center p-6 text-center">
        <div className="flex flex-col items-center gap-3 rounded-lg border border-primary/30 bg-background/80 px-8 py-7 shadow-2xl backdrop-blur-md">
          <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
            <Construction className="h-6 w-6" />
          </div>
          <div className="space-y-1">
            <h3 className="text-lg font-bold tracking-tight text-foreground">
              Work In Progress
            </h3>
            <p className="max-w-xs text-xs text-muted-foreground">
              The Penumbra DA-mode tab is currently under active development.
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
              {isDownloading ? "Downloading DA Binary" : "DA Resolution Complete"}
            </DialogTitle>
            <DialogDescription className="text-xs">
              Fetching the Download Agent for your device from the penumbra fork repository.
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
              <Progress value={downloadPercent ?? 0} className="h-2.5" />
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
              <strong className="font-mono text-foreground">{partitionName}</strong>? This action
              cannot be undone.
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


