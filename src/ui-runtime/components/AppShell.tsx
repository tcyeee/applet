import type { AppDefinition, View } from "@/app-schema/types";
import { useAppRouter } from "../router";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ChevronLeft } from "lucide-react";
import { ListView } from "./ListView";
import { FormView } from "./FormView";
import { DetailView } from "./DetailView";
import { ChartView } from "./ChartView";
import { ErrorState } from "./ErrorState";
import { useDebugLocation } from "@/debug/DebugModeContext";

const VIEW_TYPE_FILE: Record<View["type"], string> = {
  list: "src/ui-runtime/components/ListView.tsx",
  form: "src/ui-runtime/components/FormView.tsx",
  detail: "src/ui-runtime/components/DetailView.tsx",
  chart: "src/ui-runtime/components/ChartView.tsx",
};

const VIEW_TYPE_LABEL: Record<View["type"], string> = {
  list: "列表",
  form: "表单",
  detail: "详情",
  chart: "统计",
};

export function AppShell({
  appId,
  definition,
  onExit,
}: {
  appId: string;
  definition: AppDefinition;
  onExit: () => void;
}) {
  const navViews = definition.views.filter((v) => v.type === "list" || v.type === "chart");
  // Router must know about every view (including form/detail), not just the
  // sidebar nav entries — otherwise pushing to a form/detail view id (e.g.
  // "新增"/row click) can't be resolved back to a View and renders as 404.
  const router = useAppRouter(definition.views, navViews[0]?.id);

  return (
    <div className="flex h-full min-h-screen">
      <aside className="bg-muted/30 flex w-56 shrink-0 flex-col gap-1 border-r p-4">
        <Button variant="ghost" size="sm" className="mb-2 justify-start" onClick={onExit}>
          <ChevronLeft /> App 列表
        </Button>
        <p className="text-muted-foreground px-2 text-xs font-medium">{definition.name}</p>
        <Separator className="my-2" />
        {navViews.map((v) => (
          <Button
            key={v.id}
            variant={router.current.viewId === v.id ? "secondary" : "ghost"}
            size="sm"
            className="justify-start"
            onClick={() => router.push(v.id)}
          >
            {v.title ?? v.id}
            <span className="text-muted-foreground ml-auto text-xs">{VIEW_TYPE_LABEL[v.type]}</span>
          </Button>
        ))}
      </aside>
      <main className="flex-1 overflow-y-auto p-6">
        {router.canGoBack && (
          <Button variant="ghost" size="sm" className="mb-4" onClick={router.pop}>
            <ChevronLeft /> 返回
          </Button>
        )}
        <RenderView appId={appId} definition={definition} router={router} />
      </main>
    </div>
  );
}

function RenderView({
  appId,
  definition,
  router,
}: {
  appId: string;
  definition: AppDefinition;
  router: ReturnType<typeof useAppRouter>;
}) {
  const view = router.currentView;
  useDebugLocation(
    view ? `${VIEW_TYPE_LABEL[view.type]}视图` : "未知视图",
    view ? VIEW_TYPE_FILE[view.type] : "src/ui-runtime/components/AppShell.tsx",
    view
      ? {
          appId,
          viewId: view.id,
          ...(router.current.recordId ? { recordId: router.current.recordId } : {}),
        }
      : { appId },
  );
  if (!view)
    return (
      <ErrorState
        message="未找到该页面"
        filePath="src/ui-runtime/components/AppShell.tsx"
        detail={{ appId, viewId: router.current.viewId, ...(router.current.recordId ? { recordId: router.current.recordId } : {}) }}
      />
    );

  switch (view.type) {
    case "list":
      return <ListView appId={appId} definition={definition} view={view} router={router} />;
    case "form":
      return <FormView appId={appId} definition={definition} view={view} router={router} />;
    case "detail":
      return <DetailView appId={appId} definition={definition} view={view} router={router} />;
    case "chart":
      return <ChartView appId={appId} definition={definition} view={view} />;
  }
}
