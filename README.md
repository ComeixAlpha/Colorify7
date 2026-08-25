# Colorify 7

**English** | [简体中文](README.zh-CN.md)

Colorify 7 is an open-source tool for Minecraft: Bedrock Edition that converts images and three-dimensional models into in-game content. It supports particle art, pixel art, and object voxelisation, and can export the results as `mcfunction`, `mcstructure`, or `mcaddon` files, stream them directly into a running game session through a local WebSocket server, or write them straight into a world save (LevelDB) on disk.

The application is built with [Tauri 2](https://tauri.app/) and [React](https://react.dev/), and is available on Windows and Android.

## Table of Contents

- [Colorify 7](#colorify-7)
  - [Table of Contents](#table-of-contents)
  - [Features](#features)
    - [Particle Art](#particle-art)
    - [Pixel Art](#pixel-art)
    - [Obj to Voxel](#obj-to-voxel)
    - [WebSocket Streaming](#websocket-streaming)
    - [Direct LevelDB Write](#direct-leveldb-write)
  - [Platform Support](#platform-support)
  - [Getting Started](#getting-started)
    - [Prerequisites](#prerequisites)
    - [Run from Source](#run-from-source)
    - [Build a Release](#build-a-release)
  - [Usage](#usage)
  - [Gallery](#gallery)
  - [Acknowledgements](#acknowledgements)
  - [License](#license)

## Features

### Particle Art

Converts an image into an in-game particle display. Colours are either matched against a configurable set of particle mappings or emitted as per-pixel coloured particles (the latter requires Minecraft 1.20.80 or later and is bundled as a resource pack automatically). The result can be packaged as an `.mcaddon` file or streamed to the game over WebSocket.

### Pixel Art

Flattens an image into a block layout on a single plane, or as stepped staircase art. Supports configurable colour-distance metrics, ordered dithering, palette restrictions (carpet-only, wool-only), and exclusions for glass, sand, and powder blocks. Exports as `mcfunction` or `.mcstructure` files.

### Obj to Voxel

Voxelises `OBJ`, `GLB`, and `GLTF` models into solid or hollow Minecraft builds. The pipeline relies on a custom BVH-accelerated ray-casting voxeliser with configurable resolution, rotation, dithering, and multisample colouring. Exports as `.mcstructure` files.

### WebSocket Streaming

Runs a local WebSocket server that pushes generated block commands to a connected Bedrock client in real time, allowing the build to be observed as it is placed. The server reports task progress and forwards game messages back into the application.

### Direct LevelDB Write

Writes generated blocks straight into a Bedrock world save's `db/` LevelDB directory, bypassing file import or WebSocket entirely. Existing terrain is preserved — only the target coordinates are overwritten. Works on Windows (any world folder) and Android (world folders placed under `Download/colorify`; requires the "All files access" permission).

## Platform Support

| Platform | Status    |
| -------- | --------- |
| Windows  | Supported |
| Android  | Supported |

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) 20 or later
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/) (stable toolchain)
- Platform prerequisites for [Tauri 2](https://tauri.app/start/prerequisites/) — on Windows, the Microsoft Visual C++ Build Tools and the WebView2 runtime

### Run from Source

```bash
pnpm install
pnpm tauri dev
```

### Build a Release

```bash
pnpm build        # compile the frontend (TypeScript + Vite)
pnpm tauri build  # produce the platform bundle
```

## Usage

1. Choose a mode: Particle Art, Pixel Art, Obj to Voxel, or WebSocket.
2. Load an image or a 3D model.
3. Adjust the generation parameters (resolution, rotation, palette, dithering, and so on).
4. Export the result as files, or send it to the game through the WebSocket server.

To stream into the game:

- Enable "Enable WebSocket" and disable "WebSocket Requires Encryption" in the Minecraft settings.
- On Windows, enable the UWP loopback exemption first so that the game can reach a local server.
- Connect with `/connect 127.0.0.1:<port>`.

## Gallery

<table>
    <tr>
        <td colspan=2><a href="https://skfb.ly/pK9UV">"Hornet - Hollow Knight - Handpainted" by TonXx</a></td>
    </tr>
    <tr>
        <td><img src="./gallery/hornet.png" alt="示例图片" style="height:400px; width:auto;"></td>
        <td><img src="./gallery/hornet_voxel.png" alt="示例图片" style="height:400px; width:auto;"></td>
    </tr>
    <tr>
        <td colspan=2><a href="https://skfb.ly/pK9UV">"Broken Steampunk Clock" by VassKacsoHunor</a></td>
    </tr>
    <tr>
        <td><img src="./gallery/clock.png" alt="示例图片" style="height:400px; width:auto;"></td>
        <td><img src="./gallery/clock_voxel.png" alt="示例图片" style="height:400px; width:auto;"></td>
    </tr>
    <tr>
        <td colspan=2><a href="https://skfb.ly/pK9UV">"Fast-food" by Korneev Nikita Kirillovich</a></td>
    </tr>
    <tr>
        <td><img src="./gallery/fast_food.png" alt="示例图片" style="height:400px; width:auto;"></td>
        <td><img src="./gallery/fast_food_voxel.png" alt="示例图片" style="height:400px; width:auto;"></td>
    </tr>
    <tr>
        <td colspan=2><a href="https://skfb.ly/pK9UV">"Frank" by misterdevious</a></td>
    </tr>
    <tr>
        <td><img src="./gallery/frank.png" alt="示例图片" style="height:400px; width:auto;"></td>
        <td><img src="./gallery/frank_voxel.png" alt="示例图片" style="height:400px; width:auto;"></td>
    </tr>
</table>

## Acknowledgements

- [SlopeCraft](https://github.com/SlopeCraft/SlopeCraft) — map pixel art generator for Minecraft; source of inspiration for the pixel-art pipeline.
- [ObjToSchematic](https://github.com/LucasDower/ObjToSchematic) — voxelisation algorithm and block assets (BSD-3-Clause, (c) Lucas Dower 2021).
- Minecraft block textures (c) Mojang Studios, used in accordance with the Minecraft Brand Guidelines.
- All testers and contributors listed in the application's About page.

Colorify 7 is an unofficial fan project and is not affiliated with or endorsed by Mojang Studios or Microsoft.

## License

This project is licensed under a custom permissive license for **non-commercial use only**. The full text is provided in the [LICENSE](LICENSE) file.

In short: you may freely use, copy, modify, and distribute the software for any non-commercial purpose. **Commercial use is not permitted.**

Third-party components included in this repository remain under their respective licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
