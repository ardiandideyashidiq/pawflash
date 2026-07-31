import { useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { useConsole } from "@/hooks/useConsole";
import type { ConfirmAction, DeviceInfo } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";
import { Cpu, LoaderCircle, Lock, ShieldOff, Unlock, Zap } from "lucide-react";

interface MenuTabProps {
  device: DeviceInfo | null;
  onRefresh: () => Promise<void>;
}

export default function MenuTab({ device, onRefresh }: MenuTabProps) {
  const { addProgressEvent } = useConsole();
  const [fastbootLoading, setFastbootLoading] = useState(false);
  const [vbmetaLoading, setVbmetaLoading] = useState(false);
  const [lockLoading, setLockLoading] = useState(false);
  const [unlockLoading, setUnlockLoading] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmAction | null>(null);

  const connected = device?.connected ?? false;
  const vars = device?.vars ?? {};

  const forceFastboot = async () => {
    setFastbootLoading(true);
    try {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = addProgressEvent;
      await invoke("force_fastboot", { onEvent: channel });
      await onRefresh();
    } catch (e) {
      toast.error(`Force fastboot failed: ${e}`);
    }
    setFastbootLoading(false);
  };

  const disableVbmeta = async () => {
    setVbmetaLoading(true);
    try {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = addProgressEvent;
      await invoke("disable_vbmeta", { onEvent: channel });
      toast.success("Verified boot disabled");
    } catch (e) {
      toast.error(`Failed to disable verified boot: ${e}`);
    }
    setVbmetaLoading(false);
  };

  const handleLock = async () => {
    setLockLoading(true);
    try {
      await invoke<string>("lock_bootloader");
      toast.success("Bootloader locked");
      await onRefresh();
    } catch (e) {
      toast.error(`Lock failed: ${e}`);
    }
    setLockLoading(false);
  };

  const handleUnlock = async () => {
    setUnlockLoading(true);
    try {
      await invoke<string>("unlock_bootloader");
      toast.success("Bootloader unlocked");
      await onRefresh();
    } catch (e) {
      toast.error(`Unlock failed: ${e}`);
    }
    setUnlockLoading(false);
  };

  const handleSetSlot = async (slot: string) => {
    try {
      await invoke<string>("set_active_slot", { slot });
      toast.success(`Slot ${slot} set`);
      await onRefresh();
    } catch (e) {
      toast.error(`Set slot ${slot} failed: ${e}`);
    }
  };

  return (
    <div className="space-y-5">
      {/* Force fastboot */}
      <section className="panel-shell overflow-hidden">
        <div className="flex items-start gap-5 px-5 py-5">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-md bg-trace-copper/10 text-trace-copper">
            <Zap size={20} />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="text-body font-display font-medium uppercase tracking-wider text-foreground">
              Force Fastboot
            </h2>
            <p className="mt-1 max-w-lg text-label leading-normal text-muted-foreground">
              Force a MediaTek device into fastboot mode via preloader serial
              handshake.
            </p>
            <div className="mt-3 flex items-center gap-3">
              <Button
                variant="accent"
                size="default"
                onClick={() =>
                  setConfirmDialog({
                    title: "Enter Fastboot Mode",
                    description:
                      "This will attempt to force your MediaTek device into fastboot mode via preloader serial handshake. Ensure the device is powered off and connected via USB.",
                    confirmLabel: "Enter Fastboot",
                    variant: "default",
                    onConfirm: forceFastboot,
                  })
                }
                disabled={fastbootLoading}
              >
                {fastbootLoading ? (
                  <>
                    <LoaderCircle size={14} className="animate-spin" /> Connecting...
                  </>
                ) : (
                  "Force Fastboot"
                )}
              </Button>
              <span
                className={`size-1.5 rounded-full transition-colors duration-300 ${connected ? "dot-complete" : "dot-waiting animate-pulse"}`}
              />
              <span className="text-caption text-muted-foreground">
                {connected ? "Device online" : "No device"}
              </span>
            </div>
          </div>
        </div>
        {connected && (
          <div className="flex items-center gap-4 border-t border-border/50 bg-signal-green/[0.06] px-5 py-2.5 text-caption text-muted-foreground/80">
            <span className="font-mono text-trace-copper">
              {device?.serial ?? "—"}
            </span>
            <span className="h-3 w-px bg-border/50" />
            <span>{vars.product ?? "—"}</span>
            {vars["current-slot"] && (
              <>
                <span className="h-3 w-px bg-border/50" />
                <span>slot {vars["current-slot"]}</span>
              </>
            )}
          </div>
        )}
      </section>

      {/* Bootloader */}
      <section className="panel-shell flex items-center justify-between gap-5 px-5 py-3 max-sm:flex-wrap">
        <div className="flex items-center gap-3 min-w-0">
          <Lock size={16} className="shrink-0 text-muted-foreground" />
          <div>
            <p className="text-body font-display font-medium uppercase tracking-wider text-foreground/90">
              Bootloader
            </p>
            <p className="text-caption leading-tight text-muted-foreground/70">
              lock / unlock / verified boot
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <Button
            variant="destructive"
            size="sm"
            onClick={() =>
              setConfirmDialog({
                title: "Disable Verified Boot",
                description:
                  "Disabling dm-verity and AVB will weaken device security verification. This is typically needed only when flashing custom firmware. Continue?",
                confirmLabel: "Disable",
                onConfirm: disableVbmeta,
              })
            }
            disabled={vbmetaLoading || !connected}
          >
            {vbmetaLoading ? (
              <>
                <LoaderCircle size={14} className="animate-spin" /> Working...
              </>
            ) : (
              <>
                <ShieldOff size={14} className="mr-1" /> Disable VBmeta
              </>
            )}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() =>
              setConfirmDialog({
                title: "Lock Bootloader",
                description:
                  "Locking the bootloader will re-enable verified boot. This may prevent flashing custom firmware. Continue?",
                confirmLabel: "Lock",
                onConfirm: handleLock,
              })
            }
            disabled={lockLoading || !connected}
          >
            {lockLoading ? (
              <>
                <LoaderCircle size={14} className="animate-spin" /> Locking...
              </>
            ) : (
              <>
                <Lock size={14} className="mr-1" /> Lock
              </>
            )}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() =>
              setConfirmDialog({
                title: "Unlock Bootloader",
                description:
                  "Unlocking the bootloader will disable verified boot and may wipe user data. Continue?",
                confirmLabel: "Unlock",
                onConfirm: handleUnlock,
              })
            }
            disabled={unlockLoading || !connected}
          >
            {unlockLoading ? (
              <>
                <LoaderCircle size={14} className="animate-spin" /> Unlocking...
              </>
            ) : (
              <>
                <Unlock size={14} className="mr-1" /> Unlock
              </>
            )}
          </Button>
        </div>
      </section>

      {/* Active slot */}
      <section className="panel-shell flex items-center justify-between gap-5 px-5 py-3">
        <div className="flex items-center gap-3 min-w-0">
          <Cpu size={16} className="shrink-0 text-muted-foreground" />
          <span className="text-body font-display font-medium uppercase tracking-wider text-foreground/90">
            Active Slot
          </span>
        </div>
        <div className="flex overflow-hidden rounded-lg border border-border">
          {["a", "b"].map((slot) => (
            <Button
              key={slot}
              variant="ghost"
              size="sm"
              onClick={() => handleSetSlot(slot)}
              disabled={!connected}
              className={`rounded-none ${
                vars["current-slot"] === slot
                  ? "bg-trace-copper text-white hover:bg-trace-gold"
                  : "text-muted-foreground hover:bg-muted/40 hover:text-foreground"
              }`}
            >
              {slot.toUpperCase()}
            </Button>
          ))}
        </div>
      </section>

      {confirmDialog && (
        <ConfirmDialog
          open={!!confirmDialog}
          onOpenChange={(open) => {
            if (!open) setConfirmDialog(null);
          }}
          title={confirmDialog.title}
          description={confirmDialog.description}
          confirmLabel={confirmDialog.confirmLabel}
          variant={confirmDialog.variant ?? "destructive"}
          onConfirm={confirmDialog.onConfirm}
        />
      )}
    </div>
  );
}
