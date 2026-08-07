/* eslint-disable react-refresh/only-export-components */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

const SIMULATION_STORAGE_KEY = "app-simulate";

export interface SimulationState {
  simulate: boolean;
  setSimulate: (v: boolean) => void;
}

const SimulationContext = createContext<SimulationState | null>(null);

export function SimulationProvider({ children }: { children: ReactNode }) {
  const [simulate, setSimulateState] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(SIMULATION_STORAGE_KEY) === "true";
  });

  const setSimulate = useCallback((v: boolean) => {
    setSimulateState(v);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(SIMULATION_STORAGE_KEY, String(v));
    }
  }, []);

  const value = useMemo(() => ({ simulate, setSimulate }), [simulate, setSimulate]);

  return <SimulationContext.Provider value={value}>{children}</SimulationContext.Provider>;
}

export function useSimulation() {
  const ctx = useContext(SimulationContext);
  if (!ctx) throw new Error("useSimulation must be used within SimulationProvider");
  return ctx;
}
