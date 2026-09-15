import { memo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Zap, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useFlashPlan } from "@/hooks/useFlashPlan";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface ScatterPanelProps {
  daInstalled: boolean;
  disabled?: boolean;
}

export const ScatterPanel = memo(function ScatterPanel({
  daInstalled,
  disabled = false,
}: ScatterPanelProps) {
  const { simulate } = useSimulation();
  const flash = useFlashProgress();
  const { addEntry, addProgressEvent } = useConsole();
  const {
    scatterPath,
    plan,
    loading,
    options,
    setIncludePreloader,
    togglePartition,
    toggleAllPartitions,
    allSelected,
    someSelected,
    selectedFlashCount,
    rows,
    selectedRows,
    loadScatter,
  } = useFlashPlan();

  const [backupProtected, setBackupProtected] = useState(true);
  const [flashing, setFlashing] = useState(false);

  const handlePickScatter = async () => {
    try {
      const selected = await open({
        title: "Select MediaTek Scatter File",
        filters: [{ name: "Scatter files", extensions: ["txt", "xml"] }],
        multiple: false,
      });
      if (typeof selected === "string" && selected.trim()) {
        loadScatter(selected.trim());
      }
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const handleStartFlash = async () => {
    if (!scatterPath || selectedFlashCount === 0) {
      toast.error("Select at least one partition to flash");
      return;
    }
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    setFlashing(true);
    flash.reset();
    flash.openDialog();

    const selectedNames = selectedRows.map((p) => p.partition);
    addEntry({
      text: `Penumbra ScatterFlash Started path=${scatterPath} count=${selectedNames.length}`,
      level: "command",
    });

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    try {
      await invoke("penumbra_flash_scatter", {
        scatterPath,
        partitions: selectedNames,
        backupProtected,
        simulate,
        onEvent: channel,
      });
      toast.success("Firmware flash complete");
      addEntry({ text: "Penumbra ScatterFlash Succeeded", level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Flash failed: ${msg}`);
      addEntry({ text: `Penumbra ScatterFlash Error: ${msg}`, level: "error" });
    } finally {
      setFlashing(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {/* Header bar */}
      <div className="panel-shell flex flex-wrap items-center justify-between gap-3 p-3 shrink-0">
        <div className="flex items-center gap-2 flex-1 min-w-[280px]">
          <Button
            variant="outline"
            size="sm"
            disabled={disabled || loading || flashing}
            onClick={() => void handlePickScatter()}
            className="gap-2 shrink-0"
          >
            <FolderOpen className="h-4 w-4 text-trace-copper" />
            {loading ? "Parsing..." : "Choose Scatter"}
          </Button>
          <div className="font-mono text-xs truncate text-muted-foreground bg-muted/40 px-2.5 py-1.5 rounded border border-border/60 flex-1">
            {scatterPath || "No scatter file selected (.txt or .xml)"}
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-4 text-xs">
          <div
            role="button"
            tabIndex={0}
            onClick={() => setIncludePreloader(!options.includePreloader)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setIncludePreloader(!options.includePreloader);
              }
            }}
            className="flex items-center gap-2 cursor-pointer select-none text-xs font-medium"
          >
            <Checkbox
              id="scatter-include-preloader-header"
              checked={options.includePreloader}
              onCheckedChange={(checked) => setIncludePreloader(Boolean(checked))}
              disabled={disabled || loading || flashing}
              aria-label="Include preloader in flash"
            />
            <Label
              htmlFor="scatter-include-preloader-header"
              className="cursor-pointer text-xs font-medium text-muted-foreground hover:text-foreground"
            >
              Include preloader
            </Label>
          </div>

          {plan?.chipset && (
            <div className="flex items-center gap-3 text-xs text-muted-foreground">
              <span>Platform: <strong className="text-foreground font-mono">{plan.chipset}</strong></span>
              {plan.project && <span>Project: <strong className="text-foreground font-mono">{plan.project}</strong></span>}
            </div>
          )}
        </div>
      </div>

      {/* Partition table */}
      <div className="panel-shell flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="border-b border-border/80 bg-card/96">
          <Table className="table-fixed min-w-full" containerClassName="overflow-hidden">
            <colgroup>
              <col className="w-12" />
              <col className="w-36 sm:w-48" />
              <col className="w-28 hidden sm:table-column" />
              <col className="w-auto" />
            </colgroup>
            <TableHeader className="[&_th]:text-muted-foreground [&_th]:font-bold text-xs">
              <TableRow>
                <TableHead className="px-0 text-center">
                  <div className="flex justify-center">
                    <Checkbox
                      checked={allSelected}
                      indeterminate={someSelected}
                      onCheckedChange={toggleAllPartitions}
                      disabled={rows.length === 0 || flashing}
                      aria-label="Select all partitions"
                    />
                  </div>
                </TableHead>
                <TableHead>Partition</TableHead>
                <TableHead className="hidden sm:table-cell">Size</TableHead>
                <TableHead>Image File</TableHead>
              </TableRow>
            </TableHeader>
          </Table>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          {rows.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground">
              <p className="text-sm font-medium">No scatter partitions loaded</p>
              <p className="text-xs mt-1">Load a MediaTek scatter file to review partition images.</p>
            </div>
          ) : (
            <Table className="table-fixed min-w-full" containerClassName="overflow-hidden">
              <colgroup>
                <col className="w-12" />
                <col className="w-36 sm:w-48" />
                <col className="w-28 hidden sm:table-column" />
                <col className="w-auto" />
              </colgroup>
              <TableBody>
                {rows.map((part) => (
                  <TableRow key={part.partition} className={part.selected ? "row-tint-flash" : undefined}>
                    <TableCell className="px-0 text-center">
                      <div className="flex justify-center">
                        <Checkbox
                          checked={part.selected}
                          onCheckedChange={() => togglePartition(part.partition)}
                          disabled={flashing}
                          aria-label={`Select ${part.partition}`}
                        />
                      </div>
                    </TableCell>
                    <TableCell className="font-mono text-sm font-medium text-foreground truncate">
                      {part.partition}
                    </TableCell>
                    <TableCell className="hidden sm:table-cell text-xs text-muted-foreground tabular-nums">
                      {part.size_human}
                    </TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground truncate">
                      {part.image_name ? (
                        <span className="text-trace-copper font-medium">{part.image_name}</span>
                      ) : (
                        "—"
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </ScrollArea>
      </div>

      {/* Flash Action Footer */}
      <div className="panel-shell flex flex-wrap items-center justify-between gap-4 p-4 shrink-0">
        <div className="flex flex-wrap items-center gap-4">
          <div
            role="button"
            tabIndex={0}
            onClick={() => setBackupProtected(!backupProtected)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setBackupProtected(!backupProtected);
              }
            }}
            className="flex items-center gap-2 cursor-pointer text-xs font-medium select-none"
          >
            <Checkbox
              checked={backupProtected}
              onCheckedChange={(checked) => setBackupProtected(Boolean(checked))}
              disabled={flashing}
              aria-label="Backup NVRAM and calibration before flashing"
            />
            <span className="flex items-center gap-1.5">
              <ShieldCheck className="h-3.5 w-3.5 text-signal-green" />
              Backup NVRAM & Calibration
            </span>
          </div>

          <div
            role="button"
            tabIndex={0}
            onClick={() => setIncludePreloader(!options.includePreloader)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setIncludePreloader(!options.includePreloader);
              }
            }}
            className="flex items-center gap-2 cursor-pointer text-xs font-medium select-none"
          >
            <Checkbox
              id="scatter-include-preloader-footer"
              checked={options.includePreloader}
              onCheckedChange={(checked) => setIncludePreloader(Boolean(checked))}
              disabled={flashing}
              aria-label="Include preloader in flash"
            />
            <Label
              htmlFor="scatter-include-preloader-footer"
              className="cursor-pointer text-xs font-medium text-muted-foreground hover:text-foreground"
            >
              Include preloader
            </Label>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <span className="text-xs text-muted-foreground">
            Selected: <strong className="text-foreground">{selectedFlashCount}</strong> / {rows.length}
          </span>
          <Button
            size="sm"
            disabled={disabled || flashing || selectedFlashCount === 0 || !daInstalled}
            onClick={() => void handleStartFlash()}
            className="gap-2 bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold"
          >
            <Zap className="h-4 w-4" />
            {flashing ? "Flashing..." : "Flash Firmware"}
          </Button>
        </div>
      </div>
    </div>
  );
});
