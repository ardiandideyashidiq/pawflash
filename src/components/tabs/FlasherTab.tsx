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
  const {
    scatterPath,
    loadScatter,
    plan,
    loading,
    error,
    options,
    setAdvanced,
    setIncludePreloader,
    setRebootRecovery,
    togglePartition,
    toggleAllPartitions,
    allSelected,
    someSelected,
    selectedFlashCount,
    rows,
  } = useFlashPlan();

  return (
    <div className="flex min-h-full min-h-0 flex-col gap-4 lg:gap-6">
      <ScatterPicker path={scatterPath} onChange={loadScatter} />

      <FlashOptions
        advanced={options.advanced}
        onAdvancedChange={setAdvanced}
        includePreloader={options.includePreloader}
        onIncludePreloaderChange={setIncludePreloader}
        rebootRecovery={options.rebootRecovery}
        onRebootRecoveryChange={setRebootRecovery}
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
        <p className="flex items-start gap-2 rounded-md border border-error/20 bg-error/8 px-3 py-2 text-sm leading-6 text-error">
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
        <div className="grid grid-cols-2 items-center gap-3 lg:grid-cols-4 lg:gap-4">
          <SummaryCard label="Chipset" value={plan?.chipset ?? "—"} />
          <SummaryCard label="Storage" value={plan?.storage ?? "—"} />
          <SummaryCard
            label="Actions"
            value={
              plan ? (
                <span className="inline-flex items-center gap-1.5">
                  <Badge variant="success" className="px-2 py-0">
                    F {selectedFlashCount}
                  </Badge>
                </span>
              ) : loading ? (
                "Parsing..."
              ) : (
                "—"
              )
            }
            accent
          />
          <div className="flex overflow-hidden">
            <FlashFab onClick={onStartFlash} disabled={flashDisabled} />
          </div>
        </div>
        {!connected && (
          <p className="mt-3 text-xs text-muted-foreground">
            No fastboot device connected — flash will wait for a device.
          </p>
        )}
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
