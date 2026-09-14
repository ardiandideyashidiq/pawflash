import { memo, useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
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
  onClear?: () => void;
}

export const ScatterPicker = memo(function ScatterPicker({
  path,
  onChange,
  onClear,
}: ScatterPickerProps) {
  const [picking, setPicking] = useState(false);
  const [validating, setValidating] = useState(false);
  const [inputValue, setInputValue] = useState(path);
  const validatingRef = useRef(false);
  const lastAttemptedRef = useRef<string | null>(path);
  const { addEntry } = useConsole();

  useEffect(() => {
    setInputValue(path);
    lastAttemptedRef.current = path;
  }, [path]);

  const loadScatterPath = useCallback(
    async (targetPath: string) => {
      const name = targetPath.split(/[/\\]/).pop() || targetPath;
      try {
        await invoke("parse_scatter", { path: targetPath });
      } catch (error) {
        const message = errorMessage(error);
        addEntry({ text: `ScatterRejected ${name} ${message}`, level: "error" });
        toast.error(message);
        return false;
      }

      addEntry({ text: `ScatterPicked ${name}`, level: "info" });
      onChange(targetPath);
      toast.success(`Scatter loaded: ${name}`);
      return true;
    },
    [addEntry, onChange],
  );

  const commit = useCallback(
    async (raw: string) => {
      let trimmed = raw.trim();
      if (
        (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
        (trimmed.startsWith("'") && trimmed.endsWith("'"))
      ) {
        trimmed = trimmed.slice(1, -1).trim();
      }

      if (!trimmed) {
        if (path) {
          if (onClear) {
            onClear();
          } else {
            onChange("");
          }
        }
        setInputValue("");
        lastAttemptedRef.current = "";
        return;
      }

      if (trimmed === path || trimmed === lastAttemptedRef.current) {
        return;
      }

      if (validatingRef.current) return;
      validatingRef.current = true;
      setValidating(true);
      lastAttemptedRef.current = trimmed;

      try {
        const ok = await loadScatterPath(trimmed);
        if (ok) {
          setInputValue(trimmed);
        }
      } finally {
        validatingRef.current = false;
        setValidating(false);
      }
    },
    [loadScatterPath, onChange, onClear, path],
  );

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void commit(inputValue);
    } else if (e.key === "Escape") {
      e.preventDefault();
      setInputValue(path);
      lastAttemptedRef.current = path;
      e.currentTarget.blur();
    }
  };

  const handleBlur = () => {
    void commit(inputValue);
  };

  const pick = async () => {
    setPicking(true);
    try {
      const selected = await open({
        title: "Select MTK scatter file",
        filters: [{ name: "MTK scatter files", extensions: ["xml", "txt", "yaml"] }],
        multiple: false,
      });
      if (typeof selected !== "string") return;

      await loadScatterPath(selected);
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setPicking(false);
    }
  };

  return (
    <section className="flex flex-col gap-3 sm:flex-row">
      <Button
        variant="outline"
        onClick={pick}
        disabled={picking || validating}
        className="shrink-0 gap-2 sm:w-auto"
      >
        <FolderOpen className="h-4 w-4" />
        {picking ? "Opening picker..." : "Select manifest"}
      </Button>
      <Input
        value={inputValue}
        onChange={(e) => {
          setInputValue(e.target.value);
          lastAttemptedRef.current = null;
        }}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        disabled={picking || validating}
        placeholder="Select manifest or enter scatter file path..."
        className="min-w-0 flex-1"
        aria-label="Scatter file path"
        title={inputValue || "No scatter file selected"}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
      />
    </section>
  );
});
