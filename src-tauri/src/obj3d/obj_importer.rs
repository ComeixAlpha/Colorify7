//! OBJ / MTL 解析
//!
//! OBJ 支持：`v` / `vt` / `vn` / `f`（三角与 n 边扇形三角化、负索引）/ `usemtl` / `mtllib`
//! MTL 支持：`newmtl` / `Kd` / `d` / `map_Kd`；贴图支持 png/jpg/tga

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::mesh::*;
use super::vec3::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct ImportOptions {
    pub rotation: Vec3,
    pub centre: bool,
    pub decode_textures: bool,
    pub max_tris: usize,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            rotation: Vec3::ZERO,
            centre: true,
            decode_textures: true,
            max_tris: 0,
        }
    }
}

/// 从文件导入
pub fn import_obj_file(path: &Path, options: &ImportOptions) -> Result<Mesh, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let base_dir = path.parent().unwrap_or(Path::new("."));
    parse_obj(&text, base_dir, options)
}

/// 解析 OBJ 文本（`.mtl` 与贴图以 `base_dir` 为基准目录）
pub fn parse_obj(source: &str, base_dir: &Path, options: &ImportOptions) -> Result<Mesh, String> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source); // 去 UTF-8 BOM
    let mut p = Parser::new(base_dir);
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        p.parse_line(line)
            .map_err(|e| format!("第 {} 行: {e}", i + 1))?;
    }
    p.finish(options)
}

/// MTL 材质定义
#[derive(Debug, Default)]
struct MtlDef {
    kd: Option<[f32; 3]>,
    map_kd: Option<PathBuf>,
    alpha: Option<f32>,
}

struct Parser {
    vertices: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<UV>,
    tris: Vec<Tri>,
    materials: Vec<Material>,
    mat_index: HashMap<String, usize>,
    current_mat: usize,
    mtl_files: Vec<PathBuf>,
    base_dir: PathBuf,
}

impl Parser {
    fn new(base_dir: &Path) -> Self {
        // 索引 0 为默认材质，usemtl 前创建的面都指向它
        let materials = vec![Material {
            name: "DEFAULT_UNASSIGNED".into(),
            kind: MaterialKind::Solid,
            colour: [1.0, 1.0, 1.0, 1.0],
            texture: None,
            filtering: TextureFiltering::Nearest,
            wrap: TextureWrap::Repeat,
            transparency: Transparency::None,
        }];
        let mut mat_index = HashMap::new();
        mat_index.insert("DEFAULT_UNASSIGNED".to_string(), 0);
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            tris: Vec::new(),
            materials,
            mat_index,
            current_mat: 0,
            mtl_files: Vec::new(),
            base_dir: base_dir.to_path_buf(),
        }
    }

    fn parse_line(&mut self, line: &str) -> Result<(), String> {
        let mut it = line.split_whitespace();
        let tag = it.next().unwrap();
        match tag {
            "v" => self.vertices.push(parse_vec3(&mut it)?),
            "vn" => self.normals.push(parse_vec3(&mut it)?),
            "vt" => {
                let u = next_f32(&mut it)?;
                let v = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                self.uvs.push(UV::new(u, v));
            }
            "f" => self.parse_face(it.collect())?,
            "usemtl" => self.use_material(&it.collect::<Vec<_>>().join(" ")),
            "mtllib" => {
                for name in it {
                    self.mtl_files.push(self.base_dir.join(name));
                }
            }
            // o/g/s/l 及未知标签忽略
            _ => {}
        }
        Ok(())
    }

    /// usemtl：存在则切换；不存在则建占位纯色材质（MTL 合并时再补全）
    fn use_material(&mut self, name: &str) {
        if let Some(&idx) = self.mat_index.get(name) {
            self.current_mat = idx;
            return;
        }
        let idx = self.materials.len();
        self.materials.push(Material {
            name: name.to_string(),
            kind: MaterialKind::Solid,
            colour: [0.5, 0.5, 0.5, 1.0],
            texture: None,
            filtering: TextureFiltering::Nearest,
            wrap: TextureWrap::Repeat,
            transparency: Transparency::None,
        });
        self.mat_index.insert(name.to_string(), idx);
        self.current_mat = idx;
    }

    /// 解析面：`v` / `v/vt` / `v//vn` / `v/vt/vn`；n 边扇形三角化；负索引从末尾计数
    fn parse_face(&mut self, tokens: Vec<&str>) -> Result<(), String> {
        let (nv, nvt, nvn) = (
            self.vertices.len() as i64,
            self.uvs.len() as i64,
            self.normals.len() as i64,
        );
        let mut corners = Vec::with_capacity(tokens.len());
        for token in tokens {
            let parts: Vec<&str> = token.split('/').collect();
            let pos = parse_index(parts[0], nv)?;
            let tex = parts
                .get(1)
                .filter(|s| !s.is_empty())
                .map(|s| parse_index(s, nvt))
                .transpose()?;
            let nor = parts
                .get(2)
                .filter(|s| !s.is_empty())
                .map(|s| parse_index(s, nvn))
                .transpose()?;
            corners.push((pos, tex, nor));
        }
        if corners.len() < 3 {
            return Err("面至少需要 3 个顶点".into());
        }
        // 扇形三角化：0-1-2, 0-2-3, ...
        for i in 1..corners.len() - 1 {
            let (a, b, c) = (corners[0], corners[i], corners[i + 1]);
            self.tris.push(Tri {
                position: [a.0, b.0, c.0],
                texcoord: match (a.1, b.1, c.1) {
                    (Some(x), Some(y), Some(z)) => Some([x, y, z]),
                    _ => None,
                },
                colour: None,
                normal: match (a.2, b.2, c.2) {
                    (Some(x), Some(y), Some(z)) => Some([x, y, z]),
                    _ => None,
                },
                material: self.current_mat,
            });
        }
        Ok(())
    }

    /// 收尾：加载 MTL 合并材质 -> 居中 / 旋转；预览模式可截断三角形
    fn finish(mut self, options: &ImportOptions) -> Result<Mesh, String> {
        if self.vertices.is_empty() {
            return Err("未解析到任何顶点".into());
        }
        if self.tris.is_empty() {
            return Err("未解析到任何三角形".into());
        }
        self.merge_mtls(options.decode_textures)?;

        // 预览模式：超过 max_tris 时均匀采样（保留全部顶点，只减三角形）
        if options.max_tris > 0 && self.tris.len() > options.max_tris {
            let stride = (self.tris.len() / options.max_tris).max(1);
            self.tris = self.tris.iter().step_by(stride).copied().collect();
        }

        let mut mesh = Mesh {
            vertices: self.vertices,
            normals: self.normals,
            uvs: self.uvs,
            vertex_colors: Vec::new(),
            tris: self.tris,
            materials: self.materials,
        };
        if options.centre {
            mesh.centre();
        }
        mesh.rotate(options.rotation.x, options.rotation.y, options.rotation.z);
        Ok(mesh)
    }

    /// 加载所有 mtllib，按 newmtl 名合并进占位材质
    fn merge_mtls(&mut self, decode_textures: bool) -> Result<(), String> {
        let mut defs: HashMap<String, MtlDef> = HashMap::new();
        for path in &self.mtl_files {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
            parse_mtl(&text, path.parent().unwrap_or(&self.base_dir), &mut defs)?;
        }
        for mat in &mut self.materials {
            if let Some(def) = defs.remove(&mat.name) {
                apply_mtl_def(mat, def, decode_textures)?;
            }
        }
        Ok(())
    }
}

/// 解析 MTL：`newmtl` / `Kd` / `d` / `map_Kd`
fn parse_mtl(text: &str, base_dir: &Path, out: &mut HashMap<String, MtlDef>) -> Result<(), String> {
    let mut current: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let tag = it.next().unwrap();
        match tag {
            "newmtl" => {
                let name = it.collect::<Vec<_>>().join(" ");
                out.entry(name.clone()).or_default();
                current = Some(name);
            }
            "Kd" => {
                let (r, g, b) = (next_f32(&mut it)?, next_f32(&mut it)?, next_f32(&mut it)?);
                if let Some(name) = &current {
                    out.entry(name.clone()).or_default().kd = Some([r, g, b]);
                }
            }
            "d" => {
                if let Some(name) = &current {
                    if let Ok(a) = it.next().unwrap_or("1").parse::<f32>() {
                        out.entry(name.clone()).or_default().alpha = Some(a.clamp(0.0, 1.0));
                    }
                }
            }
            "map_Kd" => {
                // 取最后一个 token 作路径（兼容 `map_Kd -s 1 1 1 tex.png` 这类带选项写法）
                if let (Some(name), Some(file)) = (current.as_ref(), it.last()) {
                    out.entry(name.clone()).or_default().map_kd = Some(base_dir.join(file));
                }
            }
            // Ka/Ks/Ns/Ni/illum/Tr 等忽略
            _ => {}
        }
    }
    Ok(())
}

/// 把 MTL 定义合并进占位材质；`decode_textures=false`（预览模式）跳过贴图加载，用 Kd 纯色
fn apply_mtl_def(mat: &mut Material, def: MtlDef, decode_textures: bool) -> Result<(), String> {
    // 预览模式：不解码贴图，直接用 Kd（或灰色兜底）
    if !decode_textures {
        mat.kind = MaterialKind::Solid;
        mat.colour = def
            .kd
            .map_or([0.5, 0.5, 0.5, 1.0], |c| [c[0], c[1], c[2], 1.0]);
        if let Some(a) = def.alpha {
            mat.colour[3] = a;
        }
        return Ok(());
    }

    if let Some(path) = def.map_kd {
        let img =
            image::open(&path).map_err(|e| format!("加载贴图 {} 失败: {e}", path.display()))?;
        let rgba = img.to_rgba8();
        let has_transparency = rgba.pixels().any(|p| p[3] != 255);
        mat.kind = MaterialKind::Textured;
        mat.texture = Some(Texture { image: rgba });
        mat.transparency = match def.alpha {
            Some(a) if a < 1.0 => Transparency::UseAlphaValue(a),
            _ if has_transparency => Transparency::UseAlphaMap,
            _ => Transparency::None,
        };
    } else {
        mat.kind = MaterialKind::Solid;
        mat.colour = def
            .kd
            .map_or([0.5, 0.5, 0.5, 1.0], |c| [c[0], c[1], c[2], 1.0]);
        if let Some(a) = def.alpha {
            mat.colour[3] = a;
        }
    }
    Ok(())
}

fn parse_vec3(it: &mut std::str::SplitWhitespace) -> Result<Vec3, String> {
    Ok(Vec3::new(next_f32(it)?, next_f32(it)?, next_f32(it)?))
}

fn next_f32(it: &mut std::str::SplitWhitespace) -> Result<f32, String> {
    it.next()
        .ok_or_else(|| "数值不足".to_string())?
        .parse()
        .map_err(|_| "非法数值".to_string())
}

/// 解析 OBJ 索引：1 基；负数表示相对当前末尾
fn parse_index(s: &str, len: i64) -> Result<usize, String> {
    let n: i64 = s.parse().map_err(|_| format!("非法索引 {s}"))?;
    let idx = if n > 0 { n - 1 } else { len + n };
    if idx < 0 || idx >= len {
        return Err(format!("索引 {s} 越界"));
    }
    Ok(idx as usize)
}
