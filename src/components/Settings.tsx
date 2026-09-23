import { useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { client } from "@/ui-runtime/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ErrorState } from "@/ui-runtime/components/ErrorState";
import { LoadingState } from "@/ui-runtime/components/LoadingState";
import { ArrowLeft } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { useDebugLocation, useDebugMode } from "@/debug/DebugModeContext";

/** Settings entry reachable from the app picker home screen. Read-only: shows
 * where the runtime keeps its data and how to point an MCP client at it (see
 * `docs/mcp-server.md`). Mirrors the info `Onboarding.tsx` shows on first run,
 * so users can come back to it later without redoing onboarding. */
export function Settings({ onBack }: { onBack: () => void }) {
  useDebugLocation("设置", "src/components/Settings.tsx");
  const { enabled: debugEnabled, toggle: toggleDebug } = useDebugMode();
  const [dataDir, setDataDir] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copiedDataDir, setCopiedDataDir] = useState(false);
  const [copiedMcpConfig, setCopiedMcpConfig] = useState(false);

  useEffect(() => {
    let cancelled = false;
    client
      .getRuntimeInfo()
      .then((info) => {
        if (cancelled) return;
        setDataDir(info.dataDir);
        setVersion(info.version);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const mcpConfig = dataDir
    ? JSON.stringify(
        {
          mcpServers: {
            applet: {
              command: "/path/to/applet/src-tauri/target/release/mcp_server",
              env: { APPLET_DATA_DIR: dataDir },
            },
          },
        },
        null,
        2,
      )
    : "";

  const copy = async (text: string, setCopied: (v: boolean) => void) => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-10">
      <div className="flex items-center gap-3">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft className="size-4" />
          返回
        </Button>
        <h1 className="text-2xl font-semibold">设置{version ? ` · v${version}` : ""}</h1>
      </div>

      {error ? (
        <ErrorState message={error} />
      ) : !dataDir ? (
        <LoadingState />
      ) : (
        <>
          <Card>
            <CardHeader>
              <CardTitle>数据存储位置</CardTitle>
              <CardDescription>所有 App 的数据（注册表、数据库、文件、备份）都存放在这里</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <code className="bg-muted block rounded-md px-3 py-2 text-xs break-all">{dataDir}</code>
              <div className="flex gap-2">
                <Button variant="outline" size="sm" onClick={() => revealItemInDir(dataDir)}>
                  在文件管理器中打开
                </Button>
                <Button variant="outline" size="sm" onClick={() => copy(dataDir, setCopiedDataDir)}>
                  {copiedDataDir ? "已复制" : "复制路径"}
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>连接 AI 客户端（MCP）</CardTitle>
              <CardDescription>
                把下面的配置粘贴进 Claude Desktop / Claude Code 等 MCP 客户端的配置文件，替换
                command 为实际构建出的 mcp_server 路径（见 docs/mcp-server.md），即可让 AI 直接创建和管理
                App。MCP server 需要指向和桌面 App 相同的数据目录，已在下面的 env 中自动填好。
              </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <pre className="bg-muted overflow-x-auto rounded-md px-3 py-2 text-xs">{mcpConfig}</pre>
              <Button
                variant="outline"
                size="sm"
                className="self-start"
                onClick={() => copy(mcpConfig, setCopiedMcpConfig)}
              >
                {copiedMcpConfig ? "已复制" : "复制配置"}
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>调试模式</CardTitle>
              <CardDescription>
                开启后，窗口底部常驻一个小提示条，显示当前页面对应的源码文件路径，可一键复制发给 AI
                助手定位代码，省去口头描述的过程。
              </CardDescription>
            </CardHeader>
            <CardContent>
              <label className="flex w-fit items-center gap-2 text-sm">
                <Checkbox checked={debugEnabled} onCheckedChange={(v) => toggleDebug(v === true)} />
                启用调试模式
              </label>
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
