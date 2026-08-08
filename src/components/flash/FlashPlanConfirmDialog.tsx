import { memo } from "react";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { createDismissibleDialogRootHandler } from "@/components/shared/dialogBehavior";
import type { PartitionRow } from "@/types/api";
import type { RebootTarget } from "@/lib/reboot";

interface FlashPlanConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void | Promise<void>;
  selectedPartitions: PartitionRow[];
  rebootTargetAfter?: RebootTarget | null;
  skippedCount?: number;
  isPending?: boolean;
}

const rebootNotices: Record<RebootTarget, string> = {
  system: "The device will reboot to the system after flashing completes.",
  bootloader: "The device will reboot to the bootloader after flashing completes.",
  fastbootd: "The device will reboot to fastbootd after flashing completes.",
  recovery: "The device will reboot into recovery after flashing completes.",
};

export const FlashPlanConfirmDialog = memo(function FlashPlanConfirmDialog({
  open,
  onOpenChange,
  onConfirm,
  selectedPartitions,
  rebootTargetAfter = null,
  skippedCount = 0,
  isPending = false,
}: FlashPlanConfirmDialogProps) {
  const rebootNotice = rebootTargetAfter ? rebootNotices[rebootTargetAfter] : "";
  const selectedCount = selectedPartitions.length;

  return (
    <Dialog open={open} onOpenChange={createDismissibleDialogRootHandler(onOpenChange)}>
      <DialogContent
        className="w-[min(34rem,calc(100vw-1rem))] !max-w-none gap-4 bg-background text-foreground sm:!max-w-none"
        showCloseButton={false}
      >
        <DialogHeader>
          <DialogTitle>Confirm flash plan</DialogTitle>
        </DialogHeader>

        <div className="grid grid-cols-2 gap-2">
          <SummaryCard
            label="Selected"
            value={`${selectedCount} partition${selectedCount === 1 ? "" : "s"}`}
          />
          <SummaryCard
            label="Skipped"
            value={`${skippedCount} partition${skippedCount === 1 ? "" : "s"}`}
          />
        </div>

        {rebootNotice && (
          <p className="rounded-md border border-border/70 bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
            {rebootNotice}
          </p>
        )}

        <DialogFooter className="items-stretch sm:items-center">
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isPending}
            className="w-full sm:w-auto"
          >
            Cancel
          </Button>
          <Button onClick={onConfirm} disabled={isPending} className="w-full sm:w-auto">
            {isPending ? "Starting..." : "Flash"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

function SummaryCard({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="rounded-md border border-border/70 bg-muted/20 px-3 py-2">
      <p className="text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground">{label}</p>
      <div className="mt-1 text-sm font-semibold leading-5 text-foreground">{value}</div>
    </div>
  );
}
