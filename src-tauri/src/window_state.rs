use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const WINDOW_STATE_FILE: &str = "window-state.json";
const WINDOW_LABEL: &str = "main";

#[derive(Serialize, Deserialize)]
struct WindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn state_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(WINDOW_STATE_FILE))
}

/// 启动时恢复窗口状态(默认最大化)
pub fn restore(app: &tauri::App) {
    let Some(path) = state_path(app.handle()) else {
        return;
    };

    let state = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<WindowState>(&content).ok());

    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };

    let Some(state) = state else {
        let _ = window.maximize();
        return;
    };

    if is_on_some_monitor(&window, state.x, state.y) {
        let _ = window.set_position(PhysicalPosition::new(state.x, state.y));
    }
    let _ = window.set_size(PhysicalSize::new(state.width, state.height));

    if state.maximized {
        let _ = window.maximize();
    }
}

/// 关闭前保存窗口状态
pub fn save(window: &WebviewWindow) {
    let maximized = window.is_maximized().unwrap_or(false);
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };

    let state = WindowState {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
        maximized,
    };

    let Some(path) = state_path(window.app_handle()) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = fs::write(path, json);
    }
}

/// 校验保存的位置是否仍落在某个可用显示器内（防止拔掉外接显示器后窗口跑到屏幕外）
fn is_on_some_monitor(window: &WebviewWindow, x: i32, y: i32) -> bool {
    match window.available_monitors() {
        Ok(monitors) => monitors.iter().any(|m| {
            let p = m.position();
            let s = m.size();
            x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
        }),
        Err(_) => true,
    }
}
