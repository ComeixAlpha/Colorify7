//! 像素画生成管线

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use image::DynamicImage;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

use crate::common;
use crate::generator;
use crate::image_ditherer;
use crate::image_processor;
use crate::pack;
use crate::params::ProgressMessage;
use crate::process::{run_art_pipeline, spawn_art_task, ArtGenerator, ProgressSink};
use crate::ws;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PixelParams {
    pub resize_x: Option<u32>,
    pub resize_y: Option<u32>,
    pub resize_interpolation: String,
    pub generation_plane: String,
    pub color_distance_formula: String,
    pub use_staircase: bool,
    pub use_struct: bool,
    pub use_dithering: bool,

    #[serde(default)]
    pub use_socket: bool,
    pub dithering_algorithm: String,
    pub carpet_only: bool,
    pub wool_only: bool,
    pub no_glass: bool,
    pub no_sands_and_powders: bool,
    pub offset_x: Option<i32>,
    pub offset_y: Option<i32>,
    pub offset_z: Option<i32>,
    pub staircase_gap: i32,
    pub ws_command_delay: i32,

    #[serde(default)]
    pub auto_slice_mcfunction: bool,
    pub preview_image: bool,
    pub websocket_port: Option<u16>,

    pub pk_name: Option<String>,
    pub pk_auth: Option<String>,
    pub pk_desc: Option<String>,

    #[serde(default)]
    pub use_ldb: bool,
    pub world_path: Option<String>,
    pub origin_x: Option<i32>,
    pub origin_y: Option<i32>,
    pub origin_z: Option<i32>,
}

#[derive(Default)]
pub struct PixelProcessState {
    cancel: Arc<AtomicBool>,
}

pub(crate) struct PixelPipeline {
    params: PixelParams,
    palette: Vec<image_ditherer::PaletteColorInput>,
}

impl ArtGenerator for PixelPipeline {
    fn generate(
        &self,
        img: &mut DynamicImage,
        out_dir: Option<&Path>,
        progress: &ProgressSink,
    ) -> Result<Option<Vec<String>>, String> {
        let params = &self.params;
        let palette = &self.palette;

        let need_pack = !params.use_socket
            && (params.pk_name.as_deref().is_some_and(|s| !s.is_empty())
                || params.pk_auth.as_deref().is_some_and(|s| !s.is_empty())
                || params.pk_desc.as_deref().is_some_and(|s| !s.is_empty()));

        // 目录
        let content_dir = if need_pack {
            let zp = out_dir.expect("打包必须文件模式").join("colorified");
            Some(if params.use_struct {
                zp.join("structures")
            } else {
                zp.join("functions")
            })
        } else {
            out_dir.map(Path::to_path_buf)
        };

        if need_pack {
            progress.stage("初始化包体");
            let zp = out_dir.expect("打包必须文件模式").join("colorified");
            std::fs::create_dir_all(content_dir.as_deref().unwrap())
                .map_err(|e| format!("创建包目录失败: {e}"))?;
            let name = params
                .pk_name
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("Colorified");
            let auth = params
                .pk_auth
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("Comeix Alpha");
            let desc = params
                .pk_desc
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("Colorify 像素画");
            pack::write_manifests(&zp, name, auth, desc, false, !params.use_struct)?;
        }

        // 抖动前捕获透明度
        let alpha = if img.color().has_alpha() {
            Some(img.to_rgba8().pixels().map(|p| p[3]).collect::<Vec<u8>>())
        } else {
            None
        };

        // 抖动
        if params.use_dithering && !params.use_staircase {
            progress.stage("抖动处理");
            if progress.cancelled() {
                return Err("已取消".into());
            }
            let colors = image_ditherer::palette_colors(palette);
            if colors.is_empty() {
                eprintln!("[pixel] 无可用调色板，跳过抖动");
            } else if image_ditherer::dither_image(
                img,
                &colors,
                &params.dithering_algorithm,
                &params.color_distance_formula,
            )
            .is_none()
            {
                eprintln!("[pixel] 抖动失败");
            }
        }

        if progress.cancelled() {
            return Err("已取消".into());
        }

        // 匹配方块
        progress.stage("匹配方块");
        let gen_params = generator::GenParams {
            plane: common::plane_index(&params.generation_plane),
            color_distance: params.color_distance_formula.clone(),
            use_staircase: params.use_staircase,
            use_struct: if params.use_socket {
                false
            } else {
                params.use_struct
            },
            staircase_gap: params.staircase_gap.max(1),
            offset: [
                params.offset_x.unwrap_or(0),
                params.offset_y.unwrap_or(0),
                params.offset_z.unwrap_or(0),
            ],
            auto_slice_mcfunction: params.auto_slice_mcfunction,
            use_dithering: params.use_dithering,
            socket_output: params.use_socket,
            ldb_output: params.use_ldb,
        };
        let output = generator::generate(
            img,
            alpha.as_deref(),
            palette,
            &gen_params,
            content_dir.as_deref(),
            progress.cancel(),
            &mut |s: &str| progress.stage(s),
        )?;

        // 直写 LevelDB 世界
        if params.use_ldb {
            let blocks = output.blocks_ldb.ok_or("ldb 模式缺少方块")?;
            let world_path = params.world_path.as_deref().ok_or("未选择世界路径")?;
            let origin = [
                params.origin_x.unwrap_or(0),
                params.origin_y.unwrap_or(0),
                params.origin_z.unwrap_or(0),
            ];
            crate::ldb::write_blocks_to_world(world_path, 0, origin, &blocks, progress)?;
            return Ok(None);
        }

        // socket
        if params.use_socket {
            return Ok(output.commands);
        }

        let out_dir = out_dir.expect("文件模式必须有输出目录");

        // 预览图（可选）
        if params.preview_image {
            progress.stage("生成预览图");
            if let Some(png) =
                image_processor::encode_rgba_png(img.width(), img.height(), &output.preview)
            {
                let preview_path = out_dir.join("preview.png");
                std::fs::write(&preview_path, &png).map_err(|e| format!("预览输出失败: {e}"))?;
            } else {
                eprintln!("[pixel] 预览编码失败");
            }
        }

        // 打包
        if need_pack {
            let zp = out_dir.join("colorified");
            if !params.use_struct {
                pack::script_ticking_area(&zp.join("scripts"), output.function_files, output.size)?;
            }
            if let Some(icon) = pack::identicon_png(pack::time_based_code()) {
                std::fs::write(zp.join("pack_icon.png"), &icon)
                    .map_err(|e| format!("写入包图标失败: {e}"))?;
            }
            progress.stage("打包 .mcpack");
            pack::zip_dir(&zp, &out_dir.join("colorified.mcpack"))
                .map_err(|e| format!("打包失败: {e}"))?;
        }

        Ok(None)
    }
}

#[tauri::command]
pub fn process_image(
    app: tauri::AppHandle,
    state: tauri::State<'_, PixelProcessState>,
    ws_state: tauri::State<'_, Arc<ws::WsState>>,
    image: Vec<u8>,
    params: PixelParams,
    palette: Vec<image_ditherer::PaletteColorInput>,
    on_progress: Channel<ProgressMessage>,
) -> Result<(), String> {
    // 重置取消标记
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let ws_state = ws_state.inner().clone();

    let resize_x = params.resize_x;
    let resize_y = params.resize_y;
    let interpolation = params.resize_interpolation.clone();
    let use_socket = params.use_socket;
    let use_ldb = params.use_ldb;
    let ws_delay = params.ws_command_delay.max(1) as u64;

    spawn_art_task(
        app,
        cancel,
        ws_state,
        on_progress,
        "pixel",
        move |app, ws, progress| {
            let pipeline = PixelPipeline { params, palette };
            run_art_pipeline(
                app,
                ws,
                image,
                resize_x,
                resize_y,
                &interpolation,
                None,
                use_socket,
                use_ldb,
                ws_delay,
                &pipeline,
                progress,
            )
        },
    )
}

#[tauri::command]
pub fn cancel_process(state: tauri::State<'_, PixelProcessState>) {
    state.cancel.store(true, Ordering::SeqCst);
}
