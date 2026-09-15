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
      <div className="panel-shell flex flex-wrap items-center justify-between gap-2.5 p-2.5 sm:p-3 shrink-0">
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <Button
            variant="outline"
            size="sm"
            disabled={disabled || flashing}
            onClick={() => void handlePickScatter()}
            className="gap-1.5 sm:gap-2 shrink-0 h-8 px-2.5 sm:px-3 text-xs"
          >
            <FolderOpen className="h-4 w-4 text-trace-copper" />
            <span>Choose Scatter</span>
          </Button>
          <div className="font-mono text-[11px] sm:text-xs truncate text-muted-foreground bg-muted/40 px-2.5 py-1.5 rounded border border-border/60 flex-1 min-w-0">
            {scatterPath || "No scatter file selected (.txt or .xml)"}
          </div>
        </div>

        <div className="flex items-center gap-2 select-none shrink-0">
          <Checkbox
            id="scatter-include-preloader"
            checked={options.includePreloader}
            onCheckedChange={(checked) => setIncludePreloader(Boolean(checked))}
            disabled={disabled || flashing}
            aria-label="Include preloader in flash"
          />
          <Label
            htmlFor="scatter-include-preloader"
            className="cursor-pointer text-xs font-medium text-muted-foreground hover:text-foreground"
          >
            Include preloader
          </Label>
        </div>
      </div>

      {/* Partition table */}
      <div className="panel-shell flex min-h-0 flex-1 flex-col overflow-hidden [&_th]:border-r [&_th]:border-border/60 [&_td]:border-r [&_td]:border-border/60 [&_th:last-child]:border-r-0 [&_td:last-child]:border-r-0">
        <div className="border-b border-border/80 bg-card/96">
          <Table className="table-fixed min-w-full" containerClassName="overflow-hidden">
            <colgroup>
              <col className="w-10 sm:w-12" />
              <col className="w-28 sm:w-36 md:w-44" />
              <col className="w-20 sm:w-24 hidden md:table-column" />
              <col className="w-24 sm:w-28 hidden xl:table-column" />
              <col className="w-auto" />
            </colgroup>
            <TableHeader className="[&_th]:text-muted-foreground [&_th]:font-bold text-xs">
              <TableRow>
                <TableHead className="w-10 sm:w-12 px-0 text-center">
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
                <TableHead className="w-28 sm:w-36 md:w-44 px-3">Partition</TableHead>
                <TableHead className="w-20 sm:w-24 hidden md:table-cell px-3 text-right">Size</TableHead>
                <TableHead className="w-24 sm:w-28 hidden xl:table-cell text-center px-3">Type</TableHead>
                <TableHead className="px-3">Image File</TableHead>
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
                <col className="w-10 sm:w-12" />
                <col className="w-28 sm:w-36 md:w-44" />
                <col className="w-20 sm:w-24 hidden md:table-column" />
                <col className="w-24 sm:w-28 hidden xl:table-column" />
                <col className="w-auto" />
              </colgroup>
              <TableBody>
                {rows.map((part) => (
                  <TableRow key={part.partition} className={part.selected ? "row-tint-flash" : undefined}>
                    <TableCell className="w-10 sm:w-12 px-0 text-center">
                      <div className="flex justify-center">
                        <Checkbox
                          checked={part.selected}
                          onCheckedChange={() => togglePartition(part.partition)}
                          disabled={flashing}
                          aria-label={`Select ${part.partition}`}
                        />
                      </div>
                    </TableCell>
                    <TableCell className="w-28 sm:w-36 md:w-44 px-3 font-mono text-xs sm:text-sm font-medium text-foreground truncate">
                      {part.partition}
                    </TableCell>
                    <TableCell className="w-20 sm:w-24 hidden md:table-cell px-3 text-right text-xs text-muted-foreground tabular-nums">
                      {part.size_human}
                    </TableCell>
                    <TableCell className="w-24 sm:w-28 hidden xl:table-cell px-3 truncate text-center text-xs text-muted-foreground">
                      {part.image_type ? (
                        part.image_type
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </TableCell>
                    <TableCell className="px-3 font-mono text-xs text-muted-foreground truncate">
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
      <div className="panel-shell flex flex-wrap items-center justify-between gap-3 p-3 sm:p-4 shrink-0">
        <div className="flex flex-wrap items-center gap-3 min-w-0">
          <div className="flex items-center gap-2 select-none shrink-0">
            <Checkbox
              id="scatter-backup-nvram"
              checked={backupProtected}
              onCheckedChange={(checked) => setBackupProtected(Boolean(checked))}
              disabled={flashing}
              aria-label="Backup NVRAM and calibration before flashing"
            />
            <Label
              htmlFor="scatter-backup-nvram"
              className="flex items-center gap-1.5 cursor-pointer text-xs font-medium select-none"
            >
              <ShieldCheck className="h-3.5 w-3.5 text-signal-green shrink-0" />
              <span>Backup NVRAM & Calibration</span>
            </Label>
          </div>

          {plan?.chipset && (
            <div className="flex items-center gap-2 sm:gap-3 text-xs text-muted-foreground border-l border-border/60 pl-2.5 sm:pl-3 shrink-0">
              <span>Platform: <strong className="text-foreground font-mono">{plan.chipset}</strong></span>
              {plan.project && <span>Project: <strong className="text-foreground font-mono">{plan.project}</strong></span>}
            </div>
          )}
        </div>

        <div className="flex items-center justify-between sm:justify-end gap-3 shrink-0 w-full sm:w-auto">
          <span className="text-xs text-muted-foreground">
            Selected: <strong className="text-foreground">{selectedFlashCount}</strong> / {rows.length}
          </span>
          <Button
            size="sm"
            disabled={disabled || flashing || selectedFlashCount === 0 || !daInstalled}
            onClick={() => void handleStartFlash()}
            className="gap-2 bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold shrink-0 h-8"
          >
            <Zap className="h-4 w-4" />
            {flashing ? "Flashing..." : "Flash Firmware"}
          </Button>
        </div>
      </div>
    </div>
  );
});
