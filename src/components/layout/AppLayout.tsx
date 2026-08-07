import { Fragment, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Cpu,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Info,
  Sun,
  Terminal,
  Wrench,
  Zap,
} from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useUI } from "@/hooks/useUI";

interface AppLayoutProps {
  children: (props: { tab: string }) => ReactNode;
  sidebarActions?:
    | ReactNode
    | ((props: { sidebarOpen: boolean }) => ReactNode);
  theme: "light" | "dark";
  onThemeChange: (
    theme: "light" | "dark" | ((current: "light" | "dark") => "light" | "dark"),
  ) => void;
}

const themeOptions = [
  { value: "light" as const, label: "Light", icon: Sun },
  { value: "dark" as const, label: "Dark", icon: Moon },
];

export default function AppLayout({
  children,
  sidebarActions,
  theme,
  onThemeChange,
}: AppLayoutProps) {
  const [tab, setTab] = useState("flasher");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const userOverride = useRef(false);
  const mainRef = useRef<HTMLDivElement>(null);
  const { toggleLogPanel } = useUI();

  const navItems = useMemo(
    () => [
      { id: "flasher", label: "Flasher", icon: Zap },
      { id: "menu", label: "Menu", icon: Wrench },
      { id: "mtk", label: "mtkclient", icon: Cpu },
      { id: "extras", label: "Extras", icon: Search },
      { id: "about", label: "About", icon: Info },
    ],
    [],
  );

  useEffect(() => {
    mainRef.current?.scrollTo(0, 0);
  }, [tab]);

  useEffect(() => {
    if (typeof window === "undefined") return;

    const mq = window.matchMedia("(max-width: 1100px)");
    const handler = (e: MediaQueryListEvent | MediaQueryList) => {
      if (!userOverride.current) {
        setSidebarOpen(!e.matches);
      } else if (!e.matches) {
        userOverride.current = false;
        setSidebarOpen(true);
      }
    };
    handler(mq);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const handleSidebarToggle = useCallback(() => {
    userOverride.current = true;
    setSidebarOpen((prev) => !prev);
  }, []);

  const renderSidebarSlot = (
    slot: ReactNode | ((props: { sidebarOpen: boolean }) => ReactNode) | undefined,
  ) => {
    if (typeof slot === "function") {
      return slot({ sidebarOpen });
    }
    return slot;
  };

  return (
    <div
      className="grid h-dvh w-dvw overflow-hidden bg-background text-foreground transition-[grid-template-columns] duration-200 ease-out"
      style={{ gridTemplateColumns: sidebarOpen ? "14rem 1fr" : "4.5rem 1fr" }}
    >
      <aside
        aria-label="Sidebar"
        className="flex flex-col overflow-hidden border-r border-sidebar-border bg-sidebar"
      >
        <div
          className={cn(
            "flex items-center",
            sidebarOpen ? "justify-between px-3 py-3" : "justify-center px-2 py-3",
          )}
        >
          {sidebarOpen && (
            <span className="font-display text-sm font-medium tracking-[0.2em] text-trace-copper">
              PAWFLASH
            </span>
          )}
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
            onClick={handleSidebarToggle}
          >
            {sidebarOpen ? <PanelLeftClose className="h-4 w-4" /> : <PanelLeftOpen className="h-4 w-4" />}
          </Button>
        </div>
        <Separator />

        <nav
          aria-label="Main navigation"
          className={cn("flex flex-col gap-2 p-3", !sidebarOpen && "items-center px-2")}
        >
          {navItems.map((item, index) => {
            const Icon = item.icon;
            const active = tab === item.id;
            const notLast = index < navItems.length - 1;
            return (
              <Fragment key={item.id}>
                <button
                  onClick={() => setTab(item.id)}
                  aria-label={item.label}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium transition-[background-color,color,box-shadow] duration-200 ease-out",
                    sidebarOpen ? "w-full justify-start" : "w-11 justify-center px-0",
                    active
                      ? "border border-trace-copper/60 bg-trace-copper/15 text-trace-copper shadow-[var(--panel-shadow)]"
                      : "text-muted-foreground hover:bg-accent-soft/70 hover:text-foreground",
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  {sidebarOpen && <span className="truncate">{item.label}</span>}
                </button>
                {notLast && (
                  <Separator className={cn(sidebarOpen ? "w-full" : "mx-auto w-8")} />
                )}
              </Fragment>
            );
          })}
        </nav>

        <div className="min-h-0 flex-1" />

        {sidebarActions && (
          <div className={cn("p-3", !sidebarOpen && "px-2")}>{renderSidebarSlot(sidebarActions)}</div>
        )}

        <Separator />

        <div className={cn("space-y-2 p-3", !sidebarOpen && "px-2")}>
          {sidebarOpen ? (
            <>
              <Button
                variant="outline"
                size="sm"
                className="w-full justify-start gap-2"
                onClick={toggleLogPanel}
              >
                <Terminal className="h-4 w-4" />
                Logs
              </Button>
              <div className="grid grid-cols-2 gap-2">
                {themeOptions.map((option) => {
                  const Icon = option.icon;
                  return (
                    <Button
                      key={option.value}
                      variant={theme === option.value ? "secondary" : "ghost"}
                      size="icon-sm"
                      className="w-full"
                      aria-label={`Theme ${option.label}`}
                      title={option.label}
                      onClick={() => onThemeChange(option.value)}
                    >
                      <Icon className="h-4 w-4" />
                    </Button>
                  );
                })}
              </div>
            </>
          ) : (
            <div className="space-y-2">
              <Button
                variant="ghost"
                size="icon-sm"
                className="w-full"
                aria-label="Toggle logs"
                title="Logs"
                onClick={toggleLogPanel}
              >
                <Terminal className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                className="w-full"
                aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
                onClick={() => onThemeChange((current) => (current === "light" ? "dark" : "light"))}
              >
                {theme === "light" ? <Moon className="h-4 w-4" /> : <Sun className="h-4 w-4" />}
              </Button>
            </div>
          )}
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 overflow-hidden">
        <div
          ref={mainRef}
          className="flex min-h-0 flex-1 flex-col overflow-y-auto p-3 lg:p-4 xl:p-5"
        >
          {children({ tab })}
        </div>
      </main>
    </div>
  );
}
