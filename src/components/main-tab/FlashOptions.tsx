import { memo, useState } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

interface FlashOptionsProps {
  advanced: boolean;
  onAdvancedChange: (v: boolean) => void;
  includePreloader: boolean;
  onIncludePreloaderChange: (v: boolean) => void;
  rebootRecovery: boolean;
  onRebootRecoveryChange: (v: boolean) => void;
}

export const FlashOptions = memo(function FlashOptions({
  advanced,
  onAdvancedChange,
  includePreloader,
  onIncludePreloaderChange,
  rebootRecovery,
  onRebootRecoveryChange,
}: FlashOptionsProps) {
  const [advancedOpen, setAdvancedOpen] = useState(false);

  return (
    <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:gap-4">
      <div className="flex min-w-0 flex-1 items-center gap-3 rounded-md border border-border/70 bg-background/70 px-3 py-2">
        <Checkbox
          id="reboot-recovery"
          checked={rebootRecovery}
          onCheckedChange={(v) => onRebootRecoveryChange(!!v)}
        />
        <Label htmlFor="reboot-recovery">Reboot into recovery after flash</Label>
      </div>

      <Button
        type="button"
        variant={advanced ? "secondary" : "outline"}
        className={cn("gap-2", includePreloader && "animate-pulse")}
        onClick={() => {
          onAdvancedChange(true);
          setAdvancedOpen(true);
        }}
      >
        Advanced
      </Button>

      <Dialog open={advancedOpen} onOpenChange={setAdvancedOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Advanced plan filters</DialogTitle>
          </DialogHeader>

          <div className="flex items-center gap-3">
            <Checkbox
              id="advanced-include-preloader"
              checked={includePreloader}
              onCheckedChange={(v) => {
                onAdvancedChange(true);
                onIncludePreloaderChange(!!v);
              }}
            />
            <Label htmlFor="advanced-include-preloader">Include preloader</Label>
          </div>

          <DialogFooter className="sm:justify-between">
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                onAdvancedChange(false);
                onIncludePreloaderChange(false);
                setAdvancedOpen(false);
              }}
            >
              Reset
            </Button>
            <Button type="button" onClick={() => setAdvancedOpen(false)}>
              Done
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
});
