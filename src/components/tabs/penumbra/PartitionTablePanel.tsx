import { memo, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, Edit3, Eraser, HardDrive, RefreshCw, Search, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage, type PenumbraPartitionInfo } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface PartitionTablePanelProps {
  daInstalled: boolean;
  disabled?: boolean;
}

export const PartitionTablePanel = memo(function PartitionTablePanel({
  daInstalled,
  disabled = false,
}: PartitionTablePanelProps) {
  const { simulate } = useSimulation();
  const flash = useFlashProgress();
  const { addEntry, addProgressEvent } = useConsole();

  const [partitions, setPartitions] = useState<PenumbraPartitionInfo[]>([]);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(false);
  const [busyPartition, setBusyPartition] = useState<string | null>(null);

  const handleReadPgpt = async () => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }
    setLoading(true);
    addEntry({ text: "Penumbra PGPT Read Started", level: "command" });

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => addProgressEvent(event);

    try {
      const list = await invoke<PenumbraPartitionInfo[]>("penumbra_pgpt", {
        onEvent: channel,
        simulate,
      });
      setPartitions(list);
      toast.success(`Found ${list.length} partitions from device PGPT`);
      addEntry({ text: `Penumbra PGPT Read Success: ${list.length} partitions`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      toast.error(`Failed to read PGPT: ${msg}`);
      addEntry({ text: `Penumbra PGPT Error: ${msg}`, level: "error" });
    } finally {
      setLoading(false);
    }
  };

  const handleDump = async (part: PenumbraPartitionInfo) => {
    try {
      const savePath = await save({
        title: `Save partition dump for ${part.name}`,
        defaultPath: `${part.name}.img`,
        filters: [{ name: "Disk Image", extensions: ["img", "bin"] }],
      });
      if (!savePath) return;

      setBusyPartition(part.name);
      flash.reset();
      flash.openDialog();

      const channel = new Channel<ProgressEvent>();
      channel.onmessage = (event) => {
        flash.onEvent(event);
        addProgressEvent(event);
      };

      await invoke("penumbra_read", {
        partition: part.name,
        file: savePath,
        onEvent: channel,
        simulate,
      });

      toast.success(`Dumped ${part.name} to ${savePath.split(/[/\\]/).pop()}`);
      addEntry({ text: `Penumbra Read Success: ${part.name} -> ${savePath}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Read failed: ${msg}`);
      addEntry({ text: `Penumbra Read Error ${part.name}: ${msg}`, level: "error" });
    } finally {
      setBusyPartition(null);
    }
  };

  const handleWrite = async (part: PenumbraPartitionInfo) => {
    try {
      const filePath = await open({
        title: `Select image to flash into ${part.name}`,
        filters: [{ name: "Image files", extensions: ["img", "bin", "iso"] }],
        multiple: false,
      });
      if (typeof filePath !== "string" || !filePath.trim()) return;

      setBusyPartition(part.name);
      flash.reset();
      flash.openDialog();

      const channel = new Channel<ProgressEvent>();
      channel.onmessage = (event) => {
        flash.onEvent(event);
        addProgressEvent(event);
      };

      await invoke("penumbra_write", {
        partition: part.name,
        file: filePath.trim(),
        onEvent: channel,
        simulate,
      });

      toast.success(`Flashed ${filePath.split(/[/\\]/).pop()} to ${part.name}`);
      addEntry({ text: `Penumbra Write Success: ${filePath} -> ${part.name}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Write failed: ${msg}`);
      addEntry({ text: `Penumbra Write Error ${part.name}: ${msg}`, level: "error" });
    } finally {
      setBusyPartition(null);
    }
  };

  const handleErase = async (part: PenumbraPartitionInfo) => {
    if (!window.confirm(`Are you sure you want to erase partition "${part.name}"? This cannot be undone!`)) {
      return;
    }

    setBusyPartition(part.name);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    try {
      await invoke("penumbra_erase", {
        partition: part.name,
        onEvent: channel,
        simulate,
      });
      toast.success(`Partition ${part.name} erased`);
      addEntry({ text: `Penumbra Erase Success: ${part.name}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Erase failed: ${msg}`);
      addEntry({ text: `Penumbra Erase Error ${part.name}: ${msg}`, level: "error" });
    } finally {
      setBusyPartition(null);
    }
  };

  const handleFormat = async (part: PenumbraPartitionInfo) => {
    if (!window.confirm(`Format partition "${part.name}"? All existing data in this partition will be erased.`)) {
      return;
    }

    setBusyPartition(part.name);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    try {
      await invoke("penumbra_format", {
        partition: part.name,
        onEvent: channel,
        simulate,
      });
      toast.success(`Partition ${part.name} formatted`);
      addEntry({ text: `Penumbra Format Success: ${part.name}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Format failed: ${msg}`);
      addEntry({ text: `Penumbra Format Error ${part.name}: ${msg}`, level: "error" });
    } finally {
      setBusyPartition(null);
    }
  };

  const filteredPartitions = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return partitions;
    return partitions.filter((p) => p.name.toLowerCase().includes(q));
  }, [partitions, filter]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {/* Action and Search bar */}
      <div className="panel-shell flex flex-wrap items-center justify-between gap-3 p-3 shrink-0">
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            disabled={disabled || loading || !daInstalled}
            onClick={() => void handleReadPgpt()}
            className="gap-2 bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
            {loading ? "Reading Table..." : "Read Partition Table (PGPT)"}
          </Button>
          <span className="text-xs text-muted-foreground">
            {partitions.length > 0 ? `${partitions.length} partitions detected` : "Connect device in BROM/Preloader"}
          </span>
        </div>

        <div className="flex items-center gap-2 min-w-[200px] max-w-xs flex-1 sm:flex-initial">
          <div className="relative w-full">
            <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
            <Input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Search partition name..."
              className="h-8 pl-8 text-xs font-mono"
            />
          </div>
        </div>
      </div>

      {/* Partition List Table */}
      <div className="panel-shell flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="border-b border-border/80 bg-card/96">
          <Table className="table-fixed min-w-full" containerClassName="overflow-hidden">
            <colgroup>
              <col className="w-auto min-w-[120px]" />
              <col className="w-28 hidden xl:table-column" />
              <col className="w-24 hidden sm:table-column" />
              <col className="w-20 hidden md:table-column" />
              <col className="w-44 sm:w-60" />
            </colgroup>
            <TableHeader className="[&_th]:text-muted-foreground [&_th]:font-bold text-xs">
              <TableRow>
                <TableHead>Partition</TableHead>
                <TableHead className="hidden xl:table-cell">Address</TableHead>
                <TableHead className="hidden sm:table-cell">Size</TableHead>
                <TableHead className="hidden md:table-cell">Section</TableHead>
                <TableHead className="text-right pr-4">Operations</TableHead>
              </TableRow>
            </TableHeader>
          </Table>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          {partitions.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground">
              <HardDrive className="h-8 w-8 text-muted-foreground/50 mb-2" />
              <p className="text-sm font-medium">No partition table loaded</p>
              <p className="text-xs mt-1 max-w-sm">
                Click <strong>Read Partition Table (PGPT)</strong> above with device connected in Preloader or BROM mode.
              </p>
            </div>
          ) : (
            <Table className="table-fixed min-w-full" containerClassName="overflow-hidden">
              <colgroup>
                <col className="w-auto min-w-[120px]" />
                <col className="w-28 hidden xl:table-column" />
                <col className="w-24 hidden sm:table-column" />
                <col className="w-20 hidden md:table-column" />
                <col className="w-44 sm:w-60" />
              </colgroup>
              <TableBody>
                {filteredPartitions.map((part) => {
                  const isBusy = busyPartition === part.name;
                  return (
                    <TableRow key={part.name} className={isBusy ? "row-tint-flash" : undefined}>
                      <TableCell className="font-mono text-sm font-medium text-foreground truncate">
                        {part.name}
                      </TableCell>
                      <TableCell className="hidden xl:table-cell font-mono text-xs text-muted-foreground tabular-nums">
                        0x{part.address.toString(16).toUpperCase().padStart(8, "0")}
                      </TableCell>
                      <TableCell className="hidden sm:table-cell text-xs text-muted-foreground tabular-nums">
                        {part.sizeFormatted}
                      </TableCell>
                      <TableCell className="hidden md:table-cell font-mono text-xs text-muted-foreground">
                        {part.section}
                      </TableCell>
                      <TableCell className="text-right pr-2">
                        <div className="flex items-center justify-end gap-1">
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={disabled || Boolean(busyPartition)}
                            onClick={() => void handleDump(part)}
                            className="h-7 px-1.5 sm:px-2 text-[11px] gap-1"
                            title="Dump partition to disk"
                          >
                            <Download className="h-3 w-3 text-signal-green" />
                            <span className="hidden sm:inline">Read</span>
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={disabled || Boolean(busyPartition)}
                            onClick={() => void handleWrite(part)}
                            className="h-7 px-1.5 sm:px-2 text-[11px] gap-1"
                            title="Flash image into partition"
                          >
                            <Edit3 className="h-3 w-3 text-trace-copper" />
                            <span className="hidden sm:inline">Write</span>
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={disabled || Boolean(busyPartition)}
                            onClick={() => void handleFormat(part)}
                            className="h-7 px-1.5 sm:px-2 text-[11px] gap-1"
                            title="Format partition"
                          >
                            <Eraser className="h-3 w-3 text-signal-amber" />
                            <span className="hidden sm:inline">Format</span>
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={disabled || Boolean(busyPartition)}
                            onClick={() => void handleErase(part)}
                            className="h-7 px-1.5 sm:px-2 text-[11px] gap-1 hover:text-error"
                            title="Erase partition"
                          >
                            <Trash2 className="h-3 w-3 text-error" />
                            <span className="hidden sm:inline">Erase</span>
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          )}
        </ScrollArea>
      </div>
    </div>
  );
});
