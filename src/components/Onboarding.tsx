import { useEffect, useState } from "react";
import { client } from "@/ui-runtime/client";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LoadingState } from "@/ui-runtime/components/LoadingState";
import { ErrorState } from "@/ui-runtime/components/ErrorState";
import { useDebugLocation } from "@/debug/DebugModeContext";

/** First-run onboarding (TODO step 6, "首次安装引导"): confirms the local
 * runtime data directory is ready and shows the AI client config needed to
 * connect the MCP server (`docs/mcp-server.md`) at the same data directory. */
export function Onboarding({ onComplete }: { onComplete: () => void }) {
  useDebugLocation("引导", "src/components/Onboarding.tsx");
  const [dataDir, setDataDir] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [finishing, setFinishing] = useState(false);
  const [copied, setCopied] = useState(false);

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

  const copyConfig = async () => {
    await navigator.clipboard.writeText(mcpConfig);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const finish = async () => {
    setFinishing(true);
    try {
      await client.completeOnboarding();
      onComplete();
    } catch (err) {
      setError(String(err));
      setFinishing(false);
    }
  };

  if (error) {
    return (
      <div className="mx-auto max-w-2xl p-10">
        <ErrorState message={error} />
      </div>
    );
  }

  if (!dataDir) {
    return (
      <div className="mx-auto max-w-2xl p-10">
        <LoadingState />
      </div>
    );
  }

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-10">
      <div>
        <h1 className="text-2xl font-semibold">欢迎使用 Applet{version ? ` v${version}` : ""}</h1>
        <p className="text-muted-foreground mt-1 text-sm">
          Applet 是一个 AI-native 桌面运行时：App 不是手写代码，而是 AI 通过声明式定义创建、Applet
          安装并渲染的。开始之前，先确认本地环境已就绪。
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>本地数据库</CardTitle>
          <CardDescription>已在首次启动时自动初始化，所有 App 的数据都存放在这里</CardDescription>
        </CardHeader>
        <CardContent>
          <code className="bg-muted block rounded-md px-3 py-2 text-xs break-all">{dataDir}</code>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>连接 AI 客户端（MCP）</CardTitle>
          <CardDescription>
            把下面的配置粘贴进 Claude Desktop / Claude Code 等 MCP 客户端的配置文件，替换
            command 为实际构建出的 mcp_server 路径（见 docs/mcp-server.md），即可让 AI 直接创建和管理
            App。
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <pre className="bg-muted overflow-x-auto rounded-md px-3 py-2 text-xs">{mcpConfig}</pre>
          <Button variant="outline" size="sm" className="self-start" onClick={copyConfig}>
            {copied ? "已复制" : "复制配置"}
          </Button>
        </CardContent>
      </Card>

      <Alert>
        <AlertTitle>这一步可以跳过</AlertTitle>
        <AlertDescription>
          不接 AI 客户端也能直接在桌面 App 里手动安装和使用示例 App；随时可以回来完成 MCP 配置。
        </AlertDescription>
      </Alert>

      <Button onClick={finish} disabled={finishing} className="self-end">
        {finishing ? "准备中…" : "开始使用"}
      </Button>
    </div>
  );
}
