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
pnpm tauri dev        # 第二次成功
```

> dev 模式首次运行 panic 是因为 Tauri 把资源路径中的 `..` 映射为 `_up_` 目录，需要在 `target\debug\` 下建立 junction 指向 `libs`。正式打包不受影响。

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

## ⚠️ 注意事项

- **CPU 与 GPU 版本互斥**：MSI 文件名完全相同，后打的会覆盖前者，打包后请立即按内容加 `-CPU` / `-GPU` 后缀（GPU 版约 350MB+，CPU 版约 40MB）。
- **GPU DLL 仓库不含**：从 [v0.1.2-gpu-dlls Release](https://github.com/zengsheng-git/chessboard/releases/tag/v0.1.2-gpu-dlls) 下载 `windows-gpu.zip`（196MB），解压到 `libs/windows-gpu/` 后才能 `pnpm build:gpu`（`onnxruntime_providers_cuda.dll` 320MB 超过 GitHub 单文件 100MB 限制）。
- **dev 模式 `_up_` junction**：`cargo clean` 或删除 `target/` 后需要重新建立，否则引擎启动会 panic。
- **WebView2 与沙箱**：WebView2 需正常访问 `AppData\Local\<identifier>\EBWebView\` 目录，在某些受限终端（如 sandbox）下 webview 会启动失败导致窗口白屏，需在普通终端运行。
- **已安装冲突**：与官方版同 identifier `top.itmeng.xqlink`，同时只能装一个（`allowDowngrades: true` 允许升级/降级互盖）。
- **rotate 模型未开源**：`libs/rotate.onnx` 不在公开仓库，也未随 MSI 发布，`build:cpu:rotate` 与 `build:gpu:rotate` 不可用。