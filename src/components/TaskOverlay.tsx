import {
  M3eButton,
  M3eCircularProgressIndicator,
  M3eIcon,
} from "@m3e/react/all";

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
  onCancel,
  onDoneOk,
  onWsGoPage,
}: TaskOverlayProps) {
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
              <M3eButton onClick={onDoneOk}>{okLabel}</M3eButton>
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
