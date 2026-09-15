import { memo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { Lock, LockOpen, Power, RotateCcw, Skull } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface ServicePanelProps {
  daInstalled: boolean;
  disabled?: boolean;
}

export const ServicePanel = memo(function ServicePanel({
  daInstalled,
  disabled = false,
}: ServicePanelProps) {
  const { simulate } = useSimulation();
  const flash = useFlashProgress();
  const { addEntry, addProgressEvent } = useConsole();

  const [bootMode, setBootMode] = useState<string>("normal");
  const [busy, setBusy] = useState(false);

  const handleSeccfg = async (unlock: boolean) => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    const actionName = unlock ? "UNLOCK" : "LOCK";
    const confirmed = window.confirm(
      `Are you sure you want to ${actionName} the bootloader via seccfg?\n\n` +
      (unlock
        ? "WARNING: Unlocking will wipe all user data on initial boot and allow custom kernel/recovery execution."
        : "Locking requires verified vendor boot, recovery, and vbmeta images.")
    );
    if (!confirmed) return;

    setBusy(true);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    addEntry({ text: `Penumbra Seccfg Start unlock=${unlock}`, level: "command" });

    try {
      await invoke("penumbra_seccfg", {
        unlock,
        onEvent: channel,
        simulate,
      });
      toast.success(`Bootloader ${unlock ? "unlocked" : "locked"} successfully`);
      addEntry({ text: `Penumbra Seccfg Success unlock=${unlock}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Operation failed: ${msg}`);
      addEntry({ text: `Penumbra Seccfg Error: ${msg}`, level: "error" });
    } finally {
      setBusy(false);
    }
  };

  const handleCrash = async () => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    const confirmed = window.confirm(
      "Crash Preloader to BootROM?\n\nThis sends a dummy payload to trigger a preloader assertion fault. The SoC will immediately reboot into hardware BootROM mode (USB VID:PID 0x0E8D:0x0003)."
    );
    if (!confirmed) return;

    setBusy(true);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    addEntry({ text: "Penumbra CrashToBootrom Start", level: "command" });

    try {
      await invoke("penumbra_crash", {
        onEvent: channel,
        simulate,
      });
      toast.success("Device triggered into BootROM mode");
      addEntry({ text: "Penumbra CrashToBootrom Success", level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Crash failed: ${msg}`);
      addEntry({ text: `Penumbra CrashToBootrom Error: ${msg}`, level: "error" });
    } finally {
      setBusy(false);
    }
  };

  const handleReboot = async () => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    setBusy(true);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    addEntry({ text: `Penumbra Reboot Start mode=${bootMode}`, level: "command" });

    try {
      await invoke("penumbra_reboot", {
        mode: bootMode,
        onEvent: channel,
        simulate,
      });
      toast.success(`Reboot command sent (${bootMode})`);
      addEntry({ text: `Penumbra Reboot Success mode=${bootMode}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Reboot failed: ${msg}`);
      addEntry({ text: `Penumbra Reboot Error: ${msg}`, level: "error" });
    } finally {
      setBusy(false);
    }
  };

  const handleShutdown = async () => {
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    setBusy(true);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    addEntry({ text: "Penumbra Shutdown Start", level: "command" });

    try {
      await invoke("penumbra_shutdown", {
        onEvent: channel,
        simulate,
      });
      toast.success("Device shutdown command sent");
      addEntry({ text: "Penumbra Shutdown Success", level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Shutdown failed: ${msg}`);
      addEntry({ text: `Penumbra Shutdown Error: ${msg}`, level: "error" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4 max-w-4xl mx-auto w-full pt-2">
      {/* Bootloader Security */}
      <SectionCard title="Bootloader Lock (seccfg)" contentClassName="space-y-3">
        <p className="text-xs text-muted-foreground leading-relaxed">
          Directly writes security configuration to the <code className="text-trace-copper font-mono">seccfg</code> partition in DA mode.
        </p>

        <div className="grid grid-cols-2 gap-3 pt-2">
          <Button
            variant="outline"
            disabled={disabled || busy || !daInstalled}
            onClick={() => void handleSeccfg(true)}
            className="gap-2 border-signal-amber/40 hover:bg-signal-amber/10 hover:text-signal-amber text-xs"
          >
            <LockOpen className="h-4 w-4 text-signal-amber" />
            Unlock Bootloader
          </Button>

          <Button
            variant="outline"
            disabled={disabled || busy || !daInstalled}
            onClick={() => void handleSeccfg(false)}
            className="gap-2 text-xs"
          >
            <Lock className="h-4 w-4" />
            Lock Bootloader
          </Button>
        </div>
      </SectionCard>

      {/* Low-level BootROM Crash */}
      <SectionCard title="SoC Low-level Recovery" contentClassName="space-y-3">
        <p className="text-xs text-muted-foreground leading-relaxed">
          Force the SoC from Preloader into hardware BootROM mode (USB VID:PID 0x0E8D:0x0003) via assertion panic.
        </p>

        <Button
          variant="outline"
          disabled={disabled || busy || !daInstalled}
          onClick={() => void handleCrash()}
          className="w-full gap-2 border-error/40 hover:bg-error/10 hover:text-error text-xs mt-2"
        >
          <Skull className="h-4 w-4 text-error" />
          Crash Preloader to BootROM
        </Button>
      </SectionCard>

      {/* MediaTek Power & Reboot Modes */}
      <SectionCard title="MediaTek Power & Reboot Modes" className="md:col-span-2" contentClassName="space-y-4">
        <p className="text-xs text-muted-foreground leading-relaxed">
          Instruct the Download Agent to reset the device into specialized MediaTek hardware execution environments.
        </p>

        <div className="flex flex-wrap items-center gap-3">
          <select
            value={bootMode}
            onChange={(e) => setBootMode(e.target.value)}
            disabled={disabled || busy || !daInstalled}
            aria-label="Reboot target mode"
            className="h-9 rounded-md border border-input bg-background/80 px-3 text-xs ring-offset-background focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          >
            <option value="normal">Normal Boot (System OS)</option>
            <option value="fastboot">Fastboot Mode</option>
            <option value="recovery">Recovery Mode</option>
            <option value="meta">Meta Mode (Baseband Calibration)</option>
            <option value="test">Factory / Test Mode</option>
          </select>

          <Button
            variant="outline"
            disabled={disabled || busy || !daInstalled}
            onClick={() => void handleReboot()}
            className="gap-2 text-xs"
          >
            <RotateCcw className="h-3.5 w-3.5" />
            Reboot Device
          </Button>

          <Button
            variant="outline"
            disabled={disabled || busy || !daInstalled}
            onClick={() => void handleShutdown()}
            className="gap-2 text-xs hover:text-error hover:bg-error/10 border-error/30"
          >
            <Power className="h-3.5 w-3.5 text-error" />
            Shutdown Device
          </Button>
        </div>
      </SectionCard>
    </div>
  );
});
