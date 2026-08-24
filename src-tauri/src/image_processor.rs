use std::io::Cursor;

use fast_image_resize::{FilterType, IntoImageView, ResizeAlg, ResizeOptions, Resizer};
use image::DynamicImage;
use image::ImageReader;

/// 解码 PNG / JPEG / WebP / GIF
pub fn decode_image(bytes: &[u8]) -> Option<DynamicImage> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
}

/// 编码为 PNG 字节
#[allow(dead_code)]
pub fn encode_png(img: &DynamicImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// 把 RGBA 像素缓冲编码为 PNG
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let buf = image::RgbaImage::from_raw(width, height, rgba.to_vec())?;
    let mut out = Vec::new();
    DynamicImage::ImageRgba8(buf)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

pub fn resize_alg(interpolation: &str) -> Option<ResizeAlg> {
    Some(match interpolation {
        "Nearest" => ResizeAlg::Nearest,
        "Box" => ResizeAlg::Convolution(FilterType::Box),
        "Bilinear" => ResizeAlg::Convolution(FilterType::Bilinear),
        "Hamming" => ResizeAlg::Convolution(FilterType::Hamming),
        "CatmullRom" => ResizeAlg::Convolution(FilterType::CatmullRom),
        "Mitchell" => ResizeAlg::Convolution(FilterType::Mitchell),
        "Gaussian" => ResizeAlg::Convolution(FilterType::Gaussian),
        "Lanczos3" => ResizeAlg::Convolution(FilterType::Lanczos3),
        _ => return None,
    })
}

/// 自动尺寸
pub fn auto_size(
    w: u32,
    h: u32,
    width: Option<u32>,
    height: Option<u32>,
    max_pixels: Option<u64>,
) -> (u32, u32) {
    let wf = w as f64;
    let hf = h as f64;
    match (width, height) {
        (None, None) => {
            if let Some(max) = max_pixels {
                let area = w as u64 * h as u64;
                if area > max {
                    let zoom = (max as f64 / (wf * hf)).sqrt();
                    return (
                        (wf * zoom).floor().max(1.0) as u32,
                        (hf * zoom).floor().max(1.0) as u32,
                    );
                }
            }
            (w, h)
        }
        (Some(w2), None) => (w2, ((hf * w2 as f64 / wf).round()).max(1.0) as u32),
        (None, Some(h2)) => (((wf * h2 as f64 / hf).round()).max(1.0) as u32, h2),
        (Some(w2), Some(h2)) => (w2, h2),
    }
}

fn color_type_from_pixel_type(pt: fast_image_resize::PixelType) -> Option<image::ColorType> {
    use fast_image_resize::PixelType as PT;
    use image::ColorType as CT;
    Some(match pt {
        PT::U8 => CT::L8,
        PT::U8x2 => CT::La8,
        PT::U8x3 => CT::Rgb8,
        PT::U8x4 => CT::Rgba8,
        PT::U16 => CT::L16,
        PT::U16x2 => CT::La16,
        PT::U16x3 => CT::Rgb16,
        PT::U16x4 => CT::Rgba16,
        PT::F32x3 => CT::Rgb32F,
        PT::F32x4 => CT::Rgba32F,
        _ => return None,
    })
}

/// 缩放内存图像
pub fn resize_image(
    src: &DynamicImage,
    width: Option<u32>,
    height: Option<u32>,
    interpolation: &str,
    max_pixels: Option<u64>,
) -> Option<DynamicImage> {
    if width == Some(0) || height == Some(0) {
        return None;
    }

    let (src_w, src_h) = (src.width(), src.height());
    if src_w == 0 || src_h == 0 {
        return None;
    }

    let (dst_w, dst_h) = auto_size(src_w, src_h, width, height, max_pixels);

    if dst_w == src_w && dst_h == src_h {
        return Some(src.clone());
    }

    let alg = resize_alg(interpolation)?;

    let pixel_type = src.pixel_type()?;
    let color_type = color_type_from_pixel_type(pixel_type)?;
    let mut dst = DynamicImage::new(dst_w, dst_h, color_type);

    let mut resizer = Resizer::new();
    resizer
        .resize(src, &mut dst, &ResizeOptions::new().resize_alg(alg))
        .ok()?;

    Some(dst)
}
