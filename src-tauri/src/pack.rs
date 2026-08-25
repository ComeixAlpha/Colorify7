//! 打包器

use std::io::{BufWriter, Write};
use std::path::Path;

use uuid::Uuid;

use crate::image_processor;

pub fn write_manifests(
    dir: &Path,
    name: &str,
    auth: &str,
    desc: &str,
    include_rp: bool,
    include_script: bool,
) -> Result<(), String> {
    let uuid_bp = Uuid::new_v4().to_string();
    let uuid_rp = Uuid::new_v4().to_string();
    let uuid_data = Uuid::new_v4().to_string();
    let uuid_script = Uuid::new_v4().to_string();
    let uuid_res = Uuid::new_v4().to_string();
    let description = format!("[§bColorify§f] {desc}");

    let mut modules = vec![serde_json::json!({
        "type": "data",
        "uuid": uuid_data,
        "version": [1, 0, 0]
    })];
    if include_script {
        modules.push(serde_json::json!({
            "type": "script",
            "uuid": uuid_script,
            "entry": "scripts/index.js",
            "version": [1, 0, 0]
        }));
    }
    let mut deps = vec![serde_json::json!({
        "module_name": "@minecraft/server",
        "version": "1.10.0"
    })];
    if include_rp {
        deps.push(serde_json::json!({ "uuid": uuid_rp, "version": "1.0.0" }));
    }

    let bp_json = serde_json::json!({
        "format_version": 2,
        "header": {
            "name": name,
            "description": description,
            "uuid": uuid_bp,
            "min_engine_version": [1, 19, 50],
            "version": [1, 0, 0]
        },
        "modules": modules,
        "dependencies": deps,
        "metadata": {
            "authors": [auth],
            "generated_with": { "colorify": ["6.1.8"] }
        }
    });

    let bp_path = if include_rp {
        dir.join("behaviour_pack/manifest.json")
    } else {
        dir.join("manifest.json")
    };
    let bp_text =
        serde_json::to_string_pretty(&bp_json).map_err(|e| format!("序列化清单失败: {e}"))?;
    std::fs::write(bp_path, bp_text).map_err(|e| format!("写入清单失败: {e}"))?;

    if include_rp {
        let rp_json = serde_json::json!({
            "format_version": 2,
            "header": {
                "name": name,
                "description": description,
                "uuid": uuid_rp,
                "min_engine_version": [1, 19, 50],
                "version": [1, 0, 0]
            },
            "modules": [
                { "type": "resources", "uuid": uuid_res, "version": [1, 0, 0] }
            ],
            "dependencies": [
                { "uuid": uuid_bp, "version": "1.0.0" }
            ],
            "metadata": {
                "authors": [auth],
                "generated_with": { "colorify": ["6.1.8"] }
            }
        });
        let rp_path = dir.join("resources_pack/manifest.json");
        let rp_text =
            serde_json::to_string_pretty(&rp_json).map_err(|e| format!("序列化清单失败: {e}"))?;
        std::fs::write(rp_path, rp_text).map_err(|e| format!("写入清单失败: {e}"))?;
    }

    Ok(())
}

/// `haoziiy/avatar_generator`
const PATCH_SET: &[&[[f32; 2]]] = &[
    &[[0., 0.], [4., 0.], [4., 4.], [0., 4.]], // 0
    &[[0., 0.], [4., 0.], [0., 4.]],           // 1
    &[[2., 0.], [4., 4.], [0., 4.]],           // 2
    &[[0., 0.], [2., 0.], [2., 4.], [0., 4.]], // 3
    &[[2., 0.], [4., 2.], [2., 4.], [0., 2.]], // 4
    &[[0., 0.], [4., 2.], [4., 4.], [2., 4.]], // 5
    &[
        [2., 0.],
        [4., 4.],
        [2., 4.],
        [3., 2.],
        [1., 2.],
        [2., 4.],
        [0., 4.],
    ], // 6
    &[[0., 0.], [4., 2.], [2., 4.]],           // 7
    &[[1., 1.], [3., 1.], [3., 3.], [1., 3.]], // 8
    &[[2., 0.], [4., 0.], [0., 4.], [0., 2.], [2., 2.]], // 9
    &[[0., 0.], [2., 0.], [2., 2.], [0., 2.]], // 10
    &[[0., 2.], [4., 2.], [2., 4.]],           // 11
    &[[2., 2.], [4., 4.], [0., 4.]],           // 12
    &[[2., 0.], [2., 2.], [0., 2.]],           // 13
    &[[0., 0.], [2., 0.], [0., 2.]],           // 14
    &[],                                       // 15 空白
];

/// 中间补丁只允许这 4 种形状
const MIDDLE_PATCH_SET: [usize; 4] = [0, 4, 8, 15];

/// 每个补丁格边长（3×3 -> 144×144）
const PATCH_SIZE: u32 = 48;
const SIZE: u32 = PATCH_SIZE * 3;

/// 时间哈希
pub fn time_based_code() -> u32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let lo = now as u32;
    let hi = (now >> 32) as u32;
    lo ^ hi.rotate_left(13)
}

/// 渲染
pub fn identicon_png(code: u32) -> Option<Vec<u8>> {
    let middle_type = MIDDLE_PATCH_SET[(code & 0x03) as usize];
    let middle_invert = ((code >> 2) & 0x01) != 0;
    let corner_type = ((code >> 3) & 0x0F) as usize;
    let corner_invert = ((code >> 7) & 0x01) != 0;
    let corner_turn = ((code >> 8) & 0x03) as u32;
    let side_type = ((code >> 10) & 0x0F) as usize;
    let side_invert = ((code >> 14) & 0x01) != 0;
    let side_turn = ((code >> 15) & 0x03) as u32;
    let red = (((code >> 27) & 0x1F) << 3) as u8;
    let green = (((code >> 21) & 0x1F) << 3) as u8;
    let blue = (((code >> 16) & 0x1F) << 3) as u8;
    let fore = [red, green, blue];
    let back = [255u8, 255, 255];

    let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
    for py in 0..3u32 {
        for px in 0..3u32 {
            let (patch_type, turn, invert) = if px == 1 && py == 1 {
                (middle_type, 0u32, middle_invert)
            } else if px == 1 || py == 1 {
                let i = match (px, py) {
                    (1, 0) => 0u32,
                    (2, 1) => 1,
                    (1, 2) => 2,
                    _ => 3, // (0, 1)
                };
                (side_type, (side_turn + 1 + i) % 4, side_invert)
            } else {
                let i = match (px, py) {
                    (0, 0) => 0u32,
                    (2, 0) => 1,
                    (2, 2) => 2,
                    _ => 3,
                };
                (corner_type, (corner_turn + 1 + i) % 4, corner_invert)
            };

            let (cell_bg, cell_fg) = if invert { (fore, back) } else { (back, fore) };
            let blank = PATCH_SET[patch_type].is_empty();

            for y in 0..PATCH_SIZE {
                for x in 0..PATCH_SIZE {
                    let gx = px * PATCH_SIZE + x;
                    let gy = py * PATCH_SIZE + y;
                    let u = (x as f32 + 0.5) / PATCH_SIZE as f32;
                    let v = (y as f32 + 0.5) / PATCH_SIZE as f32;
                    let inside = if blank {
                        false
                    } else {
                        let (ru, rv) = rotate_around_center(u, v, (4 - turn) % 4);
                        point_in_polygon(ru, rv, PATCH_SET[patch_type])
                    };
                    let c = if inside { cell_fg } else { cell_bg };
                    let idx = ((gy * SIZE + gx) * 4) as usize;
                    buf[idx] = c[0];
                    buf[idx + 1] = c[1];
                    buf[idx + 2] = c[2];
                    buf[idx + 3] = 255;
                }
            }
        }
    }
    image_processor::encode_rgba_png(SIZE, SIZE, &buf)
}

fn rotate_around_center(x: f32, y: f32, turn: u32) -> (f32, f32) {
    let (dx, dy) = (x - 0.5, y - 0.5);
    match turn % 4 {
        0 => (x, y),
        1 => (0.5 - dy, 0.5 + dx),
        2 => (0.5 - dx, 0.5 - dy),
        _ => (0.5 + dy, 0.5 - dx),
    }
}

fn point_in_polygon(px: f32, py: f32, poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (xi, yi) = (poly[i][0] / 4.0, poly[i][1] / 4.0);
        let (xj, yj) = (poly[j][0] / 4.0, poly[j][1] / 4.0);
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
    }
    inside
}

/// 压缩
pub fn zip_dir(source: &Path, dest: &Path) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let file = std::fs::File::create(dest).map_err(|e| format!("创建压缩文件失败: {e}"))?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    fn walk(
        dir: &Path,
        base: &Path,
        zip: &mut zip::ZipWriter<BufWriter<std::fs::File>>,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("读取目录失败: {e}"))? {
            let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
            let path = entry.path();
            let rel = path
                .strip_prefix(base)
                .map_err(|_| "路径错误".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                zip.add_directory(rel, options)
                    .map_err(|e| format!("压缩目录失败: {e}"))?;
                walk(&path, base, zip, options)?;
            } else {
                zip.start_file(rel, options)
                    .map_err(|e| format!("压缩文件失败: {e}"))?;
                let data = std::fs::read(&path).map_err(|e| format!("读取文件失败: {e}"))?;
                zip.write_all(&data)
                    .map_err(|e| format!("写入压缩流失败: {e}"))?;
            }
        }
        Ok(())
    }

    walk(source, source, &mut zip, options)?;
    zip.finish().map_err(|e| format!("完成压缩失败: {e}"))?;
    Ok(())
}

pub fn script_ticking_area(
    scripts_dir: &Path,
    file_count: usize,
    size: [i32; 3],
) -> Result<(), String> {
    std::fs::create_dir_all(scripts_dir).map_err(|e| format!("创建脚本目录失败: {e}"))?;

    fn chain(i: usize, file_count: usize) -> String {
        if i + 1 == file_count {
            format!(
                "entity.runCommand('function output_{i}');\nServer.system.run(() => {{\n  entity.runCommand(`tickingarea remove colorify`);\n}});"
            )
        } else {
            let inner = chain(i + 1, file_count);
            format!(
                "entity.runCommand('function output_{i}');\nServer.system.run(() => {{\n{inner}\n}});"
            )
        }
    }
    let run_commands = if file_count > 0 {
        chain(0, file_count)
    } else {
        String::new()
    };

    let script = format!(
        r#"import * as Server from "@minecraft/server";

Server.system.runInterval(() => {{
  const entities = Server.world.getDimension('overworld').getEntities();
  for (let entity of entities) {{
    if (entity.nameTag == 'block' && !entity.hasTag('blocked')) {{
      entity.runCommand(`tickingarea add ~ ~ ~ ~{dx} ~{dy} ~{dz} colorify`);
      entity.addTag('blocked');
      Server.system.runTimeout(() => {{
{run_commands}
      }}, 10);
    }}
  }}
}});
"#,
        dx = size[0] - 1,
        dy = size[1] - 1,
        dz = size[2] - 1,
    );

    std::fs::write(scripts_dir.join("index.js"), script).map_err(|e| format!("写入脚本失败: {e}"))
}
