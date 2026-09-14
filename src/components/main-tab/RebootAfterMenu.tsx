import { memo, useState } from "react";
import { Menu } from "@base-ui/react/menu";
import { Check, ChevronDown, RotateCcw } from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { rebootTargets, targetMeta, type RebootTarget } from "@/lib/reboot";
import { cn } from "@/lib/utils";

interface RebootAfterMenuProps {
  target: RebootTarget | null;
  onTargetChange: (target: RebootTarget | null) => void;
  disabled?: boolean;
}

export const RebootAfterMenu = memo(function RebootAfterMenu({
  target,
  onTargetChange,
  disabled = false,
}: RebootAfterMenuProps) {
  const [menuOpen, setMenuOpen] = useState(false);

  const label = target ? targetMeta[target].label : null;

  return (
    <Menu.Root open={menuOpen} onOpenChange={setMenuOpen}>
      <Menu.Trigger
        className={cn(
          "flex min-w-0 flex-1 items-center gap-2.5 rounded-md border bg-background/70 px-3 py-2 text-sm font-medium outline-none transition-all duration-200 ease-out focus-visible:ring-2 focus-visible:ring-trace-copper/50 disabled:cursor-not-allowed disabled:opacity-50 lg:flex-none",
          target
            ? "border-trace-copper/40 text-trace-copper font-semibold"
            : "border-border/70 hover:border-trace-copper/40 hover:bg-accent-soft/80 text-foreground",
        )}
        disabled={disabled}
        aria-label="Reboot after flash target"
        title={target ? `Reboot into ${label} after flash` : "Reboot after flash"}
      >
        <RotateCcw
          className={cn(
            "h-4 w-4 shrink-0 transition-colors",
            target ? "text-trace-copper" : "text-muted-foreground",
          )}
        />
        <span className="min-w-0 flex-1 truncate text-left">
          {target ? `Reboot into ${label}` : "Reboot after flash"}
        </span>
        <ChevronDown
          className={cn(
            "h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-out",
            menuOpen && "rotate-180",
          )}
        />
      </Menu.Trigger>

      <Menu.Portal>
        <Menu.Positioner side="bottom" align="start" sideOffset={6} className="isolate z-50">
          <Menu.Popup className="z-50 w-56 rounded-lg border border-border/80 bg-popover/95 p-1.5 text-popover-foreground shadow-xl backdrop-blur-md outline-none space-y-0.5">
            <Menu.Item
              className={cn(
                "group flex w-full cursor-pointer items-center justify-between rounded-md px-3 py-2 text-sm font-medium text-foreground outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
                target === null && "bg-accent-soft/70",
              )}
              closeOnClick
              onClick={() => onTargetChange(null)}
            >
              <span className="truncate">Do not reboot</span>
              {target === null && <Check className="h-4 w-4 shrink-0 text-trace-copper" />}
            </Menu.Item>

            <Separator className="my-1 bg-border/60" />

            {rebootTargets.map((targetKey) => {
              const meta = targetMeta[targetKey];
              const isSelected = target === targetKey;
              return (
                <Menu.Item
                  key={targetKey}
                  className={cn(
                    "group flex w-full cursor-pointer items-center justify-between rounded-md px-3 py-2 text-sm font-medium text-foreground outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
                    isSelected && "bg-accent-soft/70",
                  )}
                  closeOnClick
                  onClick={() => onTargetChange(targetKey)}
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
