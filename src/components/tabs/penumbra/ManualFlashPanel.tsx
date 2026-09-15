import { memo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, FolderOpen, Send } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useFlashProgress } from "@/hooks/useFlashProgress";
import { useSimulation } from "@/hooks/useSimulation";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage } from "@/types/api";
import type { ProgressEvent } from "@/types/progress";

interface ManualFlashPanelProps {
  daInstalled: boolean;
  disabled?: boolean;
}

const CRITICAL_PARTITIONS = new Set([
  "preloader",
  "nvram",
  "nvdata",
  "protect_f",
  "protect_s",
  "proinfo",
  "seccfg",
  "sec1",
  "pgpt",
]);

export const ManualFlashPanel = memo(function ManualFlashPanel({
  daInstalled,
  disabled = false,
}: ManualFlashPanelProps) {
  const { simulate } = useSimulation();
  const flash = useFlashProgress();
  const { addEntry, addProgressEvent } = useConsole();

  const [partition, setPartition] = useState("");
  const [imagePath, setImagePath] = useState("");
  const [flashing, setFlashing] = useState(false);

  const isCritical = CRITICAL_PARTITIONS.has(partition.trim().toLowerCase());

  const handlePickImage = async () => {
    try {
      const selected = await open({
        title: "Select Image File to Flash",
        filters: [
          { name: "Partition images", extensions: ["img", "bin", "sin", "iso"] },
          { name: "All files", extensions: ["*"] },
        ],
        multiple: false,
      });
      if (typeof selected === "string" && selected.trim()) {
        setImagePath(selected.trim());
      }
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const handleFlash = async () => {
    const part = partition.trim();
    if (!part || !imagePath) {
      toast.error("Please enter a partition name and select an image file");
      return;
    }
    if (!daInstalled) {
      toast.error("Please download or select a DA first in the Device Bar");
      return;
    }

    if (isCritical) {
      const confirmed = window.confirm(
        `CAUTION: "${part}" is a critical boot/calibration partition. Flashing an invalid or mismatched image can permanently brick the device or erase IMEI. Do you want to proceed?`
      );
      if (!confirmed) return;
    }

    setFlashing(true);
    flash.reset();
    flash.openDialog();

    const channel = new Channel<ProgressEvent>();
    channel.onmessage = (event) => {
      flash.onEvent(event);
      addProgressEvent(event);
    };

    addEntry({
      text: `Penumbra ManualFlash Started: ${imagePath} -> ${part}`,
      level: "command",
    });

    try {
      await invoke("penumbra_write", {
        partition: part,
        file: imagePath,
        onEvent: channel,
        simulate,
      });
      toast.success(`Successfully flashed ${part}`);
      addEntry({ text: `Penumbra ManualFlash Success: ${part}`, level: "success" });
    } catch (error) {
      const msg = errorMessage(error);
      flash.fail(msg);
      toast.error(`Flash failed: ${msg}`);
      addEntry({ text: `Penumbra ManualFlash Error: ${msg}`, level: "error" });
    } finally {
      setFlashing(false);
    }
  };

  return (
    <div className="max-w-2xl mx-auto w-full pt-4">
      <SectionCard title="Direct Partition Flash (DA Mode)" contentClassName="space-y-4">
        <p className="text-xs text-muted-foreground leading-relaxed">
          Flash raw binary images directly to target storage partitions via the active Download Agent connection.
        </p>

        <div className="space-y-1.5">
          <span className="text-xs font-medium text-muted-foreground">Target Partition</span>
          <Input
            value={partition}
            onChange={(e) => setPartition(e.target.value)}
            placeholder="e.g. boot, recovery, vbmeta, dtbo"
            className="font-mono text-sm"
            disabled={disabled || flashing}
          />
        </div>

        {isCritical && (
          <div className="flex items-start gap-2 p-3 rounded-md bg-signal-amber/10 border border-signal-amber/30 text-signal-amber text-xs leading-5">
            <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
            <span>
              <strong>Warning:</strong> <code>{partition.trim()}</code> is a critical partition. Flash only verified vendor binaries.
            </span>
          </div>
        )}

        <div className="space-y-1.5">
          <span className="text-xs font-medium text-muted-foreground">Image File</span>
          <div className="grid gap-2 sm:grid-cols-[auto_minmax(0,1fr)]">
            <Button
              variant="outline"
              size="sm"
              disabled={disabled || flashing}
              onClick={() => void handlePickImage()}
              className="gap-2 shrink-0"
            >
              <FolderOpen className="h-4 w-4 text-trace-copper" />
              Select Image
            </Button>
            <Input
              value={imagePath}
              readOnly
              placeholder="No image selected"
              className="font-mono text-xs text-muted-foreground"
              disabled={disabled || flashing}
            />
          </div>
        </div>

        <Button
          disabled={disabled || flashing || !partition.trim() || !imagePath || !daInstalled}
          onClick={() => void handleFlash()}
          className="w-full gap-2 bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold mt-2"
        >
          <Send className="h-4 w-4" />
          {flashing ? "Writing to Device..." : "Flash Partition"}
        </Button>
      </SectionCard>
    </div>
  );
});
