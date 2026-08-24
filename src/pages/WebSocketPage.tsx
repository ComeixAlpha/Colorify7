import { M3eButton, M3eIcon, M3eLinearProgressIndicator } from "@m3e/react/all";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Storer } from "../stores/storer";
import { AppSettings } from "./SettingsPage";

interface WsStatus {
  running: boolean;
  port: number;
  connections: number;
}

type TaskState = "idle" | "running" | "paused";

interface TaskStatus {
  state: TaskState;
  sent: number;
  total: number;
}

const IDLE_TASK: TaskStatus = { state: "idle", sent: 0, total: 0 };

const UWP_LOOPBACK_URL = "https://www.minebbs.com/threads/uwp.17877/";

export default function WebSocketPage() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<WsStatus | null>(null);
  const [task, setTask] = useState<TaskStatus>(IDLE_TASK);
  const [logs, setLogs] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState<"connect" | "link" | null>(null);

  // 监听游戏消息
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      unlisten = await listen<string>("ws-message", (e) => {
        if (!cancelled) setLogs((prev) => [...prev.slice(-199), e.payload]);
      });
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // 轮询服务器与任务状态
  useEffect(() => {
    let cancelled = false;
    async function refresh() {
      try {
        const s = await invoke<WsStatus>("ws_status");
        if (cancelled) return;
        setStatus(s);
        setTask(
          s.running ? await invoke<TaskStatus>("ws_task_status") : IDLE_TASK,
        );
      } catch {
        setStatus(
          (prev) => prev ?? { running: false, port: 0, connections: 0 },
        );
      }
    }
    void refresh();
    const id = setInterval(() => void refresh(), 500);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  async function handleLaunch() {
    setBusy(true);
    setError("");
    try {
      const port = Storer.load<AppSettings>("settings").webSocketPort;
      const s = await invoke<WsStatus>("ws_launch", { port });
      setStatus(s);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy(value: string, key: "connect" | "link") {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(key);
      window.setTimeout(() => setCopied((c) => (c === key ? null : c)), 1500);
    } catch {}
  }

  async function handleCloseServer() {
    setError("");
    try {
      await invoke("ws_close");
      setStatus({ running: false, port: 0, connections: 0 });
      setLogs([]);
      setTask(IDLE_TASK);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    }
  }

  async function handlePause() {
    try {
      setTask(await invoke<TaskStatus>("ws_task_pause"));
    } catch {}
  }

  async function handleResume() {
    try {
      setTask(await invoke<TaskStatus>("ws_task_resume"));
    } catch {}
  }

  async function handleStopTask() {
    try {
      setTask(await invoke<TaskStatus>("ws_task_stop"));
    } catch {}
  }

  // 状态未知
  if (!status) {
    return <div className="flex h-full w-full items-center justify-center" />;
  }

  // 未启动
  if (!status.running) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-5 overflow-y-auto p-6">
        <div className="flex max-w-full flex-col items-center gap-3 text-center">
          <p className="text-base text-md-on-surface-variant">
            {t("pages.ws.notRunningHint1")}
          </p>
          <div className="flex max-w-full items-center gap-2 rounded-md-xl bg-md-surface-container px-4 py-2 font-mono text-sm text-md-on-surface">
            <span className="min-w-0 break-all text-left">
              {UWP_LOOPBACK_URL}
            </span>
            <button
              type="button"
              onClick={() => handleCopy(UWP_LOOPBACK_URL, "link")}
              aria-label={t("pages.ws.copyCommand")}
              className="flex shrink-0 cursor-pointer items-center gap-1 rounded-lg px-2.5 py-1.5 font-sans text-sm transition-colors hover:bg-md-surface-container-high"
            >
              <M3eIcon
                name={copied === "link" ? "check" : "content_copy"}
                className="text-base"
              />
              {copied === "link"
                ? t("pages.ws.commandCopied")
                : t("pages.ws.copyCommand")}
            </button>
          </div>
          <p className="whitespace-pre-line text-sm leading-relaxed text-md-on-surface-variant">
            {t("pages.ws.notRunningHint2")}
          </p>
        </div>
        <M3eButton variant="filled" disabled={busy} onClick={handleLaunch}>
          <M3eIcon slot="icon" name="power_settings_new" filled />
          {t("pages.ws.launchServer")}
        </M3eButton>
        {error && <span className="text-sm text-md-error">{error}</span>}
      </div>
    );
  }

  const running = task.state === "running";
  const paused = task.state === "paused";

  return (
    <div className="relative flex h-full w-full flex-col gap-4 overflow-hidden p-6">
      {/* 无连接实例 */}
      {status.connections === 0 && (
        <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-6 bg-black/60">
          <span className="max-w-md px-6 text-center text-base text-white/90">
            {t("pages.ws.noConnectionsTitle")}
          </span>
          <div className="flex items-center gap-2 rounded-md-xl bg-white/10 py-2 pl-4 pr-2 font-mono text-base text-white">
            <span className="whitespace-nowrap">
              /connect 127.0.0.1:{status.port}
            </span>
            <button
              type="button"
              onClick={() =>
                handleCopy(`/connect 127.0.0.1:${status.port}`, "connect")
              }
              aria-label={t("pages.ws.copyCommand")}
              className="flex shrink-0 cursor-pointer items-center gap-1 rounded-lg px-3 py-1.5 font-sans text-sm transition-colors hover:bg-white/10"
            >
              <M3eIcon
                name={copied === "connect" ? "check" : "content_copy"}
                className="text-base"
              />
              {copied === "connect"
                ? t("pages.ws.commandCopied")
                : t("pages.ws.copyCommand")}
            </button>
          </div>
        </div>
      )}

      {/* 连接实例数 */}
      <div className="flex shrink-0 items-center justify-between rounded-md-xl bg-md-surface-container px-5 py-4">
        <div className="flex items-center gap-3">
          <M3eIcon name="cable" className="text-2xl text-md-primary" />
          <span className="font-medium">{t("pages.ws.connections")}</span>
        </div>
        <span className="text-3xl font-bold text-md-primary">
          {status.connections}
        </span>
      </div>

      {/* 任务进度 */}
      <div className="flex shrink-0 flex-col gap-2 rounded-md-xl bg-md-surface-container px-5 py-4">
        <div className="flex items-center justify-between">
          <span className="font-medium">{t("pages.ws.taskProgress")}</span>
          <span className="text-sm text-md-on-surface-variant">
            {task.sent} / {task.total}
            {task.total > 0 &&
              ` (${Math.round((task.sent / task.total) * 100)}%)`}
          </span>
        </div>
        <M3eLinearProgressIndicator
          value={task.sent}
          variant="wavy"
          max={Math.max(task.total, 1)}
        />
        <div className="mt-2 flex gap-2">
          {running ? (
            <M3eButton variant="tonal" onClick={handlePause}>
              <M3eIcon slot="icon" name="pause" filled />
              {t("pages.ws.pauseTask")}
            </M3eButton>
          ) : paused ? (
            <M3eButton variant="tonal" onClick={handleResume}>
              <M3eIcon slot="icon" name="play_arrow" filled />
              {t("pages.ws.resumeTask")}
            </M3eButton>
          ) : null}
          <M3eButton
            variant="outlined"
            disabled={!running && !paused}
            onClick={handleStopTask}
          >
            <M3eIcon slot="icon" name="stop" />
            {t("pages.ws.stopTask")}
          </M3eButton>
        </div>
      </div>

      {/* 终端 */}
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md-xl bg-md-surface-container">
        <div className="flex items-center justify-between border-b border-md-outline-variant px-4 py-2">
          <span className="font-medium">{t("pages.ws.terminal")}</span>
          <span className="text-xs text-md-on-surface-variant">
            {t("pages.ws.logCount", { count: logs.length })}
          </span>
        </div>
        <div className="flex-1 overflow-y-auto p-3 font-mono text-xs leading-relaxed text-md-on-surface-variant scrollbar-thin">
          {logs.length === 0 ? (
            <span className="text-md-on-surface-variant/60">
              {t("pages.ws.noLogs")}
            </span>
          ) : (
            logs.map((l, i) => (
              <div key={i} className="break-all whitespace-pre-wrap">
                {l}
              </div>
            ))
          )}
        </div>
      </div>

      {/* 关闭服务器 */}
      <div className="flex shrink-0 justify-end">
        <M3eButton variant="filled" onClick={handleCloseServer}>
          <M3eIcon slot="icon" name="link_off" filled />
          {t("pages.ws.closeServer")}
        </M3eButton>
      </div>
      {error && <span className="shrink-0 text-sm text-md-error">{error}</span>}
    </div>
  );
}
