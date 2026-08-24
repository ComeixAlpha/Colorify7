//! 颜色 -> 方块匹配

use std::collections::HashMap;

use crate::obj3d::vec3::Vec3;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DitherMode {
    #[default]
    Off,
    Random,
    Ordered,
}

#[derive(Debug, Clone)]
pub struct PixelBlock {
    pub id: String,
    pub colour: [f32; 3],
}

/// 候选方块集合
pub struct BlockCollection {
    blocks: Vec<usize>,
    cache: HashMap<u32, usize>,
}

impl BlockCollection {
    pub fn new(blocks: Vec<usize>) -> Self {
        Self {
            blocks,
            cache: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[derive(Deserialize)]
struct PixelPaletteFile {
    palette: Vec<PixelPaletteEntry>,
}

#[derive(Deserialize)]
struct PixelPaletteEntry {
    id: String,
    average: [f32; 3],
}

/// 像素画调色板：从 resources/palettes/pixel_palette.json 加载
pub struct PixelPalette {
    pub blocks: Vec<PixelBlock>,
}

impl PixelPalette {
    pub fn load() -> Self {
        let raw = include_str!("../../resources/palettes/pixel_palette.json");
        let file: PixelPaletteFile =
            serde_json::from_str(raw).expect("pixel_palette.json 解析失败");
        Self {
            blocks: file
                .palette
                .into_iter()
                .map(|e| PixelBlock {
                    id: e.id,
                    colour: e.average,
                })
                .collect(),
        }
    }

    pub fn create_block_collection(&self, exclude: &[&str]) -> Vec<usize> {
        self.blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| !exclude.contains(&b.id.as_str()))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn get_block(
        &self,
        colour: [f32; 4],
        collection: &mut BlockCollection,
        _error_weight: f32,
    ) -> usize {
        let key = hash255(colour);
        if let Some(&idx) = collection.cache.get(&key) {
            return idx;
        }
        let float_colour = [
            colour[0] / 255.0,
            colour[1] / 255.0,
            colour[2] / 255.0,
            colour[3] / 255.0,
        ];
        let mut best = 0;
        let mut min_error = f32::INFINITY;
        for &i in &collection.blocks {
            let b = &self.blocks[i];
            let target = [
                b.colour[0] / 255.0,
                b.colour[1] / 255.0,
                b.colour[2] / 255.0,
                1.0,
            ];
            let err = squared_distance(float_colour, target);
            if err < min_error {
                min_error = err;
                best = i;
            }
        }
        collection.cache.insert(key, best);
        best
    }
}

pub fn hash255(colour: [f32; 4]) -> u32 {
    let t = |v: f32| (v as i32) as u8 as u32;
    (t(colour[0]) << 24) | (t(colour[1]) << 16) | (t(colour[2]) << 8) | t(colour[3])
}

fn squared_distance(a: [f32; 4], b: [f32; 4]) -> f32 {
    (0..4).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

/// bin：把 [0,1] 色按 resolution 粒度量化到 255 空间（alpha 用 ceil）
pub fn bin(colour: [f32; 4], resolution: u32) -> [f32; 4] {
    let res = resolution as f32;
    let b = |c: f32| ((c * res).floor() * (255.0 / res)).floor();
    [
        b(colour[0]),
        b(colour[1]),
        b(colour[2]),
        ((colour[3] * res).ceil() * (255.0 / res)).floor(),
    ]
}

/// 抖动
pub fn dither(colour: &mut [f32; 4], mode: DitherMode, position: Vec3, magnitude: f32) {
    let offset = match mode {
        DitherMode::Off => return,
        DitherMode::Random => (rand01(position) - 0.5) * magnitude,
        DitherMode::Ordered => {
            let x = (position.x as i32 % 4).abs() as usize;
            let y = (position.y as i32 % 4).abs() as usize;
            let z = (position.z as i32 % 4).abs() as usize;
            (DITHER_MATRIX[x + 4 * y + 16 * z] as f32 / 64.0 - 0.5) * magnitude
        }
    };
    colour[0] += offset;
    colour[1] += offset;
    colour[2] += offset;
}

const DITHER_MATRIX: [u8; 64] = [
    0, 16, 2, 18, 48, 32, 50, 34, //
    6, 22, 4, 20, 54, 38, 52, 36, //
    24, 40, 26, 42, 8, 56, 10, 58, //
    30, 46, 28, 44, 14, 62, 12, 60, //
    3, 19, 5, 21, 51, 35, 53, 37, //
    1, 17, 7, 23, 49, 33, 55, 39, //
    27, 43, 29, 45, 11, 59, 13, 61, //
    25, 41, 31, 47, 9, 57, 15, 63, //
];

/// 位置散列的确定性 [0,1)
fn rand01(pos: Vec3) -> f32 {
    let mut h = 0x811C_9DC5u32;
    for b in [pos.x.to_bits(), pos.y.to_bits(), pos.z.to_bits()] {
        h = (h ^ b).wrapping_mul(0x0100_0193);
    }
    h = (h ^ (h >> 16)).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    (h & 0xFF_FFFF) as f32 / 0x100_0000 as f32
}
