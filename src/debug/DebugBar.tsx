import { useState } from "react";
import { Copy, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { DebugLocationInfo } from "./DebugModeContext";

function toClipboardText(location: DebugLocationInfo): string {
  const lines = [location.filePath];
  if (location.detail) {
    const detail = Object.entries(location.detail)
      .map(([k, v]) => `${k}=${v}`)
      .join(" ");
    if (detail) lines.push(detail);
  }
  return lines.join("\n");
}

export function DebugBar({ location }: { location: DebugLocationInfo | null }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    if (!location) return;
    await navigator.clipboard.writeText(toClipboardText(location));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="bg-foreground text-background fixed inset-x-0 bottom-0 z-50 flex items-center gap-2 px-3 py-1.5 text-xs shadow-lg">
      <span className="rounded bg-amber-500 px-1.5 py-0.5 font-semibold text-black">调试模式</span>
      {location ? (
        <>
          <span className="truncate font-mono">{location.filePath}</span>
          {location.detail && (
            <span className="text-background/60 truncate">
              {Object.entries(location.detail)
                .map(([k, v]) => `${k}=${v}`)
                .join(" ")}
            </span>
          )}
          <Button
            variant="secondary"
            size="sm"
            className="ml-auto h-6 gap-1 px-2 text-xs"
            onClick={copy}
          >
            {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
            {copied ? "已复制" : "复制路径"}
          </Button>
        </>
      ) : (
        <span className="text-background/60">未识别当前页面</span>
      )}
    </div>
  );
}
