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

/**
 * Compute a smoothed transfer speed from a windowed ring of samples.
 *
 * Prefers a sliding-window average over the last `SPEED_WINDOW_MS`, falling
 * back to the whole-partition average when the window is too short (e.g.
 * small files or the first few samples). Returns 0 when there is no stable
 * basis yet (transfers shorter than `SPEED_MIN_DT_MS`).
 */
function computeSpeed(
  samples: SpeedSample[],
  partitionStart: { at: number; bytes: number } | null,
  currentBytes: number,
  now: number,
): number {
  if (samples.length >= 2) {
    const newest = samples[samples.length - 1];
    const oldest = samples[0];
    const dt = (newest.at - oldest.at) / 1000;
    if (dt >= SPEED_MIN_DT_MS / 1000) {
      return Math.max(0, (newest.bytes - oldest.bytes) / dt);
    }
  }
  if (partitionStart && partitionStart.at >= 0) {
    const dt = (now - partitionStart.at) / 1000;
    if (dt >= SPEED_MIN_DT_MS / 1000) {
      return Math.max(0, (currentBytes - partitionStart.bytes) / dt);
    }
  }
  return 0;
}

/** Sliding window over which transfer speed is averaged (ms). */
const SPEED_WINDOW_MS = 2000;
/** Minimum elapsed time before a windowed or partition-average speed is trusted. */
const SPEED_MIN_DT_MS = 150;

interface SpeedSample {
  at: number;
  bytes: number;
}

export function FlashProgressProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<FlashProgressData>(initialState);
  const speedSamplesRef = useRef<SpeedSample[]>([]);
  const partitionStartRef = useRef<{ partition: string; at: number; bytes: number } | null>(null);

  const reset = useCallback(() => {
    speedSamplesRef.current = [];
    partitionStartRef.current = null;
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
        // in dev StrictMode and commits the second result, so mutating refs
        // inside the updater would double-append. Event handlers are never
        // double-invoked.
        const now = performance.now();
        const { partition, bytes } = event.data;
        const partitionStart = partitionStartRef.current;

        if (!partitionStart || partitionStart.partition !== partition) {
          // New partition: reset the window and anchor the partition start.
          speedSamplesRef.current = [];
          partitionStartRef.current = { partition, at: now, bytes };
        }

        const samples = speedSamplesRef.current;
        // Bytes are per-partition; ignore regressions (e.g. a re-seeded
        // partition) so the window never slopes backwards.
        if (bytes >= (samples[samples.length - 1]?.bytes ?? 0)) {
          samples.push({ at: now, bytes });
        } else {
          samples.length = 0;
          samples.push({ at: now, bytes });
          partitionStartRef.current = { partition, at: now, bytes };
        }
        const cutoff = now - SPEED_WINDOW_MS;
        let drop = 0;
        while (drop < samples.length - 1 && samples[drop].at < cutoff) drop += 1;
        if (drop > 0) samples.splice(0, drop);

        const start = partitionStartRef.current;
        const speedBps = computeSpeed(samples, start, bytes, now);

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
