use std::sync::Mutex;

use serde::Serialize;
use tauri::async_runtime::block_on;

use crate::attack;
use crate::chess::{self, Camp, Position};
use crate::engine::interrupt_search;
use crate::engine::QueryResult;
use crate::worker::prepare_result;
use crate::SHARED_STATE;

pub type Board = [[char; 9]; 10];

// 演练会话: 保存起始局面与已走着法, 当前局面按着法重放推导;
// cursor 可指向历史中任意局面, 供"历史局面跳转后在任意局面续推"
pub struct PracticeSession {
    start_board: Board,
    start_camp: Camp,
    history: Vec<String>,
    cursor: usize, // 当前局面位于第 cursor 步之后, 0=起始局面
}

impl PracticeSession {
    // 重放着法: 返回当前局面(cursor 步之后)与行棋方, 以及完整历史的中文记谱
    fn replay(&self) -> (Board, Camp, Vec<String>) {
        let mut board = self.start_board;
        let mut camp = self.start_camp.clone();
        let mut notations = Vec::with_capacity(self.history.len());
        for (i, mv) in self.history.iter().enumerate() {
            // 记谱基于落子前的局面, 与该步是否被 cursor 跳过无关
            notations.push(chess::board_move_chinese(board, mv));
            if i < self.cursor {
                board = chess::board_move(board, mv);
                camp = camp.opposite();
            }
        }
        (board, camp, notations)
    }
}

static PRACTICE: Mutex<Option<PracticeSession>> = Mutex::new(None);

#[derive(Serialize)]
pub struct PracticeState {
    pub board: Vec<Position>,   // 当前局面
    pub camp: char,             // 当前行棋方
    pub moves: Vec<String>,     // 当前行棋方合法着法(iccs)
    pub history: Vec<String>,   // 已走着法(iccs)
    pub cursor: usize,          // 当前局面位于第 cursor 步之后
    pub notations: Vec<String>, // 每步着法的中文记谱(与 history 等长)
    pub start_camp: char,       // 起始局面行棋方
}

#[derive(Serialize)]
pub struct MoveOutcome {
    pub notation: String,       // 本步中文记谱
    pub state: PracticeState,
}

fn practice_state(session: &PracticeSession) -> PracticeState {
    let (board, camp, notations) = session.replay();
    PracticeState {
        board: chess::board_map(board),
        camp: camp.to_char(),
        moves: attack::legal_moves(&board, &camp),
        history: session.history.clone(),
        cursor: session.cursor,
        notations,
        start_camp: session.start_camp.to_char(),
    }
}

fn with_session<T>(f: impl FnOnce(&PracticeSession) -> T) -> Result<T, String> {
    let session = PRACTICE.lock().unwrap();
    let session = session.as_ref().ok_or("尚未开始演练")?;
    Ok(f(session))
}

fn board_from_positions(positions: &[Position]) -> Result<Board, String> {
    let mut board = [[' '; 9]; 10];
    for p in positions {
        let bytes = p.pos.as_bytes();
        if bytes.len() != 2 {
            return Err("坐标格式错误".to_string());
        }
        let x = (bytes[0] as char) as usize - 97;
        let y = 57 - (bytes[1] as char) as usize;
        if x > 8 || y > 9 {
            return Err("坐标越界".to_string());
        }
        board[y][x] = p.piece;
    }
    Ok(board)
}

#[tauri::command]
pub fn practice_start(board: Vec<Position>, camp: String) -> Result<PracticeState, String> {
    let start_board = board_from_positions(&board)?;
    let start_camp = match camp.as_str() {
        "w" => Camp::Red,
        "b" => Camp::Black,
        _ => return Err("阵营错误".to_string()),
    };
    let session = PracticeSession { start_board, start_camp, history: Vec::new(), cursor: 0 };
    let state = practice_state(&session);
    *PRACTICE.lock().unwrap() = Some(session);
    Ok(state)
}

#[tauri::command]
pub fn practice_moves() -> Result<Vec<String>, String> {
    with_session(|session| {
        let (board, camp, _) = session.replay();
        attack::legal_moves(&board, &camp)
    })
}

#[tauri::command]
pub fn practice_do_move(from: String, to: String, interrupt: Option<bool>) -> Result<MoveOutcome, String> {
    // 引擎思考中走子: 中断当前搜索, 让位给新局面的分析
    if interrupt.unwrap_or(false) {
        interrupt_search();
    }
    let mv = format!("{from}{to}");
    let mut session = PRACTICE.lock().unwrap();
    let session = session.as_mut().ok_or("尚未开始演练")?;
    let (board, camp, _) = session.replay();
    let legal = attack::legal_moves(&board, &camp);
    if !legal.contains(&mv) {
        return Err("非法着法".to_string());
    }
    let notation = chess::board_move_chinese(board, &mv);
    // 在历史任意局面落子: 截断该局面之后的旧推演, 以新招法续写
    session.history.truncate(session.cursor);
    session.history.push(mv);
    session.cursor += 1;
    Ok(MoveOutcome { notation, state: practice_state(session) })
}

#[tauri::command]
pub fn practice_undo() -> Result<PracticeState, String> {
    let mut session = PRACTICE.lock().unwrap();
    let session = session.as_mut().ok_or("尚未开始演练")?;
    if session.cursor == 0 {
        return Err("没有可撤销的着法".to_string());
    }
    // 悔棋退回上一局面(非破坏性, 历史保留; 在该局面落子会截断后续旧推演)
    session.cursor -= 1;
    Ok(practice_state(session))
}

#[tauri::command]
pub fn practice_reset() -> Result<PracticeState, String> {
    let mut session = PRACTICE.lock().unwrap();
    let session = session.as_mut().ok_or("尚未开始演练")?;
    // 回到起始局面(非破坏性, 历史保留, 可再点击历史局面回到后续推演)
    session.cursor = 0;
    Ok(practice_state(session))
}

// 跳转到历史中第 index 步之后的局面(index 超界自动收敛), 在任意局面继续推演
#[tauri::command]
pub fn practice_goto(index: usize) -> Result<PracticeState, String> {
    // 引擎思考中跳转: 中断旧局面的搜索, 让位给新局面的分析(引擎空闲时无副作用)
    interrupt_search();
    let mut session = PRACTICE.lock().unwrap();
    let session = session.as_mut().ok_or("尚未开始演练")?;
    session.cursor = index.min(session.history.len());
    Ok(practice_state(session))
}

// 对演练盘当前局面做一次云库优先的分析, 返回行棋方视角的完整提示数据
#[tauri::command]
pub async fn practice_analyze() -> Result<QueryResult, String> {
    let state = SHARED_STATE.get().unwrap();
    let config = state.config.read().unwrap().engine.clone();
    let (fen, board, camp) = with_session(|session| {
        let (board, camp, _) = session.replay();
        (chess::board_fen(&camp, board), board, camp)
    })?;
    let engine = state.engine.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut engine = engine.lock().map_err(|_| "引擎锁不可用".to_string())?;
        block_on(engine.search(&fen, &config)).ok_or_else(|| "分析失败".to_string())
    })
    .await
    .map_err(|e| format!("分析任务失败: {e}"))??;
    match prepare_result(result, board, &camp, None) {
        Some((result, _, _)) => Ok(result),
        None => Err("分析结果缺少着法".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_replay() {
        let positions = vec![
            Position { piece: 'k', pos: "e9".to_string() },
            Position { piece: 'K', pos: "e0".to_string() },
            Position { piece: 'R', pos: "a0".to_string() },
        ];
        let start_board = board_from_positions(&positions).unwrap();
        let mut session = PracticeSession {
            start_board,
            start_camp: Camp::Red,
            history: Vec::new(),
            cursor: 0,
        };
        // 起始局面: 红方行棋, 无历史记谱
        let (board, camp, notations) = session.replay();
        assert_eq!(camp, Camp::Red);
        assert_eq!(board[9][0], 'R');
        assert!(notations.is_empty());
        // 车 a0 -> a5 -> a1 两步, 走完后轮到红方
        session.history.push("a0a5".to_string());
        session.history.push("a5a1".to_string());
        session.cursor = 2;
        let (board, camp, notations) = session.replay();
        assert_eq!(camp, Camp::Red);
        assert_eq!(board[9][0], ' ');
        assert_eq!(board[8][0], 'R');
        assert_eq!(notations.len(), 2);
        // 回退到起始局面: 盘面复原, 历史与记谱保留
        session.cursor = 0;
        let (board, camp, notations) = session.replay();
        assert_eq!(camp, Camp::Red);
        assert_eq!(board[9][0], 'R');
        assert_eq!(notations.len(), 2);
        // 在起始局面换招: 截断旧推演后续写新招法
        session.history.truncate(session.cursor);
        session.history.push("a0a1".to_string());
        session.cursor += 1;
        let (board, camp, notations) = session.replay();
        assert_eq!(camp, Camp::Black);
        assert_eq!(board[9][0], ' ');
        assert_eq!(board[8][0], 'R');
        assert_eq!(session.history.len(), 1);
        assert_eq!(notations.len(), 1);
        // cursor 超界收敛到历史末端
        session.cursor = 99.min(session.history.len());
        assert_eq!(session.cursor, 1);
    }
}
