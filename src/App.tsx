import { useEffect, useState } from "react";
import { Settings as SettingsIcon } from "lucide-react";
import { validateAppDefinition } from "@/app-schema/validate";
import bookkeepingExample from "@/app-schema/examples/bookkeeping.json";
import { client, type AppRecord } from "@/ui-runtime/client";
import { AppShell } from "@/ui-runtime/components/AppShell";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/ui-runtime/components/EmptyState";
import { ErrorState } from "@/ui-runtime/components/ErrorState";
import { LoadingState } from "@/ui-runtime/components/LoadingState";
import { Onboarding } from "@/components/Onboarding";
import { Settings } from "@/components/Settings";
import { UpdateChecker } from "@/components/UpdateChecker";
import { useDebugLocation } from "@/debug/DebugModeContext";

function AppPicker({ onOpen, onOpenSettings }: { onOpen: (appId: string) => void; onOpenSettings: () => void }) {
  useDebugLocation("App 列表", "src/App.tsx", { component: "AppPicker" });
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
        <div className="flex items-center gap-2">
          <UpdateChecker />
          <Button onClick={installExample} disabled={installing}>
            {installing ? "加载中…" : "加载记账示例"}
          </Button>
          <Button variant="ghost" size="icon" aria-label="设置" onClick={onOpenSettings}>
            <SettingsIcon className="size-4" />
          </Button>
        </div>
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
  const [showSettings, setShowSettings] = useState(false);
  const [onboarded, setOnboarded] = useState<boolean | null>(null);

  useEffect(() => {
    client.isOnboardingComplete().then(setOnboarded);
  }, []);

  if (onboarded === null) {
    return (
      <div className="mx-auto max-w-2xl p-10">
        <LoadingState />
      </div>
    );
  }

  if (!onboarded) {
    return <Onboarding onComplete={() => setOnboarded(true)} />;
  }

  if (activeApp) {
    return (
      <AppShell appId={activeApp.id} definition={activeApp.definition} onExit={() => setActiveApp(null)} />
    );
  }

  if (showSettings) {
    return <Settings onBack={() => setShowSettings(false)} />;
  }

  return (
    <AppPicker
      onOpen={(appId) => {
        client.getApp(appId).then(setActiveApp);
      }}
      onOpenSettings={() => setShowSettings(true)}
    />
  );
}

export default App;
