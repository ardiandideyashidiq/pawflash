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

export interface FlashProgressActions {
  reset: () => void;
  fail: (message: string) => void;
  onEvent: (event: ProgressEvent) => void;
}

export interface FlashProgressState extends FlashProgressData, FlashProgressActions {}

/**
 * Coarse-grained flash session state: only `phase` and the stable action
 * callbacks. The context value changes only on phase transitions, so
 * consumers such as `App` and `LogPanel` stay out of the per-byte
 * re-render storm driven by high-frequency `Flashing` events.
 */
export interface FlashPhaseState extends FlashProgressActions {
  phase: FlashPhase;
}

const FlashProgressContext = createContext<FlashProgressState | null>(null);
const FlashPhaseContext = createContext<FlashPhaseState | null>(null);

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
  const lastSampleRef = useRef<{ partition: string; bytes: number; at: number } | null>(null);

  const reset = useCallback(() => {
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

  const onEvent = useCallback((event: ProgressEvent) => {
    switch (event.event) {
      case "Phase":
        setState((prev) => ({
          ...prev,
          phase: prev.phase === "idle" ? "waiting" : prev.phase,
          statusText: event.data.message,
        }));
        break;
      case "Flashing": {
        // Compute the sample outside the updater: React runs updaters twice
        // in dev StrictMode and commits the second result, so mutating the
        // ref inside the updater would zero out the speed (prevSample would
        // equal the current event). Event handlers are never double-invoked.
        const now = performance.now();
        const prevSample = lastSampleRef.current;
        let speedBps = 0;
        // bytes are per-partition; a partition change (or a bytes regression)
        // re-seeds the sample instead of computing a cross-partition delta
        // that spans the gap between partitions (e.g. flash-write latency).
        if (
          prevSample &&
          prevSample.partition === event.data.partition &&
          event.data.bytes >= prevSample.bytes
        ) {
          const dt = (now - prevSample.at) / 1000;
          if (dt > 0) {
            speedBps = (event.data.bytes - prevSample.bytes) / dt;
          }
        }
        lastSampleRef.current = {
          partition: event.data.partition,
          bytes: event.data.bytes,
          at: now,
        };
        setState((prev) => ({
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
        }));
        break;
      }
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
                // Cumulative overall bytes seen so far (not just the current
                // partition's total) — the `Done` event fixes the final value.
                totalBytes: prev.overallTotal,
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

  const actions = useMemo(
    () => ({ reset, fail, onEvent }),
    [reset, fail, onEvent],
  );

  const fineValue = useMemo<FlashProgressState>(
    () => ({ ...state, ...actions }),
    [state, actions],
  );

  const coarseValue = useMemo<FlashPhaseState>(
    () => ({ phase: state.phase, ...actions }),
    [state.phase, actions],
  );

  return (
    <FlashPhaseContext.Provider value={coarseValue}>
      <FlashProgressContext.Provider value={fineValue}>
        {children}
      </FlashProgressContext.Provider>
    </FlashPhaseContext.Provider>
  );
}

export function useFlashProgress() {
  const ctx = useContext(FlashProgressContext);
  if (!ctx) throw new Error("useFlashProgress must be used within FlashProgressProvider");
  return ctx;
}

export function useFlashPhase() {
  const ctx = useContext(FlashPhaseContext);
  if (!ctx) throw new Error("useFlashPhase must be used within FlashProgressProvider");
  return ctx;
}
