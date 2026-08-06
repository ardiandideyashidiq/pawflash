import { memo, useState } from "react";
import { Copy, Search, TerminalSquare } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useDevice } from "@/hooks/useDevice";
import { useConsole } from "@/hooks/useConsole";

interface FastbootVarsProps {
  disabled?: boolean;
  className?: string;
}

export const FastbootVars = memo(function FastbootVars({
  disabled = false,
  className,
}: FastbootVarsProps) {
  const { getVariable, check } = useDevice();
  const { addEntry } = useConsole();
  const [variableName, setVariableName] = useState("");
  const [variableOutput, setVariableOutput] = useState("");
  const [reading, setReading] = useState(false);

  const readVariable = async () => {
    const trimmed = variableName.trim();
    if (!trimmed) {
      toast.error("Enter a fastboot variable name");
      return;
    }
    setReading(true);
    addEntry({ text: `Getvar Started ${trimmed}`, level: "command" });
    try {
      const value = await getVariable(trimmed);
      setVariableOutput(value);
      addEntry({ text: `Getvar Complete ${trimmed}`, level: "success" });
    } catch (error) {
      addEntry({ text: `Getvar Error ${trimmed} ${error}`, level: "error" });
      toast.error(String(error));
    } finally {
      setReading(false);
    }
  };

  const readAllVariables = async () => {
    setReading(true);
    addEntry({ text: "GetvarAll Started", level: "command" });
    try {
      const info = await check();
      setVariableOutput(JSON.stringify(info.vars, null, 2));
      if (!info.connected) {
        toast.error("No fastboot device connected");
      } else {
        addEntry({ text: "GetvarAll Complete", level: "success" });
      }
    } catch (error) {
      addEntry({ text: `GetvarAll Error ${error}`, level: "error" });
      toast.error(String(error));
    } finally {
      setReading(false);
    }
  };

  return (
    <SectionCard
      title="Fastboot vars"
      className={cn("flex flex-col overflow-hidden", className)}
      contentClassName="mt-0 flex min-h-0 flex-1 flex-col gap-4 overflow-hidden min-h-[300px]"
    >
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
        <Input
          value={variableName}
          onChange={(event) => setVariableName(event.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void readVariable()}
          placeholder="e.g. current-slot"
          aria-label="Fastboot variable"
          disabled={disabled || reading}
        />
        <Button
          variant="outline"
          className="gap-2"
          disabled={disabled || reading}
          onClick={() => void readVariable()}
        >
          <Search className="h-4 w-4" />
          {reading ? "Reading..." : "Read var"}
        </Button>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <Button
          variant="outline"
          className="justify-start gap-2"
          disabled={disabled || reading}
          onClick={() => void readAllVariables()}
        >
          <TerminalSquare className="h-4 w-4" />
          Read all vars
        </Button>
        <Button
          variant="outline"
          className="justify-start gap-2"
          disabled={disabled || reading || !variableOutput}
          onClick={() => {
            navigator.clipboard.writeText(variableOutput);
            toast.success("Copied to clipboard");
          }}
        >
          <Copy className="h-4 w-4" />
          Copy vars
        </Button>
      </div>
      <pre className="min-h-0 flex-1 overflow-auto rounded-md border border-border/70 bg-muted/20 p-3 text-xs leading-5 text-muted-foreground">
        {variableOutput || "Variable output will appear here."}
      </pre>
    </SectionCard>
  );
});
