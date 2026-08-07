import { memo, useCallback, useEffect, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { Download, HardDrive, RefreshCw, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useConsole } from "@/hooks/useConsole";
import { useSimulation } from "@/hooks/useSimulation";
import { errorMessage } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface MtkStatusPayload {
  version: string | null;
  path: string | null;
  installed: boolean;
  device_visible: boolean;
  platform: string;
}

const PARTTYPES = ["user", "boot1", "boot2", "rpmb"] as const;

export default memo(function MtkTab() {
  const { addEntry, addProgressEvent } = useConsole();
  const { simulate } = useSimulation();
  const [status, setStatus] = useState<MtkStatusPayload | null>(null);
  const [busy, setBusy] = useState(false);
  const [opBytes, setOpBytes] = useState<{ bytes: number; total: number } | null>(null);
  const [downloadBytes, setDownloadBytes] = useState<{ bytes: number; total: number } | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const info = await invoke<MtkStatusPayload>("mtk_status", { simulate });
      setStatus(info);
    } catch (error) {
      toast.error(errorMessage(error));
    }
  }, [simulate]);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const runChannelOp = useCallback(
    async (
      command: string,
      args: Record<string, unknown>,
      onProgress?: (bytes: number, total: number) => void,
    ): Promise<unknown> => {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = (event) => {
        addProgressEvent(event);
        if (event.event === "MtkProgress") {
          const next = { bytes: event.data.bytes, total: event.data.total };
          if (onProgress) {
            onProgress(next.bytes, next.total);
          } else {
            setOpBytes(next);
          }
        }
        if (event.event === "MtkDone" || event.event === "Error") {
          setBusy(false);
        }
      };
      setBusy(true);
      setOpBytes(null);
      setDownloadBytes(null);
      try {
        const result = await invoke(command, { ...args, onEvent: channel, simulate });
        return result;
      } catch (error) {
        toast.error(errorMessage(error));
        return null;
      } finally {
        setBusy(false);
      }
    },
    [addProgressEvent, simulate],
  );

  const download = useCallback(async () => {
    const ok = await runChannelOp(
      "mtk_download",
      {},
      (bytes, total) => setDownloadBytes({ bytes, total }),
    );
    if (ok !== null) {
      addEntry({ text: "mtk bridge downloaded", level: "success" });
      void refreshStatus();
    }
  }, [runChannelOp, refreshStatus, addEntry]);

  const doctor = useCallback(async () => {
    await runChannelOp("mtk_doctor", {});
  }, [runChannelOp]);

  const remove = useCallback(async () => {
    await runChannelOp("mtk_remove", {});
    void refreshStatus();
  }, [runChannelOp, refreshStatus]);

  const op = useCallback(
    async (
      command: "mtk_read" | "mtk_write" | "mtk_erase",
      partition: string,
      file: string,
      parttype: string,
    ) => {
      const args: Record<string, unknown> = { partition, parttype };
      if (command !== "mtk_erase") args.file = file;
      const result = await runChannelOp(command, args);
      if (result !== null) {
        addEntry({ text: `${command} complete`, level: "success" });
      }
    },
    [runChannelOp, addEntry],
  );

  const runOpAction = useCallback(
    (action: "read" | "write" | "erase") => {
      const partition = (document.getElementById("mtk-partition") as HTMLInputElement | null)?.value ?? "";
      const file = (document.getElementById("mtk-file") as HTMLInputElement | null)?.value ?? "";
      const parttype = (document.getElementById("mtk-parttype") as HTMLSelectElement | null)?.value ?? "user";
      if (!partition) {
        toast.error("Partition is required");
        return;
      }
      if (action !== "erase" && !file) {
        toast.error("File path is required");
        return;
      }
      const command = action === "read" ? "mtk_read" : action === "write" ? "mtk_write" : "mtk_erase";
      void op(command, partition, file, parttype);
    },
    [op],
  );

  const isBusy = busy;
  const bytesLabel =
    opBytes && opBytes.total > 0 ? `${Math.round((opBytes.bytes / opBytes.total) * 100)}%` : null;

  return (
    <div className="flex min-h-full flex-col gap-5 lg:grid lg:grid-cols-2 lg:gap-6">
      <div className="flex flex-col gap-4">
        <SectionCard
          title="MTK bridge"
          description="DA-mode read/write/erase via the frozen mtkclient bridge."
          headerActions={
            <Button
              variant="ghost"
              size="icon"
              title="Refresh status"
              disabled={isBusy}
              onClick={() => void refreshStatus()}
            >
              <RefreshCw className="h-4 w-4" />
            </Button>
          }
          contentClassName="space-y-3"
        >
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm">
            <dt className="text-muted-foreground">Platform</dt>
            <dd>{status?.platform ?? "…"}</dd>
            <dt className="text-muted-foreground">Bridge</dt>
            <dd>{status?.installed ? status.version : "not installed"}</dd>
            <dt className="text-muted-foreground">Device</dt>
            <dd>{status?.device_visible ? "visible" : "not detected"}</dd>
            {status?.path ? (
              <>
                <dt className="text-muted-foreground">Path</dt>
                <dd className="break-all font-mono text-xs">{status.path}</dd>
              </>
            ) : null}
          </dl>
          <div className="flex flex-wrap gap-2">
            <Button
              className="gap-2"
              disabled={isBusy || status?.installed}
              onClick={() => void download()}
            >
              <Download className="h-4 w-4" />
              {status?.installed ? "Installed" : "Download"}
            </Button>
            <Button variant="outline" disabled={isBusy} onClick={() => void doctor()}>
              Doctor
            </Button>
            <Button
              variant="outline"
              className="gap-2 text-destructive"
              disabled={isBusy || !status?.installed}
              onClick={() => void remove()}
            >
              <Trash2 className="h-4 w-4" />
              Remove
            </Button>
          </div>
          {downloadBytes !== null &&
            (downloadBytes.total > 0 ? (
              <div className="flex items-center gap-3">
                <Progress
                  value={(downloadBytes.bytes / downloadBytes.total) * 100}
                  className="flex-1"
                />
                <span className="w-12 shrink-0 text-right text-xs text-muted-foreground">
                  {Math.round((downloadBytes.bytes / downloadBytes.total) * 100)}%
                </span>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">
                downloading… {Math.round(downloadBytes.bytes / (1024 * 1024))} MiB
              </p>
            ))}
          {simulate && (
            <p className="text-xs text-warning">SIMULATED MODE — no device will be touched.</p>
          )}
        </SectionCard>

        <SectionCard title="DA operations" contentClassName="space-y-3">
          <Input
            id="mtk-partition"
            placeholder="partition (e.g. boot)"
            aria-label="MTK partition"
            disabled={isBusy}
          />
          <Input
            id="mtk-file"
            placeholder="file path"
            aria-label="MTK file path"
            disabled={isBusy}
          />
          <select
            id="mtk-parttype"
            className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            aria-label="MTK parttype"
            defaultValue="user"
          >
            {PARTTYPES.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
          {bytesLabel !== null && (
            <div className="flex items-center gap-3">
              <Progress
                value={((opBytes?.bytes ?? 0) / Math.max(opBytes?.total ?? 1, 1)) * 100}
                className="flex-1"
              />
              <span className="w-12 shrink-0 text-right text-xs text-muted-foreground">
                {bytesLabel}
              </span>
            </div>
          )}
          <div className="grid grid-cols-3 gap-2">
            <Button className="gap-2" disabled={isBusy} onClick={() => runOpAction("read")}>
              <HardDrive className="h-4 w-4" />
              Read
            </Button>
            <Button className="gap-2" disabled={isBusy} onClick={() => runOpAction("write")}>
              <HardDrive className="h-4 w-4" />
              Write
            </Button>
            <Button variant="outline" disabled={isBusy} onClick={() => runOpAction("erase")}>
              Erase
            </Button>
          </div>
        </SectionCard>
      </div>
    </div>
  );
});
