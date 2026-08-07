import { createContext, useState, useCallback, useRef, type ReactNode } from "react";
import type { ProgressEvent, ConsoleEntry, ConsoleLevel } from "@/types/progress";
import { formatClockTime } from "@/lib/format";

export interface ConsoleContextType {
  entries: ConsoleEntry[];
  addEntry(entry: { text: string; level: ConsoleLevel }): void;
  addProgressEvent(event: ProgressEvent): void;
  clearConsole(): void;
}

const MAX_ENTRIES = 1000;

// eslint-disable-next-line react-refresh/only-export-components
export const ConsoleContext = createContext<ConsoleContextType | null>(null);

export function ConsoleProvider({ children }: { children: ReactNode }) {
  const [entries, setEntries] = useState<ConsoleEntry[]>([]);
  const nextId = useRef(0);
  const lastEntryRef = useRef<ConsoleEntry | null>(null);
  const lastFlashPct = useRef<{ partition: string; pct: number }>({ partition: "", pct: -1 });
  const lastOverallPct = useRef<{ pct: number; at: number }>({ pct: -1, at: 0 });

  const addEntry = useCallback((entry: { text: string; level: ConsoleLevel }) => {
    // Dedup and id allocation live outside the updater: React runs updaters
    // twice in dev StrictMode, so mutating refs inside them would double-run.
    const last = lastEntryRef.current;
    if (
      last &&
      last.text === entry.text &&
      Math.abs(Date.now() - last.timestamp) < 500
    ) {
      return;
    }
    const next: ConsoleEntry = {
      id: nextId.current,
      timestamp: Date.now(),
      time: formatClockTime(Date.now()),
      text: entry.text,
      level: entry.level,
    };
    nextId.current += 1;
    lastEntryRef.current = next;
    setEntries((prev) => {
      const all = [...prev, next];
      return all.length > MAX_ENTRIES ? all.slice(all.length - MAX_ENTRIES) : all;
    });
  }, []);

  const addProgressEvent = useCallback(
    (event: ProgressEvent) => {
      switch (event.event) {
        case "Phase":
          addEntry({ text: event.data.message, level: "info" });
          break;
        case "Flashing":
          // Throttle byte-level progress to one log line per 1% step per
          // partition — the full-fidelity stream drives the progress bars.
          {
            const pct = Math.round((event.data.bytes / Math.max(event.data.total, 1)) * 100);
            const last = lastFlashPct.current;
            if (last.partition === event.data.partition && last.pct === pct) {
              return;
            }
            lastFlashPct.current = { partition: event.data.partition, pct };
            addEntry({
              text: `[${event.data.partition}] ${pct}%`,
              level: "info",
            });
          }
          break;
        case "FlashProgress":
          addEntry({
            text: `[${event.data.partition}] ${Math.round(event.data.percent)}%`,
            level: "info",
          });
          break;
        case "FlashComplete": {
          const label = event.data.success ? "OK" : "FAIL";
          const resp = event.data.response ? ` — ${event.data.response}` : "";
          addEntry({
            text: `${event.data.partition}: ${label}${resp}`,
            level: event.data.success ? "success" : "error",
          });
          break;
        }
        case "DeviceAction":
          addEntry({ text: `${event.data.action}: ${event.data.detail}`, level: "command" });
          break;
        case "Overall":
          // Throttle cumulative progress to one log line per 5% step.
          {
            const now = Date.now();
            const pct = Math.round((event.data.bytes / Math.max(event.data.total, 1)) * 100);
            const last = lastOverallPct.current;
            if (pct - last.pct < 5 && now - last.at < 2000 && last.pct >= 0) {
              return;
            }
            lastOverallPct.current = { pct, at: now };
            addEntry({
              text: `Progress ${pct}%`,
              level: "info",
            });
          }
          break;
        case "ForceFastbootStage":
          addEntry({ text: event.data.message, level: "info" });
          break;
        case "MtkPhase":
          addEntry({ text: event.data.message, level: "info" });
          break;
        case "MtkProgress":
          // Throttle byte-level mtk progress to one log line per 5% step.
          {
            const pct = Math.round((event.data.bytes / Math.max(event.data.total, 1)) * 100);
            const last = lastOverallPct.current;
            if (pct - last.pct < 5 && last.pct >= 0) {
              return;
            }
            lastOverallPct.current = { pct, at: Date.now() };
            addEntry({ text: `MTK ${pct}%`, level: "info" });
          }
          break;
        case "MtkDone":
          addEntry({
            text: event.data.detail,
            level: event.data.ok ? "success" : "error",
          });
          break;
        case "Warning":
          addEntry({ text: event.data.message, level: "warning" });
          break;
        case "Error":
          addEntry({ text: event.data.message, level: "error" });
          break;
        case "Cancelled":
          addEntry({ text: event.data.message, level: "warning" });
          break;
        case "Done":
          addEntry({
            text: event.data.detail,
            level: event.data.ok ? "success" : "error",
          });
          break;
      }
    },
    [addEntry],
  );

  const clearConsole = useCallback(() => {
    setEntries([]);
    nextId.current = 0;
    lastEntryRef.current = null;
    lastFlashPct.current = { partition: "", pct: -1 };
    lastOverallPct.current = { pct: -1, at: 0 };
  }, []);

  return (
    <ConsoleContext.Provider value={{ entries, addEntry, addProgressEvent, clearConsole }}>
      {children}
    </ConsoleContext.Provider>
  );
}
