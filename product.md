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

界面"引擎配置"的 10 个设置（括号内为默认值）：

| 设置 | 默认值 | 作用 | 生效时机 |
|---|---|---|---|
| 提示强度 | 最强(不限) | 档位：最强=不限棋力；新手/入门/初级/中级/高级改用固定浅深度搜索（深度 2/4/6/8/12），每一手都是一致的中低水平，而非强引擎概率放水 | 立即 |
| 深度 | 20 | 搜索层数上限，越大棋力越强、耗时越长 | 立即 |
| 时间 | 5 秒 | 单次思考的时间上限（界面单位为秒，保存时转毫秒） | 立即 |
| 线程 | 4 | 引擎搜索线程数，越多则同样时间搜得越深 | **需重启应用** |
| 哈希 | 64MB | 置换表（搜索缓存）大小，减少重复计算 | **需重启应用** |
| 候选招数 | 3 | MultiPV 数量，决定展示几条次优候选（1~5） | 立即 |
| 次优分差 | 300 | 次优候选与最优分差不超过该值（厘分）才展示 | 立即 |
| 胜率显示 | 开启 | 评价行显示行棋方胜率（引擎 WDL / 云库招法胜率） | 立即 |
| 云库 | 开启 | 优先联网查询 chessdb.cn 云库 | 立即 |
| 云库超时 | 5 秒 | 超时后转本地引擎思考 | 立即 |

机制要点：

- **提示强度非"最强"时自动跳过云库**（云库只会返回最优招），并用固定浅深度覆盖用户深度：每一手都出自同一水平的浅思考，招法强度整体一致、可预期；深度与时间仍同时下发，档位低时通常深度先到。
- 深度与时间同时下发（`go depth N movetime M`），**谁先到谁生效**：最多思考设定的时间，提前搜满深度则立即出招。
- **云库优先于本地引擎**：开启时先查云库，命中则毫秒级返回且不启动本地思考；查不到或超时才用 pikafish 本地计算。大部分常见局面走云库，这是出招快的主要原因。
- **生效时机不同**：深度 / 时间 / 胜率显示 / 云库开关 / 云库超时改完立即生效（每次分析实时读取，胜率显示直接向引擎下发 setoption）；**线程 / 哈希改完仅保存配置，需重启应用才生效**（当前实现下界面不会触发引擎重载，Threads/Hash 只在应用启动时下发给引擎进程）。
- **胜率来源**：云库取自 `queryall` 最优招法的 winrate 字段；引擎来源需开启"胜率显示"（`UCI_ShowWDL`）。杀棋局面（几步杀）不显示胜率。
- **偏离提示**：实际走子与预测不符时，用上次预期评分与本次分析评分（均为行棋方视角）计算亏损，面板提示"我方/对方偏离提示 亏 N 分"。

调参建议：

- **出招更快**：调小时间（如 2 秒），保持云库开启。
- **棋力更强**：调大线程（不超过物理核数）和哈希（如 256MB），改完重启应用生效；引擎思考期间 CPU 会短时升高，属正常现象。

## 📖 分析面板解读

分析面板围绕"为什么要走这里"展示四层信息：

- **最佳招法 + 评分**：`我方 炮二平五`、`+120 较优`（正=行棋方占优，按均势/略优/较优/大优/胜势分档，几步杀直接显示步数），右上角来源标签（云库/引擎）与深度，以及行棋方胜率。
- **次优招法**：与最优分差不超过"次优分差"的其他候选首着，`-40` 表示不走提示招要亏 40 分。
- **后续主线**：完整推演着法，每步带 我方/对方 标注；鼠标悬停某一步可在棋盘上预演到该步的局面，移开还原。
- **偏离提示**：实际走子与预测不符时提示 `对方偏离提示, 亏 80 分`（红色=利好我方），分差很小则显示"与预期相当"。

## ♟️ 局势推演

顶栏模式切换到"局势推演"进入演练盘，面板信息与连线分析一致（提示/评分/胜率/次优分差/完整主线悬停预演），另增加：

- 进入推演模式窗口自动加宽：左侧观看页（镜像棋盘+分析）保持可见并持续跟随真实对局，右侧为推演盘与推演面板，两边并排；
- **对局棋谱任意局面开局**：推演盘左侧独立的"对局棋谱"列实时记录连线识别的真实对局（每步显示"回合. 走子方+中文记谱"，红黑着色，单列完整展示，长对局纵向滚动并自动跟随最新一步，推演模式窗口相应加宽）；推演操作按钮（重新推演/悔棋/前进/回起始）位于推演盘正下方；进入推演面板时**自动对齐最新局面开始推演**，无需手动点击（连线分析页面不后台启动会话，不产生额外分析）；点击任意一步可**以该步之后的局面重开推演**，"立即开始"按钮等效于点击最新一步；棋谱列标题旁的"清空"按钮可清空记录，确认后以当前识别到的局面重新开始记录。识别抖动/拾子帧/漏拍/快棋连走不会打乱记录：跟踪恢复时自动与棋谱基准衔接补记，捕获间隔内连走多步时按合法着法序列桥接逐补记谱，无法衔接时保留已有棋谱断档续记，检测到新对局(回到初始布局)才重新开始。
- **着法方框标注**（不用箭头，与提示箭头区分）：镜像盘常显最新真实走子的起讫格绿框（起点细、落点粗）；推演盘悬停棋谱条目即预览该步之后的局面并框出该步起讫格，移开还原当前局面；走子/悔棋/重开推演时标注随局面联动更新。
- 棋谱记录对拾子动画/漏拍健壮：单格瞬态（棋子被拿起/误检）保留原局面等待走子完成，走子完成后自动补记；跟踪恢复不清空已有棋谱，仅首次识别或检测到新对局（回到初始布局）时重置；
- 开局后鼠标点选棋子自由替双方试招，合法落点自动高亮（含蹩马腿、将帅对脸、送将等规则校验）；
- 每走一步自动给出当前行棋方的提示；自己走亏了会提示"这步亏了 N 分"（正分为行棋方视角）；
- 支持悔棋、前进与"回起始"（游标式非破坏导航：回退不丢探索线，可"前进"恢复，在任意节点落子即换招续推），主线预演基于演练盘当前局面。

## 🔍 界面缩放

工具栏提供"界面缩放"选择（80% ~ 125%）：整体等比缩放所有内容（棋盘/面板/文字），窗口尺寸随之调整，选择持久化在本地，重启后保留。

## ⚠️ 注意事项

- **CPU 与 GPU 版本互斥**：MSI 文件名完全相同，后打的会覆盖前者，打包后请立即按内容加 `-CPU` / `-GPU` 后缀（GPU 版约 350MB+，CPU 版约 75MB）。
- **GPU DLL 仓库不含**：从 [v0.1.2-gpu-dlls Release](https://github.com/zengsheng-git/chessboard/releases/tag/v0.1.2-gpu-dlls) 下载 `windows-gpu.zip`（196MB），解压到 `libs/windows-gpu/` 后才能 `pnpm build:gpu`（`onnxruntime_providers_cuda.dll` 320MB 超过 GitHub 单文件 100MB 限制）。
- **dev 模式 `_up_` junction 与 DLL**：`cargo clean` 或删除 `target/` 后需要重新建立 junction 并重新复制 `libs\windows-cpu\*.dll` 到 `target\debug\`，否则引擎启动会 panic。
- **交替打包 CPU/GPU 前先清理 `target\release\`**：`tauri build` 会把 `bundle.resources` 声明的 DLL 复制到 `server\target\release\`（exe 旁），打包时还会把该目录里**所有** DLL 扫进安装包。GPU 打包留下的 `onnxruntime_providers_cuda.dll`、`onnxruntime_providers_tensorrt.dll` 会让之后打出的 CPU 包也带上 300MB+ 的 CUDA 文件（表现为 CPU 包从 75MB 变成 266MB）。打 CPU 包前先删除 `server\target\release\onnxruntime*.dll`（打包过程会自动放入需要的）。
- **WebView2 与沙箱**：WebView2 需正常访问 `AppData\Local\<identifier>\EBWebView\` 目录，在某些受限终端（如 sandbox）下 webview 会启动失败导致窗口白屏，需在普通终端运行。
- **已安装冲突**：与官方版同 identifier `top.itmeng.xqlink`，同时只能装一个（`allowDowngrades: true` 允许升级/降级互盖）。
- **rotate 模型未开源**：`libs/rotate.onnx` 不在公开仓库，也未随 MSI 发布，`build:cpu:rotate` 与 `build:gpu:rotate` 不可用。
- **老配置迁移**：`show_wdl` 在旧配置里存的是关闭，需在设置里打开一次"胜率显示"；若曾在旧版本修改过"时间"，保存值被放大了 1000 倍（如改 5 秒实际存成 5000 秒），打开设置把时间改回正常值即可，未改过则不受影响。
