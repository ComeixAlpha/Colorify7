import "@fontsource-variable/google-sans-flex";
import { invoke } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";
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
      <App initialSettings={settings} />
    </React.StrictMode>,
  );

  showWindowWhenReady();
}

void bootstrap();
