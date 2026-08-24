//! BVH 加速结构
//! 只做 AABB 剔除：命中候选三角形后由调用方再做精确求交

use rayon::prelude::*;

use super::ray::Ray;
use super::vec3::Vec3;

/// 轴对齐包围盒
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: &[Vec3; 3]) -> Self {
        let mut min = points[0];
        let mut max = points[0];
        for p in &points[1..] {
            min = min.min(*p);
            max = max.max(*p);
        }
        Self { min, max }
    }

    pub fn centroid(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// 射线-AABB 求交（slab 法）
    pub fn intersects_ray(self, ray: &Ray) -> bool {
        let mut tmin = 0.0f32;
        let mut tmax = f32::INFINITY;
        for (o, d, lo, hi) in [
            (ray.origin.x, ray.dir.x, self.min.x, self.max.x),
            (ray.origin.y, ray.dir.y, self.min.y, self.max.y),
            (ray.origin.z, ray.dir.z, self.min.z, self.max.z),
        ] {
            if d.abs() < 1e-9 {
                if o < lo || o > hi {
                    return false;
                }
            } else {
                let inv = 1.0 / d;
                let mut t1 = (lo - o) * inv;
                let mut t2 = (hi - o) * inv;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return false;
                }
            }
        }
        tmax > 0.0
    }
}

/// BVH 节点（arena 存储，避免 Box 递归）
#[derive(Debug, Clone, Copy)]
struct Node {
    bounds: Aabb,
    /// 叶子：三角形下标；内部节点：-1
    tri: i32,
    left: i32,
    right: i32,
}

/// BVH：按三角形质心最长轴中位切分构建
pub struct Bvh {
    nodes: Vec<Node>,
    root: i32,
}

impl Bvh {
    /// 由每个三角形的 AABB 构建
    pub fn build(bounds: &[Aabb]) -> Self {
        let mut order: Vec<usize> = (0..bounds.len()).collect();
        let mut nodes = Vec::with_capacity(bounds.len() * 2);
        let root = build_node(bounds, &mut order, &mut nodes);
        Self { nodes, root }
    }

    /// 遍历所有被射线命中的候选三角形（近远序不保证；visit 收到三角形下标）
    pub fn traverse(&self, ray: &Ray, visit: &mut impl FnMut(usize)) {
        let mut stack = [0i32; 128];
        let mut sp = 1usize;
        stack[0] = self.root;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if !node.bounds.intersects_ray(ray) {
                continue;
            }
            if node.tri >= 0 {
                visit(node.tri as usize);
            } else {
                stack[sp] = node.left;
                stack[sp + 1] = node.right;
                sp += 2;
            }
        }
    }
}

fn build_node(bounds: &[Aabb], order: &mut [usize], nodes: &mut Vec<Node>) -> i32 {
    let aabb = order.iter().fold(
        Aabb::new(bounds[order[0]].min, bounds[order[0]].max),
        |acc, &i| Aabb::new(acc.min.min(bounds[i].min), acc.max.max(bounds[i].max)),
    );

    if order.len() == 1 {
        nodes.push(Node {
            bounds: aabb,
            tri: order[0] as i32,
            left: -1,
            right: -1,
        });
        return (nodes.len() - 1) as i32;
    }

    // 按质心最长轴排序后二分（中位切分，树保持平衡）
    let axis = longest_axis(aabb);
    order.par_sort_unstable_by(|&a, &b| {
        centroid_axis(bounds[a], axis)
            .partial_cmp(&centroid_axis(bounds[b], axis))
            .unwrap()
    });
    let mid = order.len() / 2;
    let (left, right) = order.split_at_mut(mid);
    let left = build_node(bounds, left, nodes);
    let right = build_node(bounds, right, nodes);
    nodes.push(Node {
        bounds: aabb,
        tri: -1,
        left,
        right,
    });
    (nodes.len() - 1) as i32
}

fn longest_axis(b: Aabb) -> usize {
    let d = b.max - b.min;
    if d.x >= d.y && d.x >= d.z {
        0
    } else if d.y >= d.z {
        1
    } else {
        2
    }
}

fn centroid_axis(b: Aabb, axis: usize) -> f32 {
    let c = b.centroid();
    match axis {
        0 => c.x,
        1 => c.y,
        _ => c.z,
    }
}
