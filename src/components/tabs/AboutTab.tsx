import { AlertTriangle, Code2, Cpu, ExternalLink, ShieldCheck, Terminal, Zap } from "lucide-react";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useConsole } from "@/hooks/useConsole";
import { useSimulation } from "@/hooks/useSimulation";

const CREDITS = [
  {
    name: "pawflash",
    author: "ardiandideyashidiq",
    url: "https://github.com/ardiandideyashidiq/pawflash",
    description: "Official pawflash repository.",
    icon: Zap,
  },
  {
    name: "fastboot-rs",
    author: "boardswarm",
    url: "https://github.com/boardswarm/fastboot-rs",
    description: "Rust fastboot protocol library and utilities. Powers pawflash's fastboot engine.",
    icon: Cpu,
  },
  {
    name: "penumbra",
    author: "shomykohai",
    url: "https://github.com/shomykohai/penumbra/",
    description: "MTK flash tool written in Rust. Native in-process DA driver for MediaTek devices.",
    icon: Code2,
  },
  {
    name: "mtkclient",
    author: "bkerler",
    url: "https://github.com/bkerler/mtkclient",
    description: "MediaTek Flash and Repair Utility. Exploitation, low-level BROM/DA mode tool.",
    icon: Terminal,
  },
];

export default function AboutTab() {
  const { simulate, setSimulate, available } = useSimulation();
  const { addEntry } = useConsole();

  const toggleSimulation = (v: boolean) => {
    setSimulate(v);
    addEntry({
      text: v ? "SimulationMode Enabled" : "SimulationMode Disabled",
      level: v ? "warning" : "info",
    });
  };

  const handleOpenUrl = (url: string) => {
    window.open(url, "_blank", "noopener,noreferrer");
  };

  return (
    <div className="relative min-h-full flex flex-col gap-4 w-full">
      {/* OPEN SOURCE CREDITS 2x2 GRID */}
      <div className="space-y-2">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground/80 px-1">
          Powered By Open Source
        </h4>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {CREDITS.map((credit) => {
            const Icon = credit.icon;
            return (
              <div
                key={credit.name}
                onClick={() => void handleOpenUrl(credit.url)}
                tabIndex={0}
                role="button"
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    void handleOpenUrl(credit.url);
                  }
                }}
                className="panel-shell group flex items-start gap-3.5 p-3.5 cursor-pointer hover:border-primary/50 hover:bg-card/80 transition-all focus:outline-none focus:ring-1 focus:ring-ring"
              >
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground group-hover:bg-primary/10 group-hover:text-primary transition-colors">
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1 space-y-1">
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-1.5">
                      <span className="font-mono text-xs font-bold text-foreground group-hover:text-primary transition-colors">
                        {credit.name}
                      </span>
                      <span className="text-[11px] text-muted-foreground/70">
                        by {credit.author}
                      </span>
                    </div>
                    <ExternalLink className="h-3.5 w-3.5 shrink-0 text-muted-foreground/50 group-hover:text-primary transition-colors" />
                  </div>
                  <p className="text-[11px] leading-relaxed text-muted-foreground/80 line-clamp-2">
                    {credit.description}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* SIMULATION MODE TOGGLE CARD (BELOW CREDITS GRID) */}
      {available && (
        <div className="panel-shell p-4 space-y-2.5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2.5">
              <ShieldCheck className="h-4.5 w-4.5 text-primary" />
              <div>
                <Label htmlFor="simulation-mode" className="text-xs font-semibold cursor-pointer">
                  Simulation Mode
                </Label>
                <p className="text-[11px] text-muted-foreground">
                  Simulate device hardware I/O operations safely
                </p>
              </div>
            </div>
            <Switch
              id="simulation-mode"
              checked={simulate}
              onCheckedChange={(v) => toggleSimulation(v)}
            />
          </div>
          {simulate && (
            <div className="flex items-center gap-2 rounded-md border border-amber-500/20 bg-amber-500/10 px-3 py-1.5 text-xs text-amber-500">
              <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
              <span>SIMULATED MODE ACTIVE — real device I/O is bypassed.</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}


