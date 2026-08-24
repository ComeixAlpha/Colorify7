//! glTF/GLB 导入：`gltf` crate -> 现有 `Mesh`
//!
//! - 几何：读 POSITION / NORMAL / TEXCOORD_0 / index accessor（支持无索引、strip/fan）
//! - 材质：PBR `baseColorFactor` -> 纯色；`baseColorTexture` -> 解码贴图
//! - 节点 TRS 层级变换合并到顶点世界坐标
//! - UV 约定与 OBJ 一致（v 不翻转，采样时顶部为 v=0）

use std::collections::HashSet;
use std::path::Path;

use gltf::image::{Data as GltfImageData, Source};
use gltf::scene::Transform;
use image::RgbaImage;
use rayon::prelude::*;

use super::mesh::{
    Material, MaterialKind, Mesh, Texture, TextureFiltering, TextureWrap, Transparency, Tri, UV,
};
use super::vec3::Vec3;

/// 导入 glTF/GLB 文件为 Mesh 完整模式
#[allow(dead_code)]
pub fn import_gltf_file(path: &Path) -> Result<Mesh, String> {
    import_gltf_file_with(path, true, 0)
}

/// 导入 glTF/GLB；`decode_textures=false`（预览模式）跳过贴图解码
/// `max_tris>0` 时三角形均匀采样截断
pub fn import_gltf_file_with(
    path: &Path,
    decode_textures: bool,
    max_tris: usize,
) -> Result<Mesh, String> {
    let base = path.parent().unwrap_or(Path::new("."));
    let bytes = std::fs::read(path).map_err(|e| format!("读取模型失败: {e}"))?;
    import_gltf_bytes_with_base(&bytes, base, decode_textures, max_tris)
}

/// 从内存字节导入 glTF/GLB
/// GLTF（非 GLB）若引用外部 .bin / 贴图会因 base 为当前目录而失败，安卓使用 GLB
pub fn import_gltf_bytes(
    bytes: Vec<u8>,
    decode_textures: bool,
    max_tris: usize,
) -> Result<Mesh, String> {
    import_gltf_bytes_with_base(&bytes, Path::new("."), decode_textures, max_tris)
}

/// 共享导入主体：`base` 用于 GLTF 外部 .bin / 贴图；GLB 内嵌数据无需 base
fn import_gltf_bytes_with_base(
    bytes: &[u8],
    base: &Path,
    decode_textures: bool,
    max_tris: usize,
) -> Result<Mesh, String> {
    // 预处理：把不支持的材质扩展（KHR_materials_pbrSpecularGlossiness 等）转成标准
    // PBR，并移除 extensionsRequired
    let processed = if is_glb(bytes) {
        preprocess_glb(bytes)?
    } else {
        preprocess_gltf_text(&String::from_utf8_lossy(bytes))?
    };

    let gltf = gltf::Gltf::from_slice(&processed).map_err(|e| format!("glTF 解析失败: {e}"))?;
    let document = gltf.document.clone();

    let blob = gltf.blob.clone();
    let buffers = gltf::import_buffers(&gltf, Some(base), blob)
        .map_err(|e| format!("glTF buffer 读取失败: {e}"))?;

    // 完整模式并行解码贴图
    let images = if decode_textures {
        decode_images_parallel(&document, &buffers, base)?
    } else {
        Vec::new()
    };

    // 1. 材质表（document.materials() 顺序，primitive.material().index() 映射）
    let materials = document
        .materials()
        .map(|mat| build_material(mat, &images, decode_textures))
        .collect::<Vec<_>>();

    // 2. 遍历所有 node（含子节点），应用全局变换，收集三角形
    let mut out = Mesh::new();
    let mut visited: HashSet<usize> = HashSet::new();
    let mut stack: Vec<(gltf::Node, Mat4)> = Vec::new();
    for scene in document.scenes() {
        for node in scene.nodes() {
            stack.push((node, Mat4::identity()));
        }
    }
    while let Some((node, parent)) = stack.pop() {
        let global = parent.mul(&node_local(&node));
        for child in node.children() {
            stack.push((child, global));
        }
        if !visited.insert(node.index()) {
            continue; // 多场景共享节点，几何只取一次
        }
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                append_primitive(&mut out, &primitive, &global, &buffers, &materials)?;
            }
        }
    }
    out.materials = materials;

    // 预览模式：三角形均匀采样截断（保留全部顶点，只减三角形）
    if max_tris > 0 && out.tris.len() > max_tris {
        let stride = (out.tris.len() / max_tris).max(1);
        out.tris = out.tris.iter().step_by(stride).copied().collect();
    }

    if out.tris.is_empty() {
        return Err("glTF 中没有可用的三角形几何".into());
    }
    Ok(out)
}

/// 判断是否为 GLB 二进制（magic "glTF"）
fn is_glb(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == b"glTF"
}

/// 预处理 glTF JSON：把 `KHR_materials_pbrSpecularGlossiness` 材质转成标准 PBR
/// diffuse -> baseColor，并移除 `extensionsRequired`
fn preprocess_json(v: &mut serde_json::Value) {
    if let Some(materials) = v.get_mut("materials").and_then(|m| m.as_array_mut()) {
        for mat in materials.iter_mut() {
            let ext_pbr = mat
                .get("extensions")
                .and_then(|e| e.get("KHR_materials_pbrSpecularGlossiness"))
                .cloned();
            if let Some(ext) = ext_pbr {
                if !mat.get("pbrMetallicRoughness").is_some() {
                    mat["pbrMetallicRoughness"] = serde_json::json!({});
                }
                let pbr = mat.get_mut("pbrMetallicRoughness").unwrap();
                if let Some(f) = ext.get("diffuseFactor") {
                    pbr["baseColorFactor"] = f.clone();
                }
                if let Some(t) = ext.get("diffuseTexture") {
                    pbr["baseColorTexture"] = t.clone();
                }
                if let Some(exts) = mat.get_mut("extensions").and_then(|e| e.as_object_mut()) {
                    exts.remove("KHR_materials_pbrSpecularGlossiness");
                }
            }
        }
    }
    if let Some(obj) = v.as_object_mut() {
        obj.remove("extensionsRequired");
    }
}

/// 预处理 GLB 二进制：解析 chunk -> 改 JSON -> 重写（chunk 4 字节对齐，JSON 用空格填充）
fn preprocess_glb(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 12 {
        return Err("GLB 文件过短".into());
    }
    let u32at = |p: usize| -> u32 {
        u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], bytes[p + 3]])
    };
    let mut pos = 12usize;
    let mut json_range: Option<(usize, usize)> = None;
    let mut bin_range: Option<(usize, usize)> = None;
    while pos + 8 <= bytes.len() {
        let len = u32at(pos) as usize;
        let ty = u32at(pos + 4);
        let data = pos + 8;
        if ty == 0x4E4F_534A {
            json_range = Some((data, len));
        } else if ty == 0x004E_4942 {
            bin_range = Some((data, len));
        }
        pos = data + len + (4 - len % 4) % 4;
    }
    let (js, jl) = json_range.ok_or("GLB 缺少 JSON chunk")?;
    let json_str = String::from_utf8_lossy(&bytes[js..js + jl]);
    let mut v: serde_json::Value = serde_json::from_str(json_str.trim_end_matches('\0').trim())
        .map_err(|e| format!("GLB JSON 解析失败: {e}"))?;
    preprocess_json(&mut v);
    let new_json = serde_json::to_string(&v).map_err(|e| format!("GLB JSON 序列化失败: {e}"))?;
    let jb = new_json.as_bytes();
    let jp = jb.len() + (4 - jb.len() % 4) % 4;

    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[0..8]); // magic + version
    let bin_total = bin_range.map(|(_, l)| l + (4 - l % 4) % 4).unwrap_or(0);
    let total = 12
        + 8
        + jp
        + if bin_range.is_some() {
            8 + bin_total
        } else {
            0
        };
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(jp as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(jb);
    out.extend(std::iter::repeat(b' ').take(jp - jb.len()));
    if let Some((bs, bl)) = bin_range {
        out.extend_from_slice(&(bin_total as u32).to_le_bytes());
        out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
        out.extend_from_slice(&bytes[bs..bs + bl]);
        out.extend(std::iter::repeat(0u8).take(bin_total - bl));
    }
    Ok(out)
}

/// 预处理 .gltf 文本 JSON
fn preprocess_gltf_text(text: &str) -> Result<Vec<u8>, String> {
    let mut v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("glTF JSON 解析失败: {e}"))?;
    preprocess_json(&mut v);
    serde_json::to_vec(&v).map_err(|e| format!("glTF JSON 序列化失败: {e}"))
}

/// 并行解码所有 glTF 贴图（替代 `gltf::import_images` 的串行解码）
fn decode_images_parallel(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    base: &Path,
) -> Result<Vec<GltfImageData>, String> {
    let images: Vec<gltf::Image> = document.images().collect();
    images
        .par_iter()
        .map(|img| decode_one_image(img, buffers, base))
        .collect()
}

/// 解码单张贴图
fn decode_one_image(
    img: &gltf::Image,
    buffers: &[gltf::buffer::Data],
    base: &Path,
) -> Result<GltfImageData, String> {
    let bytes: Vec<u8> = match img.source() {
        Source::View { view, mime_type: _ } => {
            let data = buffers
                .get(view.buffer().index())
                .ok_or_else(|| "贴图 buffer 越界".to_string())?;
            let (offset, length) = (view.offset(), view.length());
            data.0
                .get(offset..offset + length)
                .ok_or_else(|| "贴图 buffer 切片越界".to_string())?
                .to_vec()
        }
        Source::Uri { uri, .. } => {
            if let Some(stripped) = uri.strip_prefix("data:") {
                decode_data_uri(stripped)?
            } else {
                let path = base.join(uri);
                std::fs::read(&path)
                    .map_err(|e| format!("读取贴图 {} 失败: {e}", path.display()))?
            }
        }
    };

    match image::load_from_memory(&bytes) {
        Ok(dyn_img) => {
            let rgba = dyn_img.to_rgba8();
            let (w, h) = rgba.dimensions();
            Ok(GltfImageData {
                width: w,
                height: h,
                format: gltf::image::Format::R8G8B8A8,
                pixels: rgba.into_raw(),
            })
        }
        Err(e) => {
            eprintln!("[gltf] 贴图解码失败（用白色占位）: {e}");
            Ok(GltfImageData {
                width: 1,
                height: 1,
                format: gltf::image::Format::R8G8B8A8,
                pixels: vec![255, 255, 255, 255],
            })
        }
    }
}

/// 解析 `data:image/png;base64,xxxx` 的 data URI
fn decode_data_uri(uri: &str) -> Result<Vec<u8>, String> {
    let b64 = uri.split_once(',').map(|(_, b)| b).unwrap_or(uri);
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD
        .decode(b64)
        .map_err(|e| format!("data URI 解码失败: {e}"))
}

/// 材质：baseColorFactor -> 纯色；baseColorTexture -> 贴图（PBR 金属度/粗糙度忽略）
fn build_material(
    mat: gltf::Material,
    images: &[gltf::image::Data],
    decode_textures: bool,
) -> Material {
    let pbr = mat.pbr_metallic_roughness();
    let factor = pbr.base_color_factor();
    let name = mat.name().unwrap_or("").to_string();

    let mut kind = MaterialKind::Solid;
    let mut texture = None;
    let mut filtering = TextureFiltering::default();
    let mut wrap = TextureWrap::default();

    if decode_textures {
        if let Some(info) = pbr.base_color_texture() {
            let src = info.texture().source();
            if let Some(data) = images.get(src.index()) {
                let rgba = gltf_image_to_rgba(data);
                if rgba.width() > 0 && rgba.height() > 0 {
                    let sampler = info.texture().sampler();
                    filtering = match sampler.mag_filter() {
                        Some(gltf::texture::MagFilter::Linear) => TextureFiltering::Linear,
                        _ => TextureFiltering::Nearest,
                    };
                    wrap = if sampler.wrap_s() == gltf::texture::WrappingMode::ClampToEdge
                        || sampler.wrap_t() == gltf::texture::WrappingMode::ClampToEdge
                    {
                        TextureWrap::Clamp
                    } else {
                        TextureWrap::Repeat
                    };
                    kind = MaterialKind::Textured;
                    texture = Some(Texture { image: rgba });
                }
            }
        }
    }

    // 透明度：混合模式（带贴图）-> 用贴图 alpha；否则固定 alpha
    let transparency = match mat.alpha_mode() {
        gltf::material::AlphaMode::Opaque => {
            if factor[3] < 1.0 {
                Transparency::UseAlphaValue(factor[3])
            } else {
                Transparency::None
            }
        }
        gltf::material::AlphaMode::Mask => {
            Transparency::UseAlphaValue(mat.alpha_cutoff().unwrap_or(0.5))
        }
        gltf::material::AlphaMode::Blend => {
            if kind == MaterialKind::Textured {
                Transparency::UseAlphaMap
            } else {
                Transparency::UseAlphaValue(factor[3])
            }
        }
    };

    Material {
        name,
        kind,
        colour: factor,
        texture,
        filtering,
        wrap,
        transparency,
    }
}

/// 把一个 primitive 的三角形追加到 Mesh（顶点已应用全局变换）
fn append_primitive(
    out: &mut Mesh,
    primitive: &gltf::Primitive,
    global: &Mat4,
    buffers: &[gltf::buffer::Data],
    materials: &[Material],
) -> Result<(), String> {
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));
    let Some(positions_raw) = reader.read_positions() else {
        return Ok(()); // 无位置数据的图元跳过
    };

    // 顶点（应用全局变换）
    let base_vertex = out.vertices.len();
    for p in positions_raw {
        let t = global.transform_point(p);
        out.vertices.push(Vec3::new(t[0], t[1], t[2]));
    }

    // 法线（仅旋转部分 + 归一化）
    let base_normal = out.normals.len();
    let has_normal = if let Some(normals) = reader.read_normals() {
        for n in normals {
            let t = global.transform_vector(n);
            out.normals.push(Vec3::new(t[0], t[1], t[2]).normalize());
        }
        true
    } else {
        false
    };

    // UV（v 不翻转，与 OBJ 约定一致）
    let base_uv = out.uvs.len();
    let has_uv = if let Some(uvs) = reader.read_tex_coords(0) {
        for uv in uvs.into_f32() {
            out.uvs.push(UV::new(uv[0], uv[1]));
        }
        true
    } else {
        false
    };

    // 顶点色（COLOR_0，0-1 RGBA；无贴图模型常用顶点色着色）
    let base_colour = out.vertex_colors.len();
    let has_colour = if let Some(cols) = reader.read_colors(0) {
        use gltf::mesh::util::ReadColors;
        match cols {
            ReadColors::RgbaF32(it) => it.for_each(|c| out.vertex_colors.push(c)),
            ReadColors::RgbF32(it) => {
                it.for_each(|c| out.vertex_colors.push([c[0], c[1], c[2], 1.0]))
            }
            ReadColors::RgbaU8(it) => it.for_each(|c| {
                out.vertex_colors.push([
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    c[3] as f32 / 255.0,
                ])
            }),
            ReadColors::RgbU8(it) => it.for_each(|c| {
                out.vertex_colors.push([
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    1.0,
                ])
            }),
            ReadColors::RgbaU16(it) => it.for_each(|c| {
                out.vertex_colors.push([
                    c[0] as f32 / 65535.0,
                    c[1] as f32 / 65535.0,
                    c[2] as f32 / 65535.0,
                    c[3] as f32 / 65535.0,
                ])
            }),
            ReadColors::RgbU16(it) => it.for_each(|c| {
                out.vertex_colors.push([
                    c[0] as f32 / 65535.0,
                    c[1] as f32 / 65535.0,
                    c[2] as f32 / 65535.0,
                    1.0,
                ])
            }),
        }
        true
    } else {
        false
    };

    // 索引（无索引时按顶点顺序）
    let count = out.vertices.len() - base_vertex;
    let indices: Vec<u32> = match reader.read_indices() {
        Some(gltf::mesh::util::ReadIndices::U8(it)) => it.map(|x| x as u32).collect(),
        Some(gltf::mesh::util::ReadIndices::U16(it)) => it.map(|x| x as u32).collect(),
        Some(gltf::mesh::util::ReadIndices::U32(it)) => it.collect(),
        None => (0..count as u32).collect(),
    };
    if indices.len() < 3 {
        return Ok(());
    }

    let material = primitive
        .material()
        .index()
        .unwrap_or(0)
        .min(materials.len().saturating_sub(1));
    let texcoord_mark = has_uv.then_some([0usize; 3]);
    let colour_mark = has_colour.then_some([0usize; 3]);
    let normal_mark = has_normal.then_some([0usize; 3]);

    match primitive.mode() {
        gltf::mesh::Mode::Triangles => {
            for tri in indices.chunks_exact(3) {
                push_triangle(
                    out,
                    base_vertex,
                    base_uv,
                    base_colour,
                    base_normal,
                    tri[0],
                    tri[1],
                    tri[2],
                    texcoord_mark,
                    colour_mark,
                    normal_mark,
                    material,
                );
            }
        }
        gltf::mesh::Mode::TriangleStrip => {
            for i in 0..indices.len().saturating_sub(2) {
                let (a, b, c) = if i % 2 == 0 {
                    (indices[i], indices[i + 1], indices[i + 2])
                } else {
                    (indices[i + 1], indices[i], indices[i + 2])
                };
                push_triangle(
                    out,
                    base_vertex,
                    base_uv,
                    base_colour,
                    base_normal,
                    a,
                    b,
                    c,
                    texcoord_mark,
                    colour_mark,
                    normal_mark,
                    material,
                );
            }
        }
        gltf::mesh::Mode::TriangleFan => {
            for i in 1..indices.len().saturating_sub(1) {
                push_triangle(
                    out,
                    base_vertex,
                    base_uv,
                    base_colour,
                    base_normal,
                    indices[0],
                    indices[i],
                    indices[i + 1],
                    texcoord_mark,
                    colour_mark,
                    normal_mark,
                    material,
                );
            }
        }
        other => return Err(format!("不支持的图元模式: {other:?}")),
    }
    Ok(())
}

/// 追加一个三角形（a/b/c 为 primitive 局部顶点索引）
#[allow(clippy::too_many_arguments)]
fn push_triangle(
    out: &mut Mesh,
    base_vertex: usize,
    base_uv: usize,
    base_colour: usize,
    base_normal: usize,
    a: u32,
    b: u32,
    c: u32,
    texcoord: Option<[usize; 3]>,
    colour: Option<[usize; 3]>,
    normal: Option<[usize; 3]>,
    material: usize,
) {
    let (a, b, c) = (a as usize, b as usize, c as usize);
    out.tris.push(Tri {
        position: [base_vertex + a, base_vertex + b, base_vertex + c],
        texcoord: texcoord.map(|_| [base_uv + a, base_uv + b, base_uv + c]),
        colour: colour.map(|_| [base_colour + a, base_colour + b, base_colour + c]),
        normal: normal.map(|_| [base_normal + a, base_normal + b, base_normal + c]),
        material,
    });
}

/// glTF 解码后的像素 -> RgbaImage（16bit 取高 8 位；灰度/无 alpha 补全）
fn gltf_image_to_rgba(data: &gltf::image::Data) -> RgbaImage {
    use gltf::image::Format;
    let (w, h) = (data.width, data.height);
    match data.format {
        Format::R8G8B8A8 => {
            RgbaImage::from_raw(w, h, data.pixels.clone()).unwrap_or_else(|| RgbaImage::new(w, h))
        }
        Format::R8G8B8 => {
            let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
            for px in data.pixels.chunks_exact(3) {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            RgbaImage::from_raw(w, h, rgba).unwrap_or_else(|| RgbaImage::new(w, h))
        }
        Format::R8 => {
            let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
            for &v in &data.pixels {
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
            RgbaImage::from_raw(w, h, rgba).unwrap_or_else(|| RgbaImage::new(w, h))
        }
        Format::R16G16B16 => {
            let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
            for px in data.pixels.chunks_exact(6) {
                let r = u16::from_le_bytes([px[0], px[1]]) >> 8;
                let g = u16::from_le_bytes([px[2], px[3]]) >> 8;
                let b = u16::from_le_bytes([px[4], px[5]]) >> 8;
                rgba.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
            }
            RgbaImage::from_raw(w, h, rgba).unwrap_or_else(|| RgbaImage::new(w, h))
        }
        Format::R16G16B16A16 => {
            let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
            for px in data.pixels.chunks_exact(8) {
                let r = u16::from_le_bytes([px[0], px[1]]) >> 8;
                let g = u16::from_le_bytes([px[2], px[3]]) >> 8;
                let b = u16::from_le_bytes([px[4], px[5]]) >> 8;
                let a = u16::from_le_bytes([px[6], px[7]]) >> 8;
                rgba.extend_from_slice(&[r as u8, g as u8, b as u8, a as u8]);
            }
            RgbaImage::from_raw(w, h, rgba).unwrap_or_else(|| RgbaImage::new(w, h))
        }
        _ => RgbaImage::new(w, h),
    }
}

/// 节点局部变换 -> Mat4
fn node_local(node: &gltf::Node) -> Mat4 {
    match node.transform() {
        Transform::Matrix { matrix } => {
            // matrix 为 [[f32;4];4]（列主序），展平为 [f32;16]
            let mut m = [0.0f32; 16];
            for col in 0..4 {
                for row in 0..4 {
                    m[col * 4 + row] = matrix[col][row];
                }
            }
            Mat4(m)
        }
        Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => Mat4::from_trs(translation, rotation, scale),
    }
}

#[derive(Debug, Clone, Copy)]
struct Mat4([f32; 16]);

impl Mat4 {
    fn identity() -> Self {
        Mat4([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// 平移 × 旋转 × 缩放
    fn from_trs(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> Self {
        let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
        let (x2, y2, z2) = (x + x, y + y, z + z);
        let (xx, yy, zz) = (x * x2, y * y2, z * z2);
        let (xy, xz, yz) = (x * y2, x * z2, y * z2);
        let (wx, wy, wz) = (w * x2, w * y2, w * z2);
        // 旋转矩阵（列主序）
        let r = [
            1.0 - (yy + zz),
            xy + wz,
            xz - wy,
            0.0,
            xy - wz,
            1.0 - (xx + zz),
            yz + wx,
            0.0,
            xz + wy,
            yz - wx,
            1.0 - (xx + yy),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ];
        // R×S：每列乘缩放，再放入平移
        Mat4([
            r[0] * s[0],
            r[1] * s[0],
            r[2] * s[0],
            0.0,
            r[4] * s[1],
            r[5] * s[1],
            r[6] * s[1],
            0.0,
            r[8] * s[2],
            r[9] * s[2],
            r[10] * s[2],
            0.0,
            t[0],
            t[1],
            t[2],
            1.0,
        ])
    }

    fn mul(&self, other: &Mat4) -> Mat4 {
        let a = &self.0;
        let b = &other.0;
        let mut m = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += a[k * 4 + row] * b[col * 4 + k];
                }
                m[col * 4 + row] = sum;
            }
        }
        Mat4(m)
    }

    /// 变换点（含平移）
    fn transform_point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [
            m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
            m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
            m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
        ]
    }

    /// 变换方向向量
    fn transform_vector(&self, v: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [
            m[0] * v[0] + m[4] * v[1] + m[8] * v[2],
            m[1] * v[0] + m[5] * v[1] + m[9] * v[2],
            m[2] * v[0] + m[6] * v[1] + m[10] * v[2],
        ]
    }
}
