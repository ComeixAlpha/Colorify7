//! 射线与三角形求交（Möller–Trumbore）

use super::vec3::Vec3;

/// 射线：起点 + 单位方向
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, dir: Vec3) -> Self {
        Self { origin, dir }
    }
}

const EPSILON: f32 = 1e-7;

/// 射线-三角形求交。命中返回 `(交点, 重心 u, 重心 v)`，否则 None。
pub fn ray_intersect_triangle(ray: &Ray, a: Vec3, b: Vec3, c: Vec3) -> Option<(Vec3, f32, f32)> {
    let e1 = b - a;
    let e2 = c - a;
    let h = Vec3::cross(ray.dir, e2);
    let det = e1.dot(h);
    if det.abs() < EPSILON {
        return None; // 与三角形平行
    }
    let f = 1.0 / det;
    let s = ray.origin - a;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = Vec3::cross(s, e1);
    let v = f * ray.dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = f * e2.dot(q);
    if t <= EPSILON {
        return None;
    }
    Some((ray.origin + ray.dir * t, u, v))
}

/// 点相对三角形的重心坐标 `(u, v, w)`（点在面外时坐标可能越界，仍可用于插值）
pub fn barycentric(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> (f32, f32, f32) {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-12 {
        return (1.0, 0.0, 0.0);
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    (1.0 - v - w, v, w)
}
