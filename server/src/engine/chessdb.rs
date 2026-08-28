use std::time::Duration;

use super::QueryResult;
use super::QueryState;

const URL: &str = "http://www.chessdb.cn/chessdb.php";
const REFER: &str = "https://www.chessdb.cn/query/";
const AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.113 Safari/537.36";
const SOURCE_CHESSDB: &str = "云库";
pub async fn query(fen: &str, timeout: u64, multipv: usize, alt_score_gap: isize) -> QueryResult {
    let mut records = super::QueryResult::default();
    let client = reqwest::Client::new();

    // 1. querypv 获取最优线（连续着法）
    let resp = client
        .get(URL)
        .header("User-Agent", AGENT)
        .header("Referer", REFER)
        .query(&[("action", "querypv"), ("board", fen)])
        .timeout(Duration::from_secs(timeout))
        .send()
        .await;

    match resp {
        Ok(resp) => {
            let text = resp.text().await.unwrap();
            let text = text.strip_suffix('\0').unwrap();
            match text {
                "" | "unknown" => records.state = QueryState::NotResult,
                "invalid board" | "checkmate" | "stalemate" => records.state = QueryState::InvalidBoard,
                text => {
                    for pair in text.split(',') {
                        let mut parts = pair.split(':');
                        match parts.next().unwrap_or("") {
                            "score" => records.score = parts.next().unwrap().parse().unwrap_or(0),
                            "depth" => records.depth = parts.next().unwrap().parse().unwrap_or(0),
                            "pv" => {
                                let pv_text = parts.next().unwrap();
                                records.pvs = pv_text.split('|').map(|s| s.to_string()).collect();
                            }
                            _ => {}
                        }
                    }
                    records.state = QueryState::Success;
                    records.source = SOURCE_CHESSDB.to_string();
                }
            }
        }
        Err(e) => {
            println!("{}", e);
            records.state = QueryState::ServerInternalError
        }
    }

    // 2. queryall 获取次优候选首着（仅当需要多候选时）
    if matches!(records.state, QueryState::Success) && multipv > 1 {
        let resp = client
            .get(URL)
            .header("User-Agent", AGENT)
            .header("Referer", REFER)
            .query(&[("action", "queryall"), ("board", fen)])
            .timeout(Duration::from_secs(timeout))
            .send()
            .await;

        match resp {
            Ok(resp) => {
                let text = resp.text().await.unwrap();
                let text = text.strip_suffix('\0').unwrap();
                tracing::debug!("queryall raw: {}", text);
                let best = records.pvs.first().cloned();
                let max_alt = multipv.saturating_sub(1);
                for item in text.split('|') {
                    if records.alternatives.len() >= max_alt {
                        break;
                    }
                    if let Some(rest) = item.strip_prefix("move:") {
                        // rest 形如 "xxx,score:yyy,rank:z"
                        let mut parts = rest.split(',');
                        let mv = parts.next().unwrap_or("").to_string();
                        let mut sc: Option<isize> = None;
                        for part in parts {
                            if let Some(v) = part.strip_prefix("score:") {
                                sc = v.parse().ok();
                            }
                        }
                        if mv.is_empty() || Some(&mv) == best.as_ref() {
                            continue;
                        }
                        // 只保留分数接近最优的好招（score 缺失时不过滤）
                        if sc.map_or(true, |s| s >= records.score - alt_score_gap) {
                            records.alternatives.push(mv);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("queryall error: {}", e);
            }
        }
    }

    records
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_query() {
    let result = query("2baka3/9/n3b4/r1p1nr2p/4R4/2PR5/4P3P/4B1N2/4A4/2B1KA3 w", 10, 3, 200).await;
    println!("{:?}", result);
}
