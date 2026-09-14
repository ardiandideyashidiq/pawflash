import { memo, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeftRight, RotateCcw } from "lucide-react";
import { toast } from "sonner";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";
import { errorMessage, type PartitionRow } from "@/types/api";

interface PartitionTableProps {
  partitions: PartitionRow[];
  loading?: boolean;
  onToggle: (name: string) => void;
  onToggleAll: () => void;
  allSelected: boolean;
  someSelected: boolean;
  className?: string;
  onOverrideImage?: (partition: string, path: string) => void;
  onClearOverrideImage?: (partition: string) => void;
}

const columnWidths = ["w-12", "w-36", "w-28", "w-40", "w-64"];

export const PartitionTable = memo(function PartitionTable({
  partitions,
  loading = false,
  onToggle,
  onToggleAll,
  allSelected,
  someSelected,
  className,
  onOverrideImage,
  onClearOverrideImage,
}: PartitionTableProps) {
  const handleSwapImage = useCallback(
    async (partition: string) => {
      try {
        const selected = await open({
          title: `Select image for ${partition}`,
          filters: [
            {
              name: "Partition images",
              extensions: ["img", "bin", "sin", "iso"],
            },
          ],
          multiple: false,
        });
        if (typeof selected === "string" && selected.trim()) {
          onOverrideImage?.(partition, selected.trim());
        }
      } catch (error) {
        toast.error(errorMessage(error));
      }
    },
    [onOverrideImage],
  );

  if (partitions.length === 0) {
    return (
      <div
        className={cn(
          "panel-shell flex min-h-0 flex-1 items-center justify-center p-12 text-center",
          className,
        )}
      >
        <div className="max-w-[40ch] space-y-3">
          <p className="text-base font-medium text-foreground">
            {loading ? "Refreshing flash plan" : "No flash plan loaded"}
          </p>
          <p className="text-sm leading-6 text-muted-foreground">
            {loading
              ? "Reviewing the selected firmware source and rebuilding the partition list."
              : "Select a scatter file or firmware manifest to review partitions and prepare the flash set."}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("panel-shell flex min-h-0 flex-1 flex-col overflow-x-auto [&_th]:border-r [&_th]:border-border [&_td]:border-r [&_td]:border-border", className)}>
      <div className="border-b border-border/80 bg-card/96">
        <Table className="table-fixed min-w-max">
          <colgroup>
            {columnWidths.map((width, i) => (
              <col key={width} className={cn(width, (i === 2 || i === 3) && "max-lg:hidden")} />
            ))}
          </colgroup>
          <TableHeader className="[&_th]:text-muted-foreground [&_th]:text-center [&_th]:font-bold">
            <TableRow>
              <TableHead className={cn(columnWidths[0], "px-0 text-center")}>
                <div className="flex justify-center">
                  <Checkbox
                    checked={allSelected}
                    indeterminate={someSelected}
                    onCheckedChange={onToggleAll}
                    aria-label={allSelected ? "Clear all partitions" : "Select all partitions"}
                  />
                </div>
              </TableHead>
              <TableHead className={columnWidths[1]}>Partition</TableHead>
              <TableHead className={cn(columnWidths[2], "hidden lg:table-cell")}>Size</TableHead>
              <TableHead className={cn(columnWidths[3], "hidden lg:table-cell")}>Type</TableHead>
              <TableHead className={columnWidths[4]}>Image</TableHead>
            </TableRow>
          </TableHeader>
        </Table>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <Table className="table-fixed min-w-max">
          <colgroup>
            {columnWidths.map((width, i) => (
              <col key={width} className={cn(width, (i === 2 || i === 3) && "max-lg:hidden")} />
            ))}
          </colgroup>
          <TableBody>
            {partitions.map((partition) => (
              <TableRow
                key={partition.partition}
                className={cn(
                  partition.action === "flash" && "row-tint-flash",
                )}
              >
                <TableCell className="px-0 text-center">
                  <div className="flex justify-center">
                    <Checkbox
                      checked={partition.selected}
                      onCheckedChange={() => onToggle(partition.partition)}
                      aria-label={`Select ${partition.partition}`}
                    />
                  </div>
                </TableCell>
                <TableCell className="truncate text-left" title={partition.partition}>
                  <span className="font-mono">{partition.partition}</span>
                </TableCell>
                <TableCell className="hidden whitespace-nowrap text-right tabular-nums lg:table-cell">
                  {partition.size_human}
                </TableCell>
                <TableCell className="hidden truncate text-center lg:table-cell">
                  {partition.image_type ? (
                    partition.image_type
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </TableCell>
                <TableCell className="text-left">
                  <div className="flex items-center justify-between gap-2 group/cell">
                    <div className="flex min-w-0 items-center gap-1.5">
                      <span
                        className={cn(
                          "truncate font-mono",
                          !partition.image_name && "text-muted-foreground",
                          partition.is_overridden && "text-trace-copper font-medium",
                        )}
                        title={partition.image_path ?? partition.image_name ?? "No image resolved"}
                      >
                        {partition.image_name ?? "—"}
                      </span>
                      {partition.is_overridden && (
                        <span className="shrink-0 rounded bg-trace-copper/15 px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-trace-copper">
                          custom
                        </span>
                      )}
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      {partition.is_overridden && onClearOverrideImage && (
                        <button
                          type="button"
                          onClick={() => onClearOverrideImage(partition.partition)}
                          title="Reset to default image"
                          aria-label={`Reset ${partition.partition} to default image`}
                          className="cursor-pointer rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                          disabled={loading}
                        >
                          <RotateCcw className="h-3.5 w-3.5" />
                        </button>
                      )}
                      {onOverrideImage && (
                        <button
                          type="button"
                          onClick={() => void handleSwapImage(partition.partition)}
                          title="Swap image"
                          aria-label={`Swap image for ${partition.partition}`}
                          className="cursor-pointer rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                          disabled={loading}
                        >
                          <ArrowLeftRight className="h-3.5 w-3.5" />
                        </button>
                      )}
                    </div>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </ScrollArea>
    </div>
  );
});
