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
import type {
  FlashPlanDto,
  FlashPlanView,
  PartitionRow,
} from "@/types/api";
import { buildFlashPlanOptions } from "@/lib/plan";

const SCATTER_STORAGE_KEY = "last-scatter-path";

interface PlanOptions {
  advanced: boolean;
  includePreloader: boolean;
  rebootRecovery: boolean;
}

export interface FlashPlanState {
  scatterPath: string;
  plan: FlashPlanView | null;
  loading: boolean;
  error: string | null;
  options: PlanOptions;
  loadScatter: (path: string) => void;
  setAdvanced: (v: boolean) => void;
  setIncludePreloader: (v: boolean) => void;
  setRebootRecovery: (v: boolean) => void;
  togglePartition: (name: string) => void;
  toggleAllPartitions: () => void;
  allSelected: boolean;
  someSelected: boolean;
  rows: PartitionRow[];
  selectedRows: PartitionRow[];
  selectedFlashCount: number;
  buildExclude: () => string[];
}

const FlashPlanContext = createContext<FlashPlanState | null>(null);

function toPlanView(dto: FlashPlanDto): FlashPlanView {
  const rows: PartitionRow[] = dto.actions
    .filter((a) => a.action === "flash")
    .map((a, index) => {
      const imagePath = a.image?.path?.resolved_path ?? null;
      const imageName = imagePath ? (imagePath.split(/[/\\]/).pop() ?? imagePath) : null;
      return {
        index,
        partition: a.partition,
        action: a.action,
        size_human: a.size_human,
        image_path: imagePath,
        image_name: imageName,
        image_type: a.image_type,
        region: a.region,
        selected: false,
      };
    });
  return {
    chipset: dto.platform,
    storage: dto.storage_selection,
    project: dto.project,
    rows,
    warnings: dto.warnings,
    errors: dto.errors,
    flashCount: dto.summary.flash_count,
    skippedCount: dto.summary.skipped_count,
  };
}

export function FlashPlanProvider({ children }: { children: ReactNode }) {
  const [scatterPath, setScatterPath] = useState(() =>
    typeof window === "undefined" ? "" : (window.localStorage.getItem(SCATTER_STORAGE_KEY) ?? ""),
  );
  const [plan, setPlan] = useState<FlashPlanView | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [includePreloader, setIncludePreloader] = useState(false);
  const [rebootRecovery, setRebootRecovery] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);

  const requestRef = useRef(0);
  const lastScatterPathRef = useRef("");
  // Partition names from the previously rendered plan, used to tell "still
  // exists, preserve selection" apart from "newly added, default selected".
  const lastRowsRef = useRef<Set<string>>(new Set());

  const refreshPlan = useCallback(async () => {
    if (!scatterPath) return;
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const dto = await invoke<FlashPlanDto>("build_plan", {
        path: scatterPath,
        options: buildFlashPlanOptions([], includePreloader, scatterPath),
      });
      if (requestRef.current !== requestId) return;

      const view = toPlanView(dto);
      const preserveSelection = lastScatterPathRef.current === scatterPath;
      const nextRows = new Set(view.rows.map((r) => r.partition));
      setPlan(view);
      setSelected((prev) => {
        if (!preserveSelection) return nextRows;
        // Preserve the user's prior selection for partitions that still exist,
        // and default newly added rows (e.g. `preloader` after enabling the
        // include-preloader option) to selected so the option actually takes
        // effect when flashing.
        const previouslyKnown = lastRowsRef.current;
        const next = new Set<string>();
        for (const row of view.rows) {
          if (!previouslyKnown.has(row.partition) || prev.has(row.partition)) {
            next.add(row.partition);
          }
        }
        return next;
      });
      lastScatterPathRef.current = scatterPath;
      lastRowsRef.current = nextRows;
    } catch (e) {
      if (requestRef.current !== requestId) return;
      setPlan(null);
      setSelected(new Set());
      setError(String(e));
    } finally {
      if (requestRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [scatterPath, includePreloader]);

  useEffect(() => {
    if (!scatterPath) return;
    const timeoutId = window.setTimeout(() => {
      void refreshPlan();
    }, 0);
    return () => window.clearTimeout(timeoutId);
  }, [refreshPlan, reloadToken, scatterPath]);

  const loadScatter = useCallback((path: string) => {
    setPlan(null);
    setSelected(new Set());
    setError(null);
    setScatterPath(path);
    lastScatterPathRef.current = "";
    setReloadToken((t) => t + 1);
    window.localStorage.setItem(SCATTER_STORAGE_KEY, path);
  }, []);

  const togglePartition = useCallback((name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  }, []);

  const toggleAllPartitions = useCallback(() => {
    setSelected((prev) => {
      const rows = plan?.rows ?? [];
      const nextAllSelected = rows.length > 0 && rows.every((r) => prev.has(r.partition));
      return new Set(nextAllSelected ? [] : rows.map((r) => r.partition));
    });
  }, [plan]);

  const rows = useMemo(
    () =>
      (plan?.rows ?? []).map((row) => ({
        ...row,
        selected: selected.has(row.partition),
      })),
    [plan, selected],
  );
  const allSelected = rows.length > 0 && rows.every((r) => selected.has(r.partition));
  const someSelected = rows.some((r) => selected.has(r.partition)) && !allSelected;
  const selectedRows = useMemo(
    () => rows.filter((r) => selected.has(r.partition)),
    [rows, selected],
  );
  const selectedFlashCount = useMemo(
    () => selectedRows.filter((r) => r.action === "flash").length,
    [selectedRows],
  );

  const buildExclude = useCallback(() => {
    const planRows = plan?.rows ?? [];
    return planRows.filter((r) => !selected.has(r.partition)).map((r) => r.partition);
  }, [plan, selected]);

  const value = useMemo(
    () => ({
      scatterPath,
      plan,
      loading,
      error,
      options: { advanced, includePreloader, rebootRecovery },
      loadScatter,
      setAdvanced,
      setIncludePreloader,
      setRebootRecovery,
      togglePartition,
      toggleAllPartitions,
      allSelected,
      someSelected,
      rows,
      selectedRows,
      selectedFlashCount,
      buildExclude,
    }),
    [
      scatterPath,
      plan,
      loading,
      error,
      advanced,
      includePreloader,
      rebootRecovery,
      loadScatter,
      togglePartition,
      toggleAllPartitions,
      allSelected,
      someSelected,
      rows,
      selectedRows,
      selectedFlashCount,
      buildExclude,
    ],
  );

  return (
    <FlashPlanContext.Provider value={value}>
      {children}
    </FlashPlanContext.Provider>
  );
}

export function useFlashPlan() {
  const ctx = useContext(FlashPlanContext);
  if (!ctx) throw new Error("useFlashPlan must be used within FlashPlanProvider");
  return ctx;
}
