//! 体素网格

use std::collections::HashMap;

use super::vec3::Vec3;

/// 重叠体素取色规则
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelOverlapRule {
    /// 保留第一个体素颜色
    First,
    /// 运行平均
    Average,
}

/// 单个体素
#[derive(Debug, Clone)]
pub struct Voxel {
    pub position: Vec3,
    pub colour: [f32; 4],
    /// 重叠写入次数（Average 规则用）
    collisions: u32,
    /// 26 邻域位掩码
    #[allow(dead_code)]
    neighbours: u32,
}

/// 体素网格：HashMap 稀疏存储
#[derive(Debug, Clone)]
pub struct VoxelMesh {
    voxels: HashMap<u64, Voxel>,
    min: Vec3,
    max: Vec3,
    rule: VoxelOverlapRule,
}

impl VoxelMesh {
    pub fn new(rule: VoxelOverlapRule) -> Self {
        Self {
            voxels: HashMap::new(),
            min: Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
            rule,
        }
    }

    /// 预分配容量
    pub fn reserve(&mut self, additional: usize) {
        self.voxels.reserve(additional);
    }

    /// 添加体素：位置取整；alpha=0 丢弃；重叠按规则合并颜色
    pub fn add_voxel(&mut self, pos: Vec3, colour: [f32; 4]) {
        if colour[3] <= 0.0 {
            return;
        }
        let pos = pos.round();
        let key = key_of(pos);
        if let Some(v) = self.voxels.get_mut(&key) {
            if self.rule == VoxelOverlapRule::Average {
                let n = v.collisions as f32;
                for i in 0..4 {
                    v.colour[i] = (v.colour[i] * n + colour[i]) / (n + 1.0);
                }
            }
            v.collisions += 1;
        } else {
            self.min = self.min.min(pos);
            self.max = self.max.max(pos);
            self.voxels.insert(
                key,
                Voxel {
                    position: pos,
                    colour,
                    collisions: 1,
                    neighbours: 0,
                },
            );
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.voxels.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.voxels.is_empty()
    }

    pub fn is_voxel_at(&self, pos: Vec3) -> bool {
        self.voxels.contains_key(&key_of(pos.round()))
    }

    /// 该位置是否被不透明体素占据（alpha==1；面可见性判断用）
    #[allow(dead_code)]
    pub fn is_opaque_voxel_at(&self, pos: Vec3) -> bool {
        match self.get_voxel(pos) {
            Some(v) => v.colour[3] == 1.0,
            None => false,
        }
    }

    pub fn get_voxel(&self, pos: Vec3) -> Option<&Voxel> {
        self.voxels.get(&key_of(pos.round()))
    }

    pub fn voxels(&self) -> impl Iterator<Item = &Voxel> {
        self.voxels.values()
    }

    /// 体素包围盒（无体素返回 None）
    #[allow(dead_code)]
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        if self.voxels.is_empty() {
            None
        } else {
            Some((self.min, self.max))
        }
    }
}

/// 坐标 → 哈希键（每轴 21 位，范围 ±2^20，覆盖建筑尺度且无碰撞）
pub fn key_of(pos: Vec3) -> u64 {
    let (x, y, z) = (pos.x as i64, pos.y as i64, pos.z as i64);
    (((x as u64) & 0x1F_FFFF) << 42) | (((y as u64) & 0x1F_FFFF) << 21) | ((z as u64) & 0x1F_FFFF)
}
