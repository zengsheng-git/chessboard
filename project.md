# 优化与功能待办

## 高优先级（低成本高收益）

- [ ] **帧差跳帧**：棋盘画面无变化时跳过截图后的 YOLO 推理（先做廉价像素 diff），预计监听基础 CPU 再降 50% 以上，且不损失反应速度
- [ ] **线程/哈希设置立即生效**：`config.rs` 的 `set_engine_threads`/`set_engine_hash` 直接调用 `engine.set_threads/set_hash`（或前端接入现成的 `reload_engine` 命令），改完删除 product.md 中"需重启应用"的说明
- [ ] **打包防污染**：`build:cpu`/`build:gpu` 前自动清理 `server\target\release\onnxruntime*.dll`，打包完成后自动给 MSI 加 `-CPU`/`-GPU` 后缀
- [ ] **监听健壮性**：目标窗口被关闭时截图 `unwrap` 会 panic（`listen.rs`），需加窗口存活检查；识别连续失败时前端给状态提示，避免"看起来卡死"
- [ ] **配置项补全**：`timer_interval`/`confirm_interval` 增加前端设置入口；接通 `show_wdl`（`EngineConfig` 有字段、引擎有 setoption，但缺 tauri 命令和 UI）

## 中优先级（体验提升）

- [ ] **对局记录与复盘**：逐步保存 FEN + 引擎评分，对局结束后可复盘"哪步走亏了多少分"、导出棋谱
- [ ] **胜率曲线**：依赖 `show_wdl` 打通，前端绘制局面胜率随步数变化的曲线
- [ ] **建议来源标注**：UI 显示 `QueryResult.source`（云库/引擎）与 `depth`，让用户知道每招的分量
- [ ] **对手缓招提醒**：偏离预期棋盘（`expect_board` 机制已有雏形）时提示亏损分数，比单纯给招法更接近"学习"

## 低优先级（大功能/工程化）

- [ ] 悬浮窗提示（游戏全屏时无需切窗口）、识别置信度展示、多显示器/DPI 适配
- [ ] 窗口匹配规则可配置化（现在写死标题匹配"天天象棋"，可适配其他对弈平台/模拟器投屏）
- [ ] rotate 模型（未开源）：解锁斜拍/投屏场景
- [ ] GitHub Actions 自动构建并发布 MSI
- [ ] `chess.rs`/`board_diff` 纯逻辑补充单元测试
- [ ] **日志体系整改**：接通 `config.loglevel`（字段已定义但 tracer 写死 DEBUG，`lib.rs:32`，日常用 INFO 可大幅减少日志量）；启动时自动清理 N 天前的旧日志；修复日志目录拼接问题（`logger.rs` 把应用数据目录误当用户主目录又拼了一层 `AppData\Local`）。日志实际位置：`C:\Users\<用户>\AppData\Roaming\top.itmeng.xqlink\AppData\Local\xqlink\logs\`，手动清理＝删除旧日期的 `runtime.log.*` 文件（应用运行中当日文件被占用，删不掉属正常）
