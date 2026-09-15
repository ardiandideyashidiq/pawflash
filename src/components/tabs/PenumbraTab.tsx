import { memo, useCallback, useEffect, useState } from "react";
import { Menu } from "@base-ui/react/menu";
import { invoke } from "@tauri-apps/api/core";
import {
  Archive,
  Check,
  ChevronDown,
  Cpu,
  HardDrive,
  Layers,
  PanelRightClose,
  PanelRightOpen,
  Send,
  Wrench,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DeviceSidepanel } from "@/components/tabs/penumbra/DeviceSidepanel";
import { ScatterPanel } from "@/components/tabs/penumbra/ScatterPanel";
import { PartitionTablePanel } from "@/components/tabs/penumbra/PartitionTablePanel";
import { BackupPanel } from "@/components/tabs/penumbra/BackupPanel";
import { ManualFlashPanel } from "@/components/tabs/penumbra/ManualFlashPanel";
import { ServicePanel } from "@/components/tabs/penumbra/ServicePanel";
import { useSimulation } from "@/hooks/useSimulation";
import { cn } from "@/lib/utils";
import { errorMessage, type PenumbraStatusPayload } from "@/types/api";

type PenumbraSubTab = "scatter" | "pgpt" | "backup" | "manual" | "service";

interface TabDef {
  id: PenumbraSubTab;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

const TABS: TabDef[] = [
  { id: "scatter", label: "Scatter Flash", icon: Layers },
  { id: "pgpt", label: "Partition Table (PGPT)", icon: HardDrive },
  { id: "backup", label: "Backup & Calibration", icon: Archive },
  { id: "manual", label: "Manual Partition Flash", icon: Send },
  { id: "service", label: "Service & Boot", icon: Wrench },
];

export default memo(function PenumbraTab() {
  const { simulate } = useSimulation();
  const [activeTab, setActiveTab] = useState<PenumbraSubTab>("scatter");
  const [menuOpen, setMenuOpen] = useState(false);
  const [sidepanelOpen, setSidepanelOpen] = useState(false);
  const [status, setStatus] = useState<PenumbraStatusPayload | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await invoke<PenumbraStatusPayload>("penumbra_status", { simulate });
      setStatus(s);
    } catch (err) {
      console.error("Failed to fetch penumbra status:", errorMessage(err));
    }
  }, [simulate]);

  useEffect(() => {
    void refreshStatus();
    const interval = setInterval(() => {
      void refreshStatus();
    }, 4000);
    return () => clearInterval(interval);
  }, [refreshStatus]);

  useEffect(() => {
    if (!sidepanelOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setSidepanelOpen(false);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [sidepanelOpen]);

  const daInstalled = Boolean(status?.da_installed);

  const daSummaryLabel = status?.da_installed
    ? status.is_custom
      ? `Custom DA: ${status.da_path?.split(/[/\\]/).pop()}`
      : `DA: ${status.da_version}`
    : "No DA Installed";

  const currentTab = TABS.find((t) => t.id === activeTab) ?? TABS[0];
  const CurrentIcon = currentTab.icon;

  return (
    <div className="relative flex h-full min-h-0 gap-3 overflow-hidden">
      {/* Main Canvas (Sub-tabs Navigation + Content Panels) */}
      <div className="flex min-w-0 flex-1 min-h-0 flex-col gap-3 overflow-hidden">
        {/* Navigation Header with Dropdown Menu */}
        <div className="flex flex-wrap sm:flex-nowrap items-center justify-between gap-2 border-b border-border/70 pb-2 shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <Menu.Root open={menuOpen} onOpenChange={setMenuOpen}>
              <Menu.Trigger
                className="flex items-center gap-2.5 rounded-md border border-border/80 bg-card/90 px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-all duration-200 ease-out hover:border-trace-copper/40 hover:bg-accent-soft/80 focus-visible:ring-2 focus-visible:ring-trace-copper/50 cursor-pointer select-none"
                aria-label="Select operation"
                title="Switch Penumbra operation"
              >
                <CurrentIcon className="h-4 w-4 text-trace-copper shrink-0" />
                <span className="font-semibold text-foreground tracking-tight">{currentTab.label}</span>
                <ChevronDown
                  className={cn(
                    "h-3.5 w-3.5 text-muted-foreground transition-transform duration-200 ease-out shrink-0",
                    menuOpen && "rotate-180",
                  )}
                />
              </Menu.Trigger>

              <Menu.Portal>
                <Menu.Positioner side="bottom" align="start" sideOffset={6} className="isolate z-50">
                  <Menu.Popup className="z-50 w-56 rounded-lg border border-border/80 bg-popover p-1.5 text-popover-foreground shadow-xl outline-none space-y-0.5">
                    {TABS.map((tab) => {
                      const Icon = tab.icon;
                      const isSelected = activeTab === tab.id;
                      return (
                        <Menu.Item
                          key={tab.id}
                          closeOnClick
                          onClick={() => setActiveTab(tab.id)}
                          className={cn(
                            "group flex w-full cursor-pointer items-center justify-between rounded-md px-2.5 py-2 text-xs font-medium outline-none transition-colors hover:bg-accent-soft focus:bg-accent-soft",
                            isSelected && "bg-trace-copper/10 text-trace-copper font-semibold",
                          )}
                        >
                          <div className="flex items-center gap-2.5">
                            <Icon
                              className={cn(
                                "h-4 w-4 shrink-0 transition-colors",
                                isSelected ? "text-trace-copper" : "text-muted-foreground group-hover:text-foreground",
                              )}
                            />
                            <span className="truncate">{tab.label}</span>
                          </div>
                          {isSelected && <Check className="h-3.5 w-3.5 shrink-0 text-trace-copper ml-2" />}
                        </Menu.Item>
                      );
                    })}
                  </Menu.Popup>
                </Menu.Positioner>
              </Menu.Portal>
            </Menu.Root>
          </div>

          {/* Quick status & Sidepanel Toggle */}
          <div className="flex items-center gap-1.5 sm:gap-2 shrink-0">
            <Badge
              variant="outline"
              className={cn(
                "text-xs font-mono px-2 py-0.5 max-w-[110px] sm:max-w-[150px] truncate",
                status?.da_installed
                  ? "border-trace-copper/60 bg-trace-copper/10 text-trace-copper"
                  : "text-muted-foreground"
              )}
            >
              <Cpu className="mr-1 h-3 w-3 shrink-0" />
              <span className="truncate">{daSummaryLabel}</span>
            </Badge>

            {status?.device_visible && (
              <Badge variant="outline" className="border-signal-green/60 text-signal-green text-xs px-2 py-0.5">
                <span className="mr-1 inline-block h-1.5 w-1.5 rounded-full bg-signal-green animate-pulse" />
                <span className="hidden sm:inline">MTK Port</span>
              </Badge>
            )}

            <Button
              variant="outline"
              size="sm"
              onClick={() => setSidepanelOpen((prev) => !prev)}
              className="h-8 gap-1.5 px-2 sm:px-2.5 text-xs text-muted-foreground hover:text-foreground shrink-0"
              title={sidepanelOpen ? "Collapse device panel" : "Expand device panel"}
            >
              {sidepanelOpen ? (
                <PanelRightClose className="h-4 w-4 text-trace-copper" />
              ) : (
                <PanelRightOpen className="h-4 w-4" />
              )}
              <span className="hidden sm:inline">{sidepanelOpen ? "Hide Setup" : "Device Setup"}</span>
            </Button>
          </div>
        </div>

        {/* Tab Content Panels */}
        <div className="flex-1 min-h-0 overflow-hidden">
          {activeTab === "scatter" && <ScatterPanel daInstalled={daInstalled} />}
          {activeTab === "pgpt" && <PartitionTablePanel daInstalled={daInstalled} />}
          {activeTab === "backup" && <BackupPanel daInstalled={daInstalled} />}
          {activeTab === "manual" && (
            <div className="h-full min-h-0 overflow-y-auto pr-1">
              <ManualFlashPanel daInstalled={daInstalled} />
            </div>
          )}
          {activeTab === "service" && (
            <div className="h-full min-h-0 overflow-y-auto pr-1">
              <ServicePanel daInstalled={daInstalled} />
            </div>
          )}
        </div>
      </div>

      {/* Collapsable Right Sidepanel: Overlay drawer on all screen widths */}
      {sidepanelOpen && (
        <>
          {/* Backdrop */}
          <div
            role="button"
            tabIndex={0}
            aria-label="Close setup panel"
            className="fixed inset-0 z-40 bg-stone-950/40 backdrop-blur-xs transition-opacity duration-200"
            onClick={() => setSidepanelOpen(false)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setSidepanelOpen(false);
            }}
          />

          <aside className="fixed inset-y-0 right-0 z-50 w-80 max-w-[85vw] h-full shadow-2xl bg-card border-l border-border/80 shrink-0 min-h-0 flex flex-col transition-transform duration-200 ease-out">
            <DeviceSidepanel
              status={status}
              onRefresh={refreshStatus}
              onClose={() => setSidepanelOpen(false)}
            />
          </aside>
        </>
      )}
    </div>
  );
});
