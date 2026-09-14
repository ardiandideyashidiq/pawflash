import { memo, useState } from "react";
import { Copy, Search, TerminalSquare } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SectionCard } from "@/components/menu-tab/SectionCard";
import { useDevice } from "@/hooks/useDevice";
import { useConsole } from "@/hooks/useConsole";
import { errorMessage } from "@/types/api";

interface FastbootVarsProps {
  disabled?: boolean;
  className?: string;
}

export const FastbootVars = memo(function FastbootVars({
  disabled = false,
  className,
}: FastbootVarsProps) {
  const { getVariable, getAllVariables } = useDevice();
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
    if (trimmed.toLowerCase() === "all") {
      void readAllVariables();
      return;
    }
    setReading(true);
    addEntry({ text: `Getvar Started ${trimmed}`, level: "command" });
    try {
      const value = await getVariable(trimmed);
      setVariableOutput(value);
      addEntry({ text: `Getvar Complete ${trimmed}`, level: "success" });
    } catch (error) {
      addEntry({ text: `Getvar Error ${trimmed} ${errorMessage(error)}`, level: "error" });
      toast.error(errorMessage(error));
    } finally {
      setReading(false);
    }
  };

  const readAllVariables = async () => {
    setReading(true);
    addEntry({ text: "GetvarAll Started", level: "command" });
    try {
      const vars = await getAllVariables();
      const sorted = Object.keys(vars)
        .sort()
        .reduce<Record<string, string>>((acc, key) => {
          acc[key] = vars[key];
          return acc;
        }, {});
      setVariableOutput(JSON.stringify(sorted, null, 2));
      const count = Object.keys(sorted).length;
      addEntry({ text: `GetvarAll Complete (${count} variables)`, level: "success" });
    } catch (error) {
      addEntry({ text: `GetvarAll Error ${errorMessage(error)}`, level: "error" });
      toast.error(errorMessage(error));
    } finally {
      setReading(false);
    }
  };

  return (
    <SectionCard
      title="Fastboot vars"
      className={cn(
        "flex min-h-0 flex-1 flex-col overflow-hidden max-h-[calc(100dvh-2rem)] lg:max-h-[calc(100dvh-2.5rem)] xl:max-h-[calc(100dvh-3rem)]",
        className,
      )}
      contentClassName="mt-4 flex min-h-0 flex-1 flex-col gap-3.5 overflow-hidden"
    >
      <div className="shrink-0 grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
        <Input
          value={variableName}
          onChange={(event) => setVariableName(event.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void readVariable()}
          placeholder="e.g. current-slot"
          aria-label="Fastboot variable"
          disabled={disabled || reading}
          className="focus:outline-none focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/40 focus-visible:border-primary/60"
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
      <div className="shrink-0 grid grid-cols-2 gap-3">
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
      <div className="flex-1 min-h-[160px] max-h-[360px] lg:max-h-[460px] overflow-y-auto rounded-md border border-border/70 bg-muted/20 p-3">
        <pre className="font-mono text-xs leading-5 text-muted-foreground select-text whitespace-pre-wrap break-all m-0">
          {variableOutput || "Variable output will appear here."}
        </pre>
      </div>
    </SectionCard>
  );
});
