pub mod chessdb;
use std::fmt::Display;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
mod command;

use tracing::debug;
use tracing::trace;

#[derive(Debug, serde::Serialize, Default, Clone)]
pub struct QueryResult {
    pub depth: usize,            // 深度
    pub score: isize,            // 得分
    pub time: usize,             // 时间
    pub pvs: Vec<String>,        // 最优线完整着法(iccs)
    pub moves: Vec<String>,      // 最优线完整着法(chinese)
    pub alternatives: Vec<String>, // 次优候选首着(iccs)
    pub alt_moves: Vec<String>,  // 次优候选首着(chinese)
    pub state: QueryState,       // 状态
    pub source: String,          // 来源
    pub camp: char,              // 行棋方阵营 'w'/'b'
}

const SOURCE_ENGINE: &str = "引擎";

#[derive(Debug, serde::Serialize, Default, Clone, Copy)]
pub enum QueryState {
    Success,
    #[default]
    NotResult,
    InvalidBoard,
    ServerInternalError, // 内部错误
}

#[derive(Debug, serde::Serialize, Clone, serde::Deserialize, Copy)]
pub struct EngineConfig {
    pub depth: usize,
    pub time: usize,
    pub threads: usize,
    pub hash: usize,
    pub multipv: usize,
    pub show_wdl: bool,
    pub chessdb_enabled: bool,
    pub chessdb_timeout: u64,
    #[serde(default = "default_alt_score_gap")]
    pub alt_score_gap: isize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            depth: 20,
            time: 5000,
            threads: 4,
            hash: 64,
            multipv: 3,
            show_wdl: false,
            chessdb_enabled: true,
            chessdb_timeout: 5,
            alt_score_gap: 300,
        }
    }
}

fn default_alt_score_gap() -> isize {
    300
}

pub struct Engine {
    stdin: Box<dyn Write>,
    stdout: Box<dyn BufRead>,
    child: std::process::Child, // 添加子进程字段
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn new(libs: &Path) -> Self {
        let mut child = command::new(libs);

        let nnue = libs.join("pikafish.nnue");

        let stdin = Box::new(child.stdin.take().unwrap());
        let stdout = Box::new(BufReader::new(child.stdout.take().unwrap()));

        let mut eng = Engine { stdin, stdout, child };
        eng.setoption("EvalFile", nnue.display());
        eng.setoption("Sixty Move Rule", false);
        eng
    }

    pub fn reload(&mut self, libs: &Path, config: &EngineConfig) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        *self = Self::new(libs);
        self.set_hash(config.hash);
        self.set_show_wdl(config.show_wdl);
        self.set_threads(config.threads);
    }

    fn write_command<A: Display>(&mut self, args: A) {
        writeln!(self.stdin, "{}", args).expect("write command error");
        self.stdin.flush().expect("write command flush error");
        debug!("{}", args);
    }

    pub fn set_show_wdl(&mut self, open: bool) { self.setoption("UCI_ShowWDL", open); }

    pub fn set_threads(&mut self, num: usize) { self.setoption("Threads", num); }

    pub fn set_hash(&mut self, size: usize) { self.setoption("Hash", size); }

    pub fn setoption<T: Display>(&mut self, name: &str, value: T) {
        self.write_command(format!("setoption name {} value {}", name, value))
    }

    pub fn position(&mut self, fen: &str) { self.write_command(format!("position fen {}", fen)) }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        trace!("line::{}", line);
        line.trim().to_string()
    }

    // 解析一行 info，返回 (multipv编号, pv序列, depth, score, time)
    fn parse_info(&self, line: &str) -> (Option<usize>, Vec<String>, usize, isize, usize) {
        let mut iter = line.split_whitespace();
        // 跳过 "info"
        iter.next();
        let mut multipv = None;
        let mut pvs = Vec::new();
        let mut depth = 0;
        let mut score = 0;
        let mut time = 0;
        loop {
            let Some(key) = iter.next() else { break };
            match key {
                "depth" => depth = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                "time" => time = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                "multipv" => multipv = iter.next().and_then(|v| v.parse().ok()),
                "score" => match iter.next().unwrap_or("") {
                    "cp" => score = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                    "mate" => {
                        let round: isize = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        score = if round > 0 { 30000 - round } else { -(30000 + round) };
                    }
                    _ => {}
                },
                "pv" => {
                    // pv 是 info 行最后一个字段，收集剩余所有着法
                    pvs.extend(iter.by_ref().map(|s| s.to_string()));
                    break;
                }
                _ => {}
            }
        }
        (multipv, pvs, depth, score, time)
    }

    // 执行引擎搜索，返回解析后的结果（含多候选）
    fn bestmove(&mut self, depth: usize, time: usize, multipv: usize, alt_score_gap: isize) -> QueryResult {
        self.setoption("MultiPV", multipv);
        self.write_command(format!("go depth {} movetime {}", depth, time));

        let mut result = QueryResult::default();
        result.source = SOURCE_ENGINE.to_string();

        // multipv编号 -> 该候选的完整pv序列与分数
        let mut pvs_by_id: std::collections::BTreeMap<usize, (Vec<String>, isize)> = std::collections::BTreeMap::new();
        let mut last_pvs: Vec<String> = Vec::new();

        loop {
            let line = self.read_line();
            if line.starts_with("bestmove") {
                break;
            }
            if !line.starts_with("info") {
                continue;
            }
            let (multipv, pvs, depth, score, time) = self.parse_info(&line);
            if pvs.is_empty() {
                continue;
            }
            last_pvs = pvs.clone();
            match multipv {
                Some(id) => {
                    pvs_by_id.insert(id, (pvs, score));
                    if id == 1 {
                        result.depth = depth;
                        result.score = score;
                        result.time = time;
                    }
                }
                None => {
                    // 无 multipv 字段，视为单候选
                    result.depth = depth;
                    result.score = score;
                    result.time = time;
                }
            }
        }

        // 最优线
        if let Some((pv1, _)) = pvs_by_id.remove(&1) {
            result.pvs = pv1;
        } else if result.pvs.is_empty() {
            result.pvs = last_pvs;
        }
        // 次优候选首着（只保留分数接近最优的好招）
        for (pvs, sc) in pvs_by_id.values() {
            if let Some(first) = pvs.first() {
                if *sc >= result.score - alt_score_gap {
                    result.alternatives.push(first.clone());
                }
            }
        }
        result
    }

    pub async fn search(&mut self, fen: &str, params: &EngineConfig) -> Option<QueryResult> {
        let mut result = if params.chessdb_enabled {
            // 先查询云库
            chessdb::query(fen, params.chessdb_timeout, params.multipv, params.alt_score_gap).await
        } else {
            QueryResult::default()
        };

        match result.state {
            QueryState::Success => {
                // 云库命中最优但缺少次优候选时，用本地引擎补充次优
                if params.multipv > 1 && result.alternatives.is_empty() {
                    let best = result.pvs.first().cloned();
                    let max_alt = params.multipv.saturating_sub(1);
                    self.position(fen);
                    let eng = self.bestmove(params.depth, params.time, params.multipv, params.alt_score_gap);
                    for alt in eng.alternatives {
                        if result.alternatives.len() >= max_alt {
                            break;
                        }
                        if Some(&alt) != best.as_ref() && !result.alternatives.contains(&alt) {
                            result.alternatives.push(alt);
                        }
                    }
                }
                Some(result)
            }
            QueryState::InvalidBoard => None,
            QueryState::ServerInternalError | QueryState::NotResult => {
                // 查询云库失败调用引擎
                self.position(fen);
                Some(self.bestmove(params.depth, params.time, params.multipv, params.alt_score_gap))
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.write_command("quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use std::path;

    use tracing::info;
    use tracing::Level;

    use super::*;
    use crate::logger;

    #[tokio::test]
    async fn test_query() {
        logger::init_tracer(Level::TRACE, &std::path::PathBuf::from("."));
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C2C4/9/RNBAKABNR b";
        let result = chessdb::query(fen, 10, 3, 200).await;
        info!("{:?}", result);
    }

    #[tokio::test]
    async fn test_engine() {
        logger::init_tracer(Level::TRACE, &std::path::PathBuf::from("."));
        let fen = "4k4/9/6r2/9/9/9/9/9/4A4/4K4 w";
        let libs = path::PathBuf::from("f:/w/template/chessboard/libs/pikafish");
        let mut eng = Engine::new(&libs);
        let cfg = EngineConfig { chessdb_enabled: false, ..Default::default() };
        let records = eng.search(fen, &cfg).await;
        info!("{:?}", records);
    }
}
