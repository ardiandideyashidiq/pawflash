import { Fragment, useMemo } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn, formatBytes } from "@/lib/utils";
import {
  SAFETY_LEGEND,
  formatHexAddr,
  groupByRegion,
  isBlocked,
  isFlashable,
  isSelectable,
  safetyLabel,
  safetyTone,
} from "@/lib/scatter";
import type { SafetyTone } from "@/lib/scatter";
import type { ScatterPartition } from "@/types/api";

interface FlasherPartitionTableProps {
  parts: ScatterPartition[];
  excluded: Set<string>;
  onToggle: (name: string) => void;
  onToggleAll: (included: boolean) => void;
}

const DOT_CLASSES: Record<SafetyTone, string> = {
  danger: "bg-signal-red",
  identity: "bg-signal-amber",
  boot: "bg-trace-copper",
  muted: "bg-muted-foreground/40",
};

const COLUMN_COUNT = 6;

export default function FlasherPartitionTable({
  parts,
  excluded,
  onToggle,
  onToggleAll,
}: FlasherPartitionTableProps) {
  const groups = useMemo(() => groupByRegion(parts), [parts]);

  const selectableCount = useMemo(
    () => parts.filter(isSelectable).length,
    [parts],
  );
  const includedCount = useMemo(
    () =>
      parts.filter((p) => isSelectable(p) && !excluded.has(p.name)).length,
    [parts, excluded],
  );
  const allIncluded = selectableCount > 0 && includedCount === selectableCount;
  const someIncluded = includedCount > 0 && includedCount < selectableCount;

  return (
    <div>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-5 py-2 text-caption text-muted-foreground">
        {SAFETY_LEGEND.map(({ tone, label }) => (
          <span key={tone} className="flex items-center gap-1.5">
            <span className={cn("size-1.5 rounded-full", DOT_CLASSES[tone])} />
            {label}
          </span>
        ))}
        <span className="ml-auto">Identity and dangerous partitions are skipped</span>
      </div>
      <Table containerClassName="max-h-[32rem] overflow-y-auto">
          <TableHeader>
            <TableRow>
              <TableHead className="sticky top-0 z-20 w-10 bg-card">
                <Checkbox
                  checked={allIncluded}
                  indeterminate={someIncluded}
                  onCheckedChange={(c) => onToggleAll(Boolean(c))}
                  disabled={selectableCount === 0}
                  aria-label="Select all flashable partitions"
                />
              </TableHead>
              <TableHead className="sticky top-0 z-20 bg-card text-right">
                Addr
              </TableHead>
              <TableHead className="sticky top-0 z-20 bg-card">Partition</TableHead>
              <TableHead className="sticky top-0 z-20 bg-card text-right">
                Size
              </TableHead>
              <TableHead className="sticky top-0 z-20 bg-card">Type</TableHead>
              <TableHead className="sticky top-0 z-20 bg-card">Image</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {groups.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={COLUMN_COUNT}
                  className="py-6 text-center text-label text-muted-foreground"
                >
                  No partitions
                </TableCell>
              </TableRow>
            ) : (
              groups.map((group) => {
                const groupFlashable = group.parts.filter(isSelectable).length;
                return (
                  <Fragment key={group.region}>
                    <TableRow className="hover:bg-transparent">
                      <TableCell
                        colSpan={COLUMN_COUNT}
                        className="sticky top-10 z-10 border-t border-border/40 bg-card px-5 py-1.5"
                      >
                        <div className="flex items-baseline justify-between gap-4">
                          <span className="font-display text-caption font-medium uppercase tracking-wider text-foreground/80">
                            {group.region}
                          </span>
                          <span className="text-caption text-muted-foreground tabular-nums">
                            {groupFlashable} flashable
                          </span>
                        </div>
                      </TableCell>
                    </TableRow>
                    {group.parts.map((part) => {
                      const flashable = isFlashable(part);
                      const blocked = isBlocked(part);
                      const disabled = !flashable || blocked;
                      const isExcluded = excluded.has(part.name);
                      const checked = !disabled && !isExcluded;
                      const tone = safetyTone(part);
                      return (
                        <TableRow
                          key={`${group.region}:${part.name}`}
                          className={cn(
                            !disabled && isExcluded && "opacity-55",
                            disabled && "opacity-40",
                          )}
                        >
                          <TableCell>
                            <Checkbox
                              checked={checked}
                              onCheckedChange={() => onToggle(part.name)}
                              disabled={disabled}
                              aria-label={`Include ${part.name}`}
                            />
                          </TableCell>
                          <TableCell className="text-right font-mono text-caption text-muted-foreground tabular-nums">
                            {formatHexAddr(part.linear_start)}
                          </TableCell>
                          <TableCell>
                            <span
                              className="flex items-center gap-2 font-mono text-label"
                              title={`${part.name} — ${safetyLabel(part.safety_class)}`}
                            >
                              <span
                                className={cn(
                                  "size-1.5 shrink-0 rounded-full",
                                  DOT_CLASSES[tone],
                                )}
                              />
                              {part.name}
                            </span>
                          </TableCell>
                          <TableCell className="text-right font-mono text-label tabular-nums">
                            {formatBytes(part.size)}
                          </TableCell>
                          <TableCell className="text-label text-muted-foreground">
                            {part.type ?? "—"}
                          </TableCell>
                          <TableCell className="max-w-64 truncate font-mono text-label text-muted-foreground">
                            {part.file_name ?? "—"}
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </Fragment>
                );
              })
            )}
          </TableBody>
        </Table>
    </div>
  );
}
