import { memo } from "react";
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
import type { PartitionRow } from "@/types/api";

interface PartitionTableProps {
  partitions: PartitionRow[];
  loading?: boolean;
  onToggle: (name: string) => void;
  onToggleAll: () => void;
  allSelected: boolean;
  someSelected: boolean;
  className?: string;
}

const columnWidths = ["w-12", "w-36", "w-28", "w-40", "w-56"];

export const PartitionTable = memo(function PartitionTable({
  partitions,
  loading = false,
  onToggle,
  onToggleAll,
  allSelected,
  someSelected,
  className,
}: PartitionTableProps) {
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
    <div className={cn("panel-shell flex min-h-0 flex-1 flex-col overflow-x-auto", className)}>
      <div className="border-b border-border/80 bg-card/96">
        <Table className="table-fixed min-w-max">
          <colgroup>
            {columnWidths.map((width, i) => (
              <col key={width} className={cn(width, (i === 2 || i === 3) && "max-lg:hidden")} />
            ))}
          </colgroup>
          <TableHeader className="[&_th]:text-muted-foreground">
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
                  partition.action === "wipe" && "row-tint-wipe",
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
                <TableCell className="hidden whitespace-nowrap text-center lg:table-cell">
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
                  <span
                    className={cn(
                      "block min-w-0 truncate font-mono",
                      !partition.image_name && "text-muted-foreground",
                    )}
                    title={partition.image_path ?? partition.image_name ?? "No image resolved"}
                  >
                    {partition.image_name ?? "—"}
                  </span>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </ScrollArea>
    </div>
  );
});
