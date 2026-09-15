import { memo, useEffect, useMemo, useState } from "react";
import { Menu } from "@base-ui/react/menu";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronDown,
  Loader2,
  RotateCcw,
} from "lucide-react";
import { toast } from "sonner";
import { useFlashPhase } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { useSimulation } from "@/hooks/useSimulation";
import { cn } from "@/lib/utils";
import { errorMessage } from "@/types/api";
import { rebootTargets, mtkRebootTargets, targetMeta, type RebootTarget } from "@/lib/reboot";

const successLabels: Record<RebootTarget, string> = {
  system: "Rebooted to system",
  bootloader: "Rebooted to bootloader",
  fastbootd: "Rebooted to fastbootd",
  recovery: "Rebooted to recovery",
  "mtk:normal": "Rebooted to system",
  "mtk:fastboot": "Rebooted to fastboot",
  "mtk:recovery": "Rebooted to recovery",
  "mtk:meta": "Rebooted to meta mode",
  "mtk:test": "Rebooted to test mode",
  shutdown: "Shutdown command sent",
};

interface RebootMenuProps {
  disabled?: boolean;
  sidebarOpen: boolean;
  target: RebootTarget | null;
  onTargetChange: (target: RebootTarget | null) => void;
}

export const RebootMenu = memo(function RebootMenu({
  disabled = false,
  sidebarOpen,
  target,
  onTargetChange,
}: RebootMenuProps) {
  const [busy, setBusy] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [deviceMode, setDeviceMode] = useState<string>("fastboot");
  const { simulate } = useSimulation();
  const flash = useFlashPhase();
  const force = useForceFastboot();

  useEffect(() => {
    let cancelled = false;
    const checkMode = async () => {
      try {
        const mode = await invoke<string>("detect_device_mode", { simulate });
        if (!cancelled) setDeviceMode(mode);
      } catch {
        if (!cancelled) setDeviceMode("fastboot");
      }
    };
    void checkMode();
    const interval = setInterval(() => void checkMode(), 3500);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [simulate]);

  const sessionLive = useMemo(
    () => flash.phase === "waiting" || flash.phase === "flashing" || force.phase === "waiting",
    [flash.phase, force.phase],
  );

  const menuDisabled = disabled || busy || sessionLive;

  const activeTargets = useMemo(() => {
    if (deviceMode === "brom" || deviceMode === "preloader") {
      return mtkRebootTargets;
    }
    return rebootTargets;
  }, [deviceMode]);

  const handleReboot = async (nextTarget: RebootTarget) => {
    if (menuDisabled) return;
    onTargetChange(nextTarget);
    setBusy(true);
    try {
      await invoke("reboot_device", { target: nextTarget, simulate });
      toast.success(successLabels[nextTarget]);
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (menuDisabled) {
      setMenuOpen(false);
    }
  }, [menuDisabled]);

  return (
    <Menu.Root disabled={menuDisabled} open={menuOpen} onOpenChange={setMenuOpen}>
      <Menu.Trigger
        className={cn(
          "flex w-full items-center overflow-hidden rounded-md border border-border/80 bg-card text-sm font-medium shadow-[var(--panel-shadow)] transition-all duration-200 ease-out hover:border-trace-copper/40 hover:bg-accent-soft/80 focus-visible:ring-2 focus-visible:ring-trace-copper/50 disabled:cursor-not-allowed disabled:opacity-50",
          sidebarOpen ? "justify-start gap-2.5 px-3 py-2" : "justify-center gap-1.5 px-0 py-2",
          busy && "cursor-wait",
        )}
        disabled={menuDisabled}
        aria-label="Reboot menu"
        title="Reboot menu"
      >
        {busy ? (
          <Loader2 className="h-4 w-4 shrink-0 animate-spin text-trace-copper" />
        ) : (
          <RotateCcw className="h-4 w-4 shrink-0 text-trace-copper" />
        )}
        <span className={cn("truncate font-medium text-foreground", !sidebarOpen && "sr-only")}>
          Reboot
        </span>
        <ChevronDown
          className={cn(
            "h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-out",
            menuOpen && "rotate-180",
            sidebarOpen ? "ml-auto" : "ml-0",
          )}
        />
      </Menu.Trigger>

      <Menu.Portal>
        <Menu.Positioner side="right" align="start" sideOffset={8} className="isolate z-50">
          <Menu.Popup className="z-50 w-56 rounded-lg border border-border/80 bg-popover p-1.5 text-popover-foreground shadow-xl outline-none space-y-0.5">
            <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground border-b border-border/60 mb-1">
              {deviceMode === "brom" || deviceMode === "preloader" ? `MediaTek (${deviceMode})` : "Fastboot Modes"}
            </div>
            {activeTargets.map((targetKey) => {
              const meta = targetMeta[targetKey];
              const isSelected = target === targetKey;

              return (
                <Menu.Item
                  key={targetKey}
                  data-selected={isSelected || undefined}
                  className={cn(
                    "group flex w-full cursor-pointer items-center justify-between rounded-md px-3 py-2 text-sm font-medium text-foreground outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
                    isSelected && "bg-accent-soft/70",
                  )}
                  closeOnClick
                  onClick={() => {
                    void handleReboot(targetKey);
                  }}
                >
                  <span className="truncate">{meta.label}</span>
                  {isSelected && <Check className="h-4 w-4 shrink-0 text-trace-copper" />}
                </Menu.Item>
              );
            })}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
});
