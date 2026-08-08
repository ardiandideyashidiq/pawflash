import { AlertTriangle, XCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { ScatterPicker } from "@/components/main-tab/ScatterPicker";
import { FlashOptions } from "@/components/main-tab/FlashOptions";
import { PartitionTable } from "@/components/main-tab/PartitionTable";
import { FlashFab } from "@/components/main-tab/FlashFab";
import { useFlashPlan } from "@/hooks/useFlashPlan";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

interface FlasherTabProps {
  connected: boolean;
  onStartFlash: () => void;
  flashDisabled: boolean;
}

function FlashTabInner({
  connected,
  onStartFlash,
  flashDisabled,
}: FlasherTabProps) {
  void connected;
  const {
    scatterPath,
    loadScatter,
    clearScatter,
    plan,
    loading,
    error,
    options,
    setIncludePreloader,
    setRebootTarget,
    togglePartition,
    toggleAllPartitions,
    allSelected,
    someSelected,
    selectedFlashCount,
    rows,
  } = useFlashPlan();

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 lg:gap-6">
      <ScatterPicker path={scatterPath} onChange={loadScatter} />

      <FlashOptions
        includePreloader={options.includePreloader}
        onIncludePreloaderChange={setIncludePreloader}
        rebootTarget={options.rebootTarget}
        onRebootTargetChange={setRebootTarget}
        onClear={clearScatter}
        clearDisabled={!scatterPath}
      />

      <PartitionTable
        className="min-h-0 flex-1"
        partitions={rows}
        loading={loading}
        onToggle={togglePartition}
        onToggleAll={toggleAllPartitions}
        allSelected={allSelected}
        someSelected={someSelected}
      />

      {error && (
        <p className="flex shrink-0 items-start gap-2 rounded-md border border-error/20 bg-error/8 px-3 py-2 text-sm leading-6 text-error">
          <XCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {error}
        </p>
      )}

      {plan && (plan.warnings.length > 0 || plan.errors.length > 0) && (
        <div className="shrink-0 space-y-2 px-2 text-sm text-muted-foreground">
          {plan.warnings.map((warning, index) => (
            <p key={index} className="flex items-start gap-2 leading-6 text-warning">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              {warning}
            </p>
          ))}
          {plan.errors.map((error, index) => (
            <p key={index} className="flex items-start gap-2 leading-6 text-error">
              <XCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              {error}
            </p>
          ))}
        </div>
      )}

      <div className="panel-shell shrink-0 px-5 py-4 sm:px-6 sm:py-5">
        <div className="grid grid-cols-2 items-stretch gap-3 lg:grid-cols-4 lg:gap-4">
          <SummaryCard label="Chipset" value={plan?.chipset ?? "—"} />
          <SummaryCard label="Project" value={plan?.project ?? "—"} />
          <div className="panel-inset flex h-12 items-center justify-around px-3">
            <div className="flex flex-col items-center gap-0.5">
              <span className="text-[10px] leading-tight font-medium uppercase tracking-wider text-muted-foreground">
                Flash
              </span>
              <div className="text-sm leading-tight font-semibold">
                {plan ? (
                  <Badge variant="success" className="px-2 py-0">
                    {selectedFlashCount}
                  </Badge>
                ) : loading ? (
                  "..."
                ) : (
                  "—"
                )}
              </div>
            </div>
            <div className="h-6 w-px bg-border/60" />
            <div className="flex flex-col items-center gap-0.5">
              <span className="text-[10px] leading-tight font-medium uppercase tracking-wider text-muted-foreground">
                Skip
              </span>
              <div className="text-sm leading-tight font-semibold">
                {plan ? (
                  <Badge variant="warning" className="px-2 py-0">
                    {plan.skippedCount}
                  </Badge>
                ) : loading ? (
                  "..."
                ) : (
                  "—"
                )}
              </div>
            </div>
          </div>
          <div className="flex overflow-hidden">
            <FlashFab onClick={onStartFlash} disabled={flashDisabled} />
          </div>
        </div>
      </div>
    </div>
  );
}

function SummaryCard({
  label,
  value,
  accent = false,
  className,
}: {
  label: string;
  value: ReactNode;
  accent?: boolean;
  className?: string;
}) {
  return (
    <div className={cn("panel-inset flex h-12 flex-col justify-center gap-0.5 px-3", className)}>
      <p className="text-[11px] leading-tight font-medium uppercase tracking-[0.12em] text-muted-foreground">
        {label}
      </p>
      <div
        className={
          accent
            ? "text-sm leading-tight font-semibold text-trace-copper"
            : "text-sm leading-tight font-semibold"
        }
      >
        {value}
      </div>
    </div>
  );
}

export default function FlasherTab(props: FlasherTabProps) {
  return <FlashTabInner {...props} />;
}
