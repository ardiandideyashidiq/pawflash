import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { Toaster, toast } from "sonner";
import { PlugZap } from "lucide-react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import AppLayout from "@/components/layout/AppLayout";
import { LogPanel } from "@/components/console/LogPanel";
import { FlashPlanConfirmDialog } from "@/components/flash/FlashPlanConfirmDialog";
import { FlashDialog } from "@/components/flash/FlashDialog";
import { ForceFastbootDialog } from "@/components/flash/ForceFastbootDialog";
import { RebootMenu, type RebootTarget } from "@/components/sidebar/RebootMenu";
import { UIProvider, useUI } from "@/hooks/useUI";
import { SimulationProvider, useSimulation } from "@/hooks/useSimulation";
import { ConsoleProvider } from "@/components/console/ConsoleContext";
import { useConsole } from "@/hooks/useConsole";
import {
  FlashProgressProvider,
  useFlashPhase,
} from "@/hooks/useFlashProgress";
import {
  ForceFastbootProvider,
  useForceFastboot,
} from "@/hooks/useForceFastboot";
import { FlashPlanProvider, useFlashPlan } from "@/hooks/useFlashPlan";
import { buildFlashPlanOptions } from "@/lib/plan";
import { useDevice } from "@/hooks/useDevice";
import { applyDismissibleDialogChange } from "@/components/shared/dialogBehavior";
import { errorMessage } from "@/types/api";
import type { DeviceInfo, FlashResult } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

const FlasherTab = lazy(() => import("@/components/tabs/FlasherTab"));
const MenuTab = lazy(() => import("@/components/tabs/MenuTab"));
const ExtrasTab = lazy(() => import("@/components/tabs/ExtrasTab"));
const SettingsTab = lazy(() => import("@/components/tabs/SettingsTab"));

const REBOOT_TARGET_STORAGE_KEY = "last-reboot-target";

function isRebootTarget(value: string | null): value is RebootTarget {
  return value === "system" || value === "bootloader" || value === "fastbootd" || value === "recovery";
}

function buildDeviceSummary(info: DeviceInfo) {
  return [
    `serial=${info.serial || "unknown"}`,
    `product=${info.vars.product || "unknown"}`,
    `slot=${info.vars["current-slot"] || "unknown"}`,
    `connected=${info.connected}`,
  ].join(" ");
}

function AppRoot() {
  const { theme, setTheme } = useUI();
  const { simulate } = useSimulation();
  const { addProgressEvent, addEntry } = useConsole();
  const flash = useFlashPhase();
  const force = useForceFastboot();
  const planState = useFlashPlan();
  const device = useDevice();

  const handleThemeChange = useCallback(
    (next: "light" | "dark" | ((current: "light" | "dark") => "light" | "dark")) => {
      setTheme(typeof next === "function" ? next(theme) : next);
    },
    [theme, setTheme],
  );

  const [rebootTarget, setRebootTarget] = useState<RebootTarget | null>(() => {
    if (typeof window === "undefined") return null;
    const stored = window.localStorage.getItem(REBOOT_TARGET_STORAGE_KEY);
    return isRebootTarget(stored) ? stored : null;
  });
  const [deviceInfo, setDeviceInfo] = useState<DeviceInfo | null>(null);
  const [isCheckingDevice, setIsCheckingDevice] = useState(false);
  const [isStartingFlash, setIsStartingFlash] = useState(false);
  const [isCancellingFlash, setIsCancellingFlash] = useState(false);
  const [isCancellingForceFastboot, setIsCancellingForceFastboot] = useState(false);
  const [flashConfirmOpen, setFlashConfirmOpen] = useState(false);
  const [flashOpen, setFlashOpen] = useState(false);
  const [forceOpen, setForceOpen] = useState(false);

  const flashPhaseRef = useRef(flash.phase);
  const forcePhaseRef = useRef(force.phase);

  useEffect(() => {
    flashPhaseRef.current = flash.phase;
  }, [flash.phase]);

  useEffect(() => {
    forcePhaseRef.current = force.phase;
  }, [force.phase]);

  const activeFlashSession = flash.phase === "waiting" || flash.phase === "flashing";
  const activeForceSession = force.phase === "waiting";

  const fetchDevice = useCallback(
    async (notify: boolean) => {
      try {
        const info = await device.check();
        setDeviceInfo(info);
        if (notify) {
          if (info.connected) {
            toast.success(`Connected: ${info.serial || info.vars.product || "device"}`);
          } else {
            toast.info("No fastboot device connected");
          }
        }
        return info;
      } catch (error) {
        const message = errorMessage(error);
        addEntry({ text: `DeviceCheck Error ${message}`, level: "error" });
        if (notify) toast.error(message);
        return null;
      }
    },
    [device, addEntry],
  );

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void fetchDevice(false);
  }, [fetchDevice]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        e.key === "r" &&
        !e.metaKey &&
        !e.ctrlKey &&
        !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) &&
        !(e.target instanceof HTMLElement && e.target.contentEditable === "true")
      ) {
        void fetchDevice(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [fetchDevice]);

  useEffect(() => {
    if (rebootTarget) {
      window.localStorage.setItem(REBOOT_TARGET_STORAGE_KEY, rebootTarget);
    } else {
      window.localStorage.removeItem(REBOOT_TARGET_STORAGE_KEY);
    }
  }, [rebootTarget]);

  useEffect(() => {
    if (flash.phase === "complete" || flash.phase === "cancelled" || flash.phase === "error") {
      const timeoutId = window.setTimeout(() => {
        setIsCancellingFlash(false);
      }, 0);
      return () => window.clearTimeout(timeoutId);
    }
  }, [flash.phase]);

  useEffect(() => {
    if (force.phase === "complete" || force.phase === "cancelled" || force.phase === "error") {
      const timeoutId = window.setTimeout(() => {
        setIsCancellingForceFastboot(false);
      }, 0);
      return () => window.clearTimeout(timeoutId);
    }
  }, [force.phase]);

  const checkDevice = useCallback(async () => {
    const sessionLive = flashPhaseRef.current === "waiting" || flashPhaseRef.current === "flashing";
    if (isCheckingDevice || sessionLive || forcePhaseRef.current === "waiting") {
      return;
    }
    setIsCheckingDevice(true);
    addEntry({ text: "DeviceCheck Started", level: "command" });
    const info = await fetchDevice(true);
    if (info?.connected) {
      addEntry({ text: `DeviceCheck Connected ${buildDeviceSummary(info)}`, level: "success" });
    } else {
      addEntry({ text: "DeviceCheck NoDevice", level: "warning" });
    }
    setIsCheckingDevice(false);
  }, [addEntry, fetchDevice, isCheckingDevice]);

  const startFlash = useCallback(async () => {
    const sessionLive = flashPhaseRef.current === "waiting" || flashPhaseRef.current === "flashing";
    const plan = planState.plan;
    if (
      isStartingFlash ||
      planState.loading ||
      !plan ||
      planState.selectedFlashCount === 0 ||
      sessionLive ||
      forcePhaseRef.current === "waiting"
    ) {
      return;
    }

    flash.reset();
    setFlashOpen(true);
    setIsStartingFlash(true);
    setFlashConfirmOpen(false);

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    try {
      const result = await invoke<FlashResult>("execute_plan", {
        path: planState.scatterPath,
        options: buildFlashPlanOptions(
          planState.buildExclude(),
          planState.options.includePreloader,
          planState.scatterPath,
        ),
        simulate,
        onEvent: channel,
      });
      if (result.cancelled) {
        addEntry({ text: "FlashCancelled", level: "warning" });
      } else if (result.failed === 0 && planState.options.rebootRecovery) {
        addEntry({ text: "Rebooting into recovery after flash", level: "info" });
        toast.info("Rebooting into recovery...");
        try {
          await invoke("reboot_device", { target: "recovery", simulate });
        } catch (error) {
          toast.error(`Reboot failed: ${String(error)}`);
        }
      }
    } catch (error) {
      flash.fail(String(error));
    } finally {
      setIsStartingFlash(false);
    }
  }, [
    addEntry,
    addProgressEvent,
    flash,
    isStartingFlash,
    planState,
    simulate,
  ]);

  const startForceFastboot = useCallback(async () => {
    const sessionLive = flashPhaseRef.current === "waiting" || flashPhaseRef.current === "flashing";
    if (sessionLive || forcePhaseRef.current === "waiting") {
      return;
    }

    force.reset();
    setForceOpen(true);
    addEntry({ text: "ForceFastboot StartRequested", level: "command" });

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      force.onEvent(event);
      addProgressEvent(event);
    };

    try {
      await invoke("force_fastboot", { simulate, onEvent: channel });
    } catch (error) {
      const message = String(error);
      addEntry({ text: `ForceFastboot StartError ${message}`, level: "error" });
      toast.error(message);
      force.reset();
      setForceOpen(false);
    }
  }, [addEntry, addProgressEvent, force, simulate]);

  const startManualFlash = useCallback(
    async (partition: string, image: string) => {
      const sessionLive = flashPhaseRef.current === "waiting" || flashPhaseRef.current === "flashing";
      if (isStartingFlash || sessionLive || forcePhaseRef.current === "waiting") {
        return;
      }

      flash.reset();
      setFlashOpen(true);
      setIsStartingFlash(true);

      const channel = new Channel<ProgressEvent>();
      channel.onmessage = (event) => {
        flash.onEvent(event);
        addProgressEvent(event);
      };

      try {
        await invoke("flash_raw_image", {
          partition,
          imagePath: image,
          simulate,
          onEvent: channel,
        });
      } catch (error) {
        flash.fail(String(error));
        throw error;
      } finally {
        setIsStartingFlash(false);
      }
    },
    [addProgressEvent, flash, isStartingFlash, simulate],
  );

  const cancelFlash = useCallback(async () => {
    const sessionLive = flashPhaseRef.current === "waiting" || flashPhaseRef.current === "flashing";
    if (!sessionLive || isCancellingFlash) return;

    addEntry({ text: "FlashCancelRequested", level: "warning" });
    setIsCancellingFlash(true);
    try {
      await invoke("cancel_flash");
    } catch (error) {
      setIsCancellingFlash(false);
      flash.fail(String(error));
    }
  }, [addEntry, flash, isCancellingFlash]);

  const cancelForceFastboot = useCallback(async () => {
    if (forcePhaseRef.current !== "waiting" || isCancellingForceFastboot) return;

    addEntry({ text: "ForceFastboot CancelRequested", level: "warning" });
    setIsCancellingForceFastboot(true);
    try {
      await invoke("cancel_force_fastboot");
    } catch (error) {
      setIsCancellingForceFastboot(false);
      toast.error(String(error));
    }
  }, [addEntry, isCancellingForceFastboot]);

  const hideFlashDialog = useCallback(() => {
    setFlashOpen(false);
  }, []);

  const hideForceDialog = useCallback(() => {
    setForceOpen(false);
  }, []);

  const menuActionDisabled =
    isStartingFlash || isCheckingDevice || activeFlashSession || activeForceSession;

  const flashDisabled =
    !planState.plan ||
    planState.loading ||
    isStartingFlash ||
    activeFlashSession ||
    activeForceSession ||
    planState.selectedFlashCount === 0;

  const sidebarActions = ({ sidebarOpen }: { sidebarOpen: boolean }) => (
    <div className={cn("space-y-3", !sidebarOpen && "space-y-2")}>
      <RebootMenu
        disabled={menuActionDisabled}
        sidebarOpen={sidebarOpen}
        target={rebootTarget}
        onTargetChange={setRebootTarget}
      />

      <Button
        variant="outline"
        size={sidebarOpen ? "sm" : "icon-sm"}
        className={cn(
          "w-full overflow-hidden",
          sidebarOpen ? "justify-start gap-2" : "justify-center",
        )}
        disabled={isCheckingDevice || activeFlashSession || activeForceSession}
        aria-label="Check Device"
        title="Check Device"
        onClick={checkDevice}
      >
        <PlugZap className="h-4 w-4 shrink-0" />
        <span className={cn("truncate", !sidebarOpen && "sr-only")}>
          {isCheckingDevice ? "Checking device..." : "Check Device"}
        </span>
      </Button>
    </div>
  );

  return (
    <TooltipProvider>
      <Toaster richColors position="top-center" theme={theme} />
      <AppLayout
        sidebarActions={sidebarActions}
        theme={theme}
        onThemeChange={handleThemeChange}
      >
        {({ tab }) => (
          <Suspense fallback={null}>
            <div
              key={tab}
              className={cn(
                "animate-in fade-in duration-200 ease-out",
                tab === "flasher" && "h-full",
              )}
            >
              {tab === "flasher" && (
                <FlasherTab
                  connected={deviceInfo?.connected ?? false}
                  onStartFlash={() => setFlashConfirmOpen(true)}
                  flashDisabled={flashDisabled}
                />
              )}
              {tab === "menu" && (
                <MenuTab onForceFastboot={startForceFastboot} menuActionDisabled={menuActionDisabled} />
              )}
              {tab === "extras" && (
                <ExtrasTab
                  menuActionDisabled={menuActionDisabled}
                  isStartingFlash={isStartingFlash}
                  onManualFlash={startManualFlash}
                />
              )}
              {tab === "settings" && <SettingsTab />}
            </div>
          </Suspense>
        )}
      </AppLayout>

      <FlashPlanConfirmDialog
        open={flashConfirmOpen}
        onOpenChange={setFlashConfirmOpen}
        onConfirm={startFlash}
        selectedPartitions={planState.selectedRows}
        rebootRecoveryAfter={planState.options.rebootRecovery}
        skippedCount={planState.plan?.skippedCount ?? 0}
        isPending={isStartingFlash || planState.loading || activeFlashSession || activeForceSession}
      />
      <FlashDialog
        open={flashOpen}
        onOpenChange={(nextOpen, reason) => {
          applyDismissibleDialogChange(nextOpen, reason, hideFlashDialog, () => setFlashOpen(true));
        }}
        onCancel={cancelFlash}
        canCancel={activeFlashSession}
      />
      <ForceFastbootDialog
        open={forceOpen}
        onOpenChange={(nextOpen, reason) => {
          applyDismissibleDialogChange(nextOpen, reason, hideForceDialog, () => setForceOpen(true));
        }}
        onCancel={cancelForceFastboot}
      />
      <LogPanel />
    </TooltipProvider>
  );
}

export default function App() {
  return (
    <SimulationProvider>
      <UIProvider>
        <ConsoleProvider>
          <FlashProgressProvider>
            <ForceFastbootProvider>
              <FlashPlanProvider>
                <AppRoot />
              </FlashPlanProvider>
            </ForceFastbootProvider>
          </FlashProgressProvider>
        </ConsoleProvider>
      </UIProvider>
    </SimulationProvider>
  );
}
