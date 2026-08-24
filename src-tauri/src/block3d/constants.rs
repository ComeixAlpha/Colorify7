//! Assign 相关方块集合

/// 重力方块
pub const FALLABLE_BLOCKS: &[&str] = &[
    "gravel",
    "lime_concrete_powder",
    "orange_concrete_powder",
    "black_concrete_powder",
    "brown_concrete_powder",
    "cyan_concrete_powder",
    "light_gray_concrete_powder",
    "purple_concrete_powder",
    "magenta_concrete_powder",
    "light_blue_concrete_powder",
    "yellow_concrete_powder",
    "white_concrete_powder",
    "blue_concrete_powder",
    "red_concrete_powder",
    "gray_concrete_powder",
    "pink_concrete_powder",
    "green_concrete_powder",
];

/// 透明方块：不遮挡视线，草皮侧面替换判断用（像素画调色板中存在的半透明方块）
pub const TRANSPARENT_BLOCKS: &[&str] = &["powder_snow"];

/// 从体素匹配候选中排除的方块（不参与 Assign 颜色匹配）
pub const EXCLUDED_BLOCKS: &[&str] = &["cauldron"];

/// 草类方块：顶部被不透明方块覆盖时，侧面换成非草类方块
pub const GRASS_LIKE_BLOCKS: &[&str] = &[];
