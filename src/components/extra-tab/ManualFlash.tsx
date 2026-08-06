import { memo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Send } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useConsole } from "@/hooks/useConsole";

interface ManualFlashProps {
  disabled?: boolean;
  flashing?: boolean;
  onManualFlash: (partition: string, imagePath: string) => Promise<void>;
}

export const ManualFlash = memo(function ManualFlash({
  disabled = false,
  flashing = false,
  onManualFlash,
}: ManualFlashProps) {
  const [manualPartition, setManualPartition] = useState("");
  const [manualImage, setManualImage] = useState("");
  const [pickingImage, setPickingImage] = useState(false);
  const { addEntry } = useConsole();

  const manualDisabled = disabled || flashing || !manualPartition.trim() || !manualImage;

  const pickManualImage = async () => {
    setPickingImage(true);
    try {
      const selected = await open({
        title: "Select image to flash",
        filters: [{ name: "Android images", extensions: ["img"] }],
        multiple: false,
      });
      if (typeof selected === "string") {
        setManualImage(selected);
        addEntry({
          text: `ManualFlashImagePicked ${selected.split(/[/\\]/).pop() || selected}`,
          level: "info",
        });
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setPickingImage(false);
    }
  };

  const startManualFlash = async () => {
    const partition = manualPartition.trim();
    if (!partition || !manualImage) {
      toast.error("Partition and image are required");
      return;
    }
    addEntry({
      text: `ManualFlash Started partition=${partition}`,
      level: "command",
    });
    try {
      await onManualFlash(partition, manualImage);
    } catch (error) {
      toast.error(String(error));
    }
  };

  return (
    <SectionCard title="Manual flash" contentClassName="space-y-4">
      <Input
        value={manualPartition}
        onChange={(event) => setManualPartition(event.target.value)}
        placeholder="partition name"
        aria-label="Manual flash partition"
        disabled={disabled || flashing}
      />
      <div className="grid gap-3 sm:grid-cols-[auto_minmax(0,1fr)]">
        <Button
          variant="outline"
          className="gap-2"
          disabled={disabled || flashing || pickingImage}
          onClick={() => void pickManualImage()}
        >
          <FolderOpen className="h-4 w-4" />
          {pickingImage ? "Opening..." : "Select image"}
        </Button>
        <Input
          value={manualImage}
          readOnly
          placeholder="No image selected"
          aria-label="Manual flash image path"
          disabled={disabled || flashing}
        />
      </div>
      <Button
        className="w-full justify-center gap-2"
        disabled={manualDisabled}
        onClick={() => void startManualFlash()}
      >
        <Send className="h-4 w-4" />
        {flashing ? "Starting..." : "Flash partition"}
      </Button>
    </SectionCard>
  );
});
