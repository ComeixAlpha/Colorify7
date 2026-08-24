# src-tauri/resources — 3D 建筑功能资源

本目录存放 Obj -> 建筑（3D）功能所需的静态资产。

> **合规声明**：本目录文件源自 [ObjToSchematic](https://github.com/LucasDower/ObjToSchematic)
> （BSD-3-Clause，© Lucas Dower 2021），详见仓库根目录 `THIRD_PARTY_NOTICES.md`；
> 许可证全文见 `ObjToSchematic-LICENSE.txt`。

## 文件清单

| 文件                         | 来源                                                 | 用途                                                                                    | 消费方                                                                        |
| ---------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| `atlases/vanilla.atlas`      | ObjToSchematic `res/atlases/vanilla.atlas`（1.20.1） | 原版方块贴图集**元数据**：每方块 6 面贴图名、平均色、标准差(std)、atlas UV 坐标         | Rust 后端（方块匹配 assign，`include_str!` 内嵌）；前端（three.js 渲染查 UV） |
| `atlases/vanilla.png`        | ObjToSchematic `res/atlases/vanilla.png`             | 原版方块**贴图集图片**：960×960，每贴图 3×3 平铺（16×3 px，供 wrap 采样），NEAREST 过滤 | 前端 three.js 渲染                                                            |
| `palettes/all_release.json`  | 由 `palettes/all.ts`（`PALETTE_ALL_RELEASE`）转换    | 全方块调色板：309 个 `minecraft:` 方块名                                                | Rust 后端                                                                     |
| `palettes/all.ts`            | ObjToSchematic `res/palettes/all.ts`                 | 调色板源文件（保留供溯源/再生成）                                                       | 维护参考                                                                      |
| `ObjToSchematic-LICENSE.txt` | ObjToSchematic `LICENSE`                             | BSD-3-Clause 许可证全文                                                                 | 合规                                                                          |

## 结构要点（vanilla.atlas）

- `formatVersion: 3`，`atlasSize: 20`（贴图 20×20 格）
- `blocks[]`：`{ name, faces: { up/down/north/south/east/west: 贴图名 }, colour(平均色 RGBA) }`
- `textures{}`：贴图名 -> `{ atlasColumn, atlasRow, colour, std }`
  - atlas UV（贴图中心）：`u = (3*col + 1) / (atlasSize*3)`，`v = (3*row + 1) / (atlasSize*3)`
- 已校验：调色板 309 个方块在 atlas 中全部存在（0 缺失）

## 再生成

换 MC 版本或精简方块集：用 ObjToSchematic `tools/build-atlas.ts`，
输入原版 jar 的 `textures/block/*.png` + `models/block/*.json`，
输出新的 `atlas.atlas` / `atlas.png`，然后同步更新本目录与 `public/atlases/`。
