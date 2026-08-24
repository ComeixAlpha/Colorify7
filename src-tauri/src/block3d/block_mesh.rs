//! 方块网格：体素 -> MC 方块（像素画调色板匹配）

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::obj3d::vec3::Vec3;
use crate::obj3d::voxel_mesh::{key_of, Voxel, VoxelMesh};

use super::block_assigner::{bin, dither, BlockCollection, DitherMode, PixelPalette};
use super::constants::{EXCLUDED_BLOCKS, FALLABLE_BLOCKS, GRASS_LIKE_BLOCKS, TRANSPARENT_BLOCKS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FallableBehaviour {
    ReplaceFallable,
    #[default]
    ReplaceFalling,
    DoNothing,
}

#[derive(Debug, Clone, Copy)]
pub struct AssignParams {
    pub dithering: DitherMode,
    pub dithering_magnitude: f32,
    /// 颜色精度（bin 粒度，如 32；越大颜色分桶越少）
    pub resolution: u32,
    /// 智能平均：仅用可见面求平均色
    #[allow(dead_code)]
    pub contextual_averaging: bool,
    /// 平滑权重 [0,1]（0=纯色差，1=纯纹理 std）
    pub error_weight: f32,
    pub fallable: FallableBehaviour,
}

impl Default for AssignParams {
    fn default() -> Self {
        Self {
            dithering: DitherMode::Ordered,
            dithering_magnitude: 32.0,
            resolution: 32,
            contextual_averaging: true,
            error_weight: 0.0,
            fallable: FallableBehaviour::ReplaceFalling,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacedBlock {
    pub position: Vec3,
    pub name: String,
    pub colour: [f32; 3],
}

#[derive(Debug)]
pub struct BlockMesh {
    pub blocks: Vec<PlacedBlock>,
    /// 结构包围盒（含端点）
    pub min: Vec3,
    pub max: Vec3,
}

/// Assign 中间产物
enum AssignItem {
    Placed(u64, (Vec3, usize, [f32; 4])),
    Grass(u64, [f32; 4]),
}

/// 体素网格 -> 方块网格
pub fn assign(
    voxel_mesh: &VoxelMesh,
    params: &AssignParams,
    cancel: &AtomicBool,
) -> Result<BlockMesh, String> {
    let palette = PixelPalette::load();
    let all_blocks = palette.create_block_collection(EXCLUDED_BLOCKS);
    let mut nf_exclude: Vec<&str> = FALLABLE_BLOCKS.to_vec();
    nf_exclude.extend_from_slice(EXCLUDED_BLOCKS);
    let nf_blocks = palette.create_block_collection(&nf_exclude);
    if all_blocks.is_empty() {
        return Err("调色板中没有可用的方块".into());
    }

    // 主循环并行
    let voxels: Vec<&Voxel> = voxel_mesh.voxels().collect();
    let items: Vec<AssignItem> = voxels
        .par_iter()
        .fold(
            || {
                (
                    Vec::new(),
                    BlockCollection::new(all_blocks.clone()),
                    BlockCollection::new(nf_blocks.clone()),
                )
            },
            |(mut acc, mut all, mut nf), voxel| {
                if cancel.load(Ordering::SeqCst) {
                    return (acc, all, nf);
                }
                // 最终体素色：bin 量化 -> 抖动（255 空间）
                let mut colour = bin(voxel.colour, params.resolution);
                dither(
                    &mut colour,
                    params.dithering,
                    voxel.position,
                    params.dithering_magnitude,
                );

                let mut block_idx = palette.get_block(colour, &mut all, params.error_weight);
                let id = palette.blocks[block_idx].id.as_str();
                let is_fallable = FALLABLE_BLOCKS.contains(&id);
                let is_supported =
                    voxel_mesh.is_voxel_at(voxel.position + Vec3::new(0.0, -1.0, 0.0));
                let should_replace = match params.fallable {
                    FallableBehaviour::ReplaceFallable => is_fallable,
                    FallableBehaviour::ReplaceFalling => is_fallable && !is_supported,
                    FallableBehaviour::DoNothing => false,
                };
                if should_replace {
                    block_idx = palette.get_block(colour, &mut nf, params.error_weight);
                }

                let key = key_of(voxel.position);
                let id = palette.blocks[block_idx].id.as_str();
                if GRASS_LIKE_BLOCKS.contains(&id) {
                    acc.push(AssignItem::Grass(key, colour));
                }
                acc.push(AssignItem::Placed(key, (voxel.position, block_idx, colour)));
                (acc, all, nf)
            },
        )
        .map(|(acc, _, _)| acc)
        .flatten()
        .collect();

    if cancel.load(Ordering::SeqCst) {
        return Err("已取消".into());
    }

    // 位置键 -> (体素位置, 调色板方块下标, 体素色)
    let mut placed: HashMap<u64, (Vec3, usize, [f32; 4])> = HashMap::with_capacity(voxels.len());
    let mut grass_like: Vec<(u64, [f32; 4])> = Vec::new();
    for item in items {
        match item {
            AssignItem::Placed(key, val) => {
                placed.insert(key, val);
            }
            AssignItem::Grass(key, colour) => grass_like.push((key, colour)),
        }
    }

    // 草类方块二遍：正上方为不透明方块时，把草皮换成非草类方块
    let mut non_grass = BlockCollection::new(palette.create_block_collection(GRASS_LIKE_BLOCKS));
    for (key, colour) in grass_like {
        if cancel.load(Ordering::SeqCst) {
            return Err("已取消".into());
        }
        let Some(&(pos, _, _)) = placed.get(&key) else {
            continue;
        };
        let above_key = key_of(pos + Vec3::new(0.0, 1.0, 0.0));
        if let Some(&(_, above_idx, _)) = placed.get(&above_key) {
            let above_id = palette.blocks[above_idx].id.as_str();
            if !TRANSPARENT_BLOCKS.contains(&above_id) {
                let new_idx = palette.get_block(colour, &mut non_grass, params.error_weight);
                placed.insert(key, (pos, new_idx, colour));
            }
        }
    }

    // 输出
    let mut blocks = Vec::with_capacity(placed.len());
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (pos, idx, colour) in placed.into_values() {
        min = min.min(pos);
        max = max.max(pos);
        blocks.push(PlacedBlock {
            position: pos,
            name: palette.blocks[idx].id.clone(),
            colour: [colour[0], colour[1], colour[2]],
        });
    }
    if blocks.is_empty() {
        return Err("没有产出任何方块".into());
    }
    Ok(BlockMesh { blocks, min, max })
}
