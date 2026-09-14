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
  const lastFlashPartition = useRef<string>("");

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
          // Log partition milestone once when flashing/erasing begins.
          // Numerical byte-level percentage streams to progress widgets, not the log panel.
          if (lastFlashPartition.current !== event.data.partition) {
            lastFlashPartition.current = event.data.partition;
            const op = event.data.operation === "erase" ? "Erasing" : "Flashing";
            addEntry({
              text: `${op} ${event.data.partition}...`,
              level: "info",
            });
          }
          break;
        case "FlashProgress":
        case "Overall":
        case "MtkProgress":
        case "PenumbraProgress":
          // Real-time progress updates are handled by progress widgets,
          // not written to the text log to prevent log flooding.
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
        case "ForceFastbootStage":
          addEntry({ text: event.data.message, level: "info" });
          break;
        case "MtkPhase":
          addEntry({ text: event.data.message, level: "info" });
          break;
        case "MtkDone":
          addEntry({
            text: event.data.detail,
            level: event.data.ok ? "success" : "error",
          });
          break;
        case "PenumbraPhase":
          addEntry({ text: event.data.message, level: "info" });
          break;
        case "PenumbraDone":
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
    lastFlashPartition.current = "";
  }, []);

  return (
    <ConsoleContext.Provider value={{ entries, addEntry, addProgressEvent, clearConsole }}>
      {children}
    </ConsoleContext.Provider>
  );
}
