use crate::chess::Camp;
use crate::chess::get_piece_name;

pub type Board = [[char; 9]; 10];

// 子力价值(×10), 用于判断交换是否划算
pub fn piece_value(piece: char) -> i32 {
    match piece {
        'R' | 'r' => 90,
        'C' | 'c' => 45,
        'N' | 'n' => 40,
        'A' | 'a' | 'B' | 'b' => 20,
        'P' | 'p' => 10,
        'K' | 'k' => 1000,
        _ => 0,
    }
}

fn same_camp(camp: &Camp, piece: char) -> bool {
    match camp {
        Camp::Red => piece.is_ascii_uppercase(),
        Camp::Black => piece.is_ascii_lowercase(),
        Camp::None => false,
    }
}

fn my_king(camp: &Camp) -> char {
    if camp.eq(&Camp::Red) { 'K' } else { 'k' }
}

// 棋子是否在本方九宫内
fn in_palace(piece: char, x: usize, y: usize) -> bool {
    if !(3..=5).contains(&x) {
        return false;
    }
    if piece.is_ascii_uppercase() {
        (7..=9).contains(&y)
    } else {
        (0..=2).contains(&y)
    }
}

// 同一横/竖线上两点之间的棋子数(不含端点)
fn count_between(board: &Board, x: usize, y: usize, tx: usize, ty: usize) -> usize {
    let mut count = 0;
    if y == ty {
        for xx in x.min(tx) + 1..x.max(tx) {
            if board[y][xx] != ' ' {
                count += 1;
            }
        }
    } else {
        for yy in y.min(ty) + 1..y.max(ty) {
            if board[yy][x] != ' ' {
                count += 1;
            }
        }
    }
    count
}

// (x,y) 处棋子是否攻击 (tx,ty)。攻击语义: 炮需恰好一个炮架, 车需通路无阻
pub fn piece_attacks(board: &Board, x: usize, y: usize, tx: usize, ty: usize) -> bool {
    if x == tx && y == ty {
        return false;
    }
    let piece = board[y][x];
    if piece == ' ' {
        return false;
    }
    let dx = tx as i32 - x as i32;
    let dy = ty as i32 - y as i32;
    match piece {
        'R' | 'r' => (x == tx || y == ty) && count_between(board, x, y, tx, ty) == 0,
        'C' | 'c' => (x == tx || y == ty) && count_between(board, x, y, tx, ty) == 1,
        'N' | 'n' => {
            let (adx, ady) = (dx.abs(), dy.abs());
            if !((adx == 1 && ady == 2) || (adx == 2 && ady == 1)) {
                return false;
            }
            // 马腿在直线方向上相邻的格子
            let (lx, ly) =
                if adx == 2 { (x as i32 + dx / 2, y as i32) } else { (x as i32, y as i32 + dy / 2) };
            board[ly as usize][lx as usize] == ' '
        }
        'P' | 'p' => {
            let forward: i32 = if piece.is_ascii_uppercase() { -1 } else { 1 };
            if dx == 0 && dy == forward {
                return true;
            }
            let crossed = if piece.is_ascii_uppercase() { y <= 4 } else { y >= 5 };
            crossed && dy == 0 && dx.abs() == 1
        }
        'K' | 'k' => {
            // 白脸将: 双将同线且中间无子
            let target = board[ty][tx];
            let facing =
                (target == 'K' || target == 'k') && x == tx && count_between(board, x, y, tx, ty) == 0;
            facing || (dx.abs() + dy.abs() == 1 && in_palace(piece, tx, ty))
        }
        'A' | 'a' => dx.abs() == 1 && dy.abs() == 1 && in_palace(piece, tx, ty),
        'B' | 'b' => {
            if dx.abs() != 2 || dy.abs() != 2 {
                return false;
            }
            // 象不能过河
            if piece.is_ascii_uppercase() && ty < 5 {
                return false;
            }
            if !piece.is_ascii_uppercase() && ty > 4 {
                return false;
            }
            let (ex, ey) = (x as i32 + dx / 2, y as i32 + dy / 2);
            board[ey as usize][ex as usize] == ' '
        }
        _ => false,
    }
}

// (tx,ty) 是否被 camp 方任一棋子攻击
pub fn is_attacked(board: &Board, camp: &Camp, tx: usize, ty: usize) -> bool {
    for y in 0..10 {
        for x in 0..9 {
            let piece = board[y][x];
            if piece != ' ' && same_camp(camp, piece) && piece_attacks(board, x, y, tx, ty) {
                return true;
            }
        }
    }
    false
}

// (x,y) 处棋子攻击到的所有格子
pub fn attacked_squares(board: &Board, x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut squares = Vec::new();
    for ty in 0..10 {
        for tx in 0..9 {
            if piece_attacks(board, x, y, tx, ty) {
                squares.push((tx, ty));
            }
        }
    }
    squares
}

fn find_king(board: &Board, king: char) -> Option<(usize, usize)> {
    for y in 0..10 {
        for x in 0..9 {
            if board[y][x] == king {
                return Some((x, y));
            }
        }
    }
    None
}

// 解读一步棋的意图: 解将/吃子/将军/威胁/己方暴露。
// 纯静态判断, 仅供界面展示, 不参与选招。
pub fn explain_move(board: &Board, camp: &Camp, iccs: &str) -> Vec<String> {
    let mut notes = Vec::new();
    if camp.eq(&Camp::None) {
        return notes;
    }
    let mut chars = iccs.chars();
    let fx = chars.next().unwrap() as usize - 97;
    let fy = 57 - chars.next().unwrap() as usize;
    let tx = chars.next().unwrap() as usize - 97;
    let ty = 57 - chars.next().unwrap() as usize;

    let piece = board[fy][fx];
    if piece == ' ' {
        return notes;
    }
    let enemy = camp.opposite();

    // 走之前已被将军, 此步为应将
    if let Some((kx, ky)) = find_king(board, my_king(camp))
        && is_attacked(board, &enemy, kx, ky)
    {
        notes.push("解将".to_string());
    }

    // 吃掉对方棋子
    let captured = board[ty][tx];
    if captured != ' ' {
        let prefix = if tx == 4 && (captured == 'p' || captured == 'P') { "中" } else { "对方" };
        notes.push(format!("吃掉{}{}", prefix, get_piece_name(captured)));
    }

    let mut after = *board;
    after[ty][tx] = piece;
    after[fy][fx] = ' ';

    // 走完后攻击对方将
    let gave_check =
        find_king(&after, my_king(&enemy)).is_some_and(|(kx, ky)| is_attacked(&after, camp, kx, ky));
    if gave_check {
        notes.push("将军".to_string());
    }

    // 威胁: 移动后的棋子攻击到无保护、或子力价值低于己方的敌子
    let enemy_king = my_king(&enemy);
    let mut names: Vec<char> = Vec::new();
    for (sx, sy) in attacked_squares(&after, tx, ty) {
        let target = after[sy][sx];
        if target == ' ' || target == enemy_king || !same_camp(&enemy, target) {
            continue;
        }
        let defended = is_attacked(&after, &enemy, sx, sy);
        if !defended || piece_value(piece) < piece_value(target) {
            let name = get_piece_name(target);
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    for name in names {
        notes.push(format!("威胁对方{}", name));
    }

    // 走完后己方重要棋子无保护且被攻击, 提醒最高值的一个; 兵卒忽略减少噪音, 将军时对方必应将也不提醒
    if !gave_check {
        let mut worst: Option<(i32, char)> = None;
        for y in 0..10 {
            for x in 0..9 {
                let mine = after[y][x];
                if mine == ' ' || mine == my_king(camp) || !same_camp(camp, mine) {
                    continue;
                }
                let value = piece_value(mine);
                if value <= piece_value('P') {
                    continue;
                }
                if is_attacked(&after, &enemy, x, y)
                    && !is_attacked(&after, camp, x, y)
                    && worst.map_or(true, |(v, _)| value > v)
                {
                    worst = Some((value, mine));
                }
            }
        }
        if let Some((_, mine)) = worst {
            notes.push(format!("注意:{}被捉", get_piece_name(mine)));
        }
    }

    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::fen_to_board;

    #[test]
    fn test_cannon_needs_screen() {
        let board = fen_to_board("4k4/9/9/9/9/9/9/4C4/9/4K4 w");
        // 炮对将中间无炮架: 不构成攻击
        assert!(!is_attacked(&board, &Camp::Red, 4, 0));
        let screened = fen_to_board("4k4/9/9/9/4P4/9/9/4C4/9/4K4 w");
        // 恰好一个炮架: 攻击成立
        assert!(is_attacked(&screened, &Camp::Red, 4, 0));
    }

    #[test]
    fn test_knight_leg() {
        // 黑马 (3,7) 攻击红帅 (4,9), 马腿在 (3,8)
        let open = fen_to_board("3k4/9/9/9/9/9/9/3n5/9/4K4 w");
        assert!(is_attacked(&open, &Camp::Black, 4, 9));
        let blocked = fen_to_board("3k4/9/9/9/9/9/9/3n5/3r5/4K4 w");
        assert!(!is_attacked(&blocked, &Camp::Black, 4, 9));
    }

    #[test]
    fn test_explain_capture_and_check() {
        // 车吃中卒并形成同线将军
        let board = fen_to_board("4k4/9/9/9/9/R3p3/9/9/9/4K4 w");
        let notes = explain_move(&board, &Camp::Red, "a4e4");
        assert!(notes.contains(&"吃掉中卒".to_string()));
        assert!(notes.contains(&"将军".to_string()));
    }

    #[test]
    fn test_explain_threat() {
        // 炮平中路上去, 有炮架, 威胁对方无保护的马
        let board = fen_to_board("4k4/9/9/4n4/4P4/9/9/4C4/9/4K4 w");
        let notes = explain_move(&board, &Camp::Red, "e2e4");
        assert!(notes.contains(&"威胁对方马".to_string()));
    }
}
