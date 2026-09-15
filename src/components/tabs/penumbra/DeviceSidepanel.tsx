import { memo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Cpu,
  FileKey,
  FolderOpen,
  PanelRightClose,
  RefreshCw,
  Shield,
  Smartphone,
  Trash2,
  Usb,
} from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { DaPickerModal } from "@/components/tabs/penumbra/DaPickerModal";
import { DaDownloadModal } from "@/components/tabs/penumbra/DaDownloadModal";
import { useConsole } from "@/hooks/useConsole";
import { useSimulation } from "@/hooks/useSimulation";
import { useDaRepository } from "@/hooks/useDaRepository";
import { errorMessage, type PenumbraDaEntry, type PenumbraStatusPayload } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface DeviceSidepanelProps {
  status: PenumbraStatusPayload | null;
  onRefresh: () => Promise<void>;
  onClose: () => void;
  disabled?: boolean;
}

export const DeviceSidepanel = memo(function DeviceSidepanel({
  status,
  onRefresh,
  onClose,
  disabled = false,
}: DeviceSidepanelProps) {
  const { simulate } = useSimulation();
  const { addEntry, addProgressEvent } = useConsole();

  const { devices, loading: loadingDevices, refresh: refreshDevices } = useDaRepository();

  // Modal states
  const [pickerOpen, setPickerOpen] = useState(false);
  const [downloadModalOpen, setDownloadModalOpen] = useState(false);
  const [downloadEntry, setDownloadEntry] = useState<PenumbraDaEntry | null>(null);
  const [downloadDeviceName, setDownloadDeviceName] = useState<string | null>(null);
  const [downloadStatus, setDownloadStatus] = useState<"idle" | "downloading" | "success" | "error">("idle");
  const [downloadProgressBytes, setDownloadProgressBytes] = useState(0);
  const [downloadTotalBytes, setDownloadTotalBytes] = useState(0);
  const [downloadPhaseMsg, setDownloadPhaseMsg] = useState("");
  const [downloadErrMsg, setDownloadErrMsg] = useState<string | undefined>();

  const handleStartDownload = async (entry: PenumbraDaEntry, deviceName: string) => {
    setPickerOpen(false);
    setDownloadEntry(entry);
    setDownloadDeviceName(deviceName);
    setDownloadStatus("downloading");
    setDownloadProgressBytes(0);
    setDownloadTotalBytes(0);
    setDownloadPhaseMsg(`Starting download for ${deviceName}...`);
    setDownloadErrMsg(undefined);
    setDownloadModalOpen(true);

    addEntry({ text: `Penumbra DADownload Start: ${deviceName}`, level: "command" });

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      addProgressEvent(event);
      if (event.event === "PenumbraProgress") {
        setDownloadProgressBytes(event.data.bytes);
        setDownloadTotalBytes(event.data.total);
      } else if (event.event === "PenumbraPhase") {
        setDownloadPhaseMsg(event.data.message);
      } else if (event.event === "PenumbraDone") {
        if (event.data.ok) {
          setDownloadStatus("success");
          setDownloadPhaseMsg(event.data.detail);
        } else {
          setDownloadStatus("error");
          setDownloadErrMsg(event.data.detail);
        }
      } else if (event.event === "Error") {
        setDownloadStatus("error");
        setDownloadErrMsg(event.data.message);
      }
    };

    try {
      await invoke("penumbra_da_download", {
        device: deviceName,
        onEvent: channel,
        simulate,
      });
      setDownloadStatus("success");
      toast.success(`DA for ${deviceName} installed`);
      addEntry({ text: `Penumbra DADownload Success: ${deviceName}`, level: "success" });
      await onRefresh();
    } catch (error) {
      const msg = errorMessage(error);
      setDownloadStatus("error");
      setDownloadErrMsg(msg);
      toast.error(`DA download failed: ${msg}`);
      addEntry({ text: `Penumbra DADownload Error: ${msg}`, level: "error" });
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

  const daFileName = status?.da_path ? status.da_path.split(/[/\\]/).pop() : null;
  const authFileName = status?.auth_path ? status.auth_path.split(/[/\\]/).pop() : null;

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-card">
      {/* Sidepanel Header */}
      <div className="flex items-center justify-between border-b border-border/70 px-4 py-3 bg-card/96 shrink-0">
        <div className="flex items-center gap-2">
          <Usb className="h-4 w-4 text-trace-copper" />
          <span className="text-sm font-semibold text-foreground">Device & DA Setup</span>
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={disabled}
            onClick={() => {
              void onRefresh();
              void refreshDevices(false);
            }}
            className="h-7 w-7 text-muted-foreground hover:text-foreground"
            title="Refresh Status"
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onClose}
            className="h-7 w-7 text-muted-foreground hover:text-foreground"
            title="Collapse Sidepanel"
          >
            <PanelRightClose className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Scrollable Setup Body */}
      <ScrollArea className="min-h-0 flex-1 p-4">
        <div className="space-y-4">
          {/* Connection Status */}
          <div className="panel-inset space-y-2 p-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Connection
              </span>
              <span className="font-mono text-[10px] text-muted-foreground">{status?.platform ?? "auto"}</span>
            </div>
            <div className="flex items-center gap-2">
              <span
                className={`inline-block h-2 w-2 rounded-full ${
                  status?.device_visible ? "bg-signal-green animate-pulse" : "bg-muted-foreground/60"
                }`}
              />
              <span className="text-xs font-medium text-foreground">
                {status?.device_visible ? "MTK USB Port Connected" : "Waiting for MTK USB Port"}
              </span>
            </div>
          </div>

          {/* Active Download Agent */}
          <div className="panel-inset space-y-2.5 p-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Download Agent (DA)
              </span>
              {status?.da_installed && (
                <button
                  type="button"
                  onClick={() => void handleClearDa()}
                  disabled={disabled}
                  className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-error transition-colors cursor-pointer"
                  title="Remove DA cache"
                >
                  <Trash2 className="h-3 w-3" />
                  Clear
                </button>
              )}
            </div>

            {status?.da_installed ? (
              <div className="space-y-1.5">
                <div className="flex items-center gap-2">
                  <Badge
                    variant="outline"
                    className="border-trace-copper/60 bg-trace-copper/10 text-trace-copper font-mono text-[11px]"
                  >
                    <Cpu className="mr-1 h-3 w-3" />
                    {status.is_custom ? "Custom DA" : status.da_version}
                  </Badge>
                </div>
                {daFileName && (
                  <div
                    className="truncate font-mono text-[11px] text-muted-foreground"
                    title={status.da_path ?? undefined}
                  >
                    {daFileName}
                  </div>
                )}
              </div>
            ) : (
              <div className="text-xs text-muted-foreground">
                No DA installed
              </div>
            )}
          </div>

          {/* SLA / DAA Auth */}
          <div className="panel-inset space-y-2 p-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Authentication (Auth)
              </span>
              {status?.auth_path && (
                <button
                  type="button"
                  onClick={() => void handleClearAuth()}
                  disabled={disabled}
                  className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-error transition-colors cursor-pointer"
                  title="Clear Auth"
                >
                  <Trash2 className="h-3 w-3" />
                  Clear
                </button>
              )}
            </div>

            {status?.auth_path ? (
              <div className="flex items-center gap-2">
                <Badge
                  variant="outline"
                  className="border-signal-green/60 bg-signal-green/10 text-signal-green font-mono text-[11px]"
                >
                  <Shield className="mr-1 h-3 w-3" />
                  {authFileName}
                </Badge>
              </div>
            ) : (
              <div className="text-xs text-muted-foreground">
                No auth loaded (optional)
              </div>
            )}
          </div>

          {/* Remote Repository Selector */}
          <div className="space-y-2">
            <span className="text-xs font-semibold text-foreground">Download Working DA</span>
            <Button
              size="sm"
              disabled={disabled || loadingDevices}
              onClick={() => setPickerOpen(true)}
              className="w-full gap-2 bg-trace-copper font-semibold text-zinc-950 hover:bg-trace-gold text-xs"
            >
              <Smartphone className="h-3.5 w-3.5" />
              Select Device DA from Catalog
            </Button>
          </div>

          {/* Custom Files Picker */}
          <div className="space-y-2 pt-2 border-t border-border/60">
            <span className="text-xs font-semibold text-foreground">Custom Local Files</span>
            <div className="flex flex-col gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={disabled}
                onClick={() => void handlePickCustomDa()}
                className="w-full justify-start gap-2 text-xs"
              >
                <FolderOpen className="h-3.5 w-3.5 text-trace-copper" />
                Select Custom DA (.bin)
              </Button>

              <Button
                variant="outline"
                size="sm"
                disabled={disabled}
                onClick={() => void handlePickAuth()}
                className="w-full justify-start gap-2 text-xs"
              >
                <FileKey className="h-3.5 w-3.5 text-signal-green" />
                Select SLA/DAA Auth (.auth)
              </Button>
            </div>
          </div>
        </div>
      </ScrollArea>

      {/* Modals */}
      <DaPickerModal
        open={pickerOpen}
        onOpenChange={setPickerOpen}
        devices={devices}
        loading={loadingDevices}
        onSelectAndDownload={(entry, devName) => void handleStartDownload(entry, devName)}
      />

      <DaDownloadModal
        open={downloadModalOpen}
        onOpenChange={setDownloadModalOpen}
        entry={downloadEntry}
        deviceName={downloadDeviceName}
        status={downloadStatus}
        progressBytes={downloadProgressBytes}
        totalBytes={downloadTotalBytes}
        phaseMessage={downloadPhaseMsg}
        errorMessage={downloadErrMsg}
        onDone={() => setDownloadModalOpen(false)}
      />
    </div>
  );
});
