use crate::chess::Camp;

pub type Board = [[char; 9]; 10];

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

// (x,y) 处棋子走到 (tx,ty) 是否符合走法规则(不含送将检查)。
// 与攻击语义的差异仅在炮: 走空格不能有炮架, 吃子需恰好一个炮架。
fn pseudo_legal_move(board: &Board, camp: &Camp, x: usize, y: usize, tx: usize, ty: usize) -> bool {
    if x == tx && y == ty {
        return false;
    }
    let piece = board[y][x];
    if piece == ' ' {
        return false;
    }
    let target = board[ty][tx];
    if target != ' ' && same_camp(camp, target) {
        return false;
    }
    if matches!(piece, 'C' | 'c') {
        if x != tx && y != ty {
            return false;
        }
        let screens = count_between(board, x, y, tx, ty);
        return if target == ' ' { screens == 0 } else { screens == 1 };
    }
    piece_attacks(board, x, y, tx, ty)
}

fn iccs(x: usize, y: usize, tx: usize, ty: usize) -> String {
    let files = b"abcdefghi";
    format!("{}{}{}{}", files[x] as char, 9 - y, files[tx] as char, 9 - ty)
}

// camp 方所有合法着法(iccs): 走完后己方将不被攻击(含将帅对脸)。
// 局面缺将(识别异常)时跳过送将检查。
pub fn legal_moves(board: &Board, camp: &Camp) -> Vec<String> {
    let mut moves = Vec::new();
    for y in 0..10 {
        for x in 0..9 {
            let piece = board[y][x];
            if piece == ' ' || !same_camp(camp, piece) {
                continue;
            }
            for ty in 0..10 {
                for tx in 0..9 {
                    if !pseudo_legal_move(board, camp, x, y, tx, ty) {
                        continue;
                    }
                    let mut next = *board;
                    next[ty][tx] = piece;
                    next[y][x] = ' ';
                    let safe = match find_king(&next, my_king(camp)) {
                        Some((kx, ky)) => !is_attacked(&next, &camp.opposite(), kx, ky),
                        None => true,
                    };
                    if safe {
                        moves.push(iccs(x, y, tx, ty));
                    }
                }
            }
        }
    }
    moves
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
    fn test_startpos_legal_moves() {
        let board = fen_to_board("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w");
        let moves = legal_moves(&board, &Camp::Red);
        // 开局公认合法着法数: 兵5 炮24 马4 车4 相4 仕2 帅1
        assert_eq!(moves.len(), 44);
        assert!(moves.contains(&"b0c2".to_string()));
        assert!(moves.contains(&"a0a1".to_string()));
        // 不能吃自己的马
        assert!(!moves.contains(&"a0b0".to_string()));
    }

    #[test]
    fn test_cannon_move_and_capture() {
        // 炮 e3: 平移无阻, 上行有 P 作炮架可吃 r, 无炮架不可移动到炮架上
        let board = fen_to_board("4k4/9/9/4r4/9/4P4/4C4/9/9/4K4 w");
        let moves = legal_moves(&board, &Camp::Red);
        assert!(moves.contains(&"e3e6".to_string()));
        assert!(!moves.contains(&"e3e5".to_string()));
        assert!(!moves.contains(&"e3e4".to_string()));
    }

    #[test]
    fn test_facing_king_forbidden() {
        // 黑将在 d9(3,0), 红帅 e0(4,9): 帅平到 d0 会形成对脸, 被禁止
        let board = fen_to_board("3k4/9/9/9/9/9/9/9/9/4K4 w");
        let moves = legal_moves(&board, &Camp::Red);
        assert!(moves.contains(&"e0f0".to_string()));
        assert!(moves.contains(&"e0e1".to_string()));
        assert!(!moves.contains(&"e0d0".to_string()));
    }
}
