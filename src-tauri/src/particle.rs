//! 粒子画生成管线

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use image::DynamicImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

use crate::common;
use crate::pack;
use crate::params::ProgressMessage;
use crate::process::{run_art_pipeline, spawn_art_task, ArtGenerator, ProgressSink};
use crate::ws;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleMapping {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub id: String,
}

/// 粒子画生成参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleParams {
    pub resize_x: Option<u32>,
    pub resize_y: Option<u32>,
    pub resize_interpolation: String,
    pub height: Option<f64>,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
    pub rz: Option<f64>,
    pub pk_name: Option<String>,
    pub pk_auth: Option<String>,
    pub pk_desc: Option<String>,
    pub generation_plane: String,
    pub generation_mode: String,
    #[serde(default)]
    pub use_socket: bool,
    #[serde(default)]
    pub ws_command_delay: Option<i32>,
    pub mappings: Vec<ParticleMapping>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Match,
    Dust,
}

#[derive(Default)]
pub struct ParticleProcessState {
    cancel: Arc<AtomicBool>,
}

struct ParticlePoint {
    x: f64,
    y: f64,
    z: f64,
    mapping: Option<u16>,
    rgb: Option<[u8; 3]>,
}

#[tauri::command]
pub fn process_particle(
    app: tauri::AppHandle,
    state: tauri::State<'_, ParticleProcessState>,
    ws_state: tauri::State<'_, Arc<ws::WsState>>,
    image: Vec<u8>,
    params: ParticleParams,
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
    let ws_delay = params.ws_command_delay.unwrap_or(10).max(1) as u64;

    spawn_art_task(
        app,
        cancel,
        ws_state,
        on_progress,
        "particle",
        move |app, ws, progress| {
            let pipeline = ParticlePipeline { params };
            run_art_pipeline(
                app,
                ws,
                image,
                resize_x,
                resize_y,
                &interpolation,
                Some(20000), // 无宽高时默认上限 2w 粒子
                use_socket,
                false, // 粒子暂不支持直写世界
                ws_delay,
                &pipeline,
                progress,
            )
        },
    )
}

/// 取消当前粒子处理任务
#[tauri::command]
pub fn cancel_particle_process(state: tauri::State<'_, ParticleProcessState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

pub(crate) struct ParticlePipeline {
    params: ParticleParams,
}

impl ArtGenerator for ParticlePipeline {
    fn generate(
        &self,
        img: &mut DynamicImage,
        out_dir: Option<&Path>,
        progress: &ProgressSink,
    ) -> Result<Option<Vec<String>>, String> {
        let params = &self.params;

        let mode = if params.use_socket {
            Mode::Match
        } else if params.generation_mode.eq_ignore_ascii_case("dust") {
            Mode::Dust
        } else {
            Mode::Match
        };

        // 匹配粒子
        progress.stage("匹配粒子");
        let points = match_particles(img, mode, &params.mappings, progress.cancel())?;
        if points.is_empty() {
            if params.use_socket {
                return Err("没有生成任何粒子".into());
            }
            return Ok(None);
        }
        if progress.cancelled() {
            return Err("已取消".into());
        }
        let mut points = points;

        // 变换
        progress.stage("变换");
        let rot = if params.rx.is_some() || params.ry.is_some() || params.rz.is_some() {
            Some([
                params.rx.unwrap_or(0.0),
                params.ry.unwrap_or(0.0),
                params.rz.unwrap_or(0.0),
            ])
        } else {
            None
        };
        if let Some(r) = rot {
            for p in points.iter_mut() {
                let v = rotate([p.x, p.y, p.z], r);
                p.x = v[0];
                p.y = v[1];
                p.z = v[2];
            }
        } else {
            let plane = common::plane_index(&params.generation_plane);
            const PARTICLE_PLANE_TO_COMMON: [i32; 3] = [1, 0, 2];
            for p in points.iter_mut() {
                let v =
                    common::switch_xyz(PARTICLE_PLANE_TO_COMMON[plane as usize], [p.x, p.y, 0.0]);
                p.x = v[0];
                p.y = v[1];
                p.z = v[2];
            }
        }

        // 高度缩放
        let heig = params.height.unwrap_or(5.0).max(0.0);
        let factor = heig / img.height() as f64;
        for p in points.iter_mut() {
            p.x *= factor;
            p.y *= factor;
            p.z *= factor;
        }

        let need_pack = mode == Mode::Dust
            || params.pk_name.as_deref().is_some_and(|s| !s.is_empty())
            || params.pk_auth.as_deref().is_some_and(|s| !s.is_empty())
            || params.pk_desc.as_deref().is_some_and(|s| !s.is_empty());

        // socket
        if params.use_socket {
            let commands: Vec<String> = points
                .iter()
                .map(|p| {
                    format!(
                        "particle {} ~{} ~{} ~{}",
                        pid_of(p, &params.mappings),
                        common::fmt_f64(p.x),
                        common::fmt_f64(p.y),
                        common::fmt_f64(p.z)
                    )
                })
                .collect();
            return Ok(Some(commands));
        }

        let out_dir = out_dir.expect("文件模式必须有输出目录");

        // 打包
        if need_pack {
            progress.stage("构建资源包");
            build_pack(out_dir, params, mode, progress.cancel())?;
            if progress.cancelled() {
                return Err("已取消".into());
            }
        }

        // 生成
        if mode == Mode::Match {
            let func_dir = if need_pack {
                out_dir.join("colorified/behaviour_pack/functions")
            } else {
                out_dir.to_path_buf()
            };
            progress.stage("生成函数");
            let file_count = build_functions(&points, &params.mappings, &func_dir, progress)?;
            if need_pack {
                build_script_match(
                    &out_dir.join("colorified/behaviour_pack/scripts"),
                    file_count,
                )?;
            }
        } else {
            progress.stage("生成脚本");
            build_script_dust(
                &points,
                &out_dir.join("colorified/behaviour_pack/scripts"),
                progress,
            )?;
        }

        // 打包
        if need_pack {
            progress.stage("打包 .mcaddon");
            pack::zip_dir(
                &out_dir.join("colorified"),
                &out_dir.join("colorified.mcaddon"),
            )
            .map_err(|e| format!("打包失败: {e}"))?;
        }

        Ok(None)
    }
}

/// 匹配
fn match_particles(
    img: &DynamicImage,
    mode: Mode,
    mappings: &[ParticleMapping],
    cancel: &AtomicBool,
) -> Result<Vec<ParticlePoint>, String> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let rgb = &rgb;
    let points: Vec<ParticlePoint> = (0..w)
        .into_par_iter()
        .flat_map_iter(move |x| {
            (0..h).filter_map(move |y| {
                let p = rgb.get_pixel(x, y);
                let (pr, pg, pb) = (p[0] as i32, p[1] as i32, p[2] as i32);
                let mx = w as f64 / 2.0 - x as f64;
                let my = h as f64 / 2.0 - y as f64;
                match mode {
                    Mode::Dust => Some(ParticlePoint {
                        x: mx,
                        y: my,
                        z: 0.0,
                        mapping: None,
                        rgb: Some([p[0], p[1], p[2]]),
                    }),
                    Mode::Match => {
                        let mut matched: Option<u16> = None;
                        let mut mindist = i32::MAX;
                        for (i, m) in mappings.iter().enumerate() {
                            let dist = (pr - m.r as i32).abs()
                                + (pg - m.g as i32).abs()
                                + (pb - m.b as i32).abs();
                            if dist <= 30 && dist < mindist {
                                matched = Some(i as u16);
                                mindist = dist;
                            }
                        }
                        matched.map(|m| ParticlePoint {
                            x: mx,
                            y: my,
                            z: 0.0,
                            mapping: Some(m),
                            rgb: None,
                        })
                    }
                }
            })
        })
        .collect();

    if cancel.load(Ordering::SeqCst) {
        return Err("已取消".into());
    }
    Ok(points)
}

fn pid_of<'a>(p: &ParticlePoint, mappings: &'a [ParticleMapping]) -> &'a str {
    match p.mapping {
        Some(i) => &mappings[i as usize].id,
        None => "comeix:dust",
    }
}

/// 旋转
fn rotate(v: [f64; 3], fit: [f64; 3]) -> [f64; 3] {
    let len = (fit[0] * fit[0] + fit[1] * fit[1] + fit[2] * fit[2]).sqrt();
    if len < 1e-12 {
        return v;
    }
    let nfit = [fit[0] / len, fit[1] / len, fit[2] / len];

    let nnv = [0.0, 1.0, 0.0];
    let dot = (nnv[0] * nfit[0] + nnv[1] * nfit[1] + nnv[2] * nfit[2]).clamp(-1.0, 1.0);
    let axis = [
        nnv[1] * nfit[2] - nnv[2] * nfit[1],
        nnv[2] * nfit[0] - nnv[0] * nfit[2],
        nnv[0] * nfit[1] - nnv[1] * nfit[0],
    ];
    let axis_len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if axis_len < 1e-12 {
        return if dot > 0.0 { v } else { [v[0], -v[1], -v[2]] };
    }
    let naxis = [axis[0] / axis_len, axis[1] / axis_len, axis[2] / axis_len];
    let angle = dot.acos();

    let c = angle.cos();
    let s = angle.sin();
    let omc = 1.0 - c;
    let (x, y, z) = (naxis[0], naxis[1], naxis[2]);

    let n11 = c + x * x * omc;
    let n12 = x * y * omc - z * s;
    let n13 = x * z * omc + y * s;
    let n21 = y * x * omc + z * s;
    let n22 = c + y * y * omc;
    let n23 = y * z * omc - x * s;
    let n31 = z * x * omc - y * s;
    let n32 = z * y * omc + x * s;
    let n33 = c + z * z * omc;

    [
        n11 * v[0] + n12 * v[1] + n13 * v[2],
        n21 * v[0] + n22 * v[1] + n23 * v[2],
        n31 * v[0] + n32 * v[1] + n33 * v[2],
    ]
}

/// 生成 mcfunction
fn build_functions(
    points: &[ParticlePoint],
    mappings: &[ParticleMapping],
    func_dir: &Path,
    progress: &ProgressSink,
) -> Result<usize, String> {
    std::fs::create_dir_all(func_dir).map_err(|e| format!("创建函数目录失败: {e}"))?;
    let mut index = 0usize;
    let mut line_count = 0usize;
    let mut buf = String::new();
    let mut last_progress = 0usize;

    for (i, p) in points.iter().enumerate() {
        buf.push_str(&format!(
            "particle {} ~{} ~{} ~{}\n",
            pid_of(p, mappings),
            common::fmt_f64(p.x),
            common::fmt_f64(p.y),
            common::fmt_f64(p.z)
        ));
        line_count += 1;
        if line_count >= 10000 {
            std::fs::write(func_dir.join(format!("output_{index}.mcfunction")), &buf)
                .map_err(|e| format!("写入函数失败: {e}"))?;
            index += 1;
            line_count = 0;
            buf.clear();
        }
        // 进度
        let pct = i * 100 / points.len();
        if pct > last_progress {
            last_progress = pct;
            progress.stage(&format!("生成函数 {pct}%"));
        }
    }
    if !buf.is_empty() {
        std::fs::write(func_dir.join(format!("output_{index}.mcfunction")), &buf)
            .map_err(|e| format!("写入函数失败: {e}"))?;
        index += 1;
    }
    Ok(index)
}

/// Match 模式脚本
fn build_script_match(scripts_dir: &Path, file_count: usize) -> Result<(), String> {
    std::fs::create_dir_all(scripts_dir).map_err(|e| format!("创建脚本目录失败: {e}"))?;
    let mut run_commands = String::new();
    for i in 0..file_count {
        run_commands.push_str(&format!("\tentity.runCommand('function output_{i}');"));
        if i != file_count - 1 {
            run_commands.push('\n');
        }
    }
    let script = format!(
        r#"import * as Server from '@minecraft/server';

function paint(entity, tickDelay) {{
  if (!entity.isValid) return;
{run_commands}
  Server.system.runTimeout(() => paint(entity, tickDelay), tickDelay);
}}

Server.system.runInterval(() => {{
  const entities = Server.world.getDimension('overworld').getEntities();
  for (let entity of entities) {{
    if (entity.nameTag.startsWith('particle:') && !entity.hasTag('particled')) {{
      const tickDelay = Number(entity.nameTag.split(':')[1]);
      entity.addTag('particled');
      entity.addEffect('invisibility', 99999, {{ showParticles: false }});
      paint(entity, tickDelay);
    }}
  }}
}});
"#
    );
    std::fs::write(scripts_dir.join("index.js"), script).map_err(|e| format!("写入脚本失败: {e}"))
}

/// Dust 模式脚本
fn build_script_dust(
    points: &[ParticlePoint],
    scripts_dir: &Path,
    progress: &ProgressSink,
) -> Result<(), String> {
    std::fs::create_dir_all(scripts_dir).map_err(|e| format!("创建脚本目录失败: {e}"))?;
    let mut body =
        String::from("import * as Server from '@minecraft/server';\n\nconst particles = [\n");
    let mut last_progress = 0usize;
    for (i, p) in points.iter().enumerate() {
        let [r, g, b] = p.rgb.unwrap_or([255, 255, 255]);
        body.push_str(&format!(
            "\t{{ x: {}, y: {}, z: {}, r: {}, g: {}, b: {} }},\n",
            common::fmt_f64(p.x),
            common::fmt_f64(p.y),
            common::fmt_f64(p.z),
            common::fmt_f64(r as f64 / 255.0),
            common::fmt_f64(g as f64 / 255.0),
            common::fmt_f64(b as f64 / 255.0)
        ));
        let pct = i * 100 / points.len();
        if pct > last_progress {
            last_progress = pct;
            progress.stage(&format!("生成脚本 {pct}%"));
        }
    }
    body.push_str(
        "];\n\nfunction paint(entity, tickDelay) {\n  if (!entity.isValid) return;\n  for (let particle of particles) {\n    const map = new Server.MolangVariableMap();\n    map.setColorRGB(\"variable.rgb\", {\n        red: particle.r,\n        green: particle.g,\n        blue: particle.b,\n    });\n    entity.dimension.spawnParticle(\n      \"comeix:dust\",\n      { x: entity.location.x + particle.x, y: entity.location.y + particle.y, z: entity.location.z + particle.z },\n      map\n    );\n  }\n  Server.system.runTimeout(() => paint(entity, tickDelay), tickDelay);\n}\n\nServer.system.runInterval(() => {\n  const entities = Server.world.getDimension('overworld').getEntities();\n  for (let entity of entities) {\n    if (entity.nameTag.startsWith('particle:') && !entity.hasTag('particled')) {\n      const tickDelay = Number(entity.nameTag.split(':')[1]);\n      entity.addTag('particled');\n      entity.addEffect('invisibility', 99999, { showParticles: false });\n      paint(entity, tickDelay);\n    }\n  }\n});\n",
    );
    std::fs::write(scripts_dir.join("index.js"), body).map_err(|e| format!("写入脚本失败: {e}"))
}

/// 构建 Addon 包目录结构
fn build_pack(
    out_dir: &Path,
    params: &ParticleParams,
    mode: Mode,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let zp = out_dir.join("colorified");
    let bp = zp.join("behaviour_pack");
    let rp = zp.join("resources_pack");
    for dir in [&zp, &bp, &rp, &bp.join("scripts"), &rp.join("particles")] {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建资源包目录失败: {e}"))?;
    }
    if cancel.load(Ordering::SeqCst) {
        return Err("已取消".into());
    }

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
        .unwrap_or("Colorify 粒子画");
    // 清单
    pack::write_manifests(&zp, name, auth, desc, true, true)?;

    // 粒子 JSON
    let particle_dir = rp.join("particles");
    if mode == Mode::Match {
        for m in &params.mappings {
            let file_name = format!("{}.json", m.id.replace(':', "."));
            let json = match_particle_json(m);
            std::fs::write(particle_dir.join(file_name), json)
                .map_err(|e| format!("写入粒子定义失败: {e}"))?;
        }
    } else {
        std::fs::write(particle_dir.join("colorify.dust.json"), DUST_PARTICLE_JSON)
            .map_err(|e| format!("写入粒子定义失败: {e}"))?;
    }

    // Identicon
    if let Some(icon) = pack::identicon_png(pack::time_based_code()) {
        for d in [&bp, &rp] {
            std::fs::write(d.join("pack_icon.png"), &icon)
                .map_err(|e| format!("写入包图标失败: {e}"))?;
        }
    }

    Ok(())
}

/// Match 模式粒子 JSON
fn match_particle_json(m: &ParticleMapping) -> String {
    let id = &m.id;
    let r = m.r as f64 / 255.0;
    let g = m.g as f64 / 255.0;
    let b = m.b as f64 / 255.0;
    let json = serde_json::json!({
        "format_version": "1.10.0",
        "particle_effect": {
            "description": {
                "identifier": id,
                "basic_render_parameters": {
                    "material": "particles_blend",
                    "texture": "textures/particle/particles"
                }
            },
            "components": {
                "minecraft:emitter_rate_instant": { "num_particles": 1 },
                "minecraft:emitter_lifetime_once": { "active_time": 1 },
                "minecraft:emitter_shape_point": {},
                "minecraft:particle_lifetime_expression": { "max_lifetime": 0.6 },
                "minecraft:particle_initial_speed": 0,
                "minecraft:particle_motion_dynamic": {},
                "minecraft:particle_appearance_billboard": {
                    "size": [0.1, 0.1],
                    "facing_camera_mode": "lookat_xyz",
                    "uv": {
                        "texture_width": 128,
                        "texture_height": 128,
                        "uv": [56, 88],
                        "uv_size": [8, 8]
                    }
                },
                "minecraft:particle_appearance_tinting": {
                    "color": [r, g, b, 1]
                }
            }
        }
    });
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
}

/// Dust 粒子
const DUST_PARTICLE_JSON: &str = r#"{
  "format_version": "1.10.0",
  "particle_effect": {
    "description": {
      "identifier": "comeix:dust",
      "basic_render_parameters": {"material": "particles_alpha", "texture": "textures/particle/particles"}
    },
    "components": {
      "minecraft:emitter_initialization": {
        "creation_expression": "variable.size = math.random(0.13, 0.25);variable.radius = 0.6;variable.rgb;"
      },
      "minecraft:emitter_local_space": {"position": true, "rotation": true},
      "minecraft:emitter_rate_instant": {"num_particles": 1},
      "minecraft:emitter_lifetime_once": {"active_time": 1},
      "minecraft:emitter_shape_point": {},
      "minecraft:particle_lifetime_expression": {"max_lifetime": "math.random(2, 4)"},
      "minecraft:particle_initial_speed": 0.15,
      "minecraft:particle_appearance_billboard": {
        "size": ["variable.size*(1-variable.particle_age)", "variable.size*(1-variable.particle_age)"],
        "facing_camera_mode": "rotate_xyz",
        "uv": {
          "texture_width": 128,
          "texture_height": 128,
          "flipbook": {
            "base_UV": ["Math.random(-1, 1) > 0 ? 56 : 48", 0],
            "size_UV": [8, 8],
            "step_UV": [-8, 0],
            "frames_per_second": 8,
            "stretch_to_lifetime": true
          }
        }
      },
      "minecraft:particle_appearance_tinting": {
        "color": ["variable.rgb.r", "variable.rgb.g", "variable.rgb.b", 0.5]
      }
    }
  }
}"#;
