use image::DynamicImage;

struct DitherMatrix {
    div: f32,
    offsets: &'static [(isize, isize, f32)],
}

/// Atkinson
const ATKINSON: DitherMatrix = DitherMatrix {
    div: 8.0,
    offsets: &[
        (1, 0, 1.0),
        (2, 0, 1.0),
        (-1, 1, 1.0),
        (0, 1, 1.0),
        (1, 1, 1.0),
        (0, 2, 1.0),
    ],
};

/// Burkes
const BURKES: DitherMatrix = DitherMatrix {
    div: 32.0,
    offsets: &[
        (1, 0, 8.0),
        (2, 0, 4.0),
        (-2, 1, 2.0),
        (-1, 1, 4.0),
        (0, 1, 8.0),
        (1, 1, 4.0),
        (2, 1, 2.0),
    ],
};

/// Floyd-Steinberg
const FLOYD_STEINBERG: DitherMatrix = DitherMatrix {
    div: 16.0,
    offsets: &[(1, 0, 7.0), (-1, 1, 3.0), (0, 1, 5.0), (1, 1, 1.0)],
};

/// Stucki
const STUCKI: DitherMatrix = DitherMatrix {
    div: 42.0,
    offsets: &[
        (1, 0, 8.0),
        (2, 0, 4.0),
        (-2, 1, 2.0),
        (-1, 1, 4.0),
        (0, 1, 8.0),
        (1, 1, 4.0),
        (2, 1, 2.0),
        (-2, 2, 1.0),
        (-1, 2, 2.0),
        (0, 2, 4.0),
        (1, 2, 2.0),
        (2, 2, 1.0),
    ],
};

/// Jarvis-Judice-Ninke
const JARVIS_JUDICE_NINKE: DitherMatrix = DitherMatrix {
    div: 48.0,
    offsets: &[
        (1, 0, 7.0),
        (2, 0, 5.0),
        (-2, 1, 3.0),
        (-1, 1, 5.0),
        (0, 1, 7.0),
        (1, 1, 5.0),
        (2, 1, 3.0),
        (-2, 2, 1.0),
        (-1, 2, 3.0),
        (0, 2, 5.0),
        (1, 2, 3.0),
        (2, 2, 1.0),
    ],
};

/// Sierra3
const SIERRA_3: DitherMatrix = DitherMatrix {
    div: 32.0,
    offsets: &[
        (1, 0, 5.0),
        (2, 0, 3.0),
        (-2, 1, 2.0),
        (-1, 1, 4.0),
        (0, 1, 5.0),
        (1, 1, 4.0),
        (2, 1, 2.0),
        (-1, 2, 2.0),
        (0, 2, 3.0),
        (1, 2, 2.0),
    ],
};

#[derive(Clone, Copy)]
enum DistMode {
    Rgb,
    RgbPlus,
    Redmean,
    Lab(LabMode),
}

#[derive(Clone, Copy)]
enum LabMode {
    Cie76,
    Cie94,
    Ciede2000,
    Cmc,
}

fn dist_mode(name: &str) -> Option<DistMode> {
    Some(match name {
        "RGB" => DistMode::Rgb,
        "RGB+" => DistMode::RgbPlus,
        "Redmean" => DistMode::Redmean,
        "CIE76" => DistMode::Lab(LabMode::Cie76),
        "CIE94" => DistMode::Lab(LabMode::Cie94),
        "CIEDE2000" => DistMode::Lab(LabMode::Ciede2000),
        "CMC l:c" => DistMode::Lab(LabMode::Cmc),
        _ => return None,
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteColorInput {
    pub id: String,
    pub average: [u8; 3],
}

pub fn palette_colors(palette: &[PaletteColorInput]) -> Vec<[u8; 3]> {
    palette.iter().map(|c| c.average).collect()
}

pub fn dither_image(
    img: &mut DynamicImage,
    palette_colors: &[[u8; 3]],
    algorithm: &str,
    color_distance: &str,
) -> Option<()> {
    let Some(mode) = dist_mode(color_distance) else {
        eprintln!("[dither_image] 未知色差公式: {color_distance}");
        return None;
    };

    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let (width, height) = (w as usize, h as usize);

    let matrix: &DitherMatrix = match algorithm {
        "Atkinson" => &ATKINSON,
        "Burkes" => &BURKES,
        "FloydSteinberg" => &FLOYD_STEINBERG,
        "Stucki" => &STUCKI,
        "JarvisJudiceNinke" => &JARVIS_JUDICE_NINKE,
        "Sierra3" => &SIERRA_3,
        _ => {
            eprintln!("[dither_image] 未知抖动算法: {algorithm}");
            return None;
        }
    };

    // 输入像素
    let src: Vec<[f32; 3]> = rgb
        .pixels()
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
        .collect();

    // 调色板
    let palette_rgb: Vec<[f32; 3]> = palette_colors
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    if palette_rgb.is_empty() {
        return None;
    }

    let out: Vec<[f32; 3]> = match mode {
        DistMode::Rgb => diffuse(&src, width, height, matrix, |r, g, b| {
            nearest_rgb(&palette_rgb, r, g, b, d_rgb)
        }),
        DistMode::RgbPlus => diffuse(&src, width, height, matrix, |r, g, b| {
            nearest_rgb(&palette_rgb, r, g, b, d_rgb_plus)
        }),
        DistMode::Redmean => diffuse(&src, width, height, matrix, |r, g, b| {
            nearest_rgb(&palette_rgb, r, g, b, d_redmean)
        }),
        DistMode::Lab(lab_mode) => {
            let palette_lab: Vec<[f32; 3]> = palette_rgb
                .iter()
                .map(|&[r, g, b]| {
                    let (l, a, bb) = srgb_to_lab(r, g, b);
                    [l, a, bb]
                })
                .collect();
            match lab_mode {
                LabMode::Cie76 => diffuse(&src, width, height, matrix, |r, g, b| {
                    nearest_lab(&palette_rgb, &palette_lab, r, g, b, d_cie76)
                }),
                LabMode::Cie94 => diffuse(&src, width, height, matrix, |r, g, b| {
                    nearest_lab(&palette_rgb, &palette_lab, r, g, b, d_cie94)
                }),
                LabMode::Ciede2000 => diffuse(&src, width, height, matrix, |r, g, b| {
                    nearest_lab_topk(&palette_rgb, &palette_lab, r, g, b, d_ciede2000)
                }),
                LabMode::Cmc => diffuse(&src, width, height, matrix, |r, g, b| {
                    nearest_lab_topk(&palette_rgb, &palette_lab, r, g, b, d_cmc)
                }),
            }
        }
    };

    // 写回 RGB8
    let mut buf = Vec::with_capacity(out.len() * 3);
    for [r, g, b] in out {
        buf.push(clamp_u8(r));
        buf.push(clamp_u8(g));
        buf.push(clamp_u8(b));
    }
    let out_img = image::RgbImage::from_raw(w, h, buf)?;
    *img = DynamicImage::ImageRgb8(out_img);
    Some(())
}

/// 行扫描误差扩散
fn diffuse<F>(
    src: &[[f32; 3]],
    width: usize,
    height: usize,
    matrix: &DitherMatrix,
    quant: F,
) -> Vec<[f32; 3]>
where
    F: Fn(f32, f32, f32) -> [f32; 3],
{
    let pw = width + 4;
    let ph = height + 3;
    let plane = ph * pw;

    let mut sr = vec![0.0f32; plane];
    let mut sg = vec![0.0f32; plane];
    let mut sb = vec![0.0f32; plane];
    let mut out = vec![[0.0f32; 3]; src.len()];

    // 预计算线性偏移
    let lin: Vec<(isize, f32)> = matrix
        .offsets
        .iter()
        .map(|&(dx, dy, mul)| (dy * pw as isize + dx, mul / matrix.div))
        .collect();

    for y in 0..height {
        let row = (y + 1) * pw;
        for x in 0..width {
            let i = y * width + x;
            let base = (row + x + 2) as isize;

            let pr = src[i][0] + sr[base as usize];
            let pg = src[i][1] + sg[base as usize];
            let pb = src[i][2] + sb[base as usize];

            let [qr, qg, qb] = quant(pr, pg, pb);
            out[i] = [qr, qg, qb];

            let er = pr - qr;
            let eg = pg - qg;
            let eb = pb - qb;

            for &(off, scale) in &lin {
                let o = (base + off) as usize;
                sr[o] += er * scale;
                sg[o] += eg * scale;
                sb[o] += eb * scale;
            }
        }
    }
    out
}

#[inline(always)]
fn nearest_rgb<D>(palette: &[[f32; 3]], r: f32, g: f32, b: f32, dist: D) -> [f32; 3]
where
    D: Fn(f32, f32, f32, f32, f32, f32) -> f32,
{
    palette[nearest_rgb_idx(palette, r, g, b, dist)]
}

#[inline(always)]
fn nearest_rgb_idx<D>(palette: &[[f32; 3]], r: f32, g: f32, b: f32, dist: D) -> usize
where
    D: Fn(f32, f32, f32, f32, f32, f32) -> f32,
{
    let mut best = 0usize;
    let mut best_d = f32::INFINITY;
    for (i, &[pr, pg, pb]) in palette.iter().enumerate() {
        let d = dist(r, g, b, pr, pg, pb);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

#[inline(always)]
fn nearest_lab<D>(
    palette_rgb: &[[f32; 3]],
    palette_lab: &[[f32; 3]],
    r: f32,
    g: f32,
    b: f32,
    dist: D,
) -> [f32; 3]
where
    D: Fn(f32, f32, f32, f32, f32, f32) -> f32,
{
    palette_rgb[nearest_lab_idx(palette_lab, r, g, b, dist)]
}

#[inline(always)]
fn nearest_lab_idx<D>(palette_lab: &[[f32; 3]], r: f32, g: f32, b: f32, dist: D) -> usize
where
    D: Fn(f32, f32, f32, f32, f32, f32) -> f32,
{
    let (l, a, bb) = srgb_to_lab(r, g, b);
    let mut best = 0usize;
    let mut best_d = f32::INFINITY;
    for (i, &[pl, pa, pbb]) in palette_lab.iter().enumerate() {
        let d = dist(l, a, bb, pl, pa, pbb);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// CIEDE2000/CMC 最近色查找
#[inline(always)]
fn nearest_lab_topk<D>(
    palette_rgb: &[[f32; 3]],
    palette_lab: &[[f32; 3]],
    r: f32,
    g: f32,
    b: f32,
    refine: D,
) -> [f32; 3]
where
    D: Fn(f32, f32, f32, f32, f32, f32) -> f32,
{
    palette_rgb[nearest_lab_topk_idx(palette_lab, r, g, b, refine)]
}

#[inline(always)]
fn nearest_lab_topk_idx<D>(palette_lab: &[[f32; 3]], r: f32, g: f32, b: f32, refine: D) -> usize
where
    D: Fn(f32, f32, f32, f32, f32, f32) -> f32,
{
    const K: usize = 8;
    let (l, a, bb) = srgb_to_lab(r, g, b);

    // 粗筛：CIE76 平方距离的 top-K
    let mut best_idx = [0usize; K];
    let mut best_d = [f32::INFINITY; K];
    for (i, &[pl, pa, pbb]) in palette_lab.iter().enumerate() {
        let dl = l - pl;
        let da = a - pa;
        let db = bb - pbb;
        let d = dl * dl + da * da + db * db;
        for k in 0..K {
            if d < best_d[k] {
                for j in (k..K - 1).rev() {
                    best_d[j + 1] = best_d[j];
                    best_idx[j + 1] = best_idx[j];
                }
                best_d[k] = d;
                best_idx[k] = i;
                break;
            }
        }
    }

    // 精算：只对 top-K 候选做精确色差
    let mut best = best_idx[0];
    let mut best_d = f32::INFINITY;
    for &idx in &best_idx {
        let [pl, pa, pbb] = palette_lab[idx];
        let d = refine(l, a, bb, pl, pa, pbb);
        if d < best_d {
            best_d = d;
            best = idx;
        }
    }
    best
}

// ---------- 色差距离 ----------

#[inline(always)]
fn d_rgb(r: f32, g: f32, b: f32, pr: f32, pg: f32, pb: f32) -> f32 {
    let dr = r - pr;
    let dg = g - pg;
    let db = b - pb;
    dr * dr + dg * dg + db * db
}

#[inline(always)]
fn d_rgb_plus(r: f32, g: f32, b: f32, pr: f32, pg: f32, pb: f32) -> f32 {
    let dr = r - pr;
    let dg = g - pg;
    let db = b - pb;
    2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db
}

#[inline(always)]
fn d_redmean(r: f32, g: f32, b: f32, pr: f32, pg: f32, pb: f32) -> f32 {
    let dr = r - pr;
    let dg = g - pg;
    let db = b - pb;
    let rm = 0.5 * (r + pr);
    (2.0 + rm / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - rm) / 256.0) * db * db
}

#[inline(always)]
fn d_cie76(l1: f32, a1: f32, b1: f32, l2: f32, a2: f32, b2: f32) -> f32 {
    let dl = l1 - l2;
    let da = a1 - a2;
    let db = b1 - b2;
    dl * dl + da * da + db * db
}

/// CIE94（KL=1, K1=0.045, K2=0.015）
#[inline(always)]
fn d_cie94(l1: f32, a1: f32, b1: f32, l2: f32, a2: f32, b2: f32) -> f32 {
    let dl = l1 - l2;
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let dc = c1 - c2;
    let da = a1 - a2;
    let db = b1 - b2;
    let dh2 = (da * da + db * db - dc * dc).max(0.0);

    let sc = 1.0 + 0.045 * c1;
    let sh = 1.0 + 0.015 * c1;
    (dl * dl + (dc / sc) * (dc / sc) + dh2 / (sh * sh)).sqrt()
}

/// CIEDE2000 色差
#[inline(always)]
fn d_ciede2000(l1: f32, a1: f32, b1: f32, l2: f32, a2: f32, b2: f32) -> f32 {
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let c_bar = 0.5 * (c1 + c2);
    let c_bar7 = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar7 / (c_bar7 + 25.0f32.powi(7))).sqrt());

    let a1p = (1.0 + g) * a1;
    let a2p = (1.0 + g) * a2;
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();

    let h1p = (b1.atan2(a1p).to_degrees() + 360.0) % 360.0;
    let h2p = (b2.atan2(a2p).to_degrees() + 360.0) % 360.0;

    let dl = l2 - l1;
    let dc = c2p - c1p;

    let dh = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhp = 2.0 * (c1p * c2p).sqrt() * (0.5 * dh.to_radians()).sin();

    let l_bar = 0.5 * (l1 + l2);
    let c_bar_p = 0.5 * (c1p + c2p);
    let h_bar = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        0.5 * (h1p + h2p)
    } else if h1p + h2p < 360.0 {
        0.5 * (h1p + h2p + 360.0)
    } else {
        0.5 * (h1p + h2p - 360.0)
    };

    let t = 1.0 - 0.17 * (h_bar - 30.0).to_radians().cos()
        + 0.24 * (2.0 * h_bar).to_radians().cos()
        + 0.32 * (3.0 * h_bar + 6.0).to_radians().cos()
        - 0.20 * (4.0 * h_bar - 63.0).to_radians().cos();

    let d_theta = 30.0 * (-((h_bar - 275.0) / 25.0).powi(2)).exp();
    let c_bar_p7 = c_bar_p.powi(7);
    let rc = 2.0 * (c_bar_p7 / (c_bar_p7 + 25.0f32.powi(7))).sqrt();

    let l_bar_50 = (l_bar - 50.0).powi(2);
    let sl = 1.0 + 0.015 * l_bar_50 / (20.0 + l_bar_50).sqrt();
    let sc = 1.0 + 0.045 * c_bar_p;
    let sh = 1.0 + 0.015 * c_bar_p * t;
    let rt = -(2.0 * d_theta.to_radians()).sin() * rc;

    let dl_sl = dl / sl;
    let dc_sc = dc / sc;
    let dhp_sh = dhp / sh;
    (dl_sl * dl_sl + dc_sc * dc_sc + dhp_sh * dhp_sh + rt * dc_sc * dhp_sh).sqrt()
}

/// CMC l:c 色差（l=2, c=1）
#[inline(always)]
fn d_cmc(l1: f32, a1: f32, b1: f32, l2: f32, a2: f32, b2: f32) -> f32 {
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let dl = l1 - l2;
    let dc = c1 - c2;
    let da = a1 - a2;
    let db = b1 - b2;
    let dh2 = (da * da + db * db - dc * dc).max(0.0);

    let h1 = (b1.atan2(a1).to_degrees() + 360.0) % 360.0;
    let f = (c1.powi(4) / (c1.powi(4) + 1900.0)).sqrt();
    let t = if (164.0..=345.0).contains(&h1) {
        0.56 + (0.2 * (h1 + 168.0).to_radians().cos()).abs()
    } else {
        0.36 + (0.4 * (h1 + 35.0).to_radians().cos()).abs()
    };
    let sl = if l1 < 16.0 {
        0.511
    } else {
        0.040975 * l1 / (1.0 + 0.01765 * l1)
    };
    let sc = 0.0638 * c1 / (1.0 + 0.0131 * c1) + 0.638;
    let sh = sc * (f * t + 1.0 - f);

    let dl_sl = dl / (2.0 * sl);
    let dc_sc = dc / sc;
    let dh_sh = (dh2 / (sh * sh)).sqrt();
    (dl_sl * dl_sl + dc_sc * dc_sc + dh_sh * dh_sh).sqrt()
}

/// sRGB (0-255) → CIE Lab（D65 白点）
#[inline(always)]
pub(crate) fn srgb_to_lab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let linear = |c: f32| {
        let c = c / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (rl, gl, bl) = (linear(r), linear(g), linear(b));

    // sRGB → XYZ (D65)
    let x = rl * 0.4124564 + gl * 0.3575761 + bl * 0.1804375;
    let y = rl * 0.2126729 + gl * 0.7151522 + bl * 0.0721750;
    let z = rl * 0.0193339 + gl * 0.1191920 + bl * 0.9503041;

    let f = |t: f32| {
        let d = 6.0 / 29.0;
        if t > d * d * d {
            t.cbrt()
        } else {
            t / (3.0 * d * d) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.95047), f(y / 1.0), f(z / 1.08883));

    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let bb = 200.0 * (fy - fz);
    (l, a, bb)
}

#[inline(always)]
fn clamp_u8(v: f32) -> u8 {
    if v >= 255.0 {
        255
    } else if v <= 0.0 {
        0
    } else {
        v.round() as u8
    }
}

pub(crate) fn is_lab_formula(formula: &str) -> bool {
    matches!(formula, "CIE76" | "CIE94" | "CIEDE2000" | "CMC l:c")
}

pub struct PaletteIndexer {
    rgb: Vec<[f32; 3]>,
    lab: Option<Vec<[f32; 3]>>,
    mode: DistMode,
}

impl PaletteIndexer {
    pub fn new(palette: &[[u8; 3]], formula: &str) -> Option<Self> {
        let mode = dist_mode(formula)?;
        let rgb: Vec<[f32; 3]> = palette
            .iter()
            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
            .collect();
        if rgb.is_empty() {
            return None;
        }
        let lab = if is_lab_formula(formula) {
            Some(
                rgb.iter()
                    .map(|&[r, g, b]| {
                        let (l, a, bb) = srgb_to_lab(r, g, b);
                        [l, a, bb]
                    })
                    .collect(),
            )
        } else {
            None
        };
        Some(PaletteIndexer { rgb, lab, mode })
    }

    /// 返回索引
    #[inline]
    pub fn nearest(&self, r: u8, g: u8, b: u8) -> usize {
        let (r, g, b) = (r as f32, g as f32, b as f32);
        let lab = self.lab.as_ref();
        match self.mode {
            DistMode::Rgb => nearest_rgb_idx(&self.rgb, r, g, b, d_rgb),
            DistMode::RgbPlus => nearest_rgb_idx(&self.rgb, r, g, b, d_rgb_plus),
            DistMode::Redmean => nearest_rgb_idx(&self.rgb, r, g, b, d_redmean),
            DistMode::Lab(LabMode::Cie76) => nearest_lab_idx(lab.unwrap(), r, g, b, d_cie76),
            DistMode::Lab(LabMode::Cie94) => nearest_lab_idx(lab.unwrap(), r, g, b, d_cie94),
            DistMode::Lab(LabMode::Ciede2000) => {
                nearest_lab_topk_idx(lab.unwrap(), r, g, b, d_ciede2000)
            }
            DistMode::Lab(LabMode::Cmc) => nearest_lab_topk_idx(lab.unwrap(), r, g, b, d_cmc),
        }
    }
}
