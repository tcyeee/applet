import { useState } from "react";
import { AlertCircle, Check, Copy } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

function toClipboardText(message: string, filePath?: string, detail?: Record<string, string>): string {
  const lines = [`错误: ${message}`];
  if (filePath) lines.push(`文件: ${filePath}`);
  if (detail) {
    const detailLine = Object.entries(detail)
      .map(([k, v]) => `${k}=${v}`)
      .join(" ");
    if (detailLine) lines.push(`上下文: ${detailLine}`);
  }
  return lines.join("\n");
}

export function ErrorState({
  message,
  filePath,
  detail,
  onRetry,
}: {
  message: string;
  /** Repo-relative path to the component that raised this error, included in
   * the copied report so an AI agent can jump straight to the source. */
  filePath?: string;
  /** Extra key/value context (appId, viewId, ...) included in the copied report. */
  detail?: Record<string, string>;
  onRetry?: () => void;
}) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(toClipboardText(message, filePath, detail));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <Alert variant="destructive">
      <AlertCircle />
      <AlertTitle>出错了</AlertTitle>
      <AlertDescription>
        <p>{message}</p>
        <div className="mt-2 flex gap-2">
          {onRetry && (
            <Button size="sm" variant="outline" onClick={onRetry}>
              重试
            </Button>
          )}
          <Button size="sm" variant="outline" className="gap-1" onClick={copy}>
            {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
            {copied ? "已复制" : "复制调试信息"}
          </Button>
        </div>
      </AlertDescription>
    </Alert>
  );
}
