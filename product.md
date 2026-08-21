# 中国象棋学习助手 产品文档

## 项目简介

“象棋学习助手”是一款智能化的中国象棋辅助工具。通过实时识别屏幕中的棋盘局面，结合高性能引擎分析，为在线对战平台的玩家提供精准走法建议与策略指导，帮助象棋爱好者快速提升棋艺水平。

## ✨ 功能亮点

- ⚡ **轻量极速**
  一键启动，无需繁琐配置，毫秒级别识别与分析，畅享零等待。
- 🔒 **安全可靠**
  核心模块采用 Rust 开发，确保内存安全与高并发性能。
- 🌐 **多平台支持**
  深度优化 Windows & macOS，同时兼容 Linux，让你随时随地稳如磐石。
- 🚀 **GPU 智能加速**
  原生支持 CUDA、DirectML、CoreML 等主流推理框架，全力释放显卡性能。

## 🎯 核心功能

- 📷 **实时棋盘识别**
  基于 YOLOv8 与 ONNX Runtime，精准定位与分类棋子，自动捕捉对局状态。
- 🤖 **智能走法分析**
  集成 Pikafish 象棋引擎，提供深度搜索与多变策略，支持分析深度和线程数自定义。
- 🀄 **中文招法展示**
  将专业分析结果转为易读的中文走法描述，让开局、布局一目了然。
- 📚 **开局库支持**
  接入经典开局库，实时给出理论指导，助你构建坚实布局。

## 🚀 快速上手

### 方式一：直接使用（普通用户）

1. 下载 [Release](https://github.com/atopx/chessboard/releases/latest) 页面提供的 MSI 安装包
2. 双击安装，桌面启动“中国象棋”
3. 在弹窗中选择目标窗口
4. 工具将自动识别棋盘并启动引擎分析
5. 右侧面板实时展示最佳走法与评分
6. 可在“设置”中自由调整分析深度、线程数及开局库参数

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

打包安装包：

```powershell
pnpm build:cpu    # CPU 版（仓库自带运行时 DLL）
pnpm build:gpu    # GPU 版（需要把官方 xqlink_0.1.2_x64-GPU.msi 解包后的 windows-gpu DLL 放到 libs\windows-gpu\）
```

产物位置：`server\target\release\bundle\msi\xqlink_0.1.2_x64_en-US.msi`。CPU 和 GPU 版 MSI 文件名相同，会互相覆盖，打包后请立即重命名（如加 `-CPU` / `-GPU` 后缀）。

## 🛠 开发计划

- [x] 基础棋盘识别
- [x] 引擎分析集成
- [x] 跨平台适配
- [x] 可视化配置界面
- [ ] 自研轻量AI引擎接入
- [ ] 更多引擎配置项
- [ ] 开局库接入
- [ ] 对局数据导出
- [ ] 个性化学习数据统计
- [ ] 人机对战模式