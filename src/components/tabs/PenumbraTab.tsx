import { memo, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Archive,
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
import { errorMessage, type PenumbraStatusPayload } from "@/types/api";

type PenumbraSubTab = "scatter" | "pgpt" | "backup" | "manual" | "service";

interface TabDef {
  id: PenumbraSubTab;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

const TABS: TabDef[] = [
  { id: "scatter", label: "Scatter Flash", icon: Layers },
  { id: "pgpt", label: "Partition Table", icon: HardDrive },
  { id: "backup", label: "Backup & NVRAM", icon: Archive },
  { id: "manual", label: "Manual Flash", icon: Send },
  { id: "service", label: "Service & Boot", icon: Wrench },
];

export default memo(function PenumbraTab() {
  const { simulate } = useSimulation();
  const [activeTab, setActiveTab] = useState<PenumbraSubTab>("scatter");
  const [sidepanelOpen, setSidepanelOpen] = useState(true);
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

  const daInstalled = Boolean(status?.da_installed);

  const daSummaryLabel = status?.da_installed
    ? status.is_custom
      ? `Custom DA: ${status.da_path?.split(/[/\\]/).pop()}`
      : `DA: ${status.da_version}`
    : "No DA Installed";

  return (
    <div className="flex h-full min-h-0 gap-3">
      {/* Main Canvas (Sub-tabs Navigation + Content Panels) */}
      <div className="flex min-w-0 flex-1 min-h-0 flex-col gap-3">
        {/* Sub-tab Navigation Header */}
        <div className="flex items-center justify-between gap-2 border-b border-border/70 pb-2">
          <div className="flex items-center gap-1 overflow-x-auto">
            {TABS.map((tab) => {
              const Icon = tab.icon;
              const isActive = activeTab === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex items-center gap-2 rounded-md px-3 py-1.5 text-xs transition-colors cursor-pointer select-none whitespace-nowrap ${
                    isActive
                      ? "bg-trace-copper/15 text-trace-copper font-semibold border border-trace-copper/30"
                      : "text-muted-foreground hover:bg-muted/50 hover:text-foreground border border-transparent"
                  }`}
                >
                  <Icon className={`h-3.5 w-3.5 ${isActive ? "text-trace-copper" : "text-muted-foreground"}`} />
                  {tab.label}
                </button>
              );
            })}
          </div>

          {/* Quick status & Sidepanel Toggle */}
          <div className="flex items-center gap-2 shrink-0">
            <Badge
              variant="outline"
              className={
                status?.da_installed
                  ? "border-trace-copper/60 bg-trace-copper/10 text-trace-copper text-xs font-mono"
                  : "text-muted-foreground text-xs"
              }
            >
              <Cpu className="mr-1 h-3 w-3" />
              <span className="max-w-[140px] truncate">{daSummaryLabel}</span>
            </Badge>

            {status?.device_visible && (
              <Badge variant="outline" className="border-signal-green/60 text-signal-green text-xs">
                <span className="mr-1 inline-block h-1.5 w-1.5 rounded-full bg-signal-green animate-pulse" />
                MTK Port
              </Badge>
            )}

            <Button
              variant="outline"
              size="sm"
              onClick={() => setSidepanelOpen((prev) => !prev)}
              className="h-8 gap-1.5 px-2.5 text-xs text-muted-foreground hover:text-foreground"
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
        <div className="flex-1 min-h-0">
          {activeTab === "scatter" && <ScatterPanel daInstalled={daInstalled} />}
          {activeTab === "pgpt" && <PartitionTablePanel daInstalled={daInstalled} />}
          {activeTab === "backup" && <BackupPanel daInstalled={daInstalled} />}
          {activeTab === "manual" && <ManualFlashPanel daInstalled={daInstalled} />}
          {activeTab === "service" && <ServicePanel daInstalled={daInstalled} />}
        </div>
      </div>

      {/* Collapsable Right Sidepanel */}
      {sidepanelOpen && (
        <aside className="w-80 shrink-0 min-h-0 flex flex-col transition-all duration-200 ease-out">
          <DeviceSidepanel
            status={status}
            onRefresh={refreshStatus}
            onClose={() => setSidepanelOpen(false)}
          />
        </aside>
      )}
    </div>
  );
});
