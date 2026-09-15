import { memo, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Archive, HardDrive, Layers, Send, Wrench } from "lucide-react";
import { DeviceBar } from "@/components/tabs/penumbra/DeviceBar";
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

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      {/* Device & DA Configuration Bar */}
      <DeviceBar
        status={status}
        onRefresh={refreshStatus}
      />

      {/* Sub-tab Navigation */}
      <div className="flex items-center gap-1 border-b border-border/70 pb-2">
        {TABS.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 rounded-md px-3 py-1.5 text-xs transition-colors cursor-pointer select-none ${
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

      {/* Tab Panels */}
      <div className="flex-1 min-h-0">
        {activeTab === "scatter" && <ScatterPanel daInstalled={daInstalled} />}
        {activeTab === "pgpt" && <PartitionTablePanel daInstalled={daInstalled} />}
        {activeTab === "backup" && <BackupPanel daInstalled={daInstalled} />}
        {activeTab === "manual" && <ManualFlashPanel daInstalled={daInstalled} />}
        {activeTab === "service" && <ServicePanel daInstalled={daInstalled} />}
      </div>
    </div>
  );
});
