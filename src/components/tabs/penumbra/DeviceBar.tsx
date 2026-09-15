import { memo, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Download, FileKey, FolderOpen, RefreshCw, Trash2, Cpu, Shield } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage, type PenumbraDaEntry, type PenumbraStatusPayload } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface DeviceBarProps {
  status: PenumbraStatusPayload | null;
  onRefresh: () => Promise<void>;
  disabled?: boolean;
}

export const DeviceBar = memo(function DeviceBar({
  status,
  onRefresh,
  disabled = false,
}: DeviceBarProps) {
  const { simulate } = useSimulation();
  const { addEntry, addProgressEvent } = useConsole();

  const [devices, setDevices] = useState<PenumbraDaEntry[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [loadingDevices, setLoadingDevices] = useState(false);
  const [downloadingDa, setDownloadingDa] = useState(false);

  const fetchDevices = useCallback(async () => {
    setLoadingDevices(true);
    try {
      const list = await invoke<PenumbraDaEntry[]>("penumbra_list_devices", { simulate });
      setDevices(list);
      if (list.length > 0 && !selectedDevice) {
        const first = list[0].devices[0] || `${list[0].brand} ${list[0].chipset}`;
        setSelectedDevice(first);
      }
    } catch (error) {
      toast.error(`Failed to load device list: ${errorMessage(error)}`);
    } finally {
      setLoadingDevices(false);
    }
  }, [simulate, selectedDevice]);

  useEffect(() => {
    void fetchDevices();
  }, [fetchDevices]);

  const handleDownload = async () => {
    if (!selectedDevice) {
      toast.error("Please select a device model");
      return;
    }
    setDownloadingDa(true);
    addEntry({ text: `Penumbra DADownload Start: ${selectedDevice}`, level: "command" });

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => addProgressEvent(event);

    try {
      await invoke("penumbra_da_download", {
        device: selectedDevice,
        onEvent: channel,
        simulate,
      });
      toast.success(`DA for ${selectedDevice} installed`);
      addEntry({ text: `Penumbra DADownload Success: ${selectedDevice}`, level: "success" });
      await onRefresh();
    } catch (error) {
      const msg = errorMessage(error);
      toast.error(`DA download failed: ${msg}`);
      addEntry({ text: `Penumbra DADownload Error: ${msg}`, level: "error" });
    } finally {
      setDownloadingDa(false);
    }
  };

  const handlePickCustomDa = async () => {
    try {
      const selected = await open({
        title: "Select MediaTek DA Binary (.bin)",
        filters: [{ name: "DA Binary", extensions: ["bin"] }],
        multiple: false,
      });
      if (typeof selected === "string" && selected.trim()) {
        await invoke("penumbra_set_custom_da", {
          path: selected.trim(),
          authPath: status?.auth_path ?? null,
        });
        toast.success(`Custom DA set: ${selected.split(/[/\\]/).pop()}`);
        addEntry({ text: `Penumbra CustomDASet: ${selected}`, level: "info" });
        await onRefresh();
      }
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const handlePickAuth = async () => {
    try {
      const selected = await open({
        title: "Select SLA/DAA Authentication File (.auth)",
        filters: [{ name: "Auth File", extensions: ["auth", "bin"] }],
        multiple: false,
      });
      if (typeof selected === "string" && selected.trim()) {
        await invoke("penumbra_set_auth", { authPath: selected.trim() });
        toast.success(`Auth file set: ${selected.split(/[/\\]/).pop()}`);
        addEntry({ text: `Penumbra AuthSet: ${selected}`, level: "info" });
        await onRefresh();
      }
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const handleClearAuth = async () => {
    try {
      await invoke("penumbra_set_auth", { authPath: null });
      toast.success("Auth file cleared");
      await onRefresh();
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const handleClearDa = async () => {
    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => addProgressEvent(event);
    try {
      await invoke("penumbra_da_remove", { onEvent: channel });
      toast.success("DA cache cleared");
      await onRefresh();
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const daLabel = status?.da_installed
    ? status.is_custom
      ? `Custom DA: ${status.da_path?.split(/[/\\]/).pop()}`
      : `DA: ${status.da_version}`
    : "No DA Installed";

  return (
    <div className="panel-shell flex flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-3">
        <div className="flex flex-wrap items-center gap-2">
          <Badge
            variant="outline"
            className={status?.da_installed ? "border-trace-copper/60 text-trace-copper bg-trace-copper/10" : "text-muted-foreground"}
          >
            <Cpu className="mr-1.5 h-3.5 w-3.5" />
            {daLabel}
          </Badge>

          {status?.auth_path ? (
            <Badge variant="outline" className="border-signal-green/60 text-signal-green bg-signal-green/10 flex items-center gap-1">
              <Shield className="h-3 w-3" />
              Auth: {status.auth_path.split(/[/\\]/).pop()}
              <button
                type="button"
                onClick={() => void handleClearAuth()}
                className="ml-1 text-muted-foreground hover:text-foreground cursor-pointer"
                title="Clear Auth"
              >
                ×
              </button>
            </Badge>
          ) : (
            <Badge variant="outline" className="text-muted-foreground">
              No Auth File
            </Badge>
          )}

          <Badge
            variant="outline"
            className={status?.device_visible ? "border-signal-green/60 text-signal-green" : "text-muted-foreground"}
          >
            <span className={`mr-1.5 inline-block h-2 w-2 rounded-full ${status?.device_visible ? "bg-signal-green animate-pulse" : "bg-muted-foreground"}`} />
            {status?.device_visible ? "MTK Port Visible" : "Port Idle"}
          </Badge>
        </div>

        <div className="flex items-center gap-2">
          {status?.da_installed && (
            <Button
              variant="outline"
              size="sm"
              disabled={disabled}
              onClick={() => void handleClearDa()}
              className="gap-1.5 text-xs text-muted-foreground hover:text-error"
            >
              <Trash2 className="h-3.5 w-3.5" />
              Clear DA
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            disabled={disabled}
            onClick={() => void onRefresh()}
            className="gap-1.5 text-xs"
            title="Refresh Status"
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-[minmax(0,1fr)_auto_auto] gap-2.5 items-center">
        <div className="flex items-center gap-2 min-w-0">
          <select
            value={selectedDevice}
            onChange={(e) => setSelectedDevice(e.target.value)}
            disabled={disabled || loadingDevices || downloadingDa}
            aria-label="Target device"
            className="w-full truncate rounded-md border border-input bg-background/80 px-3 py-1.5 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
          >
            {devices.length === 0 ? (
              <option value="">{loadingDevices ? "Fetching device manifest..." : "No devices listed"}</option>
            ) : (
              devices.map((entry) => {
                const devName = entry.devices.join(", ") || `${entry.brand} ${entry.chipset}`;
                return (
                  <option key={`${entry.brand}-${entry.chipset}`} value={entry.devices[0] || `${entry.brand} ${entry.chipset}`}>
                    {devName} ({entry.brand.toUpperCase()} · {entry.chipset.toUpperCase()})
                  </option>
                );
              })
            )}
          </select>

          <Button
            size="sm"
            disabled={disabled || downloadingDa || !selectedDevice}
            onClick={() => void handleDownload()}
            className="shrink-0 gap-1.5 border-trace-copper/50 bg-trace-copper/15 text-trace-copper hover:bg-trace-copper/25"
          >
            <Download className="h-3.5 w-3.5" />
            {downloadingDa ? "Downloading..." : "Download DA"}
          </Button>
        </div>

        <Button
          variant="outline"
          size="sm"
          disabled={disabled}
          onClick={() => void handlePickCustomDa()}
          className="gap-1.5 text-xs shrink-0"
        >
          <FolderOpen className="h-3.5 w-3.5" />
          Custom DA (.bin)
        </Button>

        <Button
          variant="outline"
          size="sm"
          disabled={disabled}
          onClick={() => void handlePickAuth()}
          className="gap-1.5 text-xs shrink-0"
        >
          <FileKey className="h-3.5 w-3.5" />
          Auth (.auth)
        </Button>
      </div>
    </div>
  );
});
