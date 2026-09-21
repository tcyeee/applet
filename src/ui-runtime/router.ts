import { useCallback, useMemo, useState } from "react";
import type { View } from "@/app-schema/types";

export interface RouteEntry {
  viewId: string;
  recordId?: string;
}

export interface AppRouter {
  current: RouteEntry;
  currentView: View | undefined;
  canGoBack: boolean;
  push: (viewId: string, recordId?: string) => void;
  pop: () => void;
}

/**
 * In-memory navigation stack for a single App's views. Deliberately not
 * URL-based (no react-router): this runtime renders inside one Tauri window,
 * so "navigation" just means swapping which view is on top of the stack.
 */
export function useAppRouter(views: View[], initialViewId?: string): AppRouter {
  const [stack, setStack] = useState<RouteEntry[]>([
    { viewId: initialViewId ?? views[0]?.id ?? "" },
  ]);

  const push = useCallback((viewId: string, recordId?: string) => {
    setStack((s) => [...s, { viewId, recordId }]);
  }, []);

  const pop = useCallback(() => {
    setStack((s) => (s.length > 1 ? s.slice(0, -1) : s));
  }, []);

  const current = stack[stack.length - 1];
  const currentView = useMemo(() => views.find((v) => v.id === current.viewId), [views, current.viewId]);

  return { current, currentView, canGoBack: stack.length > 1, push, pop };
}
