# 中国象棋学习助手

## 🚀 快速上手

### 方式二：从源码运行/构建（开发者）

环境要求：Node.js、pnpm、Rust 工具链、WebView2 Runtime（Windows 11 自带，Windows 10 需 [手动安装](https://developer.microsoft.com/microsoft-edge/webview2/)）。

```powershell
git clone https://github.com/zengsheng-git/chessboard.git
cd chessboard
pnpm install
pnpm tauri dev        # 首次会 panic 在引擎路径，没关系
New-Item -ItemType Directory -Force server\target\debug\_up_ | Out-Null
New-Item -ItemType Junction -Path server\target\debug\_up_\libs -Target libs
Copy-Item libs\windows-cpu\*.dll server\target\debug\ -Force
pnpm tauri dev        # 第二次成功
```

> dev 模式首次运行 panic 是因为 Tauri 把资源路径中的 `..` 映射为 `_up_` 目录，需要在 `target\debug\` 下建立 junction 指向 `libs`。正式打包不受影响。
>
> 复制 CPU 版 DLL 是因为 ort 以 `load-dynamic` 方式按 exe 目录加载 `onnxruntime.dll`；dev 模式不会自动放置 DLL，若 exe 目录没有，Windows 会继续搜索 system32，可能命中其他软件遗留的旧版 DLL（如 1.10），导致 ort 版本校验 panic（要求 ≥1.20）。

dev 模式启用 GPU 推理（覆盖 DLL 法）：

```powershell
# 一次性：把 GPU 版 DLL 复制到 target\debug\（覆盖 CPU 版同名文件）
Copy-Item F:\w\template\chessboard\libs\windows-gpu\* F:\w\template\chessboard\server\target\debug\ -Force
# 之后每次启动都用 --features gpu
pnpm tauri dev --features gpu
# 切回 CPU
Copy-Item F:\w\template\chessboard\libs\windows-cpu\*.dll F:\w\template\chessboard\server\target\debug\ -Force
pnpm tauri dev
```

> GPU 版需要在 Release 下载 windows-gpu.zip 放到 `libs/windows-gpu/`；`--features gpu` 首次会重新链接 ort，较慢。无 N 卡驱动时按注册顺序自动落到 DirectML 或 CPU。

打包安装包：

```powershell
pnpm build:cpu    # CPU 版（仓库自带运行时 DLL）
pnpm build:gpu    # GPU 版（需要把官方 xqlink_0.1.2_x64-GPU.msi 解包后的 windows-gpu DLL 放到 libs\windows-gpu\）
```

产物位置：`server\target\release\bundle\msi\xqlink_0.1.2_x64_en-US.msi`。

## ⚙️ 引擎配置说明

界面"引擎配置"的 6 个设置（括号内为默认值）：

| 设置 | 默认值 | 作用 | 生效时机 |
|---|---|---|---|
| 深度 | 20 | 搜索层数上限，越大棋力越强、耗时越长 | 立即 |
| 时间 | 5 秒 | 单次思考的时间上限（界面单位为秒，保存时转毫秒） | 立即 |
| 线程 | 4 | 引擎搜索线程数，越多则同样时间搜得越深 | **需重启应用** |
| 哈希 | 64MB | 置换表（搜索缓存）大小，减少重复计算 | **需重启应用** |
| 云库 | 开启 | 优先联网查询 chessdb.cn 云库 | 立即 |
| 云库超时 | 5 秒 | 超时后转本地引擎思考 | 立即 |

机制要点：

- 深度与时间同时下发（`go depth N movetime M`），**谁先到谁生效**：最多思考设定的时间，提前搜满深度则立即出招。
- **云库优先于本地引擎**：开启时先查云库，命中则毫秒级返回且不启动本地思考；查不到或超时才用 pikafish 本地计算。大部分常见局面走云库，这是出招快的主要原因。
- **生效时机不同**：深度 / 时间 / 云库开关 / 云库超时改完立即生效（每次分析实时读取）；**线程 / 哈希改完仅保存配置，需重启应用才生效**（当前实现下界面不会触发引擎重载，Threads/Hash 只在应用启动时下发给引擎进程）。

调参建议：

- **出招更快**：调小时间（如 2 秒），保持云库开启。
- **棋力更强**：调大线程（不超过物理核数）和哈希（如 256MB），改完重启应用生效；引擎思考期间 CPU 会短时升高，属正常现象。

## ⚠️ 注意事项

- **CPU 与 GPU 版本互斥**：MSI 文件名完全相同，后打的会覆盖前者，打包后请立即按内容加 `-CPU` / `-GPU` 后缀（GPU 版约 350MB+，CPU 版约 75MB）。
- **GPU DLL 仓库不含**：从 [v0.1.2-gpu-dlls Release](https://github.com/zengsheng-git/chessboard/releases/tag/v0.1.2-gpu-dlls) 下载 `windows-gpu.zip`（196MB），解压到 `libs/windows-gpu/` 后才能 `pnpm build:gpu`（`onnxruntime_providers_cuda.dll` 320MB 超过 GitHub 单文件 100MB 限制）。
- **dev 模式 `_up_` junction 与 DLL**：`cargo clean` 或删除 `target/` 后需要重新建立 junction 并重新复制 `libs\windows-cpu\*.dll` 到 `target\debug\`，否则引擎启动会 panic。
- **交替打包 CPU/GPU 前先清理 `target\release\`**：`tauri build` 会把 `bundle.resources` 声明的 DLL 复制到 `server\target\release\`（exe 旁），打包时还会把该目录里**所有** DLL 扫进安装包。GPU 打包留下的 `onnxruntime_providers_cuda.dll`、`onnxruntime_providers_tensorrt.dll` 会让之后打出的 CPU 包也带上 300MB+ 的 CUDA 文件（表现为 CPU 包从 75MB 变成 266MB）。打 CPU 包前先删除 `server\target\release\onnxruntime*.dll`（打包过程会自动放入需要的）。
- **WebView2 与沙箱**：WebView2 需正常访问 `AppData\Local\<identifier>\EBWebView\` 目录，在某些受限终端（如 sandbox）下 webview 会启动失败导致窗口白屏，需在普通终端运行。
- **已安装冲突**：与官方版同 identifier `top.itmeng.xqlink`，同时只能装一个（`allowDowngrades: true` 允许升级/降级互盖）。
- **rotate 模型未开源**：`libs/rotate.onnx` 不在公开仓库，也未随 MSI 发布，`build:cpu:rotate` 与 `build:gpu:rotate` 不可用。