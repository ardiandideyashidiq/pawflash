import { useState } from "react";
import { Lock, LockOpen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/shared/ConfirmDialog";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useDevice } from "@/hooks/useDevice";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage } from "@/types/api";

export function BootloaderSection({ disabled = false }: { disabled?: boolean }) {
  const { unlockBootloader, lockBootloader } = useDevice();
  const { addEntry } = useConsole();
  const [unlockOpen, setUnlockOpen] = useState(false);
  const [lockOpen, setLockOpen] = useState(false);
  const [unlockSuccess, setUnlockSuccess] = useState(false);
  const [lockSuccess, setLockSuccess] = useState(false);
  const [busy, setBusy] = useState(false);

  const handleOpenUnlock = (open: boolean) => {
    setUnlockOpen(open);
    if (!open) {
      setUnlockSuccess(false);
    }
  };

  const handleOpenLock = (open: boolean) => {
    setLockOpen(open);
    if (!open) {
      setLockSuccess(false);
    }
  };

  const runUnlock = async () => {
    setBusy(true);
    addEntry({ text: "BootloaderUnlock Started", level: "info" });
    try {
      await unlockBootloader();
      addEntry({ text: "BootloaderUnlock Complete", level: "success" });
      toast.success("Bootloader unlocked");
      setUnlockSuccess(true);
    } catch (e) {
      addEntry({ text: `BootloaderUnlock Error ${errorMessage(e)}`, level: "error" });
      toast.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const runLock = async () => {
    setBusy(true);
    addEntry({ text: "BootloaderLock Started", level: "info" });
    try {
      await lockBootloader();
      addEntry({ text: "BootloaderLock Complete", level: "success" });
      toast.success("Bootloader locked");
      setLockSuccess(true);
    } catch (e) {
      addEntry({ text: `BootloaderLock Error ${errorMessage(e)}`, level: "error" });
      toast.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <SectionCard title="Bootloader" contentClassName="grid grid-cols-2 gap-3">
      <Button
        variant="destructive"
        className="w-full justify-center gap-2.5 px-4 py-2"
        disabled={disabled || busy}
        onClick={() => handleOpenUnlock(true)}
      >
        <LockOpen className="h-4 w-4 shrink-0" />
        Unlock
      </Button>
      <Button
        variant="outline"
        className="w-full justify-center gap-2.5 px-4 py-2"
        disabled={disabled || busy}
        onClick={() => handleOpenLock(true)}
      >
        <Lock className="h-4 w-4 shrink-0" />
        Lock
      </Button>

      <ConfirmDialog
        open={unlockOpen}
        onOpenChange={handleOpenUnlock}
        title="Unlock Bootloader"
        description="Unlocking the bootloader allows flashing custom images and modifying partitions, but may wipe user data."
        destructive
        confirmLabel="Unlock"
        isPending={busy}
        isSuccess={unlockSuccess}
        successMessage="Bootloader unlocked successfully."
        onConfirm={runUnlock}
      />
      <ConfirmDialog
        open={lockOpen}
        onOpenChange={handleOpenLock}
        title="Lock Bootloader"
        description="Locking the bootloader enforces signature verification and may wipe user data."
        confirmLabel="Lock"
        isPending={busy}
        isSuccess={lockSuccess}
        successMessage="Bootloader locked successfully."
        onConfirm={runLock}
      />
    </SectionCard>
  );
}
