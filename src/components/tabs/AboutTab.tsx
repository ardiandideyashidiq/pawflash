import { AlertTriangle, Code2, ExternalLink, Terminal } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { useConsole } from "@/hooks/useConsole";
import { useSimulation } from "@/hooks/useSimulation";

const CREDITS = [
  {
    name: "penumbra",
    author: "shomykohai",
    url: "https://github.com/shomykohai/penumbra/",
    description: "MTK flash tool written in Rust. Native in-process DA driver for MediaTek chipset flashing.",
    icon: Code2,
  },
  {
    name: "mtkclient",
    author: "bkerler",
    url: "https://github.com/bkerler/mtkclient",
    description: "MediaTek Flash and Repair Utility. Exploitation, low-level BROM/DA mode flashing, and repair tool.",
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
    <div className="max-w-md max-sm:max-w-full space-y-4">
      {/* App Info Panel */}
      <div className="panel-shell px-5 py-4 space-y-2">
        <InfoRow label="Name" value="pawflash" />
        <InfoRow label="Version" value="0.1.0" />
        <InfoRow label="Stack" value="Tauri v2 + React 19 + Rust" />
        <InfoRow label="License" value="GPL-3.0-or-later" />
      </div>

      {/* Credits Section */}
      <div className="space-y-2">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground/80 px-1">
          Powered By Open Source
        </h4>
        <div className="grid gap-2.5">
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

      {/* Simulation Mode Toggle */}
      {available && (
        <div className="panel-shell px-5 py-4 space-y-3">
          <div className="flex items-center gap-3">
            <Checkbox
              id="simulation-mode"
              checked={simulate}
              onCheckedChange={(v) => toggleSimulation(!!v)}
            />
            <Label htmlFor="simulation-mode">Simulation mode</Label>
          </div>
          <p className="flex items-start gap-2 text-caption leading-5 text-muted-foreground/80">
            {simulate && <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warning" />}
            Run all device operations against a simulated device with realistic
            USB and flash timing. No hardware is touched.
          </p>
        </div>
      )}

      <p className="text-caption text-muted-foreground/60 px-1 font-mono">
        · mtk device flashing toolkit
      </p>
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <span className="text-caption font-display font-medium uppercase tracking-wider text-muted-foreground/70">
        {label}
      </span>
      <span className="text-body text-foreground/90 text-right">{value}</span>
    </div>
  );
}
