import { createContext, useState, useCallback, useRef, type ReactNode } from "react";
import type { ProgressEvent, ConsoleEntry, ConsoleLevel } from "@/types/progress";

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

  const addEntry = useCallback((entry: { text: string; level: ConsoleLevel }) => {
    setEntries((prev) => {
      const last = prev[prev.length - 1];
      if (
        last &&
        last.text === entry.text &&
        Math.abs(Date.now() - last.timestamp) < 500
      ) {
        return prev;
      }
      const next: ConsoleEntry = {
        id: nextId.current,
        timestamp: Date.now(),
        text: entry.text,
        level: entry.level,
      };
      nextId.current += 1;
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
          addEntry({
            text: `[${event.data.partition}] ${Math.round((event.data.bytes / Math.max(event.data.total, 1)) * 100)}%`,
            level: "info",
          });
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
          addEntry({
            text: `Progress ${Math.round((event.data.bytes / Math.max(event.data.total, 1)) * 100)}%`,
            level: "info",
          });
          break;
        case "ForceFastbootStage":
          addEntry({ text: event.data.message, level: "info" });
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
  }, []);

  return (
    <ConsoleContext.Provider value={{ entries, addEntry, addProgressEvent, clearConsole }}>
      {children}
    </ConsoleContext.Provider>
  );
}
