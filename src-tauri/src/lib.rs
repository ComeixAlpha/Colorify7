//! 后端入口

mod block3d;
mod common;
mod generator;
mod image_ditherer;
mod image_processor;
mod nbt;
mod obj3d;
mod pack;
mod params;
mod particle;
mod pixel;
mod process;
mod process3d;
#[cfg(desktop)]
mod window_state;
mod ws;

#[cfg(desktop)]
use tauri::Manager;

/// 前端首帧渲染完成后调用，显示主窗口并关闭启动闪屏
#[tauri::command]
fn app_ready(_app: tauri::AppHandle) {
    #[cfg(desktop)]
    {
        if let Some(main) = _app.get_webview_window("main") {
            let _ = main.show();
            let _ = main.set_focus();
        }
        if let Some(splash) = _app.get_webview_window("splash") {
            let _ = splash.close();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(pixel::PixelProcessState::default())
        .manage(process3d::Process3dState::default())
        .manage(particle::ParticleProcessState::default())
        .manage(std::sync::Arc::new(ws::WsState::default()))
        .setup(|app| {
            // 恢复上次关闭前的窗口大小/位置
            #[cfg(desktop)]
            window_state::restore(app);

            // 主窗口在 tauri.conf.json 中配置为初始隐藏，启动时先显示 splash 闪屏窗口，
            // 前端渲染完成后再由前端显示主窗口并关闭闪屏；
            // 移动端（Android）直接跳过
            #[cfg(desktop)]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_background_color(Some(tauri::window::Color(17, 19, 24, 255)));
                let fallback_main = window.clone();
                let fallback_splash = app.get_webview_window("splash");
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    let _ = fallback_main.show();
                    if let Some(splash) = fallback_splash {
                        let _ = splash.close();
                    }
                });
            }
            #[cfg(not(desktop))]
            let _ = app;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关闭前记录窗口大小与位置
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(w) = window.app_handle().get_webview_window("main") {
                    window_state::save(&w);
                }
            }
            #[cfg(not(desktop))]
            let _ = (window, event);
        })
        .invoke_handler(tauri::generate_handler![
            app_ready,
            process::check_download_permission,
            pixel::process_image,
            pixel::cancel_process,
            particle::process_particle,
            particle::cancel_particle_process,
            process3d::process_obj,
            process3d::cancel_obj_process,
            process3d::get_obj_result,
            process3d::get_obj_mesh,
            ws::ws_launch,
            ws::ws_close,
            ws::ws_broadcast,
            ws::ws_status,
            ws::ws_task_start,
            ws::ws_task_pause,
            ws::ws_task_resume,
            ws::ws_task_stop,
            ws::ws_task_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
