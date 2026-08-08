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
          "flex min-w-0 flex-1 items-center gap-3 rounded-md border bg-background/70 px-3 py-2 text-sm font-medium outline-none transition-all duration-200 ease-out focus-visible:ring-2 focus-visible:ring-trace-copper/50 disabled:cursor-not-allowed disabled:opacity-50 lg:flex-none",
          target
            ? "border-trace-copper/40 text-trace-copper"
            : "border-border/70 hover:border-trace-copper/40 hover:bg-accent-soft/80",
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
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-left",
            target && "font-semibold animate-pulse",
            !target && "text-foreground",
          )}
        >
          {target ? `Reboot into ${label} after flash` : "Reboot after flash"}
        </span>
        <ChevronDown
          className={cn(
            "h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-out",
            menuOpen && "rotate-180",
          )}
        />
      </Menu.Trigger>

      <Menu.Portal>
        <Menu.Backdrop className="fixed inset-0 z-50 bg-stone-950/18 backdrop-blur-sm transition-opacity duration-150 data-closed:opacity-0 data-open:opacity-100" />
        <Menu.Positioner side="bottom" align="start" sideOffset={8} className="isolate z-50">
          <Menu.Popup className="z-50 w-64 rounded-lg border border-border/80 bg-popover/95 p-1.5 text-popover-foreground shadow-xl backdrop-blur-md outline-none">
            <Menu.Item
              className="flex w-full cursor-pointer items-center gap-3 rounded-md px-2.5 py-2 text-sm outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft"
              closeOnClick
              onClick={() => onTargetChange(null)}
            >
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-border/50 bg-background/60">
                <RotateCcw className="h-4 w-4 text-muted-foreground" />
              </span>
              <span className="flex-1 text-left text-xs font-semibold text-foreground">
                No reboot
              </span>
              {target === null && <Check className="h-4 w-4 shrink-0 text-trace-copper" />}
            </Menu.Item>

            <Separator className="my-1.5 bg-border/60" />

            {rebootTargets.map((targetKey) => {
              const meta = targetMeta[targetKey];
              const Icon = meta.icon;
              const isSelected = target === targetKey;
              return (
                <Menu.Item
                  key={targetKey}
                  className={cn(
                    "group flex w-full cursor-pointer items-center gap-3 rounded-md px-2.5 py-2 text-sm outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
                    isSelected && "bg-accent-soft/90 text-foreground font-medium",
                  )}
                  closeOnClick
                  onClick={() => onTargetChange(targetKey)}
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
            })}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
});
