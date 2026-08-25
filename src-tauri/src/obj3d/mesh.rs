//! 3D 网格数据结构

use image::RgbaImage;

use super::vec3::Vec3;

/// 贴图坐标
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UV {
    pub u: f32,
    pub v: f32,
}

impl UV {
    pub fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

/// 三角形：顶点/法线/UV/顶点色 均为索引，material 指向材质表下标
#[derive(Debug, Clone, Copy)]
pub struct Tri {
    pub position: [usize; 3],
    /// 该面未定义贴图坐标时为 None（只能按纯色着色）
    pub texcoord: Option<[usize; 3]>,
    /// 该面未定义顶点色时为 None
    pub colour: Option<[usize; 3]>,
    #[allow(dead_code)]
    pub normal: Option<[usize; 3]>,
    pub material: usize,
}

/// 材质类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialKind {
    Solid,
    Textured,
}

/// 纹理过滤方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextureFiltering {
    #[default]
    Nearest,
    Linear,
}

/// 纹理环绕方式（UV 越界时的采样规则）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextureWrap {
    #[default]
    Repeat,
    Clamp,
}

/// 透明度采样方式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transparency {
    None,
    /// 使用漫反射贴图的 alpha 通道
    UseAlphaMap,
    /// 固定透明度（0~1）
    UseAlphaValue(f32),
}

/// 已解码的贴图（RGBA8，供体素化时采样）
#[derive(Debug, Clone)]
pub struct Texture {
    pub image: RgbaImage,
}

impl Texture {
    /// 采样贴图颜色。UV 允许越界（先按 wrap 归一化）；v 不翻转
    pub fn sample(&self, uv: UV, filtering: TextureFiltering, wrap: TextureWrap) -> [f32; 4] {
        let (w, h) = self.image.dimensions();
        if w == 0 || h == 0 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        let u = wrap_coord(uv.u, wrap);
        let v = wrap_coord(uv.v, wrap);
        let x = u * w as f32;
        let y = v * h as f32;

        // 取整像素（越界钳制）
        let pixel = |px: f32, py: f32| -> [f32; 4] {
            let px = px.floor().clamp(0.0, (w - 1) as f32) as u32;
            let py = py.floor().clamp(0.0, (h - 1) as f32) as u32;
            let p = self.image.get_pixel(px, py);
            [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
                p[3] as f32 / 255.0,
            ]
        };

        match filtering {
            TextureFiltering::Nearest => pixel(x, y),
            TextureFiltering::Linear => {
                let x0 = x.floor();
                let y0 = y.floor();
                let tx = x - x0;
                let ty = y - y0;
                let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
                let c = [
                    pixel(x0, y0),
                    pixel(x0 + 1.0, y0),
                    pixel(x0, y0 + 1.0),
                    pixel(x0 + 1.0, y0 + 1.0),
                ];
                [
                    lerp(lerp(c[0][0], c[1][0], tx), lerp(c[2][0], c[3][0], tx), ty),
                    lerp(lerp(c[0][1], c[1][1], tx), lerp(c[2][1], c[3][1], tx), ty),
                    lerp(lerp(c[0][2], c[1][2], tx), lerp(c[2][2], c[3][2], tx), ty),
                    lerp(lerp(c[0][3], c[1][3], tx), lerp(c[2][3], c[3][3], tx), ty),
                ]
            }
        }
    }

    /// 贴图平均色（RGBA -> 0-1，忽略 alpha）。无 UV 的三角形回退用
    pub fn average_colour(&self) -> [f32; 4] {
        let mut sum = [0.0f64; 3];
        let mut n = 0.0f64;
        for px in self.image.pixels() {
            sum[0] += px[0] as f64;
            sum[1] += px[1] as f64;
            sum[2] += px[2] as f64;
            n += 1.0;
        }
        if n == 0.0 {
            return [1.0, 1.0, 1.0, 1.0];
        }
        [
            (sum[0] / n / 255.0) as f32,
            (sum[1] / n / 255.0) as f32,
            (sum[2] / n / 255.0) as f32,
            1.0,
        ]
    }
}

/// UV 越界归一化
fn wrap_coord(v: f32, wrap: TextureWrap) -> f32 {
    match wrap {
        TextureWrap::Clamp => v.clamp(0.0, 1.0),
        TextureWrap::Repeat => v - v.floor(),
    }
}

/// 材质
#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub kind: MaterialKind,
    /// solid 用颜色（RGBA）；textured 作贴图缺失兜底
    pub colour: [f32; 4],
    pub texture: Option<Texture>,
    pub filtering: TextureFiltering,
    pub wrap: TextureWrap,
    pub transparency: Transparency,
}

/// 三角形网格
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<UV>,
    /// 每顶点颜色（RGBA 0-1，无顶点色时为空）
    pub vertex_colors: Vec<[f32; 4]>,
    pub tris: Vec<Tri>,
    pub materials: Vec<Material>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    /// 顶点包围盒（无顶点返回 None）
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self.vertices.iter().copied();
        let first = it.next()?;
        let (mut min, mut max) = (first, first);
        for v in it {
            min = min.min(v);
            max = max.max(v);
        }
        Some((min, max))
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    pub fn triangle_vertices(&self, i: usize) -> [Vec3; 3] {
        let t = self.tris[i];
        [
            self.vertices[t.position[0]],
            self.vertices[t.position[1]],
            self.vertices[t.position[2]],
        ]
    }

    /// 三角形的三个 UV（未定义则为 0）
    pub fn triangle_uvs(&self, i: usize) -> [UV; 3] {
        let t = self.tris[i];
        match t.texcoord {
            Some(idx) => [self.uvs[idx[0]], self.uvs[idx[1]], self.uvs[idx[2]]],
            None => [UV::new(0.0, 0.0); 3],
        }
    }

    /// 三角形的三个法线（未定义则用面法线兜底）
    #[allow(dead_code)]
    pub fn triangle_normals(&self, i: usize) -> [Vec3; 3] {
        let t = self.tris[i];
        match t.normal {
            Some(idx) => [
                self.normals[idx[0]],
                self.normals[idx[1]],
                self.normals[idx[2]],
            ],
            None => {
                let face = self.face_normal(i);
                [face, face, face]
            }
        }
    }

    #[allow(dead_code)]
    pub fn face_normal(&self, i: usize) -> Vec3 {
        let [a, b, c] = self.triangle_vertices(i);
        Vec3::cross(b - a, c - a)
    }

    #[allow(dead_code)]
    pub fn material_of(&self, i: usize) -> &Material {
        &self.materials[self.tris[i].material]
    }

    pub fn translate(&mut self, delta: Vec3) {
        for v in &mut self.vertices {
            *v = *v + delta;
        }
    }

    pub fn centre(&mut self) {
        if let Some((min, max)) = self.bounds() {
            self.translate(-((min + max) * 0.5));
        }
    }

    #[allow(dead_code)]
    pub fn scale(&mut self, factor: f32) {
        for v in &mut self.vertices {
            *v = *v * factor;
        }
    }

    /// 旋转（角度制）Z(roll)-X(pitch)-Y(yaw)
    pub fn rotate(&mut self, pitch: f32, roll: f32, yaw: f32) {
        if pitch == 0.0 && roll == 0.0 && yaw == 0.0 {
            return;
        }
        let (sp, cp) = pitch.to_radians().sin_cos();
        let (sr, cr) = roll.to_radians().sin_cos();
        let (sy, cy) = yaw.to_radians().sin_cos();
        for v in &mut self.vertices {
            let mut x = v.x;
            let mut y = v.y;
            let mut z = v.z;
            // Rz(roll)
            (x, y) = (x * cr - y * sr, x * sr + y * cr);
            // Rx(pitch)
            (y, z) = (y * cp - z * sp, y * sp + z * cp);
            // Ry(yaw)
            (x, z) = (x * cy + z * sy, -x * sy + z * cy);
            *v = Vec3::new(x, y, z);
        }
    }
}
