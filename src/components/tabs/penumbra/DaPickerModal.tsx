import { memo, useMemo, useState } from "react";
import {
  Check,
  Cpu,
  Download,
  Search,
  Shield,
  Smartphone,
  Sparkles,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import type { PenumbraDaEntry } from "@/types/api";

export interface DaPickerModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  devices: PenumbraDaEntry[];
  loading?: boolean;
  onSelectAndDownload: (entry: PenumbraDaEntry, deviceName: string) => void;
}

export const DaPickerModal = memo(function DaPickerModal({
  open,
  onOpenChange,
  devices,
  loading = false,
  onSelectAndDownload,
}: DaPickerModalProps) {
  const [search, setSearch] = useState("");
  const [selectedBrand, setSelectedBrand] = useState("all");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [selectedDeviceName, setSelectedDeviceName] = useState<string | null>(null);

  // Extract unique brands with device count
  const brandStats = useMemo(() => {
    const counts = new Map<string, number>();
    for (const d of devices) {
      const b = d.brand.toLowerCase();
      counts.set(b, (counts.get(b) ?? 0) + 1);
    }
    const brands = Array.from(counts.keys()).sort();
    return [{ brand: "all", count: devices.length }, ...brands.map((b) => ({ brand: b, count: counts.get(b) ?? 0 }))];
  }, [devices]);

  // Filtered devices based on brand & search
  const filteredEntries = useMemo(() => {
    const q = search.trim().toLowerCase();
    return devices.filter((entry) => {
      // Brand filter
      if (selectedBrand !== "all" && entry.brand.toLowerCase() !== selectedBrand) {
        return false;
      }
      // Search filter
      if (!q) return true;
      if (entry.brand.toLowerCase().includes(q)) return true;
      if (entry.chipset.toLowerCase().includes(q)) return true;
      if (entry.devices.some((d) => d.toLowerCase().includes(q))) return true;
      return false;
    });
  }, [devices, selectedBrand, search]);

  const selectedEntry = useMemo(() => {
    if (!selectedKey) return null;
    return devices.find((d) => `${d.brand}-${d.chipset}` === selectedKey) ?? null;
  }, [devices, selectedKey]);

  const handleSelect = (entry: PenumbraDaEntry, devName: string) => {
    setSelectedKey(`${entry.brand}-${entry.chipset}`);
    setSelectedDeviceName(devName);
  };

  const handleDownload = () => {
    if (!selectedEntry || !selectedDeviceName) return;
    onSelectAndDownload(selectedEntry, selectedDeviceName);
  };

  const hasAuth = Boolean(selectedEntry?.auth || selectedEntry?.auth_url);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] w-full flex-col overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="border-b border-border/70 p-4 pb-3">
          <div className="flex items-center gap-2">
            <Smartphone className="h-5 w-5 text-trace-copper" />
            <DialogTitle className="text-base font-semibold">Select Working DA</DialogTitle>
          </div>
          <DialogDescription className="text-xs text-muted-foreground">
            Choose your device model to download and configure the verified Download Agent.
          </DialogDescription>

          {/* Search bar */}
          <div className="relative mt-2">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search model, brand (e.g. Redmi Note 12, Infinix, MT6789)..."
              className="h-9 pl-9 text-xs"
            />
          </div>

          {/* Brand Filter Pills */}
          <div className="flex items-center gap-1.5 overflow-x-auto pt-1 pb-0.5 no-scrollbar">
            {brandStats.map(({ brand, count }) => {
              const isSelected = selectedBrand === brand;
              return (
                <button
                  key={brand}
                  type="button"
                  onClick={() => setSelectedBrand(brand)}
                  className={cn(
                    "flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors cursor-pointer select-none whitespace-nowrap",
                    isSelected
                      ? "bg-trace-copper text-zinc-950 font-semibold shadow-xs"
                      : "bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                >
                  <span className="capitalize">{brand}</span>
                  <span
                    className={cn(
                      "text-[9px] font-mono px-1 rounded-full",
                      isSelected ? "bg-zinc-950/20 text-zinc-950" : "bg-background/80 text-muted-foreground",
                    )}
                  >
                    {count}
                  </span>
                </button>
              );
            })}
          </div>
        </DialogHeader>

        {/* Device List Body */}
        <ScrollArea className="flex-1 min-h-0 p-4">
          {loading ? (
            <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground">
              <Sparkles className="h-7 w-7 text-trace-copper animate-spin mb-2" />
              <p className="text-xs">Fetching verified device repository...</p>
            </div>
          ) : filteredEntries.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground">
              <Smartphone className="h-8 w-8 text-muted-foreground/40 mb-2" />
              <p className="text-sm font-medium text-foreground">No matching devices found</p>
              <p className="text-xs mt-1">Try refining your search query or selecting &quot;All&quot; brands.</p>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              {filteredEntries.map((entry) => {
                const entryKey = `${entry.brand}-${entry.chipset}`;
                const isEntrySelected = selectedKey === entryKey;
                const entryHasAuth = Boolean(entry.auth || entry.auth_url);
                const primaryName = entry.devices[0] || `${entry.brand} ${entry.chipset}`;
                const altNames = entry.devices.slice(1);

                return (
                  <div
                    key={entryKey}
                    role="button"
                    tabIndex={0}
                    onClick={() => handleSelect(entry, primaryName)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleSelect(entry, primaryName);
                      }
                    }}
                    className={cn(
                      "flex flex-col justify-between rounded-lg border p-3 text-left transition-all duration-150 cursor-pointer select-none",
                      isEntrySelected
                        ? "border-trace-copper bg-trace-copper/10 shadow-xs ring-1 ring-trace-copper/50"
                        : "border-border/70 bg-card/60 hover:border-trace-copper/40 hover:bg-accent-soft/60",
                    )}
                  >
                    <div className="space-y-1.5">
                      <div className="flex items-start justify-between gap-2">
                        <span className="text-xs font-semibold text-foreground leading-snug line-clamp-2">
                          {primaryName}
                        </span>
                        {isEntrySelected && (
                          <div className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-trace-copper text-zinc-950">
                            <Check className="h-2.5 w-2.5 stroke-[3]" />
                          </div>
                        )}
                      </div>

                      {altNames.length > 0 && (
                        <p className="text-[10px] text-muted-foreground line-clamp-1">
                          Also: {altNames.join(", ")}
                        </p>
                      )}
                    </div>

                    <div className="flex flex-wrap items-center gap-1.5 pt-2.5 mt-2 border-t border-border/40">
                      <Badge variant="outline" className="text-[10px] font-mono px-1.5 py-0">
                        <Cpu className="mr-1 h-2.5 w-2.5 text-trace-copper" />
                        {entry.chipset.toUpperCase()}
                      </Badge>
                      <Badge variant="outline" className="text-[10px] uppercase px-1.5 py-0">
                        {entry.brand}
                      </Badge>
                      {entryHasAuth && (
                        <Badge
                          variant="outline"
                          className="border-signal-amber/60 bg-signal-amber/10 text-signal-amber text-[10px] px-1.5 py-0 gap-0.5"
                        >
                          <Shield className="h-2.5 w-2.5" />
                          Auth Combo
                        </Badge>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </ScrollArea>

        {/* Footer with Selected Action */}
        <DialogFooter className="flex items-center justify-between border-t border-border/70 bg-card/96 p-3 px-4">
          <div className="flex min-w-0 flex-1 flex-col text-left">
            {selectedEntry ? (
              <>
                <span className="text-xs font-medium text-foreground truncate">
                  Selected: <strong className="text-trace-copper">{selectedDeviceName}</strong>
                </span>
                <span className="text-[11px] text-muted-foreground font-mono">
                  {selectedEntry.brand.toUpperCase()} · {selectedEntry.chipset.toUpperCase()}
                  {hasAuth ? " (+ SLA/DAA Auth)" : ""}
                </span>
              </>
            ) : (
              <span className="text-xs text-muted-foreground">Select a device model to proceed</span>
            )}
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <Button variant="outline" size="sm" onClick={() => onOpenChange(false)} className="text-xs">
              Cancel
            </Button>
            <Button
              size="sm"
              disabled={!selectedEntry}
              onClick={handleDownload}
              className="gap-2 bg-trace-copper text-zinc-950 font-semibold hover:bg-trace-gold text-xs"
            >
              <Download className="h-3.5 w-3.5" />
              {hasAuth ? "Download DA + Auth Combo" : "Download Working DA"}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
