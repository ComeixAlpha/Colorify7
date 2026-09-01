//! 像素画管线

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use image::DynamicImage;
use rayon::prelude::*;

use crate::common;
use crate::image_ditherer::{PaletteColorInput, PaletteIndexer};
use crate::nbt::McStructure;

pub struct GenParams {
    /// 0=xOy 1=xOz 2=yOz
    pub plane: i32,
    pub color_distance: String,
    pub use_staircase: bool,
    pub use_struct: bool,
    pub staircase_gap: i32,
    /// 阶梯式无损压缩：每级高度压到 min(gap, 2)，每段独立下沉使最低方块贴基准 0
    pub staircase_compress: bool,
    pub offset: [i32; 3],
    pub auto_slice_mcfunction: bool,
    pub use_dithering: bool,
    pub socket_output: bool,
    /// 直写 LevelDB 世界：收集方块列表 (x, y, z, id)，不写文件/命令
    pub ldb_output: bool,
}

pub struct GenOutput {
    /// 实际匹配的 RGBA 预览缓冲
    pub preview: Vec<u8>,
    pub commands: Option<Vec<String>>,
    /// 直写世界模式收集的方块（世界相对坐标，已含平面切换与偏移）
    pub blocks_ldb: Option<Vec<(i32, i32, i32, String)>>,
    /// 包围盒尺寸 [x, z, y]
    pub size: [i32; 3],
    pub function_files: usize,
}

struct PlacedBlock<'a> {
    x: i32,
    y: i32,
    z: i32,
    id: &'a str,
}

#[derive(Clone, Copy)]
struct StairCell {
    /// 放置高度；`i32::MIN` 表示透明/空
    y: i32,
    idx: u16,
    shade: u8,
}

/// 入口
pub fn generate<'a>(
    img: &DynamicImage,
    alpha: Option<&[u8]>,
    palette: &'a [PaletteColorInput],
    params: &GenParams,
    out_dir: Option<&Path>,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&str),
) -> Result<GenOutput, String> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    if w == 0 || h == 0 {
        return Err("图片尺寸为空".into());
    }
    let (width, height) = (w as usize, h as usize);

    let full_rgb: Vec<[u8; 3]> = palette.iter().map(|e| e.average).collect();
    let full_ids: Vec<&str> = palette.iter().map(|e| e.id.as_str()).collect();

    let mut commands: Vec<String> = Vec::new();
    let mut blocks: Vec<PlacedBlock> = Vec::new();
    let mut blocks_ldb: Vec<(i32, i32, i32, String)> = Vec::new();
    let mut writer = if params.use_struct || params.socket_output || params.ldb_output {
        None
    } else {
        Some(FunctionWriter::new(
            params,
            out_dir.unwrap_or(Path::new("")),
        )?)
    };
    // 包围盒
    let (mut lx, mut ly, mut lz) = (i32::MAX, i32::MAX, i32::MAX);
    let (mut mx, mut my, mut mz) = (i32::MIN, i32::MIN, i32::MIN);
    let mut emitted = 0usize;
    let mut emit = |x: i32, y: i32, z: i32, id: &'a str| -> Result<(), String> {
        lx = lx.min(x);
        ly = ly.min(y);
        lz = lz.min(z);
        mx = mx.max(x);
        my = my.max(y);
        mz = mz.max(z);
        emitted += 1;
        if params.socket_output {
            commands.push(socket_command(params, x, y, z, id));
            Ok(())
        } else if params.ldb_output {
            let (cx, cy, cz) = command_coords(params, x, y, z);
            blocks_ldb.push((cx, cy, cz, id.to_string()));
            Ok(())
        } else {
            match writer.as_mut() {
                Some(w) => w.emit(x, y, z, id),
                None => {
                    blocks.push(PlacedBlock { x, y, z, id });
                    Ok(())
                }
            }
        }
    };

    // 预览图 RGBA 缓冲（w*h*4，标准行优先：第 z 行、每行 width 像素）
    let mut preview = vec![0u8; width * height * 4];

    if params.use_staircase {
        on_progress("构建阶梯式");
        let mut orig = vec![[0u8; 3]; width * height];
        for x in 0..width {
            for z in 0..height {
                let (px, py) = (width - 1 - x, height - 1 - z);
                let p = rgb.get_pixel(px as u32, py as u32);
                orig[x * height + z] = [p[0], p[1], p[2]];
            }
        }

        // 预构建阶梯式全阴影调色板
        let mut stair_rgb: Vec<[u8; 3]> = Vec::with_capacity(full_rgb.len() * 3);
        for c in &full_rgb {
            stair_rgb.push(to_u8(shade_rgb(c, 255)));
            stair_rgb.push(to_u8(shade_rgb(c, 220)));
            stair_rgb.push(to_u8(shade_rgb(c, 180)));
        }
        let stair_indexer = PaletteIndexer::new(&stair_rgb, &params.color_distance)
            .ok_or_else(|| "色差公式无效".to_string())?;

        // 抖动
        let dither_idx3s: Option<Vec<u16>> = if params.use_dithering {
            Some(staircase_fs_dither(
                &orig,
                width,
                height,
                &stair_rgb,
                &stair_indexer,
            ))
        } else {
            None
        };

        // 启用压缩时自动调整 Step
        let step = staircase_step(params.staircase_gap, params.staircase_compress);

        // 并行偏移传播
        let mut cells = vec![
            StairCell {
                y: i32::MIN,
                idx: 0,
                shade: 0,
            };
            width * height
        ];
        cells.par_chunks_mut(height).enumerate().try_for_each(
            |(x, col)| -> Result<(), String> {
                if cancel.load(Ordering::SeqCst) {
                    return Err("已取消".into());
                }
                let mut basey: i32 = 0;
                let mut offset: i32 = 0;
                for z in (0..height).rev() {
                    let (px, py) = (width - 1 - x, height - 1 - z);
                    if alpha.map_or(false, |a| a[py * width + px] != 255) {
                        basey = 0;
                        offset = 0;
                        continue;
                    }
                    let idx3 = match &dither_idx3s {
                        Some(d) => d[x * height + z],
                        None => {
                            let p = orig[x * height + z];
                            stair_indexer.nearest(p[0], p[1], p[2]) as u16
                        }
                    };
                    let idx = (idx3 / 3) as usize;
                    let shade = (idx3 % 3) as u8;
                    let offset_now = match shade {
                        0 => -step,
                        1 => 0,
                        _ => step,
                    };
                    let y = basey + offset;
                    col[z] = StairCell {
                        y,
                        idx: idx as u16,
                        shade,
                    };
                    basey = y;
                    offset = offset_now;
                }

                // 压缩
                if params.staircase_compress {
                    compress_staircase_column(col, step);
                }
                Ok(())
            },
        )?;

        for x in 0..width {
            for z in (0..height).rev() {
                let cell = cells[x * height + z];
                if cell.y == i32::MIN {
                    continue;
                }
                let c = stair_rgb[cell.idx as usize * 3 + cell.shade as usize];
                // 行优先：第 z 行、第 x 列，与 encode_rgba_png(width, height) 的解析一致
                let i4 = (z * width + x) * 4;
                preview[i4..i4 + 3].copy_from_slice(&c);
                preview[i4 + 3] = 255;
                emit(x as i32, cell.y, z as i32, full_ids[cell.idx as usize])?;
            }
        }
    } else {
        on_progress("匹配方块");
        let indexer = PaletteIndexer::new(&full_rgb, &params.color_distance)
            .ok_or_else(|| "色差公式无效".to_string())?;
        let mut idxs = vec![u16::MAX; width * height];
        idxs.par_chunks_mut(height)
            .enumerate()
            .try_for_each(|(x, col)| -> Result<(), String> {
                if cancel.load(Ordering::SeqCst) {
                    return Err("已取消".into());
                }
                for z in 0..height {
                    let (px, py) = (width - 1 - x, height - 1 - z);
                    if alpha.map_or(false, |a| a[py * width + px] != 255) {
                        continue;
                    }
                    let p = rgb.get_pixel(px as u32, py as u32);
                    col[z] = indexer.nearest(p[0], p[1], p[2]) as u16;
                }
                Ok(())
            })?;

        for x in 0..width {
            for z in 0..height {
                let idx = idxs[x * height + z];
                if idx == u16::MAX {
                    continue;
                }
                let c = full_rgb[idx as usize];
                let i4 = (z * width + x) * 4;
                preview[i4..i4 + 3].copy_from_slice(&c);
                preview[i4 + 3] = 255;
                emit(x as i32, 0, z as i32, full_ids[idx as usize])?;
            }
        }
    }

    let size = if emitted == 0 {
        [0, 0, 0]
    } else {
        [mx - lx + 1, my - ly + 1, mz - lz + 1]
    };

    let mut function_files = 0usize;

    if params.socket_output {
    } else if params.use_struct {
        on_progress("输出结构文件");
        write_structure(&blocks, params, out_dir.unwrap())?;
    } else {
        on_progress("输出函数");
        if let Some(w) = writer {
            function_files = w.finish()?;
        }
    }

    Ok(GenOutput {
        preview,
        commands: if params.socket_output {
            Some(commands)
        } else {
            None
        },
        blocks_ldb: if params.ldb_output {
            Some(blocks_ldb)
        } else {
            None
        },
        size,
        function_files,
    })
}

/// 平面切换 + 偏移后的世界相对坐标（socket / ldb 共用）
#[inline(always)]
fn command_coords(params: &GenParams, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    let (cx, cy, cz) = if params.use_staircase {
        (x, y, z)
    } else {
        let s = common::switch_xyz(params.plane, [x, y, z]);
        (s[0], s[1], s[2])
    };
    (
        cx + params.offset[0],
        cy + params.offset[1],
        cz + params.offset[2],
    )
}

/// WebSocket 命令
#[inline(always)]
fn socket_command(params: &GenParams, x: i32, y: i32, z: i32, id: &str) -> String {
    let (cx, cy, cz) = command_coords(params, x, y, z);
    format!("setblock ~{} ~{} ~{} {}", cx, cy, cz, id)
}

#[inline(always)]
fn shade_rgb(c: &[u8; 3], shade: i32) -> [f32; 3] {
    [
        (c[0] as i32 * shade / 255) as f32,
        (c[1] as i32 * shade / 255) as f32,
        (c[2] as i32 * shade / 255) as f32,
    ]
}

#[inline(always)]
fn to_u8(v: [f32; 3]) -> [u8; 3] {
    [
        v[0].round().clamp(0.0, 255.0) as u8,
        v[1].round().clamp(0.0, 255.0) as u8,
        v[2].round().clamp(0.0, 255.0) as u8,
    ]
}

fn staircase_fs_dither(
    orig: &[[u8; 3]],
    width: usize,
    height: usize,
    stair_rgb: &[[u8; 3]],
    indexer: &PaletteIndexer,
) -> Vec<u16> {
    let coe = 16 as f32;
    let (c1, c2, c3, c4) = (7.0 / coe, 3.0 / coe, 5.0 / coe, 1.0 / coe);
    let mut out = vec![0u16; orig.len()];
    let mut spill = vec![[0.0f32; 3]; orig.len()];

    for x in 0..width {
        for z in 0..height {
            let i = x * height + z;
            let o = orig[i];
            let r = (o[0] as f32 + spill[i][0]).clamp(0.0, 255.0);
            let g = (o[1] as f32 + spill[i][1]).clamp(0.0, 255.0);
            let b = (o[2] as f32 + spill[i][2]).clamp(0.0, 255.0);

            let idx3 = indexer.nearest(r as u8, g as u8, b as u8) as u16;
            let matched = stair_rgb[idx3 as usize];
            out[i] = idx3;

            let er = r - matched[0] as f32;
            let eg = g - matched[1] as f32;
            let eb = b - matched[2] as f32;

            if x + 1 < width {
                let j = (x + 1) * height + z;
                spill[j][0] += er * c1;
                spill[j][1] += eg * c1;
                spill[j][2] += eb * c1;
            }
            if z + 1 < height {
                let j = x * height + z + 1;
                spill[j][0] += er * c3;
                spill[j][1] += eg * c3;
                spill[j][2] += eb * c3;
            }
            if x > 0 && z + 1 < height {
                let j = (x - 1) * height + z + 1;
                spill[j][0] += er * c2;
                spill[j][1] += eg * c2;
                spill[j][2] += eb * c2;
            }
            if x + 1 < width && z + 1 < height {
                let j = (x + 1) * height + z + 1;
                spill[j][0] += er * c4;
                spill[j][1] += eg * c4;
                spill[j][2] += eb * c4;
            }
        }
    }
    out
}

/// mcfunction 流式写入
struct FunctionWriter {
    plane: i32,
    offset: [i32; 3],
    use_staircase: bool,
    auto_slice: bool,
    dir: std::path::PathBuf,
    file_idx: usize,
    line_count: usize,
    writer: std::io::BufWriter<std::fs::File>,
}

impl FunctionWriter {
    fn new(params: &GenParams, out_dir: &Path) -> Result<Self, String> {
        Ok(Self {
            plane: params.plane,
            offset: params.offset,
            use_staircase: params.use_staircase,
            auto_slice: params.auto_slice_mcfunction,
            dir: out_dir.to_path_buf(),
            file_idx: 0,
            line_count: 0,
            writer: open_function(out_dir, 0)?,
        })
    }

    fn emit(&mut self, x: i32, y: i32, z: i32, id: &str) -> Result<(), String> {
        let (cx, cy, cz) = if self.use_staircase {
            (x, y, z)
        } else {
            let s = common::switch_xyz(self.plane, [x, y, z]);
            (s[0], s[1], s[2])
        };
        write!(
            self.writer,
            "setblock ~{} ~{} ~{} {}\n",
            cx + self.offset[0],
            cy + self.offset[1],
            cz + self.offset[2],
            id
        )
        .map_err(|e| format!("写入 mcfunction 失败: {e}"))?;
        self.line_count += 1;

        if self.auto_slice && self.line_count >= 10000 {
            self.writer
                .flush()
                .map_err(|e| format!("写入 mcfunction 失败: {e}"))?;
            self.file_idx += 1;
            self.line_count = 0;
            self.writer = open_function(&self.dir, self.file_idx)?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<usize, String> {
        self.writer
            .flush()
            .map_err(|e| format!("写入 mcfunction 失败: {e}"))?;
        Ok(self.file_idx + 1)
    }
}

fn open_function(dir: &Path, idx: usize) -> Result<std::io::BufWriter<std::fs::File>, String> {
    let path = dir.join(format!("output_{idx}.mcfunction"));
    let f =
        std::fs::File::create(&path).map_err(|e| format!("创建 {} 失败: {e}", path.display()))?;
    // 1MB 缓冲：减少 syscall
    Ok(std::io::BufWriter::with_capacity(1 << 20, f))
}

/// 输出 .mcstructure
fn write_structure<'a>(
    blocks: &[PlacedBlock<'a>],
    params: &GenParams,
    out_dir: &Path,
) -> Result<(), String> {
    // 包围盒
    let mut lx = i32::MAX;
    let mut ly = i32::MAX;
    let mut lz = i32::MAX;
    let mut mx = i32::MIN;
    let mut my = i32::MIN;
    let mut mz = i32::MIN;
    for b in blocks {
        lx = lx.min(b.x);
        ly = ly.min(b.y);
        lz = lz.min(b.z);
        mx = mx.max(b.x);
        my = my.max(b.y);
        mz = mz.max(b.z);
    }
    if blocks.is_empty() {
        lx = 0;
        ly = 0;
        lz = 0;
        mx = 0;
        my = 0;
        mz = 0;
    }

    let size = if params.use_staircase {
        [mx - lx + 1, my - ly + 1, mz - lz + 1]
    } else {
        common::switch_xyz(params.plane, [mx - lx + 1, my - ly + 1, mz - lz + 1])
    };

    let mut structure = McStructure::new(size);
    for b in blocks {
        let pos = if params.use_staircase {
            [b.x, b.y - ly, b.z]
        } else {
            common::switch_xyz(params.plane, [b.x, b.y, b.z])
        };
        structure.set_block(pos, b.id);
    }

    let path = out_dir.join("output.mcstructure");
    let f =
        std::fs::File::create(&path).map_err(|e| format!("创建 {} 失败: {e}", path.display()))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    structure
        .write_file(&mut w)
        .map_err(|e| format!("写入 mcstructure 失败: {e}"))?;
    w.flush()
        .map_err(|e| format!("写入 mcstructure 失败: {e}"))?;
    Ok(())
}

pub fn export_blocks_3d(
    blocks: &[(i32, i32, i32, &str)],
    use_struct: bool,
    auto_slice_mcfunction: bool,
    offset: [i32; 3],
    socket_output: bool,
    out_dir: Option<&Path>,
) -> Result<Option<Vec<String>>, String> {
    if socket_output {
        let mut commands = Vec::with_capacity(blocks.len());
        for (x, y, z, name) in blocks {
            commands.push(format!(
                "setblock ~{} ~{} ~{} {}",
                x + offset[0],
                y + offset[1],
                z + offset[2],
                name
            ));
        }
        return Ok(Some(commands));
    }
    let dir = out_dir.expect("文件模式必须提供输出目录");

    // 结构：包围盒 -> McStructure
    if use_struct {
        let mut lx = i32::MAX;
        let mut ly = i32::MAX;
        let mut lz = i32::MAX;
        let mut mx = i32::MIN;
        let mut my = i32::MIN;
        let mut mz = i32::MIN;
        for (x, y, z, _) in blocks {
            lx = lx.min(*x);
            ly = ly.min(*y);
            lz = lz.min(*z);
            mx = mx.max(*x);
            my = my.max(*y);
            mz = mz.max(*z);
        }
        if blocks.is_empty() {
            lx = 0;
            ly = 0;
            lz = 0;
            mx = 0;
            my = 0;
            mz = 0;
        }
        let size = [mx - lx + 1, my - ly + 1, mz - lz + 1];
        let mut structure = McStructure::new(size);
        for (x, y, z, name) in blocks {
            structure.set_block([x - lx, y - ly, z - lz], name);
        }

        let path = dir.join("output.mcstructure");
        let f = std::fs::File::create(&path)
            .map_err(|e| format!("创建 {} 失败: {e}", path.display()))?;
        let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
        structure
            .write_file(&mut w)
            .map_err(|e| format!("写入 mcstructure 失败: {e}"))?;
        w.flush()
            .map_err(|e| format!("写入 mcstructure 失败: {e}"))?;
        return Ok(None);
    }

    // 函数
    let mut writer = open_function(dir, 0)?;
    let mut file_idx = 0usize;
    let mut line_count = 0usize;
    for (x, y, z, name) in blocks {
        write!(
            writer,
            "setblock ~{} ~{} ~{} {}\n",
            x + offset[0],
            y + offset[1],
            z + offset[2],
            name
        )
        .map_err(|e| format!("写入 mcfunction 失败: {e}"))?;
        line_count += 1;
        if auto_slice_mcfunction && line_count >= 10000 {
            writer
                .flush()
                .map_err(|e| format!("写入 mcfunction 失败: {e}"))?;
            file_idx += 1;
            line_count = 0;
            writer = open_function(dir, file_idx)?;
        }
    }
    writer
        .flush()
        .map_err(|e| format!("写入 mcfunction 失败: {e}"))?;
    Ok(None)
}

fn staircase_step(gap: i32, compress: bool) -> i32 {
    if compress {
        gap.min(2)
    } else {
        gap
    }
}

/// 阶梯式无损压缩
fn compress_staircase_column(col: &mut [StairCell], step: i32) {
    let height = col.len();
    let mut i = 0;
    while i < height {
        // 跳过透明，定位一段连续实心区
        while i < height && col[i].y == i32::MIN {
            i += 1;
        }
        if i >= height {
            break;
        }
        let start = i;
        let mut end = i;
        while end < height && col[end].y != i32::MIN {
            end += 1;
        }
        let n = end - start;

        // 约束 k（k=0..n-2）连接 start+k 与 start+k+1，由南侧方块的阴影决定
        // （传播语义：y_z - y_{z+1} = offset(shade_{z+1})）
        let mut low = vec![0i32; n];
        // 正向：南侧下界 = max(0, 北侧下界 + L)
        for k in 0..n - 1 {
            let l = match col[start + k + 1].shade {
                0 => step,
                1 => 0,
                _ => i32::MIN,
            };
            low[k + 1] = if l == i32::MIN {
                0
            } else {
                (low[k] + l).max(0)
            };
        }
        // 反向：北侧下界 = max(北侧下界, 南侧下界 - U)
        for k in (0..n - 1).rev() {
            let u = match col[start + k + 1].shade {
                0 => i32::MAX,
                1 => 0,
                _ => -step,
            };
            if u != i32::MAX {
                let v = low[k + 1] - u;
                if v > low[k] {
                    low[k] = v;
                }
            }
        }
        for k in 0..n {
            col[start + k].y = low[k];
        }
        i = end;
    }
}