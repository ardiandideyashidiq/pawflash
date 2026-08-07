import { memo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { toast } from "sonner";
import { errorMessage } from "@/types/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useConsole } from "@/hooks/useConsole";

interface ScatterPickerProps {
  path: string;
  onChange: (path: string) => void;
}

export const ScatterPicker = memo(function ScatterPicker({
  path,
  onChange,
}: ScatterPickerProps) {
  const [picking, setPicking] = useState(false);
  const { addEntry } = useConsole();

  const pick = async () => {
    setPicking(true);
    try {
      const selected = await open({
        title: "Select MTK scatter file",
        filters: [{ name: "MTK scatter files", extensions: ["xml", "txt", "yaml"] }],
        multiple: false,
      });
      if (typeof selected !== "string") return;

      const name = selected.split(/[/\\]/).pop() || selected;
      try {
        await invoke("parse_scatter", { path: selected });
      } catch (error) {
        const message = errorMessage(error);
        addEntry({ text: `ScatterRejected ${name} ${message}`, level: "error" });
        toast.error(message);
        return;
      }

      addEntry({ text: `ScatterPicked ${name}`, level: "info" });
      onChange(selected);
      toast.success(`Scatter loaded: ${name}`);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setPicking(false);
    }
  };

  return (
    <section className="flex flex-col gap-3 sm:flex-row">
      <Button
        variant="outline"
        onClick={pick}
        disabled={picking}
        className="shrink-0 gap-2 sm:w-auto"
      >
        <FolderOpen className="h-4 w-4" />
        {picking ? "Opening picker..." : "Select manifest"}
      </Button>
      <Input
        value={path}
        readOnly
        placeholder="No scatter file selected"
        className="min-w-0 flex-1"
        aria-label="Selected scatter file path"
        title={path || "No scatter file selected"}
      />
    </section>
  );
});
