import { memo, useEffect, useMemo, useState } from "react";
import { Menu } from "@base-ui/react/menu";
import { Check, ChevronDown, RotateCcw } from "lucide-react";
import { toast } from "sonner";
import { useDevice } from "@/hooks/useDevice";
import { useFlashPhase } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { cn } from "@/lib/utils";
import { errorMessage } from "@/types/api";

export type RebootTarget = "system" | "bootloader" | "fastbootd" | "recovery";

const targetLabels: Record<RebootTarget, string> = {
  system: "System",
  bootloader: "Bootloader",
  fastbootd: "Fastbootd",
  recovery: "Recovery",
};

const successLabels: Record<RebootTarget, string> = {
  system: "Rebooted to system",
  bootloader: "Rebooted to bootloader",
  fastbootd: "Rebooted to fastbootd",
  recovery: "Rebooted to recovery",
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
  const { reboot } = useDevice();
  const flash = useFlashPhase();
  const force = useForceFastboot();

  const sessionLive = useMemo(
    () => flash.phase === "waiting" || flash.phase === "flashing" || force.phase === "waiting",
    [flash.phase, force.phase],
  );

  const menuDisabled = disabled || busy || sessionLive;

  const handleReboot = async (nextTarget: RebootTarget) => {
    if (menuDisabled) return;
    onTargetChange(nextTarget);
    setBusy(true);
    try {
      await reboot(nextTarget);
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
          "flex w-full items-center overflow-hidden rounded-md border border-border bg-card text-sm font-medium shadow-[var(--panel-shadow)] transition-[border-color,background-color,color,box-shadow] duration-200 ease-out hover:border-trace-copper/30 hover:bg-accent-soft/70 disabled:cursor-not-allowed disabled:opacity-50",
          sidebarOpen ? "justify-start gap-2 px-3 py-2" : "justify-center gap-1.5 px-0 py-2",
          busy && "cursor-wait",
        )}
        disabled={menuDisabled}
        aria-label="Reboot"
        title="Reboot"
      >
        <RotateCcw className="h-4 w-4 shrink-0" />
        <span className={cn("truncate", !sidebarOpen && "sr-only")}>Reboot</span>
        <ChevronDown
          className={cn(
            "h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-out",
            menuOpen && "rotate-180",
            sidebarOpen ? "ml-auto" : "ml-0",
          )}
        />
      </Menu.Trigger>

      <Menu.Portal>
        <Menu.Backdrop className="fixed inset-0 z-50 bg-stone-950/18 backdrop-blur-sm transition-opacity duration-150 data-closed:opacity-0 data-open:opacity-100" />
        <Menu.Positioner side="right" align="start" sideOffset={8}>
          <Menu.Popup className="z-50 min-w-52 rounded-md border border-border bg-popover p-2 text-popover-foreground shadow-[var(--overlay-shadow)] outline-none">
            <div className="flex flex-col gap-1.5">
              {(["system", "bootloader", "fastbootd", "recovery"] as RebootTarget[]).map(
                (nextTarget) => {
                  const isSelected = target === nextTarget;
                  return (
                    <Menu.Item
                      key={nextTarget}
                      data-selected={isSelected || undefined}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-md px-4 py-2.5 text-sm font-medium outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
                        isSelected && "bg-accent-soft text-foreground",
                      )}
                      closeOnClick
                      onClick={() => {
                        void handleReboot(nextTarget);
                      }}
                    >
                      <span className="flex-1 truncate">{targetLabels[nextTarget]}</span>
                      {isSelected && <Check className="h-4 w-4 shrink-0" />}
                    </Menu.Item>
                  );
                },
              )}
            </div>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
});
