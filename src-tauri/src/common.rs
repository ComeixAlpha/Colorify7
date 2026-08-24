//! 工具函数

pub fn fmt_f64(v: f64) -> String {
    if v == 0.0 {
        return "0.0".into();
    }
    if v.fract() == 0.0 && v.is_finite() {
        return format!("{:.1}", v);
    }
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.contains('.') {
        s.to_string()
    } else {
        format!("{s}.0")
    }
}

pub fn plane_index(plane: &str) -> i32 {
    match plane {
        "xOz" => 1,
        "yOz" => 2,
        _ => 0,
    }
}

#[inline]
pub fn switch_xyz<T: Copy>(plane: i32, xyz: [T; 3]) -> [T; 3] {
    match plane {
        0 => [xyz[0], xyz[2], xyz[1]],
        1 => xyz,
        _ => [xyz[1], xyz[0], xyz[2]],
    }
}
