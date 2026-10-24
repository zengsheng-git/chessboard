use std::thread;
use std::time::Duration;

use tauri::async_runtime::block_on;
use tauri::AppHandle;
use tauri::Emitter as _;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;
use xcap::image::ImageBuffer;
use xcap::image::Rgba;

use crate::attack;
use crate::chess;
use crate::common;
use crate::engine::DeviationCost;
use crate::engine::QueryResult;
use crate::listen::ListenWindow;
use crate::listen::Window;
use crate::yolo::predict;
use crate::yolo::IMAGE_HEIGHT;
use crate::yolo::IMAGE_WIDTH;
use crate::SHARED_STATE;

// 棋盘分析结果
struct BoardAnalysisResult {
    expect_move: chess::Changed,
    expect_board: [[char; 9]; 10],
}

// 真实对局棋谱的一步: 该步之后的局面 + 中文记谱, 供局势推演从任意一步开局
#[derive(Debug, serde::Serialize, Clone)]
pub struct GameStep {
    pub camp: char,                       // 该局面当前行棋方 'w'/'b'
    pub notation: String,                 // 该步中文记谱(起始局面为空串)
    pub mover: char,                      // 走子方 'w'/'b'(起始局面为 '\0')
    pub mv: String,                       // 该步 iccs 着法(如 "h2e2", 起始局面为空串)
    pub board: Vec<chess::Position>,      // 该步之后的局面
}

// 定义不同的棋盘状态
#[derive(PartialEq)]
enum ChessboardState {
    Initial,      // 初始状态，没有进行任何分析
    StartPos,     // 初始棋盘状态
    OurTurn,      // 我方行棋
    OpponentTurn, // 对方行棋
    Invalid,      // 无效状态
}

// 分析上下文，保存分析状态和共享数据
struct AnalysisContext {
    app: AppHandle,
    window: ListenWindow,
    last_board: [[char; 9]; 10],
    expect_move: chess::Changed,
    expect_board: [[char; 9]; 10],
    expect_score: isize,                    // 上次分析的评分(行棋方视角)
    expect_camp: chess::Camp,               // 上次分析的行棋方
    pending_deviation: Option<chess::Camp>, // 刚发生非预期走子的一方, 下次分析时计算代价
    invalid_change_count: usize,
    last_hint_version: u32, // 上次分析时的提示档位版本号, 用于检测切档后立即重算
    game: Vec<GameStep>,    // 真实对局棋谱(第 0 项为起始局面), 每步走子后追加并整体推给前端
    recorded_board: [[char; 9]; 10], // 最后一次记入棋谱的局面; 只在记谱/重建时更新, 不被跟踪抖动污染
    pending_board: Option<[[char; 9]; 10]>, // Initial 重建前的待确认局面(连续两拍一致才重建, 过滤抖动帧)
}

unsafe impl Send for AnalysisContext {}
unsafe impl Sync for AnalysisContext {}

impl AnalysisContext {
    fn new(app: AppHandle, window: ListenWindow) -> Self {
        Self {
            app,
            // state_for_thread: state,
            window,
            last_board: [[' '; 9]; 10],
            expect_move: chess::Changed::default(),
            expect_board: [[' '; 9]; 10],
            expect_score: 0,
            expect_camp: chess::Camp::None,
            pending_deviation: None,
            invalid_change_count: 0,
            last_hint_version: crate::hint_version(),
            game: Vec::new(),
            recorded_board: [[' '; 9]; 10],
            pending_board: None,
        }
    }

    // 检查是否需要终止分析线程
    fn should_stop(&self) -> bool {
        let state = SHARED_STATE.get().unwrap();
        state.listen_thread.lock().unwrap().is_none()
    }

    // 确认棋盘状态是否稳定
    fn confirm_board(&self, board: [[char; 9]; 10]) -> bool {
        thread::sleep(Duration::from_millis(100));
        let Some(conf_image) = self.window.capture() else { return false };
        if let Some((_, conf_board)) = get_board(conf_image) {
            return conf_board == board;
        }
        false
    }

    // 分析棋盘并返回结果
    fn analyze_board(&mut self, camp: &chess::Camp, board: [[char; 9]; 10]) -> Option<BoardAnalysisResult> {
        let fen = chess::board_fen(camp, board);
        // 先克隆配置再释放读锁: 搜索可能耗时数秒(云库超时+引擎),
        // 期间不能阻塞设置界面的配置写入命令, 否则改参数/切档位会卡住
        let engine_config = {
            let config = SHARED_STATE.get().unwrap().config.read().unwrap();
            config.engine
        };
        let state = SHARED_STATE.get().unwrap();
        let mut engine = state.engine.lock().unwrap();
        let result = block_on(engine.search(&fen, &engine_config));
        // 本次分析已按当前档位完成, 记录档位版本号, 避免下一拍重复重算
        self.last_hint_version = crate::hint_version();
        result.as_ref()?;

        // 上一步为非预期走子时, 携带当时的预期评分用于计算偏离代价
        let prev = self.pending_deviation.take().and_then(|dev_camp| {
            if self.expect_camp == dev_camp { Some((dev_camp, self.expect_score)) } else { None }
        });

        let result = result.unwrap();
        let score = result.score;
        let (expect_move, expect_board) = analyse(&self.app, result, board, camp, prev)?;
        // 记录本次预期评分与行棋方, 供下一步偏离对比
        self.expect_score = score;
        self.expect_camp = camp.clone();
        Some(BoardAnalysisResult { expect_move, expect_board })
    }

    // 更新UI显示
    fn update_ui(&self, camp: &chess::Camp, board: [[char; 9]; 10]) {
        let board_map = chess::board_map(board);
        self.app.emit("mirror", camp.is_black()).unwrap();
        self.app.emit("position", &board_map).unwrap();
    }

    // 处理移动事件
    fn handle_move(&mut self, changed: &chess::Changed) { self.app.emit("move", changed).unwrap(); }

    // 棋谱重建判定: 仅在首次识别(棋谱为空)或检测到新对局(回到初始布局)时重建;
    // 对局中跟踪恢复(拾子帧/漏拍/盘面跳变)保留已有棋谱, 避免整盘记录被清空
    fn should_rebuild_game(game_len: usize, board: [[char; 9]; 10]) -> bool {
        game_len == 0 || chess::startpos(board)
    }

    // 以当前局面为起点重建棋谱并整体推给前端; 对局中恢复时保留已有棋谱只续记
    fn reset_game(&mut self, turn: &chess::Camp, board: [[char; 9]; 10]) {
        if !Self::should_rebuild_game(self.game.len(), board) {
            // 断档续记: 中间未记录的走子已无法回溯, 接受跳步并把记谱基准同步到当前局面,
            // 否则基准永远落后, 后续所有走子都会被断档守卫拦下, 棋谱卡死
            info!("对局跟踪恢复, 保留已有棋谱(已记 {} 步), 基准同步到当前局面, 断档续记", self.game.len() - 1);
            self.recorded_board = board;
            return;
        }
        if !self.game.is_empty() {
            info!("检测到新对局(回到初始布局), 对局棋谱重新开始");
        }
        self.game.clear();
        self.game.push(GameStep {
            camp: turn.to_char(),
            notation: String::new(),
            mover: '\0',
            mv: String::new(),
            board: chess::board_map(board),
        });
        self.recorded_board = board;
        let _ = self.app.emit("game_history", &self.game);
    }

    // 记录真实对局一步: 以 recorded_board(上次记谱局面)为基准算中文记谱并推进记录
    fn record_game_step(&mut self, changed: &chess::Changed, board: [[char; 9]; 10]) {
        let mv = format!("{}{}", changed.from, changed.to);
        let notation = chess::board_move_chinese(self.recorded_board, &mv);
        info!("对局记录: {} ({})", notation, mv);
        self.game.push(GameStep {
            camp: changed.camp.opposite().to_char(),
            notation,
            mover: changed.camp.to_char(),
            mv: mv.clone(),
            board: chess::board_map(board),
        });
        self.recorded_board = board;
        let _ = self.app.emit("game_history", &self.game);
    }

    // 多步桥接尝试: 捕获间隔(截图+识别约 1 秒)内双方连走多步时, diff 会看到多格变化(Unknown),
    // 在合法着法空间内搜索 记谱基准→当前局面 的着法序列并逐步补记谱;
    // 能衔接则回到正常跟踪并分析新局面, 无法衔接返回 None 走原重建/稳定流程
    fn try_bridge(&mut self, board: [[char; 9]; 10], camp: &chess::Camp) -> Option<ChessboardState> {
        // 桥接的行棋起点 = 棋谱最后一项的行棋方(该项 camp 即走子后轮到的一方)
        let side = match self.game.last().map(|s| s.camp).unwrap_or('\0') {
            'w' => chess::Camp::Red,
            'b' => chess::Camp::Black,
            _ => return None,
        };
        let Some(path) = Self::bridge_moves(self.recorded_board, board, side) else {
            return None;
        };
        info!("跟踪恢复: 桥接连续 {} 步(捕获间隔内连走), 完整记谱", path.len());
        let mut cur = self.recorded_board;
        let mut mover = chess::Camp::None;
        for changed in &path {
            cur = chess::board_move(cur, &format!("{}{}", changed.from, changed.to));
            mover = changed.camp.clone();
            self.record_game_step(changed, cur);
        }
        self.last_board = board;
        self.update_ui(camp, board);
        Some(self.analyze_and_set_state(mover.opposite(), board, camp))
    }

    // 在合法着法空间内搜索 from→to 的连续着法序列(每步恰改 2 格, 深度按差异格数收敛, 上限 3 步)
    fn bridge_moves(from: [[char; 9]; 10], to: [[char; 9]; 10], side: chess::Camp) -> Option<Vec<chess::Changed>> {
        let mut cells = 0;
        for y in 0..10 {
            for x in 0..9 {
                if from[y][x] != to[y][x] {
                    cells += 1;
                }
            }
        }
        if cells == 0 || cells % 2 != 0 {
            return None;
        }
        let mut path = Vec::new();
        if Self::bridge_dfs(from, to, side, (cells / 2).min(3), &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn bridge_dfs(
        board: [[char; 9]; 10], to: [[char; 9]; 10], side: chess::Camp, depth: usize, path: &mut Vec<chess::Changed>,
    ) -> bool {
        for mv in attack::legal_moves(&board, &side) {
            let next = chess::board_move(board, &mv);
            if next == to {
                path.push(chess::Changed::from_pv(&mv, board));
                return true;
            }
            if depth > 1 && Self::bridge_dfs(next, to, side.opposite(), depth - 1, path) {
                path.insert(0, chess::Changed::from_pv(&mv, board));
                return true;
            }
        }
        false
    }

    // 按当前档位重算给定行棋方的提示并更新预期(用于切档后棋盘未变时的立即刷新)
    fn reanalyze(&mut self, camp: &chess::Camp) {
        if let Some(result) = self.analyze_board(camp, self.last_board) {
            self.expect_move = result.expect_move;
            self.expect_board = result.expect_board;
        }
    }

    // 分析指定行棋方并更新预期，返回新状态
    fn analyze_and_set_state(&mut self, turn: chess::Camp, board: [[char; 9]; 10], camp: &chess::Camp) -> ChessboardState {
        if let Some(result) = self.analyze_board(&turn, board) {
            self.expect_move = result.expect_move;
            self.expect_board = result.expect_board;
        }
        if turn.eq(camp) {
            ChessboardState::OurTurn
        } else {
            ChessboardState::OpponentTurn
        }
    }

    // 处理一次走棋：更新UI、分析下一行动方并返回新状态
    fn handle_move_and_next(&mut self, changed: &chess::Changed, board: [[char; 9]; 10], camp: &chess::Camp) -> ChessboardState {
        // 仅当跟踪基准与记谱基准一致(连续跟踪)时记谱, 否则说明中间有断档, 本步跳过记录
        if self.last_board == self.recorded_board {
            self.record_game_step(changed, board);
        } else {
            debug!("局面基准与棋谱断档, 本步跳过记录");
        }
        self.advance(changed, board, camp)
    }

    // 记谱后推进: 更新基准与UI、分析下一行动方(恢复补记与正常走子共用)
    fn advance(&mut self, changed: &chess::Changed, board: [[char; 9]; 10], camp: &chess::Camp) -> ChessboardState {
        self.last_board = board;
        self.handle_move(changed);
        // 本步为非预期走子(与引擎预测不符), 下次分析时计算偏离代价
        self.pending_deviation = Some(changed.camp.clone());
        self.analyze_and_set_state(changed.camp.opposite(), board, camp)
    }

    // 处理错误变化计数
    fn handle_invalid_change(
        &mut self, last_board: [[char; 9]; 10], board: [[char; 9]; 10], camp: &chess::Camp,
    ) -> ChessboardState {
        if self.invalid_change_count < 3 {
            self.invalid_change_count += 1;
            let last_fen = chess::board_fen(camp, last_board);
            let current = chess::board_fen(camp, board);
            debug!("OneChanged last {}", last_fen);
            debug!("OneChanged current {}", current);
            ChessboardState::Invalid
        } else {
            // 如果出现次数超过3次，重置为初始状态
            debug!("OneChanged count=3, reload");
            self.invalid_change_count = 0;
            ChessboardState::Initial
        }
    }
}

pub fn get_board(image: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Option<(chess::Camp, [[char; 9]; 10])> {
    let data = predict(image).unwrap();
    if let Ok((camp, mut board)) = common::detections_to_board(&data) {
        chess::board_fix(&camp, &mut board);
        Some((camp, board))
    } else {
        None
    }
}

// 将引擎/云库结果整理为完整展示数据(中文主线/次优/局面/偏离等), 镜像与推演共用
pub fn prepare_result(
    mut result: QueryResult, board: [[char; 9]; 10], camp: &chess::Camp, prev: Option<(chess::Camp, isize)>,
) -> Option<(QueryResult, chess::Changed, [[char; 9]; 10])> {
    // 云库/引擎可能返回成功状态但缺少着法, 跳过本次展示, 避免监听线程 panic
    let Some(best_pv) = result.pvs.first().cloned() else {
        warn!("分析结果缺少着法, 跳过展示: {:?}", result.state);
        return None;
    };

    // 上一步为非预期走子时, 计算相对预期的亏损(前后评分均为行棋方视角)
    if let Some((dev_camp, prev_score)) = prev
        && dev_camp == camp.opposite()
    {
        result.deviation = Some(DeviationCost { camp: dev_camp.to_char(), loss: prev_score + result.score });
    }
    let best_move = chess::board_move_chinese(board, &best_pv);
    let expect_board = chess::board_move(board, &best_pv);
    let expect_move = chess::Changed::from_pv(&best_pv, board);
    // 携带分析时局面, 供前端主线预演
    result.board = chess::board_map(board);

    let mut tmp_board = expect_board;
    result.moves.push(best_move);
    for pv in result.pvs.iter().skip(1) {
        let mv = chess::board_move_chinese(tmp_board, pv);
        result.moves.push(mv);
        tmp_board = chess::board_move(tmp_board, pv);
    }
    // 次优候选首着（与最优并列，均从当前局面出发）
    for alt in result.alternatives.clone() {
        result.alt_moves.push(chess::board_move_chinese(board, &alt));
    }
    // 标记本次分析的行棋方阵营
    result.camp = camp.to_char();
    Some((result, expect_move, expect_board))
}

pub fn analyse(app: &AppHandle, result: QueryResult, board: [[char; 9]; 10], camp: &chess::Camp, prev: Option<(chess::Camp, isize)>) -> Option<(chess::Changed, [[char; 9]; 10])> {
    let (result, expect_move, expect_board) = prepare_result(result, board, camp, prev)?;
    // 把结果发送给前端
    info!("分析结果 {:?}", result);
    app.emit("analyse", result).unwrap();

    // 返回一个预期move和预期board
    Some((expect_move, expect_board))
}

// 处理循环逻辑的主函数
// 包一层捕获 panic: 线程内任何未预期错误(截图/识别/引擎异常)只终止本次监听,
// 不让 join 方再因线程 panic 而连锁 panic 卡死界面
fn process_analysis_loop_guarded(context: AnalysisContext) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        process_analysis_loop(context);
    }));
    if let Err(e) = result {
        let msg = e.downcast_ref::<String>().map(|s| s.as_str())
            .or_else(|| e.downcast_ref::<&str>().copied())
            .unwrap_or("未知错误");
        error!("分析线程异常退出: {}", msg);
    }
}
fn process_analysis_loop(mut context: AnalysisContext) {
    let mut current_state = ChessboardState::Initial;
    let mut capture_failures = 0usize;

    loop {
        // 检查是否需要停止监听
        if context.should_stop() {
            debug!("listen stopped");
            break;
        }

        // 获取等待间隔
        let interval = SHARED_STATE.get().unwrap().config.read().unwrap().timer_interval;
        thread::sleep(Duration::from_millis(interval));

        // 截图失败多为目标窗口已关闭, 连续失败达到上限则通知前端并结束监听
        let Some(image) = context.window.capture() else {
            capture_failures += 1;
            if capture_failures >= 3 {
                error!("目标窗口连续截图失败, 可能已关闭, 停止监听");
                let _ = context.app.emit("listen_stopped", "目标窗口可能已关闭, 监听已停止");
                break;
            }
            continue;
        };
        capture_failures = 0;

        // 捕获并分析棋盘
        let board_result = get_board(image);
        if board_result.is_none() {
            continue;
        }

        let (camp, board) = board_result.unwrap();
        trace!("{:?} {:?}", camp, board);

        // 根据不同状态处理棋盘
        current_state = match current_state {
            ChessboardState::Initial => {
                // 识别局面无效(缺子/多子): 不分析不重建, 快速等下一拍, 避免把抖动帧当基准
                if !chess::board_check(board) {
                    debug!("识别局面无效, 等待识别稳定");
                    ChessboardState::Invalid
                } else {
                // 中途恢复跟踪(拾子帧/漏拍/抖动后回到 Initial): 以棋谱基准衔接, 而非直接重置
                let mut recovered: Option<ChessboardState> = None;
                if !context.game.is_empty() {
                    if board == context.recorded_board {
                        // 抖动幻影消失后盘面与记谱基准一致: 回退跟踪, 不做无谓重分析
                        context.pending_board = None;
                        context.last_board = board;
                        recovered = Some(ChessboardState::OurTurn);
                    } else {
                        let (changed, diff_state) = chess::board_diff(context.recorded_board, board);
                        match diff_state {
                            // 恰好比记谱基准多一步(拾子帧 One→Invalid→Initial 后走子完成): 补记衔接
                            chess::BoardChangeState::Move => {
                                info!("跟踪恢复: 与棋谱基准衔接一步");
                                context.pending_board = None;
                                context.record_game_step(&changed, board);
                                recovered = Some(context.advance(&changed, board, &camp));
                            }
                            // 仍只差一格(拾子帧/瞬时误检): 基准回退到记谱局面, 等走子完成
                            chess::BoardChangeState::One => {
                                debug!("跟踪恢复: 单格变化, 等待走子完成");
                                context.pending_board = None;
                                context.last_board = context.recorded_board;
                                recovered = Some(ChessboardState::OurTurn);
                            }
                            // 多格变化: 先尝试桥接补记(捕获间隔内连走多步), 无法桥接才走稳定确认
                            _ => {
                                if let Some(next) = context.try_bridge(board, &camp) {
                                    context.pending_board = None;
                                    recovered = Some(next);
                                } else if context.pending_board == Some(board) {
                                    context.pending_board = None;
                                    info!("局面跳变已稳定, 以当前局面重建基准(断档续记)");
                                } else {
                                    context.pending_board = Some(board);
                                    debug!("局面跳变待稳定, 暂缓重建");
                                    recovered = Some(ChessboardState::Initial);
                                }
                            }
                        }
                    }
                }
                if let Some(next) = recovered {
                    next
                } else {
                    // 初始状态，做第一次分析
                    debug!("首次启动，立即分析");

                    // 设置前端棋盘
                    context.update_ui(&camp, board);

                    // 确定初始局面轮到谁：初始棋盘红方先手，否则按玩家阵营近似
                    let turn = if chess::startpos(board) {
                        chess::Camp::Red
                    } else if camp.eq(&chess::Camp::Red) {
                        chess::Camp::Red
                    } else {
                        chess::Camp::Black
                    };

                    // 重建对局棋谱(仅首次识别或新对局时清空; 对局中恢复保留已有棋谱)
                    context.reset_game(&turn, board);

                    // 分析当前行棋方
                    if let Some(result) = context.analyze_board(&turn, board) {
                        context.expect_move = result.expect_move;
                        context.expect_board = result.expect_board;
                    }

                    context.last_board = board;

                    // 如果是初始棋盘，进入初始状态，否则进入一般状态
                    if chess::startpos(board) {
                        ChessboardState::StartPos
                    } else if turn.eq(&camp) {
                        ChessboardState::OurTurn
                    } else {
                        ChessboardState::OpponentTurn
                    }
                }
                }
            }

            ChessboardState::StartPos => {
                // 判断棋盘是否仍然是初始棋盘
                if !chess::startpos(board) {
                    // 不再是初始棋盘，处理正常的棋局变化
                    if board == context.last_board {
                        ChessboardState::StartPos // 没有变化
                    } else {
                        // 有变化，更新UI并分析
                        let (changed, board_state) = chess::board_diff(context.last_board, board);

                        match board_state {
                            chess::BoardChangeState::Move => {
                                context.handle_move_and_next(&changed, board, &camp)
                            }
                            chess::BoardChangeState::One => {
                                context.handle_invalid_change(context.last_board, board, &camp)
                            }
                            chess::BoardChangeState::Unknown => {
                                // 多格变化: 无效局面(识别抖动)就地等待, 有效局面才重置跟踪基准
                                if !chess::board_check(board) {
                                    debug!("棋局变化未知且局面无效, 等待识别稳定");
                                    current_state
                                } else if let Some(next) = context.try_bridge(board, &camp) {
                                    next
                                } else {
                                    debug!("棋局变化未知，重置上下文");
                                    context.update_ui(&camp, board);
                                    context.last_board = board;
                                    ChessboardState::Initial
                                }
                            }
                        }
                    }
                } else if chess::Camp::Red.eq(&camp) {
                    // 仍然是初始棋盘，且我方先手
                    if context.last_board == board {
                        // 防止重复分析; 但档位已切换时按新档位重算开局提示
                        let version = crate::hint_version();
                        if version != context.last_hint_version {
                            context.last_hint_version = version;
                            info!("提示档位已切换, 按新档位重算当前局面");
                            context.reanalyze(&camp);
                        }
                        ChessboardState::StartPos
                    } else {
                        // 设置前端棋盘
                        context.last_board = board;
                        context.update_ui(&camp, board);

                        // 调用引擎查询
                        if let Some(result) = context.analyze_board(&camp, board) {
                            context.expect_move = result.expect_move;
                            context.expect_board = result.expect_board;
                        }

                        ChessboardState::OurTurn
                    }
                } else {
                    // 对方先手，跳过分析
                    debug!("对方先手，跳过分析");
                    context.last_board = board;
                    context.update_ui(&camp, board);
                    ChessboardState::OpponentTurn
                }
            }

            ChessboardState::OurTurn | ChessboardState::OpponentTurn => {
                // 判断棋盘是否未发生变化
                if board == context.last_board {
                    // 棋盘未变但档位已切换: 立即按新档位重算当前局面提示
                    let version = crate::hint_version();
                    if version != context.last_hint_version {
                        context.last_hint_version = version;
                        info!("提示档位已切换, 按新档位重算当前局面");
                        context.reanalyze(&camp);
                    } else {
                        debug!("棋盘未发生变化，跳过分析");
                    }
                    current_state // 保持当前状态
                } else if board == context.expect_board {
                    // 符合预期棋盘，分析下一行动方
                    debug!("棋盘为预期棋盘，分析下一行动方");
                    let expect_move = context.expect_move.clone();
                    let expect_board = context.expect_board;
                    // 预测招法兑现也是真实对局的一步, 记入棋谱(基准断档时跳过记录)
                    if context.last_board == context.recorded_board {
                        context.record_game_step(&expect_move, expect_board);
                    } else {
                        debug!("预期兑现但基准断档, 本步跳过记录");
                    }
                    context.last_board = expect_board;
                    context.handle_move(&expect_move);
                    context.analyze_and_set_state(expect_move.camp.opposite(), expect_board, &camp)
                } else {
                    // 确认棋盘变化是否稳定
                    if !context.confirm_board(board) {
                        debug!("棋盘延迟确认失败");
                        let confirm_interval = SHARED_STATE.get().unwrap().config.read().unwrap().confirm_interval;
                        thread::sleep(Duration::from_millis(confirm_interval));
                        current_state // 保持当前状态
                    } else if !chess::board_check(board) {
                        // 检测棋盘是否有效
                        let debug_fen = chess::board_fen(&camp, board);
                        debug!("棋盘识别无效: {}", debug_fen);
                        current_state // 保持当前状态
                    } else {
                        // 处理正常棋盘变化
                        let (changed, board_state) = chess::board_diff(context.last_board, board);

                        match board_state {
                            chess::BoardChangeState::Move => {
                                context.handle_move_and_next(&changed, board, &camp)
                            }
                            chess::BoardChangeState::One => {
                                context.handle_invalid_change(context.last_board, board, &camp)
                            }
                            chess::BoardChangeState::Unknown => {
                                // 多格变化: 无效局面(识别抖动)就地等待; 有效局面先尝试桥接补记
                                if !chess::board_check(board) {
                                    debug!("棋局变化未知且局面无效, 等待识别稳定");
                                    current_state
                                } else if let Some(next) = context.try_bridge(board, &camp) {
                                    next
                                } else {
                                    debug!("棋局变化未知，重置上下文");
                                    context.update_ui(&camp, board);
                                    context.last_board = board;
                                    ChessboardState::Initial
                                }
                            }
                        }
                    }
                }
            }

            ChessboardState::Invalid => {
                // 复位到初始状态，等待下一次有效的变化
                ChessboardState::Initial
            }
        };
    }
}

// 初始化Tauri的command处理
#[tauri::command]
pub async fn start_listen(app: AppHandle, target: Window) -> Result<(), String> {
    trace!("start_listen");
    {
        // 先检查再占用, 整个过程持锁防止并发双开
        let shared_state = SHARED_STATE.get().unwrap();
        let mut listen_slot = shared_state.listen_thread.lock().unwrap();
        if listen_slot.is_some() {
            error!("current listen thread is running, please stop it first");
            return Err("已经在监听中".to_string());
        }

        // 初始化监听窗口模块
        let Some(mut window) = ListenWindow::new(&target, IMAGE_WIDTH, IMAGE_HEIGHT) else {
            return Err("目标窗口不存在".to_string());
        };
        let Some(image) = window.capture() else {
            return Err("无法截取目标窗口画面".to_string());
        };

        let image_h = image.height();
        let image_w = image.width();

        let detections = predict(image).unwrap();

        match common::detections_bound(image_w, image_h, &detections) {
            Ok((x, y, w, h)) => {
                window.set_sub_bound(x, y, w, h); // 设置窗口边界
            }
            Err(e) => {
                return Err(e); // 未识别到棋盘
            }
        }

        // 创建分析上下文
        let context = AnalysisContext::new(app.clone(), window);

        // 启动后台线程进行截图和处理
        let listen_thread = thread::spawn(move || {
            trace!("into thread");
            process_analysis_loop_guarded(context);
        });

        *listen_slot = Some(listen_thread);
    }

    Ok(())
}

#[tauri::command]
pub fn stop_listen() {
    info!("stop listen");
    let shared_state = SHARED_STATE.get().unwrap();
    if let Ok(mut state) = shared_state.listen_thread.lock()
        && let Some(listen_thread) = state.take()
    {
        // 释放锁，停止后台线程
        debug!("释放锁，停止后台线程");
        drop(state);
        // join 移到独立线程: 后台线程最多在下一个循环检查点退出(通常数百毫秒),
        // 但停止命令必须立即返回, 保证界面永不因停止操作而冻结
        thread::spawn(move || {
            if let Err(e) = listen_thread.join() {
                error!("等待监听线程退出失败: {:?}", e);
            }
        });
    }
    debug!("stoped");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start_board() -> [[char; 9]; 10] {
        [
            ['r', 'n', 'b', 'a', 'k', 'a', 'b', 'n', 'r'],
            [' '; 9],
            [' ', 'c', ' ', ' ', ' ', ' ', ' ', 'c', ' '],
            ['p', ' ', 'p', ' ', 'p', ' ', 'p', ' ', 'p'],
            [' '; 9],
            [' '; 9],
            ['P', ' ', 'P', ' ', 'P', ' ', 'P', ' ', 'P'],
            [' ', 'C', ' ', ' ', ' ', ' ', ' ', 'C', ' '],
            [' '; 9],
            ['R', 'N', 'B', 'A', 'K', 'A', 'B', 'N', 'R'],
        ]
    }

    fn mid_board() -> [[char; 9]; 10] {
        let mut board = [[' '; 9]; 10];
        board[9][4] = 'K';
        board[0][3] = 'k';
        board
    }

    #[test]
    fn test_should_rebuild_game() {
        // 首次识别(棋谱为空): 总是重建
        assert!(AnalysisContext::should_rebuild_game(0, start_board()));
        assert!(AnalysisContext::should_rebuild_game(0, mid_board()));
        // 对局中跟踪恢复(非初始布局): 保留已有棋谱, 不清空
        assert!(!AnalysisContext::should_rebuild_game(3, mid_board()));
        // 检测到新对局(回到初始布局): 重建
        assert!(AnalysisContext::should_rebuild_game(3, start_board()));
    }

    #[test]
    fn test_bridge_moves() {
        // 单步桥接: 红马 b0c2
        let b1 = chess::board_move(start_board(), "b0c2");
        let path = AnalysisContext::bridge_moves(start_board(), b1, chess::Camp::Red).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].from, "b0");
        assert_eq!(path[0].to, "c2");
        assert_eq!(path[0].camp, chess::Camp::Red);
        // 两步桥接: 红马 b0c2 + 黑车 a9a8(捕获间隔内连走一轮)
        let b2 = chess::board_move(b1, "a9a8");
        let path = AnalysisContext::bridge_moves(start_board(), b2, chess::Camp::Red).unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].to, "c2");
        assert_eq!(path[1].to, "a8");
        assert_eq!(path[1].camp, chess::Camp::Black);
        // 奇数格差异无法桥接
        let mut b3 = b2;
        b3[8][4] = 'C';
        assert!(AnalysisContext::bridge_moves(b2, b3, chess::Camp::Red).is_none());
        // 偶数格但不可达(黑车凭空变红兵+黑马消失)无法桥接
        let mut b4 = b2;
        b4[0][8] = 'P';
        b4[0][7] = ' ';
        assert!(AnalysisContext::bridge_moves(b2, b4, chess::Camp::Red).is_none());
    }
}
