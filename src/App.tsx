import { useEffect, useState } from "react";
import { validateAppDefinition } from "@/app-schema/validate";
import bookkeepingExample from "@/app-schema/examples/bookkeeping.json";
import { client, type AppRecord } from "@/ui-runtime/client";
import { AppShell } from "@/ui-runtime/components/AppShell";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/ui-runtime/components/EmptyState";
import { ErrorState } from "@/ui-runtime/components/ErrorState";
import { LoadingState } from "@/ui-runtime/components/LoadingState";

function AppPicker({ onOpen }: { onOpen: (appId: string) => void }) {
  const [apps, setApps] = useState<AppRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);
  const reload = () => setReloadToken((t) => t + 1);

  useEffect(() => {
    let cancelled = false;
    async function run() {
      setLoading(true);
      setError(null);
      try {
        const data = await client.listApps();
        if (!cancelled) setApps(data);
      } catch (err) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void run();
    return () => {
      cancelled = true;
    };
  }, [reloadToken]);

  const installExample = async () => {
    setInstalling(true);
    setError(null);
    try {
      const result = validateAppDefinition(bookkeepingExample);
      if (!result.success) {
        setError(result.errors.join("; "));
        return;
      }
      await client.installApp(result.data);
      reload();
    } catch (err) {
      setError(String(err));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-10">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">已安装的 App</h1>
        <Button onClick={installExample} disabled={installing}>
          {installing ? "加载中…" : "加载记账示例"}
        </Button>
      </div>

      {loading ? (
        <LoadingState />
      ) : error ? (
        <ErrorState message={error} onRetry={reload} />
      ) : apps.length === 0 ? (
        <EmptyState message="还没有安装任何 App" actionLabel="加载记账示例" onAction={installExample} />
      ) : (
        <div className="flex flex-col gap-3">
          {apps.map((app) => (
            <Card key={app.id} className="cursor-pointer" onClick={() => onOpen(app.id)}>
              <CardHeader>
                <CardTitle>{app.name}</CardTitle>
              </CardHeader>
              {app.description && <CardContent className="text-muted-foreground text-sm">{app.description}</CardContent>}
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function App() {
  const [activeApp, setActiveApp] = useState<AppRecord | null>(null);

  if (activeApp) {
    return (
      <AppShell appId={activeApp.id} definition={activeApp.definition} onExit={() => setActiveApp(null)} />
    );
  }

  return (
    <AppPicker
      onOpen={(appId) => {
        client.getApp(appId).then(setActiveApp);
      }}
    />
  );
}

export default App;
