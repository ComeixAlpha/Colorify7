import { M3eIcon } from "@m3e/react/all";

export interface WorldInfo {
  folderName: string;
  folderPath: string;
  levelName: string | null;
}

export interface WorldPickerProps {
  open: boolean;
  worlds: WorldInfo[];
  loading?: boolean;
  error?: string;
  hint?: string;
  onOpenSettings?: () => void;
  settingsLabel?: string;
  title: string;
  emptyText?: string;
  onSelect: (path: string) => void;
  onClose: () => void;
}

export default function WorldPicker({
  open,
  worlds,
  loading = false,
  error,
  hint,
  onOpenSettings,
  settingsLabel = "去设置",
  title,
  emptyText = "未发现世界",
  onSelect,
  onClose,
}: WorldPickerProps) {
  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-md-scrim/60"
      onClick={onClose}
    >
      <div
        className="flex max-h-[70vh] w-96 max-w-[90vw] flex-col rounded-md-xl bg-md-surface-container p-4 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between gap-3 pb-2">
          <span className="font-medium text-md-on-surface">{title}</span>
          <button
            type="button"
            aria-label="close"
            onClick={onClose}
            className="grid size-9 shrink-0 place-items-center rounded-full text-md-on-surface-variant hover:bg-md-surface-container/60 hover:text-md-on-surface focus:outline-none"
          >
            <M3eIcon name="close" />
          </button>
        </div>
        {hint && (
          <p className="mb-2 rounded-md-xl bg-md-surface-container/60 px-3 py-2 text-xs leading-relaxed text-md-on-surface-variant">
            {hint}
          </p>
        )}
        <div className="flex min-h-24 flex-col gap-1 overflow-y-auto">
          {loading ? (
            <span className="py-6 text-center text-sm text-md-on-surface-variant">
              …
            </span>
          ) : error ? (
            <>
              <span className="py-6 text-center text-sm text-md-error">
                {error}
              </span>
              {onOpenSettings && (
                <button
                  type="button"
                  className="mx-auto my-2 rounded-md-xl bg-md-primary-container px-4 py-2 text-sm font-medium text-md-on-primary-container hover:opacity-90 focus:outline-none"
                  onClick={onOpenSettings}
                >
                  {settingsLabel}
                </button>
              )}
            </>
          ) : worlds.length === 0 ? (
            <span className="py-6 text-center text-sm text-md-on-surface-variant">
              {emptyText}
            </span>
          ) : (
            worlds.map((w) => (
              <button
                key={w.folderPath}
                className="flex w-full flex-col gap-0.5 rounded-md-xl px-3 py-2 text-left hover:bg-md-surface-container/60 focus:outline-none"
                onClick={() => onSelect(w.folderPath)}
              >
                <span className="truncate text-sm font-medium text-md-on-surface">
                  {w.levelName || w.folderName}
                </span>
                <span className="truncate text-xs text-md-on-surface-variant">
                  {w.folderPath}
                </span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
