import { memo } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";

interface FlashOptionsProps {
  includePreloader: boolean;
  onIncludePreloaderChange: (v: boolean) => void;
  rebootRecovery: boolean;
  onRebootRecoveryChange: (v: boolean) => void;
  onClear: () => void;
  clearDisabled?: boolean;
}

export const FlashOptions = memo(function FlashOptions({
  includePreloader,
  onIncludePreloaderChange,
  rebootRecovery,
  onRebootRecoveryChange,
  onClear,
  clearDisabled = false,
}: FlashOptionsProps) {
  return (
    <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:gap-4">
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-3">
        <div className="flex min-w-0 flex-1 items-center gap-3 rounded-md border border-border/70 bg-background/70 px-3 py-2 lg:flex-none">
          <Checkbox
            id="reboot-recovery"
            checked={rebootRecovery}
            onCheckedChange={(v) => onRebootRecoveryChange(!!v)}
          />
          <Label htmlFor="reboot-recovery">Reboot into recovery after flash</Label>
        </div>

        <div className="flex min-w-0 flex-1 items-center gap-3 rounded-md border border-border/70 bg-background/70 px-3 py-2 lg:flex-none">
          <Checkbox
            id="include-preloader"
            checked={includePreloader}
            onCheckedChange={(v) => onIncludePreloaderChange(!!v)}
          />
          <Label htmlFor="include-preloader">Include preloader</Label>
        </div>
      </div>

      <Button
        type="button"
        variant="outline"
        onClick={onClear}
        disabled={clearDisabled}
        className="gap-2"
      >
        Clear
      </Button>
    </div>
  );
});
