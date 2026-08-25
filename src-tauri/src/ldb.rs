//! Bedrock LevelDB 世界直读验证（feat/ldb-rw 调研，路线 A）
//!
//! 最小验证：① 自动发现世界 ② 打开世界（只读） ③ 读取指定坐标方块状态。

use std::collections::BTreeMap;
use std::path::Path;

use bedrock_world::discover::{discover_worlds, WorldDiscovery};
use bedrock_world::mcstructure::{
    McStructureFile, McStructurePaletteEntry, McStructurePlacement, McStructureRotation,
    McStructureSize,
};
use bedrock_world::{
    block_storage_index, BedrockWorld, BlockPos, BlockState, ChunkKey, ChunkPos, ChunkRecordTag,
    Dimension, OpenOptions, SubChunkDecodeMode, SubChunkFormat, WriteGuard,
};
use serde::Serialize;

/// 世界摘要（前端列表用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub folder_name: String,
    pub folder_path: String,
    pub level_name: Option<String>,
}

/// 方块状态视图（序列化给前端）
#[derive(Debug, Clone, Serialize)]
pub struct BlockStateView {
    pub name: String,
    pub states: BTreeMap<String, String>,
    pub version: Option<i32>,
}

impl From<BlockState> for BlockStateView {
    fn from(state: BlockState) -> Self {
        Self {
            name: state.name,
            states: state
                .states
                .into_iter()
                .map(|(k, v)| (k, format!("{v:?}")))
                .collect(),
            version: state.version,
        }
    }
}

/// 常见 Bedrock 世界根目录
fn world_roots() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();

    #[cfg(desktop)]
    {
        // 非 UWP 安装（新版启动器/Store 之外的发行版）：
        // %APPDATA%\Minecraft Bedrock\Users\<uid>\games\com.mojang\minecraftWorlds
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let users_dir = std::path::Path::new(&appdata).join("Minecraft Bedrock/Users");
            if let Ok(entries) = std::fs::read_dir(&users_dir) {
                for entry in entries.flatten() {
                    let p = entry.path().join("games/com.mojang/minecraftWorlds");
                    if p.is_dir() {
                        roots.push(p);
                    }
                }
            }
        }

        // UWP 安装
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let p = std::path::Path::new(&local).join(
                "Packages/Microsoft.MinecraftUWP_8wekyb3d8bbwe/LocalState/games/com.mojang/minecraftWorlds",
            );
            if p.is_dir() {
                roots.push(p);
            }
        }
    }

    #[cfg(mobile)]
    {
        let p = std::path::PathBuf::from(
            "/storage/emulated/0/Android/data/com.mojang.minecraftpe/files/games/com.mojang/minecraftWorlds",
        );
        if p.is_dir() {
            roots.push(p);
        }
    }

    roots
}

/// 发现本机 Bedrock 世界（返回摘要列表）
#[tauri::command]
pub fn ldb_discover_worlds() -> Result<Vec<WorldInfo>, String> {
    // 安卓：Android/data 目录受系统保护，无「所有文件访问」权限时无法读取，
    // 提前探测并把明确的权限提示返回给前端（避免静默显示"未发现世界"）。
    #[cfg(mobile)]
    {
        let worlds_dir = std::path::Path::new(
            "/storage/emulated/0/Android/data/com.mojang.minecraftpe/files/games/com.mojang/minecraftWorlds",
        );
        match std::fs::read_dir(worlds_dir) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(
                    "无法读取 Minecraft 世界目录（没有访问权限）。请在系统设置中授予 \
                     Colorify「所有文件访问」权限（设置 -> 应用 -> Colorify -> 所有文件访问），\
                     并确认系统允许访问 Android/data 目录"
                        .into(),
                )
            }
            Err(_) => {
                // 路径不存在 → 未安装或没有世界存档
                return Ok(Vec::new());
            }
        }
    }

    let roots = world_roots();
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let mut worlds = Vec::new();
    for root in roots {
        let discovered = discover_worlds(&WorldDiscovery::new(vec![root]))
            .map_err(|e| format!("发现世界失败: {e}"))?;
        for w in discovered {
            worlds.push(WorldInfo {
                folder_name: w.folder_name,
                folder_path: w.folder_path.to_string_lossy().into_owned(),
                level_name: w.level_name,
            });
        }
    }
    Ok(worlds)
}

/// 世界目录根（安卓：共享 Download/colorify；桌面：Documents/colorify）
fn worlds_root_dir() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    {
        std::path::PathBuf::from("/storage/emulated/0/Download/colorify")
    }
    #[cfg(not(target_os = "android"))]
    {
        std::env::var("USERPROFILE")
            .map(|p| std::path::PathBuf::from(p).join("Documents/colorify"))
            .unwrap_or_default()
    }
}

/// 读取世界目录的世界名
fn read_world_dir_level_name(path: &Path) -> Option<String> {
    if let Ok(s) = std::fs::read_to_string(path.join("levelname.txt")) {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

/// 列出世界目录根下的世界目录（含 db/ 子目录才算世界）
/// 安卓无法直接读 Android/data 下 Minecraft 的世界存档（系统限制）
/// 由用户把世界目录（含 db 文件夹）放到该目录下
#[tauri::command]
pub fn ldb_list_world_dirs() -> Result<Vec<WorldInfo>, String> {
    let dir = worlds_root_dir();
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("无法读取 {} 目录: {e}", dir.display()))?;
    let mut worlds = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join("db").is_dir() {
            continue;
        }
        let folder_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let level_name = read_world_dir_level_name(&path).unwrap_or_else(|| folder_name.clone());
        worlds.push(WorldInfo {
            folder_name,
            folder_path: path.to_string_lossy().into_owned(),
            level_name: Some(level_name),
        });
    }
    worlds.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));
    Ok(worlds)
}

/// 打开系统设置页，引导用户授予 MANAGE_EXTERNAL_STORAGE
#[tauri::command]
pub fn open_all_files_settings(
    state: tauri::State<'_, crate::SettingsPluginHandle>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        state
            .0
            .run_mobile_plugin::<()>("openAllFilesAccess", ())
            .map_err(|e| format!("打开「所有文件访问」设置失败: {e}"))?;
    }
    #[cfg(not(target_os = "android"))]
    let _ = state;
    Ok(())
}

/// 检测安卓端文件访问权限 （经 Kotlin）
#[tauri::command]
pub fn check_all_files_access(
    state: tauri::State<'_, crate::SettingsPluginHandle>,
) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let resp: serde_json::Value = state
            .0
            .run_mobile_plugin("checkAllFilesAccess", ())
            .map_err(|e| format!("检测权限失败: {e}"))?;
        let granted = resp
            .as_bool()
            .or_else(|| resp.get("value").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        Ok(granted)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = state;
        Ok(true)
    }
}

/// 打开世界（只读）并读取指定坐标的方块状态
#[tauri::command]
pub fn ldb_read_block(
    world_path: String,
    dimension: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<Option<BlockStateView>, String> {
    let world = bedrock_world::BedrockWorld::open_typed_blocking(
        &world_path,
        OpenOptions {
            read_only: true,
            ..OpenOptions::default()
        },
    )
    .map_err(|e| format!("打开世界失败: {e}"))?;

    let state = world
        .get_block_state_at_blocking(Dimension::from_id(dimension), BlockPos { x, y, z })
        .map_err(|e| format!("读取方块失败: {e}"))?;

    Ok(state.map(BlockStateView::from))
}

/// DB 锁
fn world_is_locked(world_path: &Path) -> bool {
    let lock = world_path.join("db").join("LOCK");
    if !lock.exists() {
        return false;
    }
    std::fs::OpenOptions::new().write(true).open(&lock).is_err()
}

/// 默认空气方块状态
fn air_state() -> BlockState {
    BlockState {
        name: "minecraft:air".to_string(),
        states: BTreeMap::new(),
        version: Some(18_002_711),
    }
}

/// 写方块到世界
pub(crate) fn write_blocks_to_world(
    world_path: &str,
    dimension: i32,
    origin: [i32; 3],
    // 相对坐标
    blocks: &[(i32, i32, i32, String)],
    progress: &crate::process::ProgressSink,
) -> Result<(), String> {
    if blocks.is_empty() {
        return Err("没有生成任何方块".into());
    }

    // 目标必须是含 db/ 的世界目录
    if !Path::new(world_path).join("db").is_dir() {
        return Err(format!(
            "{world_path} 不是世界目录（缺少 db 文件夹），请选择含 db 目录的世界文件夹"
        ));
    }

    let t0 = std::time::Instant::now();
    let mut last = t0;
    macro_rules! timed {
        ($label:expr) => {{
            let now = std::time::Instant::now();
            eprintln!("[ldb] {:<12} {:>8?}", $label, now.duration_since(last));
            last = now;
        }};
    }

    // DB 锁
    progress.stage("检查世界状态");
    if world_is_locked(Path::new(world_path)) {
        return Err("检测到该世界正在游戏中打开，请先完全退出游戏（不是暂停）再重试".into());
    }
    timed!("锁检测");

    // 打开世界
    progress.stage("打开世界");
    let world = BedrockWorld::open_typed_blocking(
        world_path,
        OpenOptions {
            read_only: false,
            ..OpenOptions::default()
        },
    )
    .map_err(|e| format!("无法打开世界数据库（请确认游戏已关闭该世界）: {e}"))?;
    timed!("打开世界");

    // 世界维度
    let dim = Dimension::from_id(dimension);

    // 逐 subchunk 合并写入
    progress.stage("写入世界");
    let mut groups: std::collections::BTreeMap<(i32, i32, i8), Vec<(u8, u8, u8, String)>> =
        std::collections::BTreeMap::new();
    for (bx, by, bz, id) in blocks {
        let (wx, wy, wz) = (origin[0] + bx, origin[1] + by, origin[2] + bz);
        let (cx, cz) = (wx.div_euclid(16), wz.div_euclid(16));
        let sy = i8::try_from(wy.div_euclid(16)).map_err(|_| format!("Y 越界: {wy}"))?;
        groups.entry((cx, cz, sy)).or_default().push((
            (wx.rem_euclid(16)) as u8,
            (wy.rem_euclid(16)) as u8,
            (wz.rem_euclid(16)) as u8,
            id.clone(),
        ));
    }
    let total = groups.len();
    let mut affected: std::collections::BTreeSet<ChunkPos> = std::collections::BTreeSet::new();
    let mut done = 0usize;
    for ((cx, cz, sy), placements) in groups {
        let chunk = ChunkPos {
            x: cx,
            z: cz,
            dimension: dim,
        };
        affected.insert(chunk);

        let mut states: Vec<BlockState> = vec![air_state(); 4096];
        let sub = world
            .get_subchunk_layer_blocking(chunk, i32::from(sy) * 16, SubChunkDecodeMode::FullIndices)
            .map_err(|e| format!("读取子区块失败: {e}"))?;
        if let Some(sub) = sub {
            match &sub.format {
                SubChunkFormat::Paletted { storages, .. } => {
                    if let Some(s) = storages.first() {
                        if let Some(indices) = &s.indices {
                            for i in 0..4096 {
                                states[i] = s.states[indices[i] as usize].clone();
                            }
                        }
                    }
                }
                _ => {
                    return Err(format!(
                        "区块 ({cx},{cz}) 子区块 {sy} 无法解码，已中止以避免损坏"
                    ))
                }
            }
        }

        // 覆盖冲突
        for (lx, ly, lz, id) in placements {
            let si = block_storage_index(lx, ly, lz);
            states[si] = BlockState {
                name: id,
                states: BTreeMap::new(),
                version: Some(18_002_711),
            };
        }

        // 构造 16^3 结构
        let size = McStructureSize::new(16, 16, 16).map_err(|e| format!("结构尺寸错误: {e}"))?;
        let mut structure = McStructureFile::new_air(size, [cx * 16, i32::from(sy) * 16, cz * 16])
            .map_err(|e| format!("创建结构失败: {e}"))?;
        structure.palette.clear();
        structure.palette.push(McStructurePaletteEntry::air()); // 0 = air
        let mut pmap: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for i in 0..4096 {
            let (x, z, y) = (i / 256, (i / 16) % 16, i % 16);
            let st = &states[i];
            let key = format!("{}|{:?}|{:?}", st.name, st.states, st.version);
            let idx = if let Some(&j) = pmap.get(&key) {
                j
            } else {
                let j = structure.palette.len();
                structure.palette.push(McStructurePaletteEntry {
                    name: st.name.clone(),
                    states: st.states.clone(),
                    version: st.version,
                });
                pmap.insert(key, j);
                j
            };
            structure.primary_indices[x * 256 + y * 16 + z] = idx as i32;
        }

        let guard = WriteGuard::confirmed(world_path, "colorify-ldb-write");
        structure
            .write_to_world_blocking(
                &world,
                McStructurePlacement {
                    source_anchor: chunk,
                    target_anchor: chunk,
                    origin_y: i32::from(sy) * 16,
                    rotation: McStructureRotation::None,
                    mirror_x: false,
                    mirror_z: false,
                },
                &guard,
                |_| {},
            )
            .map_err(|e| format!("写入子区块失败: {e}"))?;
        done += 1;
        progress.stage(&format!("写入世界 {done}/{total}"));
    }
    timed!("写入世界");

    // 补 Version
    let version_keys: Vec<_> = affected
        .iter()
        .map(|c| ChunkKey::new(*c, ChunkRecordTag::Version).encode())
        .collect();
    let version_values = world
        .storage()
        .get_many(&version_keys)
        .map_err(|e| format!("查询 Version 记录失败: {e}"))?;
    for (chunk, value) in affected.iter().zip(version_values.iter()) {
        if value.is_none() {
            world
                .put_raw_record_blocking(&ChunkKey::new(*chunk, ChunkRecordTag::Version), &[42])
                .map_err(|e| format!("补 Version 记录失败: {e}"))?;
        }
    }
    timed!("补 Version");

    eprintln!(
        "[ldb] 写入 {} 方块到 {}，影响 {} 个区块，总计 {:?}",
        blocks.len(),
        world_path,
        affected.len(),
        t0.elapsed()
    );
    let _ = last; // 最后一个 timed! 之后 last 不再被读，消除 unused_assignments warning
    Ok(())
}
