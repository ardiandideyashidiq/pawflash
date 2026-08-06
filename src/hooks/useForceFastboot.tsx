/* eslint-disable react-refresh/only-export-components */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { ProgressEvent } from "@/types/progress";

export type ForceStage = "waiting_preloader" | "sending" | "confirmed" | "detected" | null;

export interface ForceFastbootState {
  phase: "idle" | "waiting" | "complete" | "cancelled" | "error";
  stage: ForceStage;
  message: string;
  reset: () => void;
  onEvent: (event: ProgressEvent) => void;
}

const ForceFastbootContext = createContext<ForceFastbootState | null>(null);

export function ForceFastbootProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<ForceFastbootState["phase"]>("idle");
  const [stage, setStage] = useState<ForceStage>(null);
  const [message, setMessage] = useState("");

  const onEvent = useCallback((event: ProgressEvent) => {
    switch (event.event) {
      case "ForceFastbootStage": {
        const nextStage = event.data.stage as ForceStage;
        setPhase("waiting");
        setStage(nextStage);
        setMessage(event.data.message);
        break;
      }
      case "Phase":
        setMessage(event.data.message);
        break;
      case "Done":
        setPhase(event.data.ok ? "complete" : "error");
        setStage("detected");
        setMessage(event.data.detail);
        break;
      case "Cancelled":
        setPhase("cancelled");
        setStage(null);
        setMessage(event.data.message);
        break;
      case "Error":
        setPhase("error");
        setStage(null);
        setMessage(event.data.message);
        break;
      default:
        break;
    }
  }, []);

  const reset = useCallback(() => {
    setPhase("idle");
    setStage(null);
    setMessage("");
  }, []);

  const value = useMemo(
    () => ({ phase, stage, message, reset, onEvent }),
    [phase, stage, message, reset, onEvent],
  );

  return (
    <ForceFastbootContext.Provider value={value}>
      {children}
    </ForceFastbootContext.Provider>
  );
}

export function useForceFastboot() {
  const ctx = useContext(ForceFastbootContext);
  if (!ctx) throw new Error("useForceFastboot must be used within ForceFastbootProvider");
  return ctx;
}
