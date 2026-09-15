/* eslint-disable react-refresh/only-export-components */
import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { toast } from "sonner";
import { useSimulation } from "@/hooks/useSimulation";
import type { PenumbraDaEntry } from "@/types/api";
import { errorMessage } from "@/types/api";

export interface DaRepositoryState {
  devices: PenumbraDaEntry[];
  loading: boolean;
  error: string | null;
  lastUpdated: number | null;
  refresh: (silent?: boolean) => Promise<void>;
  addCustomDa: (entry: PenumbraDaEntry) => Promise<void>;
}

const DaRepositoryContext = createContext<DaRepositoryState | null>(null);

export function DaRepositoryProvider({ children }: { children: ReactNode }) {
  const { simulate } = useSimulation();
  const [devices, setDevices] = useState<PenumbraDaEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  const inFlightRef = useRef<Promise<PenumbraDaEntry[]> | null>(null);

  const refresh = useCallback(
    async (silent = false) => {
      if (inFlightRef.current) {
        await inFlightRef.current;
        return;
      }
      setLoading(true);
      setError(null);
      const promise = invoke<PenumbraDaEntry[]>("penumbra_list_devices", { simulate });
      inFlightRef.current = promise;
      try {
        const list = await promise;
        setDevices(list);
        setLastUpdated(Date.now());
        if (!silent && list.length > 0) {
          toast.success(`Loaded ${list.length} verified DA configurations`);
        }
      } catch (err) {
        const msg = errorMessage(err);
        setError(msg);
        // Only show toast feedback on explicit user-triggered refresh
        if (!silent) {
          toast.error(`Failed to load device catalog: ${msg}`);
        }
      } finally {
        inFlightRef.current = null;
        setLoading(false);
      }
    },
    [simulate],
  );

  // Always auto fetch on app launch (lazy load, non-blocking background request)
  useEffect(() => {
    const timeoutId = window.setTimeout(() => {
      void refresh(true);
    }, 0);
    return () => window.clearTimeout(timeoutId);
  }, [refresh]);

  const addCustomDa = useCallback(
    async (entry: PenumbraDaEntry) => {
      try {
        await invoke("penumbra_add_custom_da", { entry, simulate });
        await refresh(true);
        toast.success(`Custom DA added for ${entry.brand} ${entry.chipset}`);
      } catch (err) {
        const msg = errorMessage(err);
        toast.error(`Failed to add custom DA: ${msg}`);
        throw err;
      }
    },
    [refresh, simulate],
  );

  const value = useMemo(
    () => ({
      devices,
      loading,
      error,
      lastUpdated,
      refresh,
      addCustomDa,
    }),
    [devices, loading, error, lastUpdated, refresh, addCustomDa],
  );

  return (
    <DaRepositoryContext.Provider value={value}>
      {children}
    </DaRepositoryContext.Provider>
  );
}

export function useDaRepository() {
  const ctx = useContext(DaRepositoryContext);
  if (!ctx) {
    throw new Error("useDaRepository must be used within DaRepositoryProvider");
  }
  return ctx;
}
