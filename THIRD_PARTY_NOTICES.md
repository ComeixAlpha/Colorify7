# 第三方声明 / Third-Party Notices

本应用的部分资源与算法移植自以下开源项目，特此声明。

## ObjToSchematic（BSD-3-Clause）

本项目集成了 [ObjToSchematic](https://github.com/LucasDower/ObjToSchematic)
（作者：Lucas Dower）的 OBJ -> Minecraft 建筑转换流程。

包含/移植的内容：

- 体素化（Voxelise）与方块匹配（Assign）流程的设计与算法思路
- `src-tauri/resources/atlases/vanilla.atlas` — 原版方块纹理贴图集元数据
- `src-tauri/resources/atlases/vanilla.png` — 原版方块纹理贴图集图片
- `src-tauri/resources/palettes/all_release.json` — 全方块调色板
  （源自其 `res/palettes/all.ts` 的 `PALETTE_ALL_RELEASE`）

许可证全文见 `src-tauri/resources/ObjToSchematic-LICENSE.txt`（BSD 3-Clause License）：

```
BSD 3-Clause License
Copyright (c) 2021, Lucas Dower
All rights reserved.
...（完整文本见 LICENSE 文件）
```

### 合规说明

- ObjToSchematic 以 BSD-3-Clause 授权，本应用以二进制/源码形式再分发时保留了上述版权声明与免责条款。
- `vanilla.atlas` / `vanilla.png` 内含《Minecraft》原版方块纹理，
  这些纹理为 Mojang Studios 资产，依据
  [Mojang Brand and Asset Usage Guidelines](https://account.mojang.com/terms#brand)
  用于本非商业、非官方的转换工具。本项目与 Mojang Studios 无任何关联或背书关系。
