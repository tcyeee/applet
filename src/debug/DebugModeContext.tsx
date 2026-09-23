import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { DebugBar } from "./DebugBar";

const STORAGE_KEY = "applet:debugMode";

export interface DebugLocationInfo {
  /** Human-readable label for the current screen, e.g. "列表视图". */
  screen: string;
  /** Repo-relative path to the component file rendering this screen. */
  filePath: string;
  /** Extra key/value context (appId, viewId, viewType, ...) — the same
   * component file renders many different screens in this schema-driven
   * runtime, so the file path alone often isn't enough to locate "this
   * exact page". */
  detail?: Record<string, string>;
}

interface DebugModeContextValue {
  enabled: boolean;
  toggle: (enabled: boolean) => void;
  location: DebugLocationInfo | null;
  setLocation: (location: DebugLocationInfo | null) => void;
}

const DebugModeContext = createContext<DebugModeContextValue | null>(null);

export function DebugModeProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabled] = useState(() => localStorage.getItem(STORAGE_KEY) === "1");
  const [location, setLocation] = useState<DebugLocationInfo | null>(null);

  const toggle = useCallback((next: boolean) => {
    setEnabled(next);
    localStorage.setItem(STORAGE_KEY, next ? "1" : "0");
  }, []);

  const value = useMemo(
    () => ({ enabled, toggle, location, setLocation }),
    [enabled, toggle, location],
  );

  return (
    <DebugModeContext.Provider value={value}>
      {children}
      {enabled && <DebugBar location={location} />}
    </DebugModeContext.Provider>
  );
}

function useDebugModeContext(): DebugModeContextValue {
  const ctx = useContext(DebugModeContext);
  if (!ctx) throw new Error("useDebugMode/useDebugLocation must be used within a DebugModeProvider");
  return ctx;
}

export function useDebugMode(): { enabled: boolean; toggle: (enabled: boolean) => void } {
  const { enabled, toggle } = useDebugModeContext();
  return { enabled, toggle };
}

/** Registers the calling screen as the current debug location while it's
 * mounted. Call this once near the top of any top-level screen component. */
export function useDebugLocation(screen: string, filePath: string, detail?: Record<string, string>): void {
  const { setLocation } = useDebugModeContext();
  const detailKey = detail ? JSON.stringify(detail) : "";

  // Re-registers whenever the identifying fields change; clears on unmount
  // so the bar never shows a stale screen after navigating away.
  useEffect(() => {
    setLocation({ screen, filePath, detail });
    return () => setLocation(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [screen, filePath, detailKey]);
}
