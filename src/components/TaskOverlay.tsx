import {
  M3eButton,
  M3eCircularProgressIndicator,
  M3eIcon,
} from "@m3e/react/all";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

export function formatElapsed(ms: number): string {
  const s = ms / 1000;
  if (s < 60) {
    return `${s.toFixed(1)} s`;
  }
  const m = Math.floor(s / 60);
  const rest = Math.round(s % 60);
  return `${m} min ${rest} s`;
}

export interface TaskOverlayResult {
  elapsedMs?: number;
  outputDir?: string;
}

export interface TaskOverlayProps {
  processing: boolean;
  done: boolean;
  error?: boolean;
  progressText: string;
  wsMode: boolean;
  resultInfo?: TaskOverlayResult;
  wsHint?: string;
  wsGoPageLabel?: string;
  wsNoThanksLabel?: string;
  elapsedLabel?: string;
  outputLabel?: string;
  cancelLabel?: string;
  okLabel?: string;
  /** 「打开文件夹」按钮文案；不传则不显示该按钮 */
  openFolderLabel?: string;
  onCancel?: () => void;
  onDoneOk: () => void;
  onWsGoPage?: () => void;
}

export default function TaskOverlay({
  processing,
  done,
  error = false,
  progressText,
  wsMode,
  resultInfo = {},
  wsHint,
  wsGoPageLabel,
  wsNoThanksLabel,
  elapsedLabel,
  outputLabel,
  cancelLabel = "取消",
  okLabel = "OK",
  openFolderLabel,
  onCancel,
  onDoneOk,
  onWsGoPage,
}: TaskOverlayProps) {
  const outputDir = resultInfo.outputDir;
  const [openFolderError, setOpenFolderError] = useState<string | null>(null);

  useEffect(() => {
    setOpenFolderError(null);
  }, [outputDir]);

  // 仅桌面端
  const canOpenFolder =
    !!openFolderLabel &&
    !!outputDir &&
    !wsMode &&
    !error &&
    typeof window !== "undefined" &&
    !/Android/i.test(navigator.userAgent) &&
    "__TAURI_INTERNALS__" in window;

  async function handleOpenFolder() {
    if (!outputDir) return;
    try {
      await invoke("open_output_dir", { path: outputDir });
      setOpenFolderError(null);
    } catch (err) {
      console.error("open output folder failed:", err);
      setOpenFolderError(typeof err === "string" ? err : String(err));
    }
  }

  if (!processing) return null;
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-md-scrim/60">
      <div className="flex items-center gap-6 rounded-md-xl bg-md-surface-container px-8 py-6 shadow-lg">
        {done ? (
          <>
            <M3eIcon
              name={error ? "error" : "done_all"}
              filled
              className={`text-3xl ${error ? "text-md-error" : "text-md-primary"}`}
            />
            <div className="flex max-w-md flex-col gap-1">
              <span className="text-md-on-surface">{progressText}</span>
              {wsMode && !error ? (
                wsHint && (
                  <span className="text-sm text-md-on-surface-variant">
                    {wsHint}
                  </span>
                )
              ) : (
                <>
                  {resultInfo.elapsedMs != null && elapsedLabel && (
                    <span className="text-sm text-md-on-surface-variant">
                      {elapsedLabel} {formatElapsed(resultInfo.elapsedMs)}
                    </span>
                  )}
                  {resultInfo.outputDir && outputLabel && (
                    <span className="break-all text-sm text-md-on-surface-variant">
                      {outputLabel} {resultInfo.outputDir}
                    </span>
                  )}
                  {openFolderError && (
                    <span className="break-all text-sm text-md-error">
                      {openFolderError}
                    </span>
                  )}
                </>
              )}
            </div>
            {wsMode && !error ? (
              <>
                <M3eButton variant="filled" onClick={onWsGoPage}>
                  {wsGoPageLabel}
                </M3eButton>
                <M3eButton variant="tonal" onClick={onDoneOk}>
                  {wsNoThanksLabel}
                </M3eButton>
              </>
            ) : (
              <>
                {canOpenFolder && (
                  <M3eButton variant="tonal" onClick={handleOpenFolder}>
                    {openFolderLabel}
                  </M3eButton>
                )}
                <M3eButton onClick={onDoneOk}>{okLabel}</M3eButton>
              </>
            )}
          </>
        ) : (
          <>
            <M3eCircularProgressIndicator variant="wavy" indeterminate />
            <span className="min-w-40 text-md-on-surface">{progressText}</span>
            <M3eButton onClick={onCancel}>{cancelLabel}</M3eButton>
          </>
        )}
      </div>
    </div>
  );
}
