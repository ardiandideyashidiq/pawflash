import { useMemo, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { useConsole } from "@/hooks/useConsole";
import type {
  DeviceInfo,
  FlashPlanOptions,
  FlashResult,
  ScatterFile,
  ScatterPartition,
} from "@/types/api";
import type { ProgressEvent } from "@/types/progress";
import { FileText, FolderOpen, LoaderCircle, Play, RefreshCw } from "lucide-react";

interface FlasherTabProps {
  device: DeviceInfo | null;
  onRefresh: () => Promise<void>;
}

function preferredLayout(layouts: Record<string, ScatterPartition[]>): string {
  const keys = Object.keys(layouts);
  for (const wanted of ["UFS", "EMMC"]) {
    const key = keys.find((k) => k.toUpperCase() === wanted);
    if (key) return key;
  }
  return keys[0] ?? "";
}

function partitionCounts(scatter: ScatterFile): { total: number; flashable: number } {
  const layout = preferredLayout(scatter.layouts);
  const parts = scatter.layouts[layout] ?? [];
  return {
    total: parts.length,
    flashable: parts.filter(
      (p) => p.is_download && p.file_name !== null && p.size > 0,
    ).length,
  };
}

export default function FlasherTab({ device, onRefresh }: FlasherTabProps) {
  const { addProgressEvent } = useConsole();
  const [scatterPath, setScatterPath] = useState("");
  const [scatterMeta, setScatterMeta] = useState<ScatterFile | null>(null);
  const [scatterLoading, setScatterLoading] = useState(false);
  const [includePreloader, setIncludePreloader] = useState(false);
  const [rebootAfter, setRebootAfter] = useState(false);
  const [planLoading, setPlanLoading] = useState(false);

  const connected = device?.connected ?? false;

  const counts = useMemo(
    () => (scatterMeta ? partitionCounts(scatterMeta) : null),
    [scatterMeta],
  );

  const parseScatter = async (path: string) => {
    setScatterLoading(true);
    try {
      const meta = await invoke<ScatterFile>("parse_scatter", { path });
      setScatterMeta(meta);
    } catch (e) {
      setScatterMeta(null);
      toast.error(`Failed to parse scatter: ${e}`);
    }
    setScatterLoading(false);
  };

  const pickScatter = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        filters: [{ name: "Scatter", extensions: ["txt", "xml", "yaml"] }],
      });
      if (typeof picked === "string") {
        setScatterPath(picked);
        await parseScatter(picked);
      }
    } catch (e) {
      toast.error(`File dialog error: ${e}`);
    }
  };

  const executeFullFlash = async () => {
    if (!scatterMeta) {
      toast.error("Load a scatter file first");
      return;
    }
    setPlanLoading(true);
    const channel = new Channel<ProgressEvent>();
    channel.onmessage = addProgressEvent;
    const options: FlashPlanOptions = {
      storage: "auto",
      exclude: [],
      firmware_dir: null,
      package_root: null,
      image_verification: { check_images: false, image_search: false },
      allowance: { include_preloader: includePreloader, allow_incomplete_slots: false },
      clean: "no",
    };
    let shouldReboot = false;
    try {
      const result = await invoke<FlashResult>("execute_plan", {
        path: scatterPath,
        options,
        onEvent: channel,
      });
      if (result.failed > 0) {
        toast.error(`${result.failed}/${result.total} partitions failed`);
      } else {
        toast.success(`${result.succeeded} partitions flashed`);
        shouldReboot = rebootAfter;
      }
    } catch (e) {
      toast.error(`Flash plan failed: ${e}`);
    }
    try {
      if (shouldReboot) {
        toast.info("Rebooting into recovery...");
        await invoke("reboot_device", { target: "recovery" });
      }
      await onRefresh();
    } catch (e) {
      toast.error(`Device reboot/refresh failed: ${e}`);
    } finally {
      setPlanLoading(false);
    }
  };

  return (
    <div className="space-y-5">
      {/* Scatter file */}
      <section className="panel-shell overflow-hidden">
        <div className="flex items-start gap-4 px-5 py-5">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-md bg-trace-copper/10 text-trace-copper">
            <FileText size={18} />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="text-body font-display font-medium uppercase tracking-wider text-foreground">
              Scatter File
            </h2>
            <p className="mt-1 text-label leading-normal text-muted-foreground">
              Select a MediaTek scatter file to load the partition layout, then
              flash the safe firmware and Android partitions in full.
            </p>
            <div className="mt-3 flex items-center gap-2 max-sm:flex-wrap">
              <Button variant="outline" size="sm" onClick={pickScatter}>
                <FolderOpen size={14} className="mr-1" />
                Select
              </Button>
              <div className="flex min-w-0 max-w-md flex-1 items-center gap-2">
                <Input
                  value={scatterPath}
                  onChange={(e) => setScatterPath(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && scatterPath.trim()) {
                      parseScatter(scatterPath.trim());
                    }
                  }}
                  placeholder="Full path to scatter file (e.g. /path/MT6789_Android_scatter.txt)"
                  className="font-mono text-label"
                />
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => {
                    if (scatterPath.trim()) parseScatter(scatterPath.trim());
                  }}
                  disabled={scatterLoading || !scatterPath.trim()}
                  aria-label="Reload scatter file"
                >
                  <RefreshCw
                    size={16}
                    className={scatterLoading ? "animate-spin" : ""}
                  />
                </Button>
              </div>
            </div>
            {scatterLoading && (
              <p className="mt-2 text-label text-muted-foreground">
                <LoaderCircle size={14} className="mr-1 inline animate-spin" />
                Parsing...
              </p>
            )}
          </div>
        </div>
      </section>

      {scatterMeta && !scatterLoading && (
        <>
          {/* Options + execute */}
          <section className="panel-shell flex flex-wrap items-center gap-x-6 gap-y-2 px-5 py-3">
            {/* eslint-disable-next-line jsx-a11y/label-has-associated-control */}
            <label className="flex cursor-pointer items-center gap-2 text-label select-none">
              <Checkbox
                checked={includePreloader}
                onCheckedChange={(c) => setIncludePreloader(c)}
              />
              Include preloader
            </label>
            {/* eslint-disable-next-line jsx-a11y/label-has-associated-control */}
            <label className="flex cursor-pointer items-center gap-2 text-label select-none">
              <Checkbox
                checked={rebootAfter}
                onCheckedChange={(c) => setRebootAfter(c)}
              />
              Reboot into recovery after flash
            </label>
            <div className="ml-auto flex items-center gap-3">
              <span className="text-caption text-muted-foreground tabular-nums">
                {counts ? `${counts.flashable} flashable / ${counts.total} partitions` : ""}
              </span>
              <Button
                variant="accent"
                size="sm"
                onClick={executeFullFlash}
                disabled={planLoading || !connected}
              >
                {planLoading ? (
                  <>
                    <LoaderCircle size={14} className="animate-spin" /> Flashing...
                  </>
                ) : (
                  <>
                    <Play size={14} className="mr-1" /> Flash
                  </>
                )}
              </Button>
            </div>
          </section>

          {/* Partition summary */}
          <section className="space-y-2">
            <h3 className="text-caption font-display font-medium uppercase tracking-wider text-muted-foreground">
              Layout — {preferredLayout(scatterMeta.layouts)}
            </h3>
            <p className="text-label leading-normal text-muted-foreground">
              Full flash writes all {counts?.flashable} safe partitions
              ({counts?.total} total in scatter). Identity, calibration, and
              dangerous partitions (e.g. nvram, pgpt, userdata) are skipped.
            </p>
          </section>
        </>
      )}
    </div>
  );
}
