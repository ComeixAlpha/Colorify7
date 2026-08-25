//! 3D 建筑管线：OBJ -> 体素 -> 方块 -> 导出 / WebSocket

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

use crate::block3d::block_assigner::DitherMode;
use crate::block3d::block_mesh::{self, FallableBehaviour};
use crate::generator;
use crate::obj3d::gltf_importer;
use crate::obj3d::mesh::{MaterialKind, Mesh};
use crate::obj3d::obj_importer::{self, ImportOptions};
use crate::obj3d::vec3::Vec3;
use crate::obj3d::voxel_mesh::VoxelOverlapRule;
use crate::obj3d::voxeliser::{self, Axis, VoxeliserKind};
use crate::params::ProgressMessage;
use crate::process::{output_dir, ProgressSink};
use crate::ws;

/// 预览方块（前端渲染）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewBlock {
    /// 调色板 ID
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub colour: [f32; 3],
}

/// 预览结果：方块列表 + 包围盒（含端点）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub blocks: Vec<PreviewBlock>,
    pub min: [i32; 3],
    pub max: [i32; 3],
}

/// OBJ 原始网格预览
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjMeshPreview {
    pub vertices: Vec<f32>,
    pub colors: Vec<f32>,
}

#[derive(Default)]
pub struct Process3dState {
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<PreviewResult>>>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjParams {
    /// 模型文件路径（PC）
    pub obj_path: String,
    /// 模型字节（安卓）
    #[serde(default)]
    pub obj_data: Option<Vec<u8>>,
    pub rotation: [f32; 3],
    pub constraint_axis: String,
    pub algorithm: String,
    #[serde(default = "default_true")]
    pub solid: bool,
    pub size: u32,
    pub use_multisample_colouring: bool,
    pub voxel_overlap_rule: String,
    pub dithering: String,
    pub dithering_magnitude: f32,
    pub resolution: u32,
    pub contextual_averaging: bool,
    pub error_weight: f32,
    pub fallable: String,
    pub use_struct: bool,
    pub auto_slice_mcfunction: bool,
    pub offset_x: Option<i32>,
    pub offset_y: Option<i32>,
    pub offset_z: Option<i32>,
    pub use_socket: bool,
    pub ws_command_delay: i32,

    #[serde(default)]
    pub use_ldb: bool,
    pub world_path: Option<String>,
    pub origin_x: Option<i32>,
    pub origin_y: Option<i32>,
    pub origin_z: Option<i32>,
}

/// 处理 OBJ
#[tauri::command]
pub fn process_obj(
    app: tauri::AppHandle,
    state: tauri::State<'_, Process3dState>,
    ws_state: tauri::State<'_, Arc<ws::WsState>>,
    params: ObjParams,
    on_progress: Channel<ProgressMessage>,
) -> Result<(), String> {
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let result = state.result.clone();
    let ws_state = ws_state.inner().clone();

    thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(move || {
            let progress = ProgressSink::new(&on_progress, &cancel);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_pipeline(&app, &ws_state, &params, &progress, &result)
            }));
            match result {
                Ok(Ok(Some(out_dir))) => {
                    eprintln!("[process3d] 已输出到: {}", out_dir.display());
                    progress.finish("完成", Some(out_dir.display().to_string()));
                }
                Ok(Ok(None)) => {
                    eprintln!("[process3d] 已通过 WebSocket 发送命令");
                    progress.finish("已通过 WebSocket 发送", None);
                }
                Ok(Err(msg)) => {
                    eprintln!("[process3d] 失败: {msg}");
                    if msg != "已取消" {
                        progress.finish(&msg, None);
                    }
                }
                Err(panic) => {
                    let msg = panic
                        .downcast_ref::<&str>()
                        .copied()
                        .unwrap_or("未知内部错误");
                    eprintln!("[process3d] 内部错误: {msg}");
                    progress.finish(&format!("内部错误: {msg}"), None);
                }
            }
        })
        .expect("创建处理线程失败");

    Ok(())
}

/// 取消当前任务
#[tauri::command]
pub fn cancel_obj_process(state: tauri::State<'_, Process3dState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

/// 取走最近一次结果
#[tauri::command]
pub fn get_obj_result(state: tauri::State<'_, Process3dState>) -> Option<PreviewResult> {
    state.result.lock().ok()?.take()
}

/// 按扩展名分发导入；统一应用居中/旋转
fn import_model_file(path: &Path, options: &ImportOptions) -> Result<Mesh, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let mesh = match ext.as_str() {
        "obj" => obj_importer::import_obj_file(path, options)?,
        "glb" | "gltf" => {
            let mut m = gltf_importer::import_gltf_file_with(
                path,
                options.decode_textures,
                options.max_tris,
            )?;
            if options.centre {
                m.centre();
            }
            let r = options.rotation;
            if r != Vec3::ZERO {
                m.rotate(r.x, r.y, r.z);
            }
            m
        }
        _ => return Err(format!("不支持的模型格式: {ext}（支持 obj / glb / gltf）")),
    };
    Ok(mesh)
}

/// 从内存字节导入（安卓）
fn import_model_file_bytes(data: &[u8], options: &ImportOptions) -> Result<Mesh, String> {
    let mut m =
        gltf_importer::import_gltf_bytes(data.to_vec(), options.decode_textures, options.max_tris)?;
    if options.centre {
        m.centre();
    }
    let r = options.rotation;
    if r != Vec3::ZERO {
        m.rotate(r.x, r.y, r.z);
    }
    Ok(m)
}

/// 解析模型
#[tauri::command]
pub async fn get_obj_mesh(path: String, data: Option<Vec<u8>>) -> Result<ObjMeshPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let options = ImportOptions {
            rotation: Vec3::ZERO,
            centre: true,
            // 跳过贴图解码
            decode_textures: false,
            max_tris: MAX_PREVIEW_TRIS,
        };
        let mesh = match data {
            Some(bytes) => import_model_file_bytes(&bytes, &options)?,
            None => import_model_file(Path::new(&path), &options)?,
        };
        build_obj_preview(&mesh)
    })
    .await
    .map_err(|e| format!("预览线程异常: {e}"))?
}

/// 预览最多保留的三角形数
const MAX_PREVIEW_TRIS: usize = 150_000;

/// 构建可渲染预览
/// 超过 MAX_PREVIEW_TRIS 时均匀降采样
fn build_obj_preview(mesh: &Mesh) -> Result<ObjMeshPreview, String> {
    // 预计算显示色
    let mat_colours: Vec<[f32; 4]> = mesh
        .materials
        .iter()
        .map(|mat| {
            if mat.kind == MaterialKind::Textured {
                mat.texture
                    .as_ref()
                    .map(|t| t.average_colour())
                    .unwrap_or(mat.colour)
            } else {
                mat.colour
            }
        })
        .collect();

    let total = mesh.tris.len();
    // 均匀降采样
    let stride = (total / MAX_PREVIEW_TRIS).max(1);
    let kept = (total + stride - 1) / stride;

    let mut vertices = Vec::with_capacity(kept * 9);
    let mut colors = Vec::with_capacity(kept * 9);
    for (i, tri) in mesh.tris.iter().enumerate() {
        if i % stride != 0 {
            continue;
        }
        let c = mat_colours[tri.material];
        for &vi in &tri.position {
            let v = mesh.vertices[vi];
            // NaN/无穷钳制到原点
            let (x, y, z) = if v.x.is_finite() && v.y.is_finite() && v.z.is_finite() {
                (v.x, v.y, v.z)
            } else {
                (0.0, 0.0, 0.0)
            };
            vertices.extend_from_slice(&[x, y, z]);
            colors.extend_from_slice(&[c[0], c[1], c[2]]);
        }
    }
    Ok(ObjMeshPreview { vertices, colors })
}

/// 管线主体
fn run_pipeline(
    app: &tauri::AppHandle,
    ws_state: &Arc<ws::WsState>,
    params: &ObjParams,
    progress: &ProgressSink,
    result: &Arc<Mutex<Option<PreviewResult>>>,
) -> Result<Option<PathBuf>, String> {
    // 导入
    progress.stage("导入模型");
    if progress.cancelled() {
        return Err("已取消".into());
    }
    let options = ImportOptions {
        rotation: Vec3::new(params.rotation[0], params.rotation[1], params.rotation[2]),
        centre: true,
        ..Default::default()
    };
    let mesh = match &params.obj_data {
        Some(bytes) => import_model_file_bytes(bytes, &options)?,
        None => import_model_file(Path::new(&params.obj_path), &options)?,
    };

    // 体素化
    progress.stage("体素化");
    if progress.cancelled() {
        return Err("已取消".into());
    }
    let voxel_mesh = voxeliser::voxelise(
        &mesh,
        &voxeliser::VoxeliseParams {
            algorithm: match params.algorithm.as_str() {
                "bvh-ray" => VoxeliserKind::BvhRay,
                _ => VoxeliserKind::Triplane,
            },
            constraint_axis: match params.constraint_axis.as_str() {
                "x" => Axis::X,
                "z" => Axis::Z,
                _ => Axis::Y,
            },
            size: params.size.max(1),
            use_multisample_colouring: params.use_multisample_colouring,
            solid: params.solid,
            overlap_rule: if params.voxel_overlap_rule == "first" {
                VoxelOverlapRule::First
            } else {
                VoxelOverlapRule::Average
            },
        },
    )?;

    // 方块匹配
    progress.stage("匹配方块");
    if progress.cancelled() {
        return Err("已取消".into());
    }
    let bm = block_mesh::assign(
        &voxel_mesh,
        &block_mesh::AssignParams {
            dithering: match params.dithering.as_str() {
                "random" => DitherMode::Random,
                "ordered" => DitherMode::Ordered,
                _ => DitherMode::Off,
            },
            dithering_magnitude: params.dithering_magnitude,
            resolution: params.resolution.max(1),
            contextual_averaging: params.contextual_averaging,
            error_weight: params.error_weight.clamp(0.0, 1.0),
            fallable: match params.fallable.as_str() {
                "replace-fallable" => FallableBehaviour::ReplaceFallable,
                "do-nothing" => FallableBehaviour::DoNothing,
                _ => FallableBehaviour::ReplaceFalling,
            },
        },
        progress.cancel(),
    )?;

    // 写入预览结果
    *result.lock().map_err(|_| "预览结果锁失效".to_string())? = Some(PreviewResult {
        blocks: bm
            .blocks
            .iter()
            .map(|b| PreviewBlock {
                name: b.name.clone(),
                x: b.position.x as i32,
                y: b.position.y as i32,
                z: b.position.z as i32,
                colour: [
                    b.colour[0] / 255.0,
                    b.colour[1] / 255.0,
                    b.colour[2] / 255.0,
                ],
            })
            .collect(),
        min: [bm.min.x as i32, bm.min.y as i32, bm.min.z as i32],
        max: [bm.max.x as i32, bm.max.y as i32, bm.max.z as i32],
    });

    // 导出
    let out_dir = if params.use_socket || params.use_ldb {
        None
    } else {
        Some(output_dir(app)?)
    };
    progress.stage(if params.use_socket {
        "生成命令"
    } else if params.use_struct {
        "输出结构文件"
    } else {
        "输出函数"
    });
    if progress.cancelled() {
        return Err("已取消".into());
    }

    let refs: Vec<(i32, i32, i32, &str)> = bm
        .blocks
        .iter()
        .map(|b| {
            (
                b.position.x as i32,
                b.position.y as i32,
                b.position.z as i32,
                b.name.as_str(),
            )
        })
        .collect();

    // 直写 LevelDB 世界
    if params.use_ldb {
        let world_path = params.world_path.as_deref().ok_or("未选择世界路径")?;
        let origin = [
            params.origin_x.unwrap_or(0),
            params.origin_y.unwrap_or(0),
            params.origin_z.unwrap_or(0),
        ];
        let offset = [
            params.offset_x.unwrap_or(0),
            params.offset_y.unwrap_or(0),
            params.offset_z.unwrap_or(0),
        ];
        let blocks: Vec<(i32, i32, i32, String)> = refs
            .iter()
            .map(|(x, y, z, n)| {
                (
                    x + offset[0],
                    y + offset[1],
                    z + offset[2],
                    (*n).to_string(),
                )
            })
            .collect();
        crate::ldb::write_blocks_to_world(world_path, 0, origin, &blocks, progress)?;
        return Ok(None);
    }

    let commands = generator::export_blocks_3d(
        &refs,
        params.use_struct && !params.use_socket,
        params.auto_slice_mcfunction,
        [
            params.offset_x.unwrap_or(0),
            params.offset_y.unwrap_or(0),
            params.offset_z.unwrap_or(0),
        ],
        params.use_socket,
        out_dir.as_deref(),
    )?;

    if params.use_socket {
        let commands = commands.ok_or("socket 模式缺少命令")?;
        let delay = params.ws_command_delay.max(1) as u64;
        tauri::async_runtime::block_on(ws_state.task_start(commands, delay))
            .map_err(|e| format!("WebSocket 发送失败: {e}"))?;
        return Ok(None);
    }
    Ok(out_dir)
}
