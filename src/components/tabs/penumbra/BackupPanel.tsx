import { memo, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Archive, CheckCircle2, FolderDown, HardDrive, RefreshCw, ShieldCheck, AlertTriangle } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage, type PenumbraPartitionInfo } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface BackupPanelProps {
  daInstalled: boolean;
  disabled?: boolean;
}

interface BackupPartitionItem {
  name: string;
  sizeFormatted: string;
  selected: boolean;
}

export const BackupPanel = memo(function BackupPanel({
  daInstalled,
  disabled = false,
}: BackupPanelProps) {
  const { simulate } = useSimulation();
  const flash = useFlashProgress();
  const { addEntry, addProgressEvent } = useConsole();

  const [backingUpCalibration, setBackingUpCalibration] = useState(false);
  const [calibrationSuccessList, setCalibrationSuccessList] = useState<string[]>([]);
  const [partitions, setPartitions] = useState<BackupPartitionItem[]>([]);
  const [excludeUserdata, setExcludeUserdata] = useState(true);
  const [loadingPgpt, setLoadingPgpt] = useState(false);
  const [backingUpFull, setBackingUpFull] = useState(false);

  // 1-Click NVRAM & Calibration Backup
  const handleBackupCalibration = async () => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    try {
      const dir = await open({
        title: "Select Directory to Save NVRAM & Calibration Files",
        directory: true,
        multiple: false,
      });
      if (typeof dir !== "string" || !dir.trim()) return;

      setBackingUpCalibration(true);
      flash.reset();
      flash.openDialog();

      const channel = new Channel<ProgressEvent>();
      channel.onmessage = (event) => {
        flash.onEvent(event);
        addProgressEvent(event);
      };

      addEntry({ text: `Penumbra CalibrationBackup Started -> ${dir}`, level: "command" });
      const backedUp = await invoke<string[]>("penumbra_backup_calibration", {
        dir: dir.trim(),
        onEvent: channel,
        simulate,
      });

      setCalibrationSuccessList(backedUp);
      toast.success(`Successfully backed up ${backedUp.length} calibration partitions!`);
      addEntry({
        text: `Penumbra CalibrationBackup Success: ${backedUp.join(", ")}`,
        level: "success",
      });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Calibration backup failed: ${msg}`);
      addEntry({ text: `Penumbra CalibrationBackup Error: ${msg}`, level: "error" });
    } finally {
      setBackingUpCalibration(false);
    }
  };

  // Load partitions for full backup
  const handleLoadPgpt = async () => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }
    setLoadingPgpt(true);

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => addProgressEvent(event);

    try {
      const list = await invoke<PenumbraPartitionInfo[]>("penumbra_pgpt", {
        onEvent: channel,
        simulate,
      });
      const items: BackupPartitionItem[] = list.map((p) => ({
        name: p.name,
        sizeFormatted: p.sizeFormatted,
        selected: excludeUserdata ? p.name.toLowerCase() !== "userdata" : true,
      }));
      setPartitions(items);
      toast.success(`Loaded ${list.length} partitions from device`);
    } catch (error) {
      toast.error(`Failed to read partition table: ${errorMessage(error)}`);
    } finally {
      setLoadingPgpt(false);
    }
  };

  const togglePartition = (name: string) => {
    setPartitions((prev) =>
      prev.map((p) => (p.name === name ? { ...p, selected: !p.selected } : p))
    );
  };

  const handleToggleExcludeUserdata = (checked: boolean) => {
    setExcludeUserdata(checked);
    if (checked) {
      setPartitions((prev) =>
        prev.map((p) =>
          p.name.toLowerCase() === "userdata" ? { ...p, selected: false } : p
        )
      );
    }
  };

  const allSelected = useMemo(
    () => partitions.length > 0 && partitions.every((p) => p.selected),
    [partitions]
  );

  const toggleAll = () => {
    const next = !allSelected;
    setPartitions((prev) =>
      prev.map((p) => ({
        ...p,
        selected: excludeUserdata && p.name.toLowerCase() === "userdata" ? false : next,
      }))
    );
  };

  const selectedCount = useMemo(
    () => partitions.filter((p) => p.selected).length,
    [partitions]
  );

  const handleBackupSelected = async () => {
    if (selectedCount === 0) {
      toast.error("No partitions selected for backup");
      return;
    }

    try {
      const dir = await open({
        title: "Select Directory to Save Partition Dumps",
        directory: true,
        multiple: false,
      });
      if (typeof dir !== "string" || !dir.trim()) return;

      setBackingUpFull(true);
      flash.reset();
      flash.openDialog();

      const selected = partitions.filter((p) => p.selected);
      let successCount = 0;

      for (const part of selected) {
        const filePath = `${dir.trim()}/${part.name}.img`;
        const channel = new Channel<ProgressEvent>();
        channel.onmessage = (event) => {
          flash.onEvent(event);
          addProgressEvent(event);
        };

        try {
          await invoke("penumbra_read", {
            partition: part.name,
            file: filePath,
            onEvent: channel,
            simulate,
          });
          successCount++;
        } catch (err) {
          addEntry({ text: `Backup error on ${part.name}: ${errorMessage(err)}`, level: "error" });
        }
      }

      toast.success(`Completed backup: ${successCount} / ${selected.length} partitions saved`);
      addEntry({
        text: `Full backup completed: ${successCount}/${selected.length} partitions dumped to ${dir}`,
        level: "success",
      });
    } catch (error) {
      toast.error(`Backup error: ${errorMessage(error)}`);
    } finally {
      setBackingUpFull(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      {/* Hero NVRAM Card */}
      <div className="panel-shell p-5 border-trace-copper/40 bg-trace-copper/5 shrink-0">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-base font-semibold text-foreground flex items-center gap-2">
              <ShieldCheck className="h-5 w-5 text-signal-green" />
              Backup NVRAM & Calibration (IMEI / Radio / Keys)
            </h3>
            <p className="text-xs text-muted-foreground leading-relaxed max-w-2xl">
              Dumps vital device-specific calibration partitions:{" "}
              <code className="font-mono text-trace-copper">
                nvram, nvdata, protect_f, protect_s, nvcfg, proinfo, persist, seccfg, sec1
              </code>. Safeguards IMEI and carrier configuration against accidental bricking or baseband loss.
            </p>
          </div>

          <Button
            size="sm"
            disabled={disabled || backingUpCalibration || !daInstalled}
            onClick={() => void handleBackupCalibration()}
            className="gap-2 bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold shrink-0"
          >
            <FolderDown className="h-4 w-4" />
            {backingUpCalibration ? "Backing up..." : "Backup Calibration"}
          </Button>
        </div>

        {calibrationSuccessList.length > 0 && (
          <div className="mt-3.5 pt-3 border-t border-border/60 flex flex-wrap items-center gap-2 text-xs">
            <span className="text-signal-green font-medium flex items-center gap-1">
              <CheckCircle2 className="h-3.5 w-3.5" /> Backed up:
            </span>
            {calibrationSuccessList.map((name) => (
              <span key={name} className="px-2 py-0.5 rounded bg-card/80 border border-border/60 font-mono text-[11px]">
                {name}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Bulk Partition Backup */}
      <SectionCard
        title="Custom & Full Partition Backup"
        className="flex-1 flex flex-col min-h-0 overflow-hidden"
        contentClassName="flex-1 flex flex-col min-h-0 mt-2 gap-3"
      >
        <div className="flex flex-wrap items-center justify-between gap-3 shrink-0">
          <div className="flex items-center gap-3">
            <Button
              variant="outline"
              size="sm"
              disabled={disabled || loadingPgpt || !daInstalled}
              onClick={() => void handleLoadPgpt()}
              className="gap-2 text-xs"
            >
              <RefreshCw className={`h-3.5 w-3.5 ${loadingPgpt ? "animate-spin" : ""}`} />
              {partitions.length > 0 ? "Reload Partition List" : "Load Device Partitions"}
            </Button>

            {partitions.length > 0 && (
              <Button
                variant="outline"
                size="sm"
                onClick={toggleAll}
                className="text-xs"
              >
                {allSelected ? "Deselect All" : "Select All"}
              </Button>
            )}

            <div
              role="button"
              tabIndex={0}
              onClick={() => handleToggleExcludeUserdata(!excludeUserdata)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleToggleExcludeUserdata(!excludeUserdata);
                }
              }}
              className="flex items-center gap-2 cursor-pointer text-xs font-medium select-none"
            >
              <Checkbox
                checked={excludeUserdata}
                onCheckedChange={(c) => handleToggleExcludeUserdata(Boolean(c))}
              />
              <span className="flex items-center gap-1 text-muted-foreground">
                <AlertTriangle className="h-3.5 w-3.5 text-signal-amber" />
                Exclude Userdata (Recommended)
              </span>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <span className="text-xs text-muted-foreground">
              Selected: <strong className="text-foreground">{selectedCount}</strong> / {partitions.length}
            </span>
            <Button
              size="sm"
              disabled={disabled || backingUpFull || selectedCount === 0 || !daInstalled}
              onClick={() => void handleBackupSelected()}
              className="gap-2 text-xs bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold"
            >
              <Archive className="h-3.5 w-3.5" />
              {backingUpFull ? "Dumping..." : "Dump Selected Partitions"}
            </Button>
          </div>
        </div>

        <div className="panel-shell flex-1 min-h-0 flex flex-col overflow-hidden">
          <ScrollArea className="min-h-0 flex-1 p-3">
            {partitions.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-8 text-center text-muted-foreground">
                <HardDrive className="h-7 w-7 text-muted-foreground/40 mb-2" />
                <p className="text-xs">Click <strong>Load Device Partitions</strong> to choose partitions to back up.</p>
              </div>
            ) : (
              <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2">
                {partitions.map((part) => (
                  <div
                    key={part.name}
                    role="button"
                    tabIndex={0}
                    onClick={() => togglePartition(part.name)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        togglePartition(part.name);
                      }
                    }}
                    className={`flex items-center gap-2 p-2 rounded border cursor-pointer select-none transition-colors text-xs ${
                      part.selected
                        ? "border-trace-copper/60 bg-trace-copper/10 font-medium text-foreground"
                        : "border-border/60 bg-card/60 text-muted-foreground hover:bg-muted/40"
                    }`}
                  >
                    <Checkbox
                      checked={part.selected}
                      onCheckedChange={() => togglePartition(part.name)}
                      aria-label={`Select partition ${part.name}`}
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate font-mono">{part.name}</div>
                      <div className="text-[10px] text-muted-foreground">{part.sizeFormatted}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </ScrollArea>
        </div>
      </SectionCard>
    </div>
  );
});
