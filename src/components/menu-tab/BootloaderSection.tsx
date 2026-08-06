import { useState } from "react";
import { Lock, LockOpen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/shared/ConfirmDialog";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useDevice } from "@/hooks/useDevice";
import { useConsole } from "@/hooks/useConsole";

export function BootloaderSection({ disabled = false }: { disabled?: boolean }) {
  const { unlockBootloader, lockBootloader } = useDevice();
  const { addEntry } = useConsole();
  const [unlockOpen, setUnlockOpen] = useState(false);
  const [lockOpen, setLockOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<string>, action: string, successMsg: string) => {
    setBusy(true);
    addEntry({ text: `${action} Started`, level: "info" });
    try {
      await fn();
      addEntry({ text: `${action} Complete`, level: "success" });
      toast.success(successMsg);
    } catch (e) {
      addEntry({ text: `${action} Error ${e}`, level: "error" });
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <SectionCard title="Bootloader" contentClassName="grid grid-cols-2 gap-3">
      <Button
        variant="destructive"
        className="w-full justify-start gap-3"
        disabled={disabled || busy}
        onClick={() => setUnlockOpen(true)}
      >
        <LockOpen className="h-4 w-4" />
        Unlock
      </Button>
      <Button
        variant="outline"
        className="w-full justify-start gap-3"
        disabled={disabled || busy}
        onClick={() => setLockOpen(true)}
      >
        <Lock className="h-4 w-4" />
        Lock
      </Button>

      <ConfirmDialog
        open={unlockOpen}
        onOpenChange={setUnlockOpen}
        title="Unlock Bootloader"
        destructive
        confirmLabel="Unlock"
        isPending={busy}
        onConfirm={() => run(unlockBootloader, "BootloaderUnlock", "Bootloader unlocked")}
      />
      <ConfirmDialog
        open={lockOpen}
        onOpenChange={setLockOpen}
        title="Lock Bootloader"
        confirmLabel="Lock"
        isPending={busy}
        onConfirm={() => run(lockBootloader, "BootloaderLock", "Bootloader locked")}
      />
    </SectionCard>
  );
}
