import { useEffect, useState } from "react";
import { Settings as SettingsIcon, Trash2, RefreshCw } from "lucide-react";
import { validateAppDefinition } from "@/app-schema/validate";
import bookkeepingExample from "@/app-schema/examples/bookkeeping.json";
import { client, type AppRecord } from "@/ui-runtime/client";
import { AppShell } from "@/ui-runtime/components/AppShell";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/ui-runtime/components/EmptyState";
import { ErrorState } from "@/ui-runtime/components/ErrorState";
import { LoadingState } from "@/ui-runtime/components/LoadingState";
import { Onboarding } from "@/components/Onboarding";
import { Settings } from "@/components/Settings";
import { UpdateChecker } from "@/components/UpdateChecker";
import { useDebugLocation } from "@/debug/DebugModeContext";

// Rust's AppDefinition mirror (src-tauri/src/app_schema.rs) serializes absent
// optional fields (label, required, unique, description, ...) back out as
// explicit `null`, while the TS/Zod side simply omits them. Strip nullish
// values on both sides before comparing so that round-trip noise from the
// registry isn't mistaken for a real definition change.
function normalizeForCompare(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(normalizeForCompare);
  if (value && typeof value === "object") {
    return Object.keys(value)
      .sort()
      .reduce((acc: Record<string, unknown>, k) => {
        const v = (value as Record<string, unknown>)[k];
        if (v !== null && v !== undefined) acc[k] = normalizeForCompare(v);
        return acc;
      }, {});
  }
  return value;
}

function stableStringify(value: unknown): string {
  return JSON.stringify(normalizeForCompare(value));
}

function AppPicker({ onOpen, onOpenSettings }: { onOpen: (appId: string) => void; onOpenSettings: () => void }) {
  useDebugLocation("App 列表", "src/App.tsx", { component: "AppPicker" });
  const [apps, setApps] = useState<AppRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const [updatingId, setUpdatingId] = useState<string | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [uninstallingId, setUninstallingId] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const reload = () => setReloadToken((t) => t + 1);
  const installedExample = apps.find((app) => app.id === bookkeepingExample.id);
  const exampleInstalled = installedExample !== undefined;
  const exampleValidation = validateAppDefinition(bookkeepingExample);
  const exampleOutdated =
    installedExample !== undefined &&
    exampleValidation.success &&
    stableStringify(installedExample.definition) !== stableStringify(exampleValidation.data);

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
    setInstallError(null);
    try {
      const result = validateAppDefinition(bookkeepingExample);
      if (!result.success) {
        setInstallError(result.errors.join("; "));
        return;
      }
      await client.installApp(result.data);
      reload();
    } catch (err) {
      setInstallError(String(err));
    } finally {
      setInstalling(false);
    }
  };

  const updateExample = async (force = false) => {
    if (!exampleValidation.success) {
      setUpdateError(exampleValidation.errors.join("; "));
      return;
    }
    setUpdatingId(bookkeepingExample.id);
    setUpdateError(null);
    try {
      await client.updateApp(bookkeepingExample.id, exampleValidation.data, force);
      reload();
    } catch (err) {
      setUpdateError(String(err));
    } finally {
      setUpdatingId(null);
    }
  };

  const uninstallAppById = async (app: AppRecord) => {
    if (!window.confirm(`确定要卸载 "${app.name}" 吗？\n数据不会被删除，重新安装同 id 的 App 后仍会保留。`)) return;
    setUninstallingId(app.id);
    setError(null);
    try {
      await client.uninstallApp(app.id, false);
      reload();
    } catch (err) {
      setError(String(err));
    } finally {
      setUninstallingId(null);
    }
  };

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-10">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">已安装的 App</h1>
        <div className="flex items-center gap-2">
          <UpdateChecker />
          {!exampleInstalled && (
            <Button onClick={installExample} disabled={installing}>
              {installing ? "加载中…" : "加载记账示例"}
            </Button>
          )}
          {exampleOutdated && (
            <Button variant="outline" onClick={() => updateExample(false)} disabled={updatingId === bookkeepingExample.id}>
              <RefreshCw className="size-4" />
              {updatingId === bookkeepingExample.id ? "同步中…" : "示例已更新，点击同步"}
            </Button>
          )}
          <Button variant="ghost" size="icon" aria-label="设置" onClick={onOpenSettings}>
            <SettingsIcon className="size-4" />
          </Button>
        </div>
      </div>

      {installError && <ErrorState message={installError} filePath="src/App.tsx" detail={{ component: "AppPicker" }} />}
      {updateError && (
        <ErrorState
          message={updateError}
          filePath="src/App.tsx"
          detail={{ component: "AppPicker", action: "updateExample" }}
          onRetry={() => {
            if (window.confirm("普通同步失败，通常是因为字段类型变化可能丢失数据。是否强制同步？这可能导致部分数据丢失。")) {
              void updateExample(true);
            }
          }}
        />
      )}

      {loading ? (
        <LoadingState />
      ) : error ? (
        <ErrorState message={error} filePath="src/App.tsx" detail={{ component: "AppPicker" }} onRetry={reload} />
      ) : apps.length === 0 ? (
        <EmptyState message="还没有安装任何 App" actionLabel="加载记账示例" onAction={installExample} />
      ) : (
        <div className="flex flex-col gap-3">
          {apps.map((app) => (
            <Card key={app.id} className="cursor-pointer" onClick={() => onOpen(app.id)}>
              <CardHeader>
                <CardTitle>{app.name}</CardTitle>
                <CardAction>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="卸载"
                    disabled={uninstallingId === app.id}
                    onClick={(e) => {
                      e.stopPropagation();
                      void uninstallAppById(app);
                    }}
                  >
                    <Trash2 className="size-4" />
                  </Button>
                </CardAction>
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
