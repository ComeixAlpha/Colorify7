import "@fontsource-variable/google-sans-flex";
import { invoke } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";
import ErrorBoundary from "./components/ErrorBoundary";
import { initI18n, normalizeLanguage } from "./i18n";
import { AppSettings, DEFAULT_SETTINGS } from "./pages/SettingsPage";
import { Storer } from "./stores/storer";

Storer.registerDefaults("settings", DEFAULT_SETTINGS);

/** 等首帧渲染完成后再通知后端显示主窗口并关闭启动闪屏，避免启动白屏 */
function showWindowWhenReady() {
  let shown = false;
  const show = () => {
    if (shown) return;
    shown = true;
    void invoke("app_ready").catch(() => {});
  };
  // 若 rAF 被抑制则用超时兜底
  requestAnimationFrame(() => requestAnimationFrame(show));
  setTimeout(show, 500);
}

async function bootstrap() {
  const settings = await Storer.loadPref<AppSettings>("settings");
  await initI18n(normalizeLanguage(settings.language));

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ErrorBoundary>
        <App initialSettings={settings} />
      </ErrorBoundary>
    </React.StrictMode>,
  );

  // 通知 index.html 的启动兜底脚本：应用已挂载，可以移除加载提示/取消超时诊断
  window.__colorifyBooted?.();
  showWindowWhenReady();
}

void bootstrap().catch((error) => {
  // 渲染前的失败（设置读取、i18n、动态 import 等）交给启动兜底脚本展示
  window.__colorifyShowError?.(error);
  console.error("[colorify] bootstrap failed:", error);
});
