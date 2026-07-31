import { Checkbox } from "@/components/ui/checkbox";
import type { PartitionRow } from "@/types/api";

interface PartitionTableProps {
  partitions: PartitionRow[];
  selected: ReadonlySet<string>;
  selectAllChecked: boolean;
  selectAllIndeterminate: boolean;
  onToggle: (name: string) => void;
  onToggleAll: () => void;
}

function humanSize(bytes: number): string {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export default function PartitionTable({
  partitions,
  selected,
  selectAllChecked,
  selectAllIndeterminate,
  onToggle,
  onToggleAll,
}: PartitionTableProps) {
  return (
    <div className="overflow-x-auto rounded-md border border-border/80 bg-card/96">
      <table className="w-full min-w-[34rem] text-left text-sm">
        <thead>
          <tr className="border-b border-border/80 text-caption uppercase tracking-wider text-muted-foreground">
            <th className="w-10 px-3 py-2">
              <Checkbox
                checked={selectAllChecked}
                indeterminate={selectAllIndeterminate}
                onCheckedChange={onToggleAll}
                aria-label="Select all partitions"
              />
            </th>
            <th className="px-3 py-2 font-medium">Partition</th>
            <th className="px-3 py-2 text-right font-medium">Size</th>
            <th className="px-3 py-2 font-medium">Type</th>
            <th className="px-3 py-2 font-medium">Images</th>
          </tr>
        </thead>
        <tbody>
          {partitions.map((part) => (
            <tr
              key={part.name}
              className="border-b border-border/40 last:border-0"
            >
              <td className="px-3 py-1.5">
                <Checkbox
                  checked={selected.has(part.name)}
                  disabled={!part.flashable}
                  onCheckedChange={() => onToggle(part.name)}
                  aria-label={`Select ${part.name}`}
                />
              </td>
              <td className="px-3 py-1.5 font-mono text-label">
                {part.name}
              </td>
              <td className="px-3 py-1.5 text-right text-label tabular-nums text-muted-foreground">
                {humanSize(part.size)}
              </td>
              <td className="px-3 py-1.5 text-label text-muted-foreground">
                {part.imageType ?? "—"}
              </td>
              <td className="px-3 py-1.5 font-mono text-label text-muted-foreground">
                {part.fileName ?? "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
