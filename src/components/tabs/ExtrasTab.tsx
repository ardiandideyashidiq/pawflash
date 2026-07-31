import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { DeviceInfo } from "@/types/api";
import { Copy, ListTree, LoaderCircle, Search } from "lucide-react";

interface ExtrasTabProps {
  device: DeviceInfo | null;
}

export default function ExtrasTab({ device }: ExtrasTabProps) {
  const connected = device?.connected ?? false;
  const [varName, setVarName] = useState("");
  const [varResult, setVarResult] = useState("");
  const [varLoading, setVarLoading] = useState(false);
  const [allVars, setAllVars] = useState<Record<string, string> | null>(null);
  const [allVarsLoading, setAllVarsLoading] = useState(false);

  const handleGetVar = async () => {
    if (!varName.trim()) {
      toast.error("Enter a variable name");
      return;
    }
    setVarLoading(true);
    try {
      const value = await invoke<string>("get_var", { name: varName.trim() });
      setVarResult(value);
    } catch (e) {
      setVarResult(`Error: ${e}`);
    }
    setVarLoading(false);
  };

  const handleGetAllVars = async () => {
    setAllVarsLoading(true);
    try {
      const info = await invoke<DeviceInfo>("get_device_info");
      setAllVars(info.vars);
      if (!info.connected) {
        toast.error("No fastboot device connected");
      }
    } catch (e) {
      toast.error(`Failed to fetch variables: ${e}`);
    }
    setAllVarsLoading(false);
  };

  const copyText = async (text: string) => {
    await navigator.clipboard.writeText(text);
    toast.success("Copied to clipboard");
  };

  const sortedVars = allVars
    ? Object.entries(allVars).sort(([a], [b]) => a.localeCompare(b))
    : [];

  return (
    <div className="space-y-5">
      {/* Read variable */}
      <section className="panel-shell px-5 py-3">
        <Label
          htmlFor="var-name"
          className="mb-2 block text-caption font-display font-medium uppercase tracking-wider text-muted-foreground"
        >
          Fastboot Getvar
        </Label>
        <div className="flex max-w-sm items-end gap-2">
          <div className="flex-1">
            <Input
              id="var-name"
              placeholder="e.g. max-download-size"
              value={varName}
              onChange={(e) => setVarName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleGetVar()}
              className="text-body"
            />
          </div>
          <Button
            variant="outline"
            size="icon"
            onClick={handleGetVar}
            disabled={varLoading || !varName.trim() || !connected}
            aria-label="Get variable"
          >
            {varLoading ? (
              <LoaderCircle size={16} className="animate-spin" />
            ) : (
              <Search size={16} />
            )}
          </Button>
        </div>
        {varResult && (
          <div className="mt-2 flex items-start gap-2 rounded border border-border/50 bg-muted/30 px-2.5 py-1.5 animate-in fade-in slide-in-from-top-1 duration-200">
            <code className="min-w-0 flex-1 break-all font-mono text-label text-foreground/80">
              {varResult}
            </code>
            <Button
              variant="ghost"
              size="icon-xs"
              className="mt-0.5 shrink-0"
              onClick={() => copyText(varResult)}
              aria-label="Copy value"
            >
              <Copy size={14} />
            </Button>
          </div>
        )}
      </section>

      {/* Getvar all */}
      <section className="panel-shell overflow-hidden">
        <div className="flex items-center justify-between gap-3 px-5 py-3">
          <div className="flex items-center gap-3 min-w-0">
            <ListTree size={16} className="shrink-0 text-muted-foreground" />
            <div>
              <p className="text-body font-display font-medium uppercase tracking-wider text-foreground/90">
                Get All Variables
              </p>
              <p className="text-caption leading-tight text-muted-foreground/70">
                fastboot getvar all
              </p>
            </div>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={handleGetAllVars}
            disabled={allVarsLoading || !connected}
          >
            {allVarsLoading ? (
              <>
                <LoaderCircle size={14} className="animate-spin" /> Fetching...
              </>
            ) : (
              <>
                <ListTree size={14} className="mr-1" /> Getvar All
              </>
            )}
          </Button>
        </div>
        {sortedVars.length > 0 && (
          <div className="max-h-80 overflow-y-auto border-t border-border/50">
            {sortedVars.map(([name, value]) => (
              <div
                key={name}
                className="flex items-start gap-2 border-b border-border/40 px-5 py-1.5 last:border-0"
              >
                <span className="min-w-36 shrink-0 font-mono text-label text-muted-foreground">
                  {name}
                </span>
                <code className="min-w-0 flex-1 break-all font-mono text-label text-foreground/80">
                  {value}
                </code>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="shrink-0"
                  onClick={() => copyText(value)}
                  aria-label={`Copy ${name}`}
                >
                  <Copy size={14} />
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
