import { memo, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { ShieldOff, Zap } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/shared/ConfirmDialog";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useConsole } from "@/hooks/useConsole";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import type { ProgressEvent } from "@/types/progress";

interface DeviceSectionProps {
  onForceFastboot: () => void;
  forceFastbootDisabled?: boolean;
  disableVbmetaDisabled?: boolean;
  disabled?: boolean;
}

export const DeviceSection = memo(function DeviceSection({
  onForceFastboot,
  forceFastbootDisabled = false,
  disableVbmetaDisabled = false,
  disabled = false,
}: DeviceSectionProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const { addProgressEvent } = useConsole();
  const { reset } = useFlashProgress();

  const disableVbmeta = async () => {
    setBusy(true);
    try {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = addProgressEvent;
      await invoke("disable_vbmeta", { onEvent: channel });
      reset();
      toast.success("Vbmeta disabled");
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy(false);
      setOpen(false);
    }
  };

  return (
    <SectionCard title="Device" contentClassName="space-y-3">
      <Button
        className="w-full justify-start gap-3"
        disabled={disabled || busy || forceFastbootDisabled}
        onClick={onForceFastboot}
      >
        <Zap className="h-4 w-4" />
        Force reboot fastboot
      </Button>
      <Button
        variant="outline"
        className="w-full justify-start gap-3"
        disabled={busy || disableVbmetaDisabled}
        onClick={() => setOpen(true)}
      >
        <ShieldOff className="h-4 w-4" />
        Disable Vbmeta
      </Button>

      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        title="Disable Vbmeta"
        destructive
        confirmLabel="Disable"
        isPending={busy}
        onConfirm={disableVbmeta}
      />
    </SectionCard>
  );
});
