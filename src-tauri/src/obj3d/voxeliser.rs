//! 体素化

use std::collections::{HashMap, VecDeque};

use rayon::prelude::*;

use super::bvh::{Aabb, Bvh};
use super::mesh::{MaterialKind, Mesh, Transparency, UV};
use super::ray::{barycentric, ray_intersect_triangle, Ray};
use super::vec3::Vec3;
use super::voxel_mesh::{key_of, VoxelMesh, VoxelOverlapRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxeliserKind {
    BvhRay,
    Triplane,
}

#[derive(Debug, Clone, Copy)]
pub struct VoxeliseParams {
    pub algorithm: VoxeliserKind,
    pub constraint_axis: Axis,
    pub size: u32,
    pub use_multisample_colouring: bool,
    pub solid: bool,
    pub overlap_rule: VoxelOverlapRule,
}

impl Default for VoxeliseParams {
    fn default() -> Self {
        Self {
            algorithm: VoxeliserKind::Triplane,
            constraint_axis: Axis::Y,
            size: 80,
            use_multisample_colouring: true,
            solid: true,
            overlap_rule: VoxelOverlapRule::Average,
        }
    }
}

/// 纹理多采样数
const MULTISAMPLE_COUNT: u32 = 8;

/// 顶点聚类减面阈值
const CLUSTER_THRESHOLD: usize = 200_000;

/// 主入口
pub fn voxelise(mesh: &Mesh, params: &VoxeliseParams) -> Result<VoxelMesh, String> {
    match params.algorithm {
        VoxeliserKind::BvhRay => voxelise_bvh_ray(mesh, params),
        VoxeliserKind::Triplane => voxelise_triplane(mesh, params),
    }
}

/// 预计算（变换/减面/包围盒/中心/材质平均色）
struct PreparedMesh<'a> {
    mesh: &'a Mesh,
    tris: Vec<[Vec3; 3]>,
    tri_mats: Vec<usize>,
    tri_colours: Vec<Option<[[f32; 4]; 3]>>,
    bmin: Vec3,
    bmax: Vec3,
    centre: Vec3,
    mat_avgs: Vec<[f32; 4]>,
}

fn prepare<'a>(mesh: &'a Mesh, params: &VoxeliseParams) -> Result<PreparedMesh<'a>, String> {
    let (min, max) = mesh.bounds().ok_or("网格无顶点")?;
    let dim = max - min;
    let mesh_dim = component(dim, params.constraint_axis);
    if mesh_dim <= 0.0 {
        return Err("约束轴尺寸为 0".into());
    }
    // 约束轴方向远小于其他轴时，自动改用最长轴作为约束轴
    let longest = dim.x.max(dim.y).max(dim.z);
    let axis = if mesh_dim < longest / 10.0 {
        if dim.x >= dim.y && dim.x >= dim.z {
            Axis::X
        } else if dim.y >= dim.z {
            Axis::Y
        } else {
            Axis::Z
        }
    } else {
        params.constraint_axis
    };
    let mesh_dim = component(dim, axis);

    // 缩放 + 偶尺寸偏移
    let scale = (params.size as f32 - 1.0) / mesh_dim;
    let offset = if params.size % 2 == 0 { 0.5 } else { 0.0 };
    let transform = |v: Vec3| -> Vec3 {
        match axis {
            Axis::X => Vec3::new(v.x * scale + offset, v.y * scale, v.z * scale),
            Axis::Y => Vec3::new(v.x * scale, v.y * scale + offset, v.z * scale),
            Axis::Z => Vec3::new(v.x * scale, v.y * scale, v.z * scale + offset),
        }
    };

    // 预变换三角形
    let tris_orig: Vec<[Vec3; 3]> = (0..mesh.triangle_count())
        .into_par_iter()
        .map(|i| {
            let [a, b, c] = mesh.triangle_vertices(i);
            [transform(a), transform(b), transform(c)]
        })
        .collect();
    // 三角形 -> 材质索引
    let mut tri_mats: Vec<usize> = (0..mesh.triangle_count())
        .map(|i| mesh.tris[i].material)
        .collect();
    let mut tri_colours: Vec<Option<[[f32; 4]; 3]>> = (0..mesh.triangle_count())
        .map(|i| {
            let t = &mesh.tris[i];
            match t.colour {
                Some(idx) => Some([
                    mesh.vertex_colors[idx[0]],
                    mesh.vertex_colors[idx[1]],
                    mesh.vertex_colors[idx[2]],
                ]),
                None => None,
            }
        })
        .collect();
    // 高模顶点聚类减面
    let mut tris = tris_orig;
    let all_solid = mesh
        .materials
        .iter()
        .all(|m| m.kind != MaterialKind::Textured);
    if all_solid && tris.len() > CLUSTER_THRESHOLD {
        let (decimated, decim_mats, decim_colours) =
            cluster_decimate(&tris, &tri_mats, &tri_colours, 1.0);
        tris = decimated;
        tri_mats = decim_mats;
        tri_colours = decim_colours;
    }

    // 网格包围盒
    let mut bmin = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut bmax = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for t in &tris {
        for v in t {
            bmin = bmin.min(*v);
            bmax = bmax.max(*v);
        }
    }
    let bmin = bmin.floor();
    let bmax = bmax.ceil();
    let grid = bmax - bmin;
    let max_grid = grid.x.max(grid.y).max(grid.z);
    if max_grid > 512.0 {
        return Err("模型比例异常（网格过大），请调整约束轴或旋转模型".into());
    }

    let centre = (bmin + bmax) * 0.5;

    // 贴图材质平均色
    let mat_avgs: Vec<[f32; 4]> = mesh
        .materials
        .par_iter()
        .map(|m| {
            if m.kind == MaterialKind::Textured {
                m.texture
                    .as_ref()
                    .map(|t| t.average_colour())
                    .unwrap_or(m.colour)
            } else {
                m.colour
            }
        })
        .collect();

    Ok(PreparedMesh {
        mesh,
        tris,
        tri_mats,
        tri_colours,
        bmin,
        bmax,
        centre,
        mat_avgs,
    })
}

/// BVH 射线体素化
fn voxelise_bvh_ray(mesh: &Mesh, params: &VoxeliseParams) -> Result<VoxelMesh, String> {
    let prep = prepare(mesh, params)?;
    let PreparedMesh {
        mesh,
        tris,
        tri_mats,
        tri_colours,
        bmin,
        bmax,
        centre,
        mat_avgs,
    } = prep;

    let aabbs: Vec<Aabb> = tris.par_iter().map(|t| Aabb::from_points(t)).collect();
    let bvh = Bvh::build(&aabbs);

    // 朝外标记（相对模型中心）
    let outward_flag: Vec<bool> = tris
        .iter()
        .map(|tri| {
            let a = tri[0];
            let b = tri[1];
            let c = tri[2];
            let n = (b - a).cross(c - a);
            let mid = (a + b + c) * (1.0 / 3.0);
            n.dot(mid - centre) > 0.0
        })
        .collect();

    // 打射线 → 收集命中（位置, 颜色, 是否朝外）
    let hits: Vec<(Vec3, [f32; 4], bool)> = generate_rays(bmin, bmax)
        .par_iter()
        .flat_map_iter(|ray| {
            let mut out: Vec<(Vec3, [f32; 4], bool)> = Vec::new();
            bvh.traverse(ray, &mut |tri| {
                if let Some((point, _u, _v)) =
                    ray_intersect_triangle(ray, tris[tri][0], tris[tri][1], tris[tri][2])
                {
                    let colour = voxel_colour(
                        mesh,
                        &tris,
                        tri,
                        tri_mats[tri],
                        tri_colours[tri],
                        point,
                        params.use_multisample_colouring,
                        &mat_avgs[tri_mats[tri]],
                    );
                    out.push((point, colour, outward_flag[tri]));
                }
            });
            out
        })
        .collect();

    // 顺序落体素：先落朝外命中，再落非朝外命中并跳过已有位置
    let mut voxel_mesh = VoxelMesh::new(params.overlap_rule);
    voxel_mesh.reserve(hits.len());
    for (pos, colour, is_outward) in &hits {
        if *is_outward {
            voxel_mesh.add_voxel(*pos, *colour);
        }
    }
    for (pos, colour, is_outward) in &hits {
        if !*is_outward && !voxel_mesh.is_voxel_at(*pos) {
            voxel_mesh.add_voxel(*pos, *colour);
        }
    }
    Ok(voxel_mesh)
}

/// 体素命中
#[derive(Debug, Clone, Copy)]
struct Stamp {
    pos: Vec3,
    colour: [f32; 4],
    outward: bool,
}

/// 三平面光栅化体素化
fn voxelise_triplane(mesh: &Mesh, params: &VoxeliseParams) -> Result<VoxelMesh, String> {
    let prep = prepare(mesh, params)?;
    let PreparedMesh {
        mesh,
        tris,
        tri_mats,
        tri_colours,
        bmin,
        bmax,
        centre,
        mat_avgs,
    } = prep;

    // 实心填充上限
    // 超过上限自动降级为空心外壳
    let vol = bmax - bmin;
    let solid = params.solid && vol.x * vol.y * vol.z <= 8_000_000.0;
    if params.solid && !solid {
        eprintln!(
            "[voxeliser] 实心填充网格过大，自动降级为空心外壳（可减小体素数量或关闭实心填充）"
        );
    }

    // 并行光栅化
    let stamps: Vec<Stamp> = tris
        .par_iter()
        .enumerate()
        .flat_map_iter(|(i, tri)| {
            rasterise_tri(
                *tri,
                bmin,
                bmax,
                centre,
                mesh,
                &tris,
                i,
                tri_mats[i],
                tri_colours[i],
                params.use_multisample_colouring,
                &mat_avgs[tri_mats[i]],
            )
        })
        .collect();

    let mut voxel_mesh = VoxelMesh::new(params.overlap_rule);
    voxel_mesh.reserve(stamps.len());

    // 1) 表面体素：按位置合并，朝外优先
    let mut cells: HashMap<u64, (Vec3, [f32; 4], bool)> = HashMap::with_capacity(stamps.len());
    for s in &stamps {
        let key = key_of(s.pos);
        match cells.get_mut(&key) {
            Some(entry) => {
                if s.outward && !entry.2 {
                    *entry = (s.pos, s.colour, true);
                }
            }
            None => {
                cells.insert(key, (s.pos, s.colour, s.outward));
            }
        }
    }
    for (_, (pos, colour, outward)) in &cells {
        if *outward {
            voxel_mesh.add_voxel(*pos, *colour);
        }
    }
    for (_, (pos, colour, outward)) in &cells {
        if !*outward && !voxel_mesh.is_voxel_at(*pos) {
            voxel_mesh.add_voxel(*pos, *colour);
        }
    }

    // 2) 内部填充：外部洪水填充
    if !solid {
        return Ok(voxel_mesh);
    }
    //    网格标记：0=空 1=表面 2=外部(可达) 3=内部(待填)
    fn idx(x0: i32, y0: i32, z0: i32, nx: usize, ny: usize, x: i32, y: i32, z: i32) -> usize {
        ((x - x0) as usize) + ((y - y0) as usize) * nx + ((z - z0) as usize) * nx * ny
    }
    let (x0, y0, z0) = (bmin.x as i32, bmin.y as i32, bmin.z as i32);
    let (nx, ny, nz) = (
        (bmax.x as i32 - x0 + 1) as usize,
        (bmax.y as i32 - y0 + 1) as usize,
        (bmax.z as i32 - z0 + 1) as usize,
    );
    let mut grid = vec![0u8; nx * ny * nz];
    let mut surf_colour = vec![[0.0f32; 4]; nx * ny * nz];
    for (_, (pos, colour, _)) in &cells {
        let i = idx(x0, y0, z0, nx, ny, pos.x as i32, pos.y as i32, pos.z as i32);
        grid[i] = 1;
        surf_colour[i] = *colour;
    }

    // 外部 BFS：从 6 个边界面上的空单元向内部扩散（6 邻域，不穿表面）
    fn seed(grid: &mut [u8], queue: &mut VecDeque<usize>, i: usize) {
        if grid[i] == 0 {
            grid[i] = 2;
            queue.push_back(i);
        }
    }
    let (xmax, ymax, zmax) = (nx as i32 - 1, ny as i32 - 1, nz as i32 - 1);
    let mut queue: VecDeque<usize> = VecDeque::new();
    // idx 接收世界坐标（内部做 x-x0），网格索引需加回 x0/y0/z0
    for x in 0..nx as i32 {
        for y in 0..ny as i32 {
            seed(
                &mut grid,
                &mut queue,
                idx(x0, y0, z0, nx, ny, x0 + x, y0 + y, z0),
            );
            seed(
                &mut grid,
                &mut queue,
                idx(x0, y0, z0, nx, ny, x0 + x, y0 + y, z0 + zmax),
            );
        }
    }
    for x in 0..nx as i32 {
        for z in 0..nz as i32 {
            seed(
                &mut grid,
                &mut queue,
                idx(x0, y0, z0, nx, ny, x0 + x, y0, z0 + z),
            );
            seed(
                &mut grid,
                &mut queue,
                idx(x0, y0, z0, nx, ny, x0 + x, y0 + ymax, z0 + z),
            );
        }
    }
    for y in 0..ny as i32 {
        for z in 0..nz as i32 {
            seed(
                &mut grid,
                &mut queue,
                idx(x0, y0, z0, nx, ny, x0, y0 + y, z0 + z),
            );
            seed(
                &mut grid,
                &mut queue,
                idx(x0, y0, z0, nx, ny, x0 + xmax, y0 + y, z0 + z),
            );
        }
    }
    while let Some(i) = queue.pop_front() {
        let (gx, gy, gz) = (
            (i % nx) as i32,
            ((i / nx) % ny) as i32,
            (i / (nx * ny)) as i32,
        );
        for (dx, dy, dz) in [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (hx, hy, hz) = (gx + dx, gy + dy, gz + dz);
            if hx < 0 || hy < 0 || hz < 0 || hx > xmax || hy > ymax || hz > zmax {
                continue;
            }
            let j = idx(x0, y0, z0, nx, ny, x0 + hx, y0 + hy, z0 + hz);
            if grid[j] == 0 {
                grid[j] = 2;
                queue.push_back(j);
            }
        }
    }

    // 内部着色：从表面多源 BFS 向内部扩散颜色（取最近表面色）
    queue.clear();
    for (i, g) in grid.iter().enumerate() {
        if *g == 1 {
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let (gx, gy, gz) = (
            (i % nx) as i32,
            ((i / nx) % ny) as i32,
            (i / (nx * ny)) as i32,
        );
        let col = surf_colour[i];
        for (dx, dy, dz) in [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (hx, hy, hz) = (gx + dx, gy + dy, gz + dz);
            if hx < 0 || hy < 0 || hz < 0 || hx > xmax || hy > ymax || hz > zmax {
                continue;
            }
            let j = idx(x0, y0, z0, nx, ny, x0 + hx, y0 + hy, z0 + hz);
            if grid[j] == 0 {
                grid[j] = 3;
                surf_colour[j] = col;
                queue.push_back(j);
            }
        }
    }

    // 落内部体素
    for (i, g) in grid.iter().enumerate() {
        if *g == 3 {
            let (gx, gy, gz) = (
                (i % nx) as i32,
                ((i / nx) % ny) as i32,
                (i / (nx * ny)) as i32,
            );
            voxel_mesh.add_voxel(
                Vec3::new((x0 + gx) as f32, (y0 + gy) as f32, (z0 + gz) as f32),
                surf_colour[i],
            );
        }
    }

    Ok(voxel_mesh)
}

/// 三角形向三个轴平面做光栅化：对每个体素列求平面交点，命中点在三角形内则采样颜色
fn rasterise_tri(
    tri: [Vec3; 3],
    bmin: Vec3,
    bmax: Vec3,
    centre: Vec3,
    mesh: &Mesh,
    tris: &[[Vec3; 3]],
    tri_idx: usize,
    mat_idx: usize,
    tri_colour: Option<[[f32; 4]; 3]>,
    multisample: bool,
    mat_avg: &[f32; 4],
) -> Vec<Stamp> {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    let n = (b - a).cross(c - a);
    let mid = (a + b + c) * (1.0 / 3.0);
    let outward = n.dot(mid - centre) > 0.0;
    let mut out = Vec::new();

    for axis in 0..3u8 {
        // 法线沿该轴分量过小
        // 三角形与该轴近平行，射线不命中，跳过
        let nd = comp(n, axis);
        if nd.abs() < 1e-9 {
            continue;
        }
        // 足迹轴
        let (ua, va) = match axis {
            0 => (1u8, 2u8),
            1 => (0u8, 2u8),
            _ => (0u8, 1u8),
        };
        // 足迹 AABB
        let umin = comp(a, ua).min(comp(b, ua)).min(comp(c, ua));
        let umax = comp(a, ua).max(comp(b, ua)).max(comp(c, ua));
        let vmin = comp(a, va).min(comp(b, va)).min(comp(c, va));
        let vmax = comp(a, va).max(comp(b, va)).max(comp(c, va));
        let u0 = umin.floor().max(comp(bmin, ua)) as i32;
        let u1 = umax.ceil().min(comp(bmax, ua)) as i32;
        let v0 = vmin.floor().max(comp(bmin, va)) as i32;
        let v1 = vmax.ceil().min(comp(bmax, va)) as i32;
        if u0 > u1 || v0 > v1 {
            continue;
        }

        for u in u0..=u1 {
            for v in v0..=v1 {
                // 射线起点：深度轴坐标 0，另两轴 = (u, v)（与 bvh-ray 的整点射线一致）
                let c0 = match axis {
                    0 => Vec3::new(0.0, u as f32, v as f32),
                    1 => Vec3::new(u as f32, 0.0, v as f32),
                    _ => Vec3::new(u as f32, v as f32, 0.0),
                };
                // 平面 n·(p-a)=0：p = c0 + t·e_axis → t = n·(a-c0)/nd
                let t = (n.dot(a) - n.dot(c0)) / nd;
                let p = match axis {
                    0 => Vec3::new(t, u as f32, v as f32),
                    1 => Vec3::new(u as f32, t, v as f32),
                    _ => Vec3::new(u as f32, v as f32, t),
                };
                // 命中点在三角形内（重心坐标 ≥ 0，容差防浮点误判）
                let (b0, b1, b2) = barycentric(a, b, c, p);
                if b0 < -1e-4 || b1 < -1e-4 || b2 < -1e-4 {
                    continue;
                }
                let colour = voxel_colour(
                    mesh,
                    tris,
                    tri_idx,
                    mat_idx,
                    tri_colour,
                    p,
                    multisample,
                    mat_avg,
                );
                out.push(Stamp {
                    pos: p.round(),
                    colour,
                    outward,
                });
            }
        }
    }
    out
}

/// 生成射线
fn generate_rays(bmin: Vec3, bmax: Vec3) -> Vec<Ray> {
    let (x0, x1) = (bmin.x as i32, bmax.x as i32);
    let (y0, y1) = (bmin.y as i32, bmax.y as i32);
    let (z0, z1) = (bmin.z as i32, bmax.z as i32);
    let (nx, ny, nz) = (
        (x1 - x0 + 1) as usize,
        (y1 - y0 + 1) as usize,
        (z1 - z0 + 1) as usize,
    );
    let mut rays = Vec::with_capacity(ny * nz + nx * nz + nx * ny);
    // X
    for y in y0..=y1 {
        for z in z0..=z1 {
            rays.push(Ray::new(
                Vec3::new(x0 as f32 - 1.0, y as f32, z as f32),
                Vec3::new(1.0, 0.0, 0.0),
            ));
        }
    }
    // Y
    for x in x0..=x1 {
        for z in z0..=z1 {
            rays.push(Ray::new(
                Vec3::new(x as f32, y0 as f32 - 1.0, z as f32),
                Vec3::new(0.0, 1.0, 0.0),
            ));
        }
    }
    // Z
    for x in x0..=x1 {
        for y in y0..=y1 {
            rays.push(Ray::new(
                Vec3::new(x as f32, y as f32, z0 as f32 - 1.0),
                Vec3::new(0.0, 0.0, 1.0),
            ));
        }
    }
    rays
}

/// 顶点聚类减面：把顶点归入 `grid` 大小的网格单元，取单元平均顶点重建三角形并去掉退化面
/// 顶点色随三角形同步保留。
fn cluster_decimate(
    tris: &[[Vec3; 3]],
    mats: &[usize],
    tri_colours: &[Option<[[f32; 4]; 3]>],
    grid: f32,
) -> (Vec<[Vec3; 3]>, Vec<usize>, Vec<Option<[[f32; 4]; 3]>>) {
    // 包围盒（聚类网格原点）
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    for t in tris {
        for v in t {
            min = min.min(*v);
        }
    }
    let key_of = |v: &Vec3| -> u64 {
        let x = (((v.x - min.x) / grid).floor() as i64) as u64;
        let y = (((v.y - min.y) / grid).floor() as i64) as u64;
        let z = (((v.z - min.z) / grid).floor() as i64) as u64;
        (x & 0xFFFFF) << 40 | (y & 0xFFFFF) << 20 | (z & 0xFFFFF)
    };

    // 顶点累加到所属单元
    let mut cells: HashMap<u64, (Vec3, u32)> = HashMap::with_capacity(tris.len());
    for t in tris {
        for v in t {
            let e = cells.entry(key_of(v)).or_insert((*v, 0));
            e.0 = Vec3::new(e.0.x + v.x, e.0.y + v.y, e.0.z + v.z);
            e.1 += 1;
        }
    }

    // 单元平均顶点 -> 新顶点表 + 索引映射
    let mut index: HashMap<u64, usize> = HashMap::with_capacity(cells.len());
    let mut verts: Vec<Vec3> = Vec::with_capacity(cells.len());
    for (k, (sum, n)) in cells {
        index.insert(k, verts.len());
        verts.push(Vec3::new(
            sum.x / n as f32,
            sum.y / n as f32,
            sum.z / n as f32,
        ));
    }

    // 重建三角形（保留材质与顶点色；去掉顶点退化面）
    let mut out_tris = Vec::with_capacity(tris.len());
    let mut out_mats = Vec::with_capacity(tris.len());
    let mut out_colours = Vec::with_capacity(tris.len());
    for (i, t) in tris.iter().enumerate() {
        let a = index[&key_of(&t[0])];
        let b = index[&key_of(&t[1])];
        let c = index[&key_of(&t[2])];
        if a == b || b == c || a == c {
            continue;
        }
        out_tris.push([verts[a], verts[b], verts[c]]);
        out_mats.push(mats[i]);
        out_colours.push(tri_colours[i]);
    }
    (out_tris, out_mats, out_colours)
}

/// 交点处的体素颜色：纯色材质直接取色（有顶点色则按重心插值顶点色）；
/// 纹理材质按重心坐标插值 UV 采样。无 UV 的三角形用贴图平均色（`mat_avg`）。
fn voxel_colour(
    mesh: &Mesh,
    tris: &[[Vec3; 3]],
    tri: usize,
    mat_idx: usize,
    tri_colour: Option<[[f32; 4]; 3]>,
    point: Vec3,
    multisample: bool,
    mat_avg: &[f32; 4],
) -> [f32; 4] {
    let mat = &mesh.materials[mat_idx];
    match mat.kind {
        MaterialKind::Solid => {
            // 有顶点色：按重心坐标插值（无贴图模型常用顶点色着色）
            if let Some(cols) = tri_colour {
                let [a, b, c] = tris[tri];
                let (u, v, w) = barycentric(a, b, c, point);
                [
                    cols[0][0] * u + cols[1][0] * v + cols[2][0] * w,
                    cols[0][1] * u + cols[1][1] * v + cols[2][1] * w,
                    cols[0][2] * u + cols[1][2] * v + cols[2][2] * w,
                    cols[0][3] * u + cols[1][3] * v + cols[2][3] * w,
                ]
            } else {
                mat.colour
            }
        }
        MaterialKind::Textured => {
            let Some(tex) = mat.texture.as_ref() else {
                return mat.colour;
            };
            // 三角形无 UV：无法在贴图上定位，用贴图平均色兜底
            if mesh.tris[tri].texcoord.is_none() {
                return *mat_avg;
            }
            let [a, b, c] = tris[tri];
            let uvs = mesh.triangle_uvs(tri);
            // 单点采样：按重心坐标插值 UV
            let sample = |p: Vec3| -> [f32; 4] {
                let (u, v, w) = barycentric(a, b, c, p);
                let uv = UV::new(
                    uvs[0].u * u + uvs[1].u * v + uvs[2].u * w,
                    uvs[0].v * u + uvs[1].v * v + uvs[2].v * w,
                );
                let mut colour = tex.sample(uv, mat.filtering, mat.wrap);
                if let Transparency::UseAlphaValue(alpha) = mat.transparency {
                    colour[3] = alpha;
                }
                colour
            };

            // 多采样：取体素内确定性偏移多点平均（偏移由位置散列生成，可复现）
            let n = if multisample { MULTISAMPLE_COUNT } else { 1 };
            let mut acc = [0.0f32; 4];
            for s in 0..n {
                let p = if multisample {
                    point + sample_offset(point, s)
                } else {
                    point
                };
                let c = sample(p);
                for i in 0..4 {
                    acc[i] += c[i];
                }
            }
            for i in 0..4 {
                acc[i] /= n as f32;
            }
            acc
        }
    }
}

/// 确定性采样偏移（[-0.5, 0.5]³，免随机数状态、可复现）
fn sample_offset(pos: Vec3, seed: u32) -> Vec3 {
    let h = |axis: u32| -> f32 {
        let mut x =
            hash_pos(pos) ^ (seed + 1).wrapping_mul(0x9E37_79B9) ^ axis.wrapping_mul(0x85EB_CA6B);
        x = (x ^ (x >> 16)).wrapping_mul(0xC2B2_AE35);
        let x = x ^ (x >> 16);
        (x & 0xFF_FFFF) as f32 / 0x100_0000 as f32
    };
    Vec3::new(h(0) - 0.5, h(1) - 0.5, h(2) - 0.5)
}

/// FNV-1a 散列（仅用于采样偏移的确定性伪随机）
fn hash_pos(pos: Vec3) -> u32 {
    let mut h = 0x811C_9DC5u32;
    for b in [pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()] {
        h = (h ^ b).wrapping_mul(0x0100_0193);
    }
    h
}

fn component(v: Vec3, axis: Axis) -> f32 {
    match axis {
        Axis::X => v.x,
        Axis::Y => v.y,
        Axis::Z => v.z,
    }
}

/// 按 u8 轴取分量（光栅化内部用）
fn comp(v: Vec3, axis: u8) -> f32 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}
