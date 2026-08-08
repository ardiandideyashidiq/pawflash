import { memo, useEffect, useMemo, useState } from "react";
import { Menu } from "@base-ui/react/menu";
import {
  Check,
  ChevronDown,
  Cpu,
  Loader2,
  RotateCcw,
  ShieldAlert,
  Smartphone,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
import { Separator } from "@/components/ui/separator";
import { useDevice } from "@/hooks/useDevice";
import { useFlashPhase } from "@/hooks/useFlashProgress";
import { useForceFastboot } from "@/hooks/useForceFastboot";
import { cn } from "@/lib/utils";
import { errorMessage } from "@/types/api";

export type RebootTarget = "system" | "bootloader" | "fastbootd" | "recovery";

interface RebootTargetMeta {
  label: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
  iconColor: string;
}

const targetMeta: Record<RebootTarget, RebootTargetMeta> = {
  system: {
    label: "System",
    description: "Reboot normally to Android OS",
    icon: Smartphone,
    iconColor: "text-emerald-400",
  },
  bootloader: {
    label: "Bootloader",
    description: "Reboot into Fastboot BL mode",
    icon: Cpu,
    iconColor: "text-amber-400",
  },
  fastbootd: {
    label: "Fastbootd",
    description: "Reboot into Userspace Fastboot",
    icon: Zap,
    iconColor: "text-trace-copper",
  },
  recovery: {
    label: "Recovery",
    description: "Reboot into Android Recovery",
    icon: ShieldAlert,
    iconColor: "text-rose-400",
  },
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

  const renderMenuItem = (targetKey: RebootTarget) => {
    const meta = targetMeta[targetKey];
    const Icon = meta.icon;
    const isSelected = target === targetKey;

    return (
      <Menu.Item
        key={targetKey}
        data-selected={isSelected || undefined}
        className={cn(
          "group flex w-full cursor-pointer items-center gap-3 rounded-md px-2.5 py-2 text-sm outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
          isSelected && "bg-accent-soft/90 text-foreground font-medium",
        )}
        closeOnClick
        onClick={() => {
          void handleReboot(targetKey);
        }}
      >
        <div
          className={cn(
            "flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-border/50 bg-background/60 transition-colors group-hover:border-border",
            meta.iconColor,
          )}
        >
          <Icon className="h-4 w-4" />
        </div>

        <div className="flex flex-1 flex-col justify-center text-left min-w-0">
          <span className="text-xs font-semibold leading-tight text-foreground truncate">
            {meta.label}
          </span>
          <span className="text-[11px] leading-tight text-muted-foreground truncate">
            {meta.description}
          </span>
        </div>

        {isSelected && <Check className="h-4 w-4 shrink-0 text-trace-copper" />}
      </Menu.Item>
    );
  };

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
        <Menu.Backdrop className="fixed inset-0 z-50 bg-stone-950/18 backdrop-blur-sm transition-opacity duration-150 data-closed:opacity-0 data-open:opacity-100" />
        <Menu.Positioner side="right" align="start" sideOffset={8} className="isolate z-50">
          <Menu.Popup className="z-50 w-64 rounded-lg border border-border/80 bg-popover/95 p-1.5 text-popover-foreground shadow-xl backdrop-blur-md outline-none">
            <div className="px-2.5 pt-1.5 pb-1 text-[10px] font-semibold tracking-wider text-muted-foreground/70 uppercase select-none">
              Standard Reboot
            </div>

            <div className="space-y-0.5">
              {(["system"] as RebootTarget[]).map((t) => renderMenuItem(t))}
            </div>

            <Separator className="my-1.5 bg-border/60" />

            <div className="px-2.5 pt-1 pb-1 text-[10px] font-semibold tracking-wider text-muted-foreground/70 uppercase select-none">
              Advanced Modes
            </div>

            <div className="space-y-0.5">
              {(["bootloader", "fastbootd", "recovery"] as RebootTarget[]).map((t) =>
                renderMenuItem(t),
              )}
            </div>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
});

