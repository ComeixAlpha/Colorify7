//! 图像处理管线

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use image::DynamicImage;
use tauri::ipc::Channel;
#[cfg(not(target_os = "android"))]
use tauri::Manager;

use crate::image_processor;
use crate::params::ProgressMessage;
use crate::ws;

pub(crate) struct ProgressSink<'a> {
    channel: &'a Channel<ProgressMessage>,
    cancel: &'a AtomicBool,
    start: Instant,
}

impl<'a> ProgressSink<'a> {
    pub(crate) fn new(channel: &'a Channel<ProgressMessage>, cancel: &'a AtomicBool) -> Self {
        Self {
            channel,
            cancel,
            start: Instant::now(),
        }
    }

    pub(crate) fn elapsed(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// 阶段进度
    pub(crate) fn stage(&self, name: &str) {
        let _ = self.channel.send(ProgressMessage {
            stage: name.to_string(),
            finished: false,
            elapsed_ms: None,
            output_dir: None,
            error: None,
        });
    }

    /// 成功
    pub(crate) fn finish(&self, name: &str, output_dir: Option<String>) {
        let _ = self.channel.send(ProgressMessage {
            stage: name.to_string(),
            finished: true,
            elapsed_ms: Some(self.elapsed()),
            output_dir,
            error: None,
        });
    }

    /// 失败
    pub(crate) fn fail(&self, name: &str) {
        let _ = self.channel.send(ProgressMessage {
            stage: name.to_string(),
            finished: true,
            elapsed_ms: Some(self.elapsed()),
            output_dir: None,
            error: Some(name.to_string()),
        });
    }

    pub(crate) fn cancelled(&self) -> bool {
        if self.cancel.load(Ordering::SeqCst) {
            self.finish("已取消", None);
            true
        } else {
            false
        }
    }

    /// 暴露取消标记
    pub(crate) fn cancel(&self) -> &AtomicBool {
        self.cancel
    }
}

/// 生成器
pub(crate) trait ArtGenerator {
    fn generate(
        &self,
        img: &mut DynamicImage,
        out_dir: Option<&Path>,
        progress: &ProgressSink,
    ) -> Result<Option<Vec<String>>, String>;
}

/// 管线主体
pub(crate) fn run_art_pipeline(
    app: &tauri::AppHandle,
    ws_state: &Arc<ws::WsState>,
    image: Vec<u8>,
    resize_x: Option<u32>,
    resize_y: Option<u32>,
    interpolation: &str,
    max_pixels: Option<u64>,
    use_socket: bool,
    use_ldb: bool,
    ws_delay_ms: u64,
    generator: &dyn ArtGenerator,
    progress: &ProgressSink,
) -> Result<Option<PathBuf>, String> {
    // 解码
    progress.stage("读取图片");
    if progress.cancelled() {
        return Err("已取消".into());
    }
    let mut img = image_processor::decode_image(&image).ok_or("图片解码失败")?;

    // 缩放
    progress.stage("缩放图片");
    if progress.cancelled() {
        return Err("已取消".into());
    }
    img = image_processor::resize_image(&img, resize_x, resize_y, interpolation, max_pixels)
        .ok_or("图片缩放失败")?;

    // 输出目录
    let out_dir = if use_socket || use_ldb {
        None
    } else {
        Some(output_dir(app)?)
    };

    // 生成
    let commands = generator.generate(&mut img, out_dir.as_deref(), progress)?;

    // WebSocket 模式
    if let Some(commands) = commands {
        if commands.is_empty() {
            return Err("没有生成任何命令".into());
        }
        progress.stage("发送命令");
        tauri::async_runtime::block_on(ws_state.task_start(commands, ws_delay_ms))
            .map_err(|e| format!("WebSocket 发送失败: {e}"))?;
        return Ok(None);
    }

    Ok(out_dir)
}

pub(crate) fn spawn_art_task(
    app: tauri::AppHandle,
    cancel: Arc<AtomicBool>,
    ws_state: Arc<ws::WsState>,
    on_progress: Channel<ProgressMessage>,
    tag: &'static str,
    task: impl FnOnce(
            &tauri::AppHandle,
            &Arc<ws::WsState>,
            &ProgressSink,
        ) -> Result<Option<PathBuf>, String>
        + Send
        + 'static,
) -> Result<(), String> {
    thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(move || {
            let progress = ProgressSink::new(&on_progress, &cancel);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                task(&app, &ws_state, &progress)
            }));
            match result {
                Ok(Ok(Some(out_dir))) => {
                    eprintln!("[{tag}] 已输出到: {}", out_dir.display());
                    progress.finish("完成", Some(out_dir.display().to_string()));
                }
                Ok(Ok(None)) => {
                    eprintln!("[{tag}] 已通过 WebSocket 发送命令");
                    progress.finish("已通过 WebSocket 发送", None);
                }
                Ok(Err(msg)) => {
                    eprintln!("[{tag}] 失败: {msg}");
                    if msg != "已取消" {
                        progress.fail(&msg);
                    }
                }
                Err(panic) => {
                    let msg = panic
                        .downcast_ref::<&str>()
                        .copied()
                        .unwrap_or("未知内部错误");
                    eprintln!("[{tag}] 内部错误: {msg}");
                    progress.fail(&format!("内部错误: {msg}"));
                }
            }
        })
        .expect("创建处理线程失败");

    Ok(())
}

/// 输出目录：桌面端 Documents/colorify/<时间戳>/；安卓端 Download/colorify/<时间戳>/
pub(crate) fn output_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    let base: PathBuf = {
        let _ = app;
        PathBuf::from("/storage/emulated/0/Download")
    };
    #[cfg(not(target_os = "android"))]
    let base: PathBuf = app
        .path()
        .document_dir()
        .map_err(|_| "无法获取文档目录".to_string())?;

    let dir = base.join("colorify").join(timestamp_folder());
    std::fs::create_dir_all(&dir).map_err(|e| {
        #[cfg(target_os = "android")]
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return "无法写入 Download 目录。请在系统设置中授予 Colorify「所有文件访问」权限（设置 -> 应用 -> Colorify -> 所有文件访问）"
                .to_string();
        }
        format!("创建输出目录失败: {e}")
    })?;
    Ok(dir)
}

/// 检测权限
#[tauri::command]
pub fn check_download_permission() -> bool {
    #[cfg(target_os = "android")]
    {
        std::fs::create_dir_all("/storage/emulated/0/Download/colorify").is_ok()
    }
    #[cfg(not(target_os = "android"))]
    {
        true
    }
}

/// 生成时间戳
fn timestamp_folder() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Howard Hinnant
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}_{hh:02}-{mm:02}-{ss:02}")
}
