/* eslint-disable react-refresh/only-export-components */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type Theme = "light" | "dark";

const THEME_STORAGE_KEY = "app-theme";
const PANEL_WIDTH_KEY = "log-panel-width";

// Mirror the LogPanel drag constraints so a persisted width can never open
// the panel outside the viewport.
const PANEL_MIN_WIDTH = 300;
const PANEL_MAX_FACTOR = 0.9;

function clampPanelWidth(width: number): number {
  const max = Math.max(window.innerWidth * PANEL_MAX_FACTOR, PANEL_MIN_WIDTH);
  return Math.min(Math.max(width, PANEL_MIN_WIDTH), max);
}

function resolveInitialTheme(): Theme {
  if (typeof window === "undefined") return "light";
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (stored === "light" || stored === "dark") return stored;
  return media.matches ? "dark" : "light";
}

function resolveInitialWidth(): number {
  if (typeof window === "undefined") return 800;
  const stored = Number(window.localStorage.getItem(PANEL_WIDTH_KEY));
  const base = Number.isFinite(stored) && stored > 0 ? stored : window.innerWidth * 0.4;
  return clampPanelWidth(base);
}

export interface UIState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  logPanelOpen: boolean;
  openLogPanel: () => void;
  closeLogPanel: () => void;
  toggleLogPanel: () => void;
  logPanelWidth: number;
  setLogPanelWidth: (width: number) => void;
}

const UIContext = createContext<UIState | null>(null);

export function UIProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(resolveInitialTheme);
  const [logPanelOpen, setLogPanelOpen] = useState(false);
  const [logPanelWidth, setLogPanelWidthState] = useState(resolveInitialWidth);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    document.documentElement.classList.toggle("dark", theme === "dark");
  }, [theme]);

  const setTheme = useCallback((next: Theme) => setThemeState(next), []);
  const openLogPanel = useCallback(() => setLogPanelOpen(true), []);
  const closeLogPanel = useCallback(() => setLogPanelOpen(false), []);
  const toggleLogPanel = useCallback(() => setLogPanelOpen((v) => !v), []);

  const setLogPanelWidth = useCallback((width: number) => {
    setLogPanelWidthState(width);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(PANEL_WIDTH_KEY, String(width));
    }
  }, []);

  const value = useMemo(
    () => ({
      theme,
      setTheme,
      logPanelOpen,
      openLogPanel,
      closeLogPanel,
      toggleLogPanel,
      logPanelWidth,
      setLogPanelWidth,
    }),
    [theme, setTheme, logPanelOpen, openLogPanel, closeLogPanel, toggleLogPanel, logPanelWidth, setLogPanelWidth],
  );

  return <UIContext.Provider value={value}>{children}</UIContext.Provider>;
}

export function useUI() {
  const ctx = useContext(UIContext);
  if (!ctx) throw new Error("useUI must be used within UIProvider");
  return ctx;
}
