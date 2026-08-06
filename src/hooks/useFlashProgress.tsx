/* eslint-disable react-refresh/only-export-components */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { ProgressEvent } from "@/types/progress";

export type FlashPhase =
  | "idle"
  | "waiting"
  | "flashing"
  | "complete"
  | "cancelled"
  | "error";

export type FlashOperation = "" | "flash" | "format" | "erase";

export interface FlashSummary {
  flashed: number;
  failed: number;
  skipped: number;
  totalBytes: number;
}

export interface FlashProgressData {
  phase: FlashPhase;
  operation: FlashOperation;
  partition: string;
  bytes: number;
  total: number;
  speedBps: number;
  overallBytes: number;
  overallTotal: number;
  summary: FlashSummary | null;
  errorMessage: string;
  statusText: string;
}

export interface FlashProgressState extends FlashProgressData {
  reset: () => void;
  fail: (message: string) => void;
  setIsMinimized: (v: boolean) => void;
  onEvent: (event: ProgressEvent) => void;
}

const FlashProgressContext = createContext<FlashProgressState | null>(null);

const initialState: FlashProgressData = {
  phase: "idle",
  operation: "",
  partition: "",
  bytes: 0,
  total: 0,
  speedBps: 0,
  overallBytes: 0,
  overallTotal: 0,
  summary: null,
  errorMessage: "",
  statusText: "",
};

export function FlashProgressProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<FlashProgressData>(initialState);
  const isMinimizedRef = useRef(false);
  const lastSampleRef = useRef<{ bytes: number; at: number } | null>(null);

  const reset = useCallback(() => {
    isMinimizedRef.current = false;
    lastSampleRef.current = null;
    setState(initialState);
  }, []);

  const fail = useCallback((message: string) => {
    setState((prev) => ({
      ...prev,
      phase: "error",
      errorMessage: message,
      statusText: "",
    }));
  }, []);

  const setIsMinimized = useCallback((v: boolean) => {
    isMinimizedRef.current = v;
  }, []);

  const onEvent = useCallback((event: ProgressEvent) => {
    switch (event.event) {
      case "Phase":
        setState((prev) => ({
          ...prev,
          phase: prev.phase === "idle" ? "waiting" : prev.phase,
          statusText: event.data.message,
        }));
        break;
      case "Flashing":
        setState((prev) => {
          const now = performance.now();
          const prevSample = lastSampleRef.current;
          let speedBps = 0;
          if (prevSample && event.data.bytes >= prevSample.bytes) {
            const dt = (now - prevSample.at) / 1000;
            if (dt > 0) {
              speedBps = (event.data.bytes - prevSample.bytes) / dt;
            }
          }
          lastSampleRef.current = { bytes: event.data.bytes, at: now };
          return {
            ...prev,
            phase: "flashing",
            operation: event.data.operation === "erase" ? "erase" : "flash",
            partition: event.data.partition,
            bytes: event.data.bytes,
            total: event.data.total,
            speedBps,
            overallBytes: event.data.overall_bytes,
            overallTotal: event.data.overall_total,
            statusText: "",
          };
        });
        break;
      case "FlashProgress":
        setState((prev) => ({ ...prev, partition: event.data.partition }));
        break;
      case "FlashComplete":
        setState((prev) => ({
          ...prev,
          partition: event.data.partition,
          summary: prev.summary
            ? {
                ...prev.summary,
                flashed: prev.summary.flashed + (event.data.success ? 1 : 0),
                failed: prev.summary.failed + (event.data.success ? 0 : 1),
              }
            : {
                flashed: event.data.success ? 1 : 0,
                failed: event.data.success ? 0 : 1,
                skipped: 0,
                totalBytes: prev.total,
              },
        }));
        break;
      case "Overall":
        setState((prev) => ({
          ...prev,
          overallBytes: event.data.bytes,
          overallTotal: event.data.total,
        }));
        break;
      case "Cancelled":
        setState((prev) => ({
          ...prev,
          phase: "cancelled",
          errorMessage: event.data.message,
          statusText: "",
        }));
        break;
      case "Error":
        setState((prev) => ({
          ...prev,
          phase: "error",
          errorMessage: event.data.message,
          statusText: "",
        }));
        break;
      case "Done":
        setState((prev) => ({
          ...prev,
          phase: event.data.ok ? "complete" : "error",
          statusText: "",
          summary: prev.summary
            ? { ...prev.summary, totalBytes: prev.overallTotal }
            : null,
        }));
        break;
      default:
        break;
    }
  }, []);

  const value = useMemo(
    () => ({ ...state, reset, fail, setIsMinimized, onEvent }),
    [state, reset, fail, setIsMinimized, onEvent],
  );

  return (
    <FlashProgressContext.Provider value={value}>
      {children}
    </FlashProgressContext.Provider>
  );
}

export function useFlashProgress() {
  const ctx = useContext(FlashProgressContext);
  if (!ctx) throw new Error("useFlashProgress must be used within FlashProgressProvider");
  return ctx;
}
