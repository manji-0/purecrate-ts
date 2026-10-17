// Chess rules as pure functions: FEN in and out, legal moves, the position
// after a move, game status, UCI move text, and perft.
//
// Squares are numbered 0..64 from a1 (0) to h8 (63): `file + 8 * rank`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceKind,
}

/// One square of the board. An array of `Option<Piece>` is not in the
/// subset, so the empty square is a variant of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Square {
    Empty,
    Occupied(Piece),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

/// A position that came from a valid FEN or from legal moves on one: one king
/// per side, no pawn on the first or last rank, the side not to move not in
/// check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    board: [Square; 64],
    side: Color,
    castling: CastlingRights,
    en_passant: Option<u8>,
    halfmove: u32,
    fullmove: u32,
}

/// A move as UCI writes it: from, to, and the piece a pawn becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    from: u8,
    to: u8,
    promotion: Option<PieceKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenError {
    FieldCount,
    BadPiece(char),
    BadRank,
    BadSide,
    BadCastling,
    BadEnPassant,
    BadHalfmove,
    BadFullmove,
    KingCount,
    PawnOnBackRank,
    OpponentInCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    SquareOutOfRange,
    BadPromotion,
    Syntax,
    Illegal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ongoing,
    Checkmate,
    Stalemate,
    FiftyMoveRule,
    InsufficientMaterial,
}

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

// Steps of a king as (files, ranks), in order: E, NE, N, NW, W, SW, S, SE.
// Even indices are the rook's directions, odd ones the bishop's.
const KING: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const KNIGHT: [(i32, i32); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];

impl Move {
    pub fn new(from: u8, to: u8, promotion: Option<PieceKind>) -> Result<Move, MoveError> {
        if from >= 64 || to >= 64 {
            return Err(MoveError::SquareOutOfRange);
        }
        if matches!(promotion, Some(PieceKind::Pawn | PieceKind::King)) {
            return Err(MoveError::BadPromotion);
        }
        Ok(Move { from, to, promotion })
    }

    pub fn from(&self) -> u8 {
        self.from
    }

    pub fn to(&self) -> u8 {
        self.to
    }

    pub fn promotion(&self) -> Option<PieceKind> {
        self.promotion
    }
}

impl Position {
    pub fn piece_at(&self, square: u8) -> Option<Piece> {
        if square >= 64 {
            return None;
        }
        match self.board[usize::from(square)] {
            Square::Occupied(p) => Some(p),
            Square::Empty => None,
        }
    }

    pub fn side_to_move(&self) -> Color {
        self.side
    }

    pub fn castling(&self) -> CastlingRights {
        self.castling
    }

    pub fn en_passant(&self) -> Option<u8> {
        self.en_passant
    }

    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove
    }

    pub fn fullmove_number(&self) -> u32 {
        self.fullmove
    }
}

fn other(c: Color) -> Color {
    match c {
        Color::White => Color::Black,
        Color::Black => Color::White,
    }
}

/// The square `step.0` files and `step.1` ranks away, if it is on the board.
fn offset(square: u8, step: (i32, i32)) -> Option<u8> {
    let f = i32::from(square % 8) + step.0;
    let r = i32::from(square / 8) + step.1;
    if f < 0 || f > 7 || r < 0 || r > 7 {
        None
    } else {
        Some((r * 8 + f) as u8)
    }
}

fn holds(board: &[Square; 64], at: Option<u8>, color: Color, kind: PieceKind) -> bool {
    match at {
        Some(s) => board[usize::from(s)] == Square::Occupied(Piece { color, kind }),
        None => false,
    }
}

/// Whether a piece of `by` attacks `square`.
fn attacked(board: &[Square; 64], square: u8, by: Color) -> bool {
    let pawn_dr = match by {
        Color::White => -1,
        Color::Black => 1,
    };
    if holds(board, offset(square, (-1, pawn_dr)), by, PieceKind::Pawn)
        || holds(board, offset(square, (1, pawn_dr)), by, PieceKind::Pawn)
    {
        return true;
    }
    for d in 0..8 {
        if holds(board, offset(square, KNIGHT[d]), by, PieceKind::Knight)
            || holds(board, offset(square, KING[d]), by, PieceKind::King)
        {
            return true;
        }
        let mut cur = offset(square, KING[d]);
        while let Some(s) = cur {
            match board[usize::from(s)] {
                Square::Empty => cur = offset(s, KING[d]),
                Square::Occupied(p) => {
                    if p.color == by
                        && (p.kind == PieceKind::Queen
                            || (d % 2 == 0 && p.kind == PieceKind::Rook)
                            || (d % 2 == 1 && p.kind == PieceKind::Bishop))
                    {
                        return true;
                    }
                    cur = None;
                }
            }
        }
    }
    false
}

fn king_square(board: &[Square; 64], color: Color) -> Option<u8> {
    let king = Square::Occupied(Piece { color, kind: PieceKind::King });
    board.iter().position(|c| *c == king).map(|i| i as u8)
}

fn king_attacked(board: &[Square; 64], color: Color) -> bool {
    match king_square(board, color) {
        Some(k) => attacked(board, k, other(color)),
        None => false,
    }
}

/// The squares a knight or king on `from` may step to: empty or an enemy.
fn step_targets(board: &[Square; 64], from: u8, us: Color, steps: &[(i32, i32)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for step in steps {
        if let Some(s) = offset(from, *step) {
            match board[usize::from(s)] {
                Square::Empty => out.push(s),
                Square::Occupied(p) => {
                    if p.color != us {
                        out.push(s);
                    }
                }
            }
        }
    }
    out
}

/// The squares a slider on `from` reaches along directions `first`,
/// `first + step`, .. of the king's eight.
fn slide_targets(board: &[Square; 64], from: u8, us: Color, first: usize, step: usize) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for d in (first..8).step_by(step) {
        let mut cur = offset(from, KING[d]);
        while let Some(s) = cur {
            match board[usize::from(s)] {
                Square::Empty => {
                    out.push(s);
                    cur = offset(s, KING[d]);
                }
                Square::Occupied(p) => {
                    if p.color != us {
                        out.push(s);
                    }
                    cur = None;
                }
            }
        }
    }
    out
}

fn pawn_moves(pos: &Position, from: u8) -> Vec<Move> {
    let us = pos.side;
    let (dr, start_rank, last_rank): (i32, u8, u8) = match us {
        Color::White => (1, 1, 7),
        Color::Black => (-1, 6, 0),
    };
    let mut targets: Vec<u8> = Vec::new();
    if let Some(one) = offset(from, (0, dr)) {
        if pos.board[usize::from(one)] == Square::Empty {
            targets.push(one);
            if from / 8 == start_rank {
                if let Some(two) = offset(one, (0, dr)) {
                    if pos.board[usize::from(two)] == Square::Empty {
                        targets.push(two);
                    }
                }
            }
        }
    }
    for df in [-1, 1] {
        if let Some(s) = offset(from, (df, dr)) {
            let enemy = match pos.board[usize::from(s)] {
                Square::Occupied(p) => p.color != us,
                Square::Empty => pos.en_passant == Some(s),
            };
            if enemy {
                targets.push(s);
            }
        }
    }
    let mut out: Vec<Move> = Vec::new();
    for to in &targets {
        if to / 8 == last_rank {
            for kind in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
                out.push(Move { from, to: *to, promotion: Some(kind) });
            }
        } else {
            out.push(Move { from, to: *to, promotion: None });
        }
    }
    out
}

fn castling_moves(pos: &Position) -> Vec<Move> {
    let us = pos.side;
    let them = other(us);
    let b = &pos.board;
    let (base, kingside, queenside): (u8, bool, bool) = match us {
        Color::White => (0, pos.castling.white_kingside, pos.castling.white_queenside),
        Color::Black => (56, pos.castling.black_kingside, pos.castling.black_queenside),
    };
    let mut out: Vec<Move> = Vec::new();
    let king = Square::Occupied(Piece { color: us, kind: PieceKind::King });
    let rook = Square::Occupied(Piece { color: us, kind: PieceKind::Rook });
    if b[usize::from(base + 4)] != king || attacked(b, base + 4, them) {
        return out;
    }
    if kingside
        && b[usize::from(base + 7)] == rook
        && b[usize::from(base + 5)] == Square::Empty
        && b[usize::from(base + 6)] == Square::Empty
        && !attacked(b, base + 5, them)
        && !attacked(b, base + 6, them)
    {
        out.push(Move { from: base + 4, to: base + 6, promotion: None });
    }
    if queenside
        && b[usize::from(base)] == rook
        && b[usize::from(base + 1)] == Square::Empty
        && b[usize::from(base + 2)] == Square::Empty
        && b[usize::from(base + 3)] == Square::Empty
        && !attacked(b, base + 3, them)
        && !attacked(b, base + 2, them)
    {
        out.push(Move { from: base + 4, to: base + 2, promotion: None });
    }
    out
}

/// Moves that follow each piece's rules, before the own king is considered.
fn pseudo_moves(pos: &Position) -> Vec<Move> {
    let us = pos.side;
    let mut out: Vec<Move> = Vec::new();
    for from in 0..64u8 {
        let Square::Occupied(p) = pos.board[usize::from(from)] else {
            continue;
        };
        if p.color != us {
            continue;
        }
        let targets = match p.kind {
            PieceKind::Pawn => {
                for m in &pawn_moves(pos, from) {
                    out.push(*m);
                }
                continue;
            }
            PieceKind::Knight => step_targets(&pos.board, from, us, &KNIGHT),
            PieceKind::King => step_targets(&pos.board, from, us, &KING),
            PieceKind::Bishop => slide_targets(&pos.board, from, us, 1, 2),
            PieceKind::Rook => slide_targets(&pos.board, from, us, 0, 2),
            PieceKind::Queen => slide_targets(&pos.board, from, us, 0, 1),
        };
        for to in &targets {
            out.push(Move { from, to: *to, promotion: None });
        }
    }
    for m in &castling_moves(pos) {
        out.push(*m);
    }
    out
}

/// The position after `mv`, which should be one of `legal_moves(pos)`. Any
/// other move still gives a position, moving whatever stands on `from`.
pub fn make_move(pos: &Position, mv: Move) -> Position {
    let us = pos.side;
    let mut b = pos.board;
    let from = usize::from(mv.from);
    let to = usize::from(mv.to);
    let moving = b[from];
    let captured = b[to];
    let (is_pawn, is_king) = match moving {
        Square::Occupied(p) => (p.kind == PieceKind::Pawn, p.kind == PieceKind::King),
        Square::Empty => (false, false),
    };
    if is_pawn && captured == Square::Empty && pos.en_passant == Some(mv.to) && mv.from % 8 != mv.to % 8 {
        b[usize::from(mv.from / 8 * 8 + mv.to % 8)] = Square::Empty;
    }
    b[to] = match mv.promotion {
        Some(kind) => Square::Occupied(Piece { color: us, kind }),
        None => moving,
    };
    b[from] = Square::Empty;
    if is_king && mv.from % 8 == 4 && mv.to % 8 == 6 {
        b[to - 1] = b[to + 1];
        b[to + 1] = Square::Empty;
    }
    if is_king && mv.from % 8 == 4 && mv.to % 8 == 2 {
        b[to + 1] = b[to - 2];
        b[to - 2] = Square::Empty;
    }
    let c = pos.castling;
    let touched = |s: u8| mv.from == s || mv.to == s;
    let castling = CastlingRights {
        white_kingside: c.white_kingside && !touched(7) && !touched(4),
        white_queenside: c.white_queenside && !touched(0) && !touched(4),
        black_kingside: c.black_kingside && !touched(63) && !touched(60),
        black_queenside: c.black_queenside && !touched(56) && !touched(60),
    };
    let en_passant =
        if is_pawn && (mv.from + 16 == mv.to || mv.to + 16 == mv.from) { Some((mv.from + mv.to) / 2) } else { None };
    let halfmove = if is_pawn || captured != Square::Empty { 0 } else { pos.halfmove.saturating_add(1) };
    let fullmove = match us {
        Color::White => pos.fullmove,
        Color::Black => pos.fullmove.saturating_add(1),
    };
    Position { board: b, side: other(us), castling, en_passant, halfmove, fullmove }
}

/// Every legal move, in board order from a1 and castling last.
pub fn legal_moves(pos: &Position) -> Vec<Move> {
    let us = pos.side;
    pseudo_moves(pos).iter().copied().filter(|m| !king_attacked(&make_move(pos, *m).board, us)).collect()
}

pub fn is_check(pos: &Position) -> bool {
    king_attacked(&pos.board, pos.side)
}

fn insufficient_material(board: &[Square; 64]) -> bool {
    let mut knights = 0u32;
    let mut light_bishops = 0u32;
    let mut dark_bishops = 0u32;
    for s in 0..64u8 {
        if let Square::Occupied(p) = board[usize::from(s)] {
            match p.kind {
                PieceKind::King => {}
                PieceKind::Knight => knights += 1,
                PieceKind::Bishop => {
                    if (s % 8 + s / 8) % 2 == 0 {
                        dark_bishops += 1;
                    } else {
                        light_bishops += 1;
                    }
                }
                PieceKind::Pawn | PieceKind::Rook | PieceKind::Queen => return false,
            }
        }
    }
    (knights == 0 && (light_bishops == 0 || dark_bishops == 0))
        || (knights == 1 && light_bishops == 0 && dark_bishops == 0)
}

/// Mate and stalemate first; then the fifty-move rule and dead positions.
pub fn game_status(pos: &Position) -> Status {
    if legal_moves(pos).is_empty() {
        if is_check(pos) {
            Status::Checkmate
        } else {
            Status::Stalemate
        }
    } else if pos.halfmove >= 100 {
        Status::FiftyMoveRule
    } else if insufficient_material(&pos.board) {
        Status::InsufficientMaterial
    } else {
        Status::Ongoing
    }
}

/// Leaf nodes of the legal move tree `depth` plies deep.
pub fn perft(pos: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = legal_moves(pos);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut n = 0u64;
    for m in &moves {
        n += perft(&make_move(pos, *m), depth - 1);
    }
    n
}

// ---- FEN ----

fn piece_from_char(c: char) -> Option<Piece> {
    let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
    let kind = match c.to_ascii_lowercase() {
        'p' => PieceKind::Pawn,
        'n' => PieceKind::Knight,
        'b' => PieceKind::Bishop,
        'r' => PieceKind::Rook,
        'q' => PieceKind::Queen,
        'k' => PieceKind::King,
        _ => return None,
    };
    Some(Piece { color, kind })
}

fn kind_char(kind: PieceKind) -> char {
    match kind {
        PieceKind::Pawn => 'p',
        PieceKind::Knight => 'n',
        PieceKind::Bishop => 'b',
        PieceKind::Rook => 'r',
        PieceKind::Queen => 'q',
        PieceKind::King => 'k',
    }
}

fn piece_char(p: Piece) -> char {
    match p.color {
        Color::White => kind_char(p.kind).to_ascii_uppercase(),
        Color::Black => kind_char(p.kind),
    }
}

fn parse_board(text: &str) -> Result<[Square; 64], FenError> {
    let mut board = [Square::Empty; 64];
    let mut rank: i32 = 7;
    let mut file: i32 = 0;
    for c in text.chars() {
        if c == '/' {
            if file != 8 || rank == 0 {
                return Err(FenError::BadRank);
            }
            rank -= 1;
            file = 0;
        } else if matches!(c, '1'..='8') {
            file += (u32::from(c) - u32::from('0')) as i32;
            if file > 8 {
                return Err(FenError::BadRank);
            }
        } else {
            let p = piece_from_char(c).ok_or(FenError::BadPiece(c))?;
            if file >= 8 {
                return Err(FenError::BadRank);
            }
            board[(rank * 8 + file) as usize] = Square::Occupied(p);
            file += 1;
        }
    }
    if rank != 0 || file != 8 {
        return Err(FenError::BadRank);
    }
    Ok(board)
}

fn parse_castling(text: &str) -> Result<CastlingRights, FenError> {
    let none =
        CastlingRights { white_kingside: false, white_queenside: false, black_kingside: false, black_queenside: false };
    if text == "-" {
        return Ok(none);
    }
    let mut seen: Vec<char> = Vec::new();
    for c in text.chars() {
        if !matches!(c, 'K' | 'Q' | 'k' | 'q') || seen.contains(&c) {
            return Err(FenError::BadCastling);
        }
        seen.push(c);
    }
    Ok(CastlingRights {
        white_kingside: seen.contains(&'K'),
        white_queenside: seen.contains(&'Q'),
        black_kingside: seen.contains(&'k'),
        black_queenside: seen.contains(&'q'),
    })
}

/// The square named by a file byte `a`..`h` and a rank byte `1`..`8`.
fn square_of(file: u8, rank: u8) -> Option<u8> {
    if !matches!(file, b'a'..=b'h') || !matches!(rank, b'1'..=b'8') {
        return None;
    }
    Some((rank - b'1') * 8 + (file - b'a'))
}

fn square_name(square: u8) -> String {
    let file = char::from(b'a' + square % 8);
    let rank = char::from(b'1' + square / 8);
    format!("{file}{rank}")
}

pub fn parse_fen(text: &str) -> Result<Position, FenError> {
    let fields: Vec<&str> = text.split_whitespace().collect();
    if fields.len() != 6 {
        return Err(FenError::FieldCount);
    }
    let board = parse_board(fields[0])?;
    let side = match fields[1] {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err(FenError::BadSide),
    };
    let castling = parse_castling(fields[2])?;
    let en_passant = if fields[3] == "-" {
        None
    } else {
        let b = fields[3].as_bytes();
        if b.len() != 2 {
            return Err(FenError::BadEnPassant);
        }
        let s = square_of(b[0], b[1]).ok_or(FenError::BadEnPassant)?;
        let want = match side {
            Color::White => 5,
            Color::Black => 2,
        };
        if s / 8 != want {
            return Err(FenError::BadEnPassant);
        }
        Some(s)
    };
    let halfmove = fields[4].parse::<u32>().map_err(|_| FenError::BadHalfmove)?;
    let fullmove = fields[5].parse::<u32>().map_err(|_| FenError::BadFullmove)?;
    let mut white_kings = 0u32;
    let mut black_kings = 0u32;
    for s in 0..64u8 {
        if let Square::Occupied(p) = board[usize::from(s)] {
            if p.kind == PieceKind::Pawn && (s / 8 == 0 || s / 8 == 7) {
                return Err(FenError::PawnOnBackRank);
            }
            if p.kind == PieceKind::King {
                match p.color {
                    Color::White => white_kings += 1,
                    Color::Black => black_kings += 1,
                }
            }
        }
    }
    if white_kings != 1 || black_kings != 1 {
        return Err(FenError::KingCount);
    }
    if king_attacked(&board, other(side)) {
        return Err(FenError::OpponentInCheck);
    }
    Ok(Position { board, side, castling, en_passant, halfmove, fullmove })
}

pub fn start_position() -> Position {
    match parse_fen(START_FEN) {
        Ok(p) => p,
        Err(_) => unreachable!(),
    }
}

pub fn to_fen(pos: &Position) -> String {
    let mut s = String::new();
    for rank in (0..8u8).rev() {
        let mut empty = 0u32;
        for file in 0..8u8 {
            match pos.board[usize::from(rank * 8 + file)] {
                Square::Empty => empty += 1,
                Square::Occupied(p) => {
                    if empty > 0 {
                        s.push_str(&empty.to_string());
                        empty = 0;
                    }
                    s.push(piece_char(p));
                }
            }
        }
        if empty > 0 {
            s.push_str(&empty.to_string());
        }
        if rank > 0 {
            s.push('/');
        }
    }
    let side = match pos.side {
        Color::White => 'w',
        Color::Black => 'b',
    };
    let c = pos.castling;
    let mut rights = String::new();
    if c.white_kingside {
        rights.push('K');
    }
    if c.white_queenside {
        rights.push('Q');
    }
    if c.black_kingside {
        rights.push('k');
    }
    if c.black_queenside {
        rights.push('q');
    }
    if rights.is_empty() {
        rights.push('-');
    }
    let ep = match pos.en_passant {
        Some(sq) => square_name(sq),
        None => String::from("-"),
    };
    let half = pos.halfmove;
    let full = pos.fullmove;
    format!("{s} {side} {rights} {ep} {half} {full}")
}

// ---- UCI ----

pub fn move_to_uci(mv: Move) -> String {
    let mut s = square_name(mv.from);
    s.push_str(&square_name(mv.to));
    if let Some(kind) = mv.promotion {
        s.push(kind_char(kind));
    }
    s
}

/// Reads `e2e4` or `e7e8q` without a position.
pub fn parse_uci(text: &str) -> Result<Move, MoveError> {
    let b = text.as_bytes();
    if b.len() != 4 && b.len() != 5 {
        return Err(MoveError::Syntax);
    }
    let from = square_of(b[0], b[1]).ok_or(MoveError::Syntax)?;
    let to = square_of(b[2], b[3]).ok_or(MoveError::Syntax)?;
    let promotion = if b.len() == 5 {
        match b[4] {
            b'q' => Some(PieceKind::Queen),
            b'r' => Some(PieceKind::Rook),
            b'b' => Some(PieceKind::Bishop),
            b'n' => Some(PieceKind::Knight),
            _ => return Err(MoveError::BadPromotion),
        }
    } else {
        None
    };
    Ok(Move { from, to, promotion })
}

/// Plays a move given as UCI text, if it is legal in `pos`.
pub fn play_uci(pos: &Position, text: &str) -> Result<Position, MoveError> {
    let mv = parse_uci(text)?;
    if legal_moves(pos).contains(&mv) {
        Ok(make_move(pos, mv))
    } else {
        Err(MoveError::Illegal)
    }
}
