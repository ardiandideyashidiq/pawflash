import { memo, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Zap, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface ParsedScatterPartition {
  name: string;
  fileName: string | null;
  sizeHuman: string;
  isDownload: boolean;
  selected: boolean;
}

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

  const [scatterPath, setScatterPath] = useState("");
  const [platform, setPlatform] = useState<string | null>(null);
  const [project, setProject] = useState<string | null>(null);
  const [partitions, setPartitions] = useState<ParsedScatterPartition[]>([]);
  const [backupProtected, setBackupProtected] = useState(true);
  const [loading, setLoading] = useState(false);
  const [flashing, setFlashing] = useState(false);

  const handlePickScatter = async () => {
    try {
      const selected = await open({
        title: "Select MediaTek Scatter File",
        filters: [{ name: "Scatter files", extensions: ["txt", "xml"] }],
        multiple: false,
      });
      if (typeof selected === "string" && selected.trim()) {
        await loadScatter(selected.trim());
      }
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const loadScatter = async (path: string) => {
    setLoading(true);
    try {
      // parse_scatter returns ScatterFile DTO
      const parsed = await invoke<{
        platform?: string;
        project?: string;
        layouts: Record<string, Array<{
          name: string;
          file_name?: string;
          size: number;
          is_download: boolean;
        }>>;
      }>("parse_scatter", { path });

      setScatterPath(path);
      setPlatform(parsed.platform ?? null);
      setProject(parsed.project ?? null);

      // Collect partitions from first layout
      const firstLayoutKey = Object.keys(parsed.layouts)[0];
      const items = firstLayoutKey ? parsed.layouts[firstLayoutKey] : [];
      const parts: ParsedScatterPartition[] = items.map((p) => ({
        name: p.name,
        fileName: p.file_name ?? null,
        sizeHuman: p.size > 0 ? `${(p.size / (1024 * 1024)).toFixed(1)} MB` : "—",
        isDownload: p.is_download && Boolean(p.file_name),
        selected: p.is_download && Boolean(p.file_name),
      }));

      setPartitions(parts);
      toast.success(`Loaded scatter with ${parts.length} partitions`);
      addEntry({ text: `ScatterLoaded: ${path} (${parts.length} partitions)`, level: "info" });
    } catch (error) {
      const msg = errorMessage(error);
      toast.error(`Failed to parse scatter: ${msg}`);
      addEntry({ text: `ScatterParseError: ${msg}`, level: "error" });
    } finally {
      setLoading(false);
    }
  };

  const togglePartition = (name: string) => {
    setPartitions((prev) =>
      prev.map((p) => (p.name === name ? { ...p, selected: !p.selected } : p))
    );
  };

  const allSelected = useMemo(
    () => partitions.length > 0 && partitions.every((p) => p.selected),
    [partitions]
  );
  const someSelected = useMemo(
    () => partitions.some((p) => p.selected) && !allSelected,
    [partitions, allSelected]
  );

  const toggleAll = () => {
    const next = !allSelected;
    setPartitions((prev) => prev.map((p) => ({ ...p, selected: next })));
  };

  const selectedCount = useMemo(
    () => partitions.filter((p) => p.selected).length,
    [partitions]
  );

  const handleStartFlash = async () => {
    if (!scatterPath || selectedCount === 0) {
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

    const selectedNames = partitions.filter((p) => p.selected).map((p) => p.name);
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
        partitions: allSelected ? null : selectedNames,
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

        {platform && (
          <div className="flex items-center gap-3 text-xs text-muted-foreground">
            <span>Platform: <strong className="text-foreground font-mono">{platform}</strong></span>
            {project && <span>Project: <strong className="text-foreground font-mono">{project}</strong></span>}
          </div>
        )}
      </div>

      {/* Partition table */}
      <div className="panel-shell flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="border-b border-border/80 bg-card/96">
          <Table className="table-fixed min-w-full">
            <colgroup>
              <col className="w-12" />
              <col className="w-48" />
              <col className="w-32" />
              <col className="w-auto" />
            </colgroup>
            <TableHeader className="[&_th]:text-muted-foreground [&_th]:font-bold text-xs">
              <TableRow>
                <TableHead className="px-0 text-center">
                  <div className="flex justify-center">
                    <Checkbox
                      checked={allSelected}
                      indeterminate={someSelected}
                      onCheckedChange={toggleAll}
                      disabled={partitions.length === 0 || flashing}
                      aria-label="Select all partitions"
                    />
                  </div>
                </TableHead>
                <TableHead>Partition</TableHead>
                <TableHead>Size</TableHead>
                <TableHead>Image File</TableHead>
              </TableRow>
            </TableHeader>
          </Table>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          {partitions.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground">
              <p className="text-sm font-medium">No scatter partitions loaded</p>
              <p className="text-xs mt-1">Load a MediaTek scatter file to review partition images.</p>
            </div>
          ) : (
            <Table className="table-fixed min-w-full">
              <colgroup>
                <col className="w-12" />
                <col className="w-48" />
                <col className="w-32" />
                <col className="w-auto" />
              </colgroup>
              <TableBody>
                {partitions.map((part) => (
                  <TableRow key={part.name} className={part.selected ? "row-tint-flash" : undefined}>
                    <TableCell className="px-0 text-center">
                      <div className="flex justify-center">
                        <Checkbox
                          checked={part.selected}
                          onCheckedChange={() => togglePartition(part.name)}
                          disabled={flashing}
                          aria-label={`Select ${part.name}`}
                        />
                      </div>
                    </TableCell>
                    <TableCell className="font-mono text-sm font-medium text-foreground truncate">
                      {part.name}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground tabular-nums">
                      {part.sizeHuman}
                    </TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground truncate">
                      {part.fileName ? (
                        <span className="text-trace-copper font-medium">{part.fileName}</span>
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
        <div className="flex items-center gap-3">
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
              Backup NVRAM & Calibration before flashing
            </span>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <span className="text-xs text-muted-foreground">
            Selected: <strong className="text-foreground">{selectedCount}</strong> / {partitions.length}
          </span>
          <Button
            size="sm"
            disabled={disabled || flashing || selectedCount === 0 || !daInstalled}
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
