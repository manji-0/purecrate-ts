//! `examples/chess`, the rules of chess (FIDE Laws, articles 3 and 5, and
//! 9.3's fifty-move rule) as a game server and its client must agree on
//! them, with FEN and UCI move text:
//!
//! - the published perft counts come out as published (Chess Programming
//!   Wiki, "Perft Results": the start position, Kiwipete, and positions 3
//!   to 6), each FEN prints back as read, and the rules the counts rest on
//!   are asserted one by one: castling out of, through, and into check, en
//!   passant with the capturing pawn pinned along the rank, promotion to
//!   each of four pieces, mate, stalemate, the fifty-move rule, dead
//!   positions, every FEN error, and UCI text both ways;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: `[Option<Piece>; 64]`, `&mut self` move making
//!   and move lists, iterator adaptors, `split('/')`, slice patterns, a
//!   bit set for castling rights) agree on random games from the start
//!   position, the five perft positions and three endgames (legal moves,
//!   check, status, FEN after every move, perft 2), on FEN text with one
//!   character changed or dropped, and on every UCI string over a small
//!   alphabet;
//! - the generated package agrees with Rust on a sample of each: the legal
//!   move list, status, check, accessors, and FEN after each legal move of
//!   positions along those games, the FEN and UCI errors, and perft 1 to 3.

use crate::support::{self, Js, Rng};

/// The crate and its driver, with `Show` and `Js` written out by hand as
/// `purecrate_canon::fixture!` would derive them: the fixture also derives
/// `serde::Serialize` for every struct, and serde has none for an array
/// longer than 32, so `Position`'s `[Square; 64]` does not compile there.
#[allow(dead_code, clippy::all)]
mod chess {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/../../../examples/chess/src/lib.rs"));
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chess_driver.rs"));

    pub const SOURCE: &str = concat!(
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/../../../examples/chess/src/lib.rs")),
        "\n",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chess_driver.rs")),
    );

    use crate::support::{Js, Show};

    /// `E::V` and `({ kind: "V" })`, or with one field `E::V(x)` and
    /// `({ kind: "V", value: x })`.
    macro_rules! enum_canon {
        ($t:ident: $($v:ident $(($x:ident))?),* $(,)?) => {
            impl Show for $t {
                fn show(&self) -> String {
                    match self {
                        $($t::$v $(($x))? => {
                            let text = concat!(stringify!($t), "::", stringify!($v)).to_string();
                            $(let text = format!("{text}({})", $x.show());)?
                            text
                        })*
                    }
                }
            }
            impl Js for $t {
                fn js(&self) -> String {
                    match self {
                        $($t::$v $(($x))? => {
                            let text = concat!("kind: \"", stringify!($v), "\"").to_string();
                            $(let text = format!("{text}, value: {}", $x.js());)?
                            format!("({{ {text} }})")
                        })*
                    }
                }
            }
        };
    }

    /// `S { a: x, b: y }` and `({ a: x, b: y })`.
    macro_rules! struct_canon {
        ($t:ident { $($f:ident),* $(,)? }) => {
            impl Show for $t {
                fn show(&self) -> String {
                    let parts: Vec<String> = vec![$(format!("{}: {}", stringify!($f), self.$f.show())),*];
                    format!("{} {{ {} }}", stringify!($t), parts.join(", "))
                }
            }
            impl Js for $t {
                fn js(&self) -> String {
                    let parts: Vec<String> = vec![$(format!("{}: {}", stringify!($f), self.$f.js())),*];
                    format!("({{ {} }})", parts.join(", "))
                }
            }
        };
    }

    enum_canon!(Color: White, Black);
    enum_canon!(PieceKind: Pawn, Knight, Bishop, Rook, Queen, King);
    enum_canon!(Square: Empty, Occupied(p));
    enum_canon!(FenError: FieldCount, BadPiece(c), BadRank, BadSide, BadCastling, BadEnPassant, BadHalfmove, BadFullmove, KingCount, PawnOnBackRank, OpponentInCheck);
    enum_canon!(MoveError: SquareOutOfRange, BadPromotion, Syntax, Illegal);
    enum_canon!(Status: Ongoing, Checkmate, Stalemate, FiftyMoveRule, InsufficientMaterial);
    struct_canon!(Piece { color, kind });
    struct_canon!(CastlingRights { white_kingside, white_queenside, black_kingside, black_queenside });
    struct_canon!(Position { board, side, castling, en_passant, halfmove, fullmove });
    struct_canon!(Move { from, to, promotion });
}

/// Chess as one would write it with no constraint: an array of `Option`,
/// moves pushed onto one `&mut Vec`, a move made in place, castling rights
/// as bits, iterators and slice patterns. Not converted; the reference
/// only. Errors and statuses carry the constrained side's names, so `Debug`
/// compares them; positions compare by FEN and moves by UCI text.
mod idiomatic {
    use PieceKind::*;

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Move {
        pub from: u8,
        pub to: u8,
        pub promotion: Option<PieceKind>,
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

    /// White kingside, white queenside, black kingside, black queenside;
    /// the letters FEN writes, and the squares whose move or capture clears
    /// each bit.
    const RIGHTS: [(u8, char, [u8; 2]); 4] =
        [(1, 'K', [4, 7]), (2, 'Q', [4, 0]), (4, 'k', [60, 63]), (8, 'q', [60, 56])];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Position {
        board: [Option<Piece>; 64],
        side: Color,
        castling: u8,
        en_passant: Option<u8>,
        halfmove: u32,
        fullmove: u32,
    }

    const KING: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
    const KNIGHT: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];

    impl Color {
        fn other(self) -> Color {
            match self {
                Color::White => Color::Black,
                Color::Black => Color::White,
            }
        }
    }

    impl Move {
        pub fn new(from: u8, to: u8, promotion: Option<PieceKind>) -> Result<Move, MoveError> {
            if from >= 64 || to >= 64 {
                return Err(MoveError::SquareOutOfRange);
            }
            if matches!(promotion, Some(Pawn | King)) {
                return Err(MoveError::BadPromotion);
            }
            Ok(Move { from, to, promotion })
        }
    }

    fn offset(square: u8, (df, dr): (i8, i8)) -> Option<u8> {
        let (f, r) = ((square % 8) as i8 + df, (square / 8) as i8 + dr);
        ((0..8).contains(&f) && (0..8).contains(&r)).then(|| (r * 8 + f) as u8)
    }

    impl Position {
        pub fn piece_at(&self, square: u8) -> Option<Piece> {
            self.board.get(usize::from(square)).copied().flatten()
        }

        pub fn side_to_move(&self) -> Color {
            self.side
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

        fn holds(&self, at: Option<u8>, color: Color, kind: PieceKind) -> bool {
            at.and_then(|s| self.board[s as usize]) == Some(Piece { color, kind })
        }

        fn attacked(&self, square: u8, by: Color) -> bool {
            let dr = if by == Color::White { -1 } else { 1 };
            [-1, 1].iter().any(|&df| self.holds(offset(square, (df, dr)), by, Pawn))
                || KNIGHT.iter().any(|&d| self.holds(offset(square, d), by, Knight))
                || KING.iter().any(|&d| self.holds(offset(square, d), by, King))
                || KING.iter().enumerate().any(|(i, &d)| {
                    let mut s = square;
                    while let Some(n) = offset(s, d) {
                        if let Some(p) = self.board[n as usize] {
                            let slider = if i % 2 == 0 { Rook } else { Bishop };
                            return p.color == by && (p.kind == Queen || p.kind == slider);
                        }
                        s = n;
                    }
                    false
                })
        }

        fn king_attacked(&self, color: Color) -> bool {
            let king = Some(Piece { color, kind: King });
            self.board.iter().position(|&p| p == king).is_some_and(|k| self.attacked(k as u8, color.other()))
        }

        fn push_targets(&self, from: u8, dirs: &[(i8, i8)], slide: bool, out: &mut Vec<Move>) {
            for &d in dirs {
                let mut s = from;
                while let Some(n) = offset(s, d) {
                    match self.board[n as usize] {
                        None => out.push(Move { from, to: n, promotion: None }),
                        Some(p) => {
                            if p.color != self.side {
                                out.push(Move { from, to: n, promotion: None });
                            }
                            break;
                        }
                    }
                    if !slide {
                        break;
                    }
                    s = n;
                }
            }
        }

        fn pawn_moves(&self, from: u8, out: &mut Vec<Move>) {
            let (dr, start, last) = if self.side == Color::White { (1, 1, 7) } else { (-1, 6, 0) };
            let mut targets = Vec::new();
            if let Some(one) = offset(from, (0, dr)).filter(|&s| self.board[s as usize].is_none()) {
                targets.push(one);
                if from / 8 == start {
                    targets.extend(offset(one, (0, dr)).filter(|&s| self.board[s as usize].is_none()));
                }
            }
            for df in [-1, 1] {
                targets.extend(offset(from, (df, dr)).filter(|&s| match self.board[s as usize] {
                    Some(p) => p.color != self.side,
                    None => self.en_passant == Some(s),
                }));
            }
            for to in targets {
                if to / 8 == last {
                    out.extend([Queen, Rook, Bishop, Knight].map(|k| Move { from, to, promotion: Some(k) }));
                } else {
                    out.push(Move { from, to, promotion: None });
                }
            }
        }

        fn castling_moves(&self, out: &mut Vec<Move>) {
            let (base, shift) = if self.side == Color::White { (0, 0) } else { (56, 2) };
            let them = self.side.other();
            let mine = |kind| Some(Piece { color: self.side, kind });
            if self.board[base as usize + 4] != mine(King) || self.attacked(base + 4, them) {
                return;
            }
            let empty = |squares: &[u8]| squares.iter().all(|&s| self.board[(base + s) as usize].is_none());
            let safe = |squares: &[u8]| squares.iter().all(|&s| !self.attacked(base + s, them));
            if self.castling & (1 << shift) != 0
                && self.board[base as usize + 7] == mine(Rook)
                && empty(&[5, 6])
                && safe(&[5, 6])
            {
                out.push(Move { from: base + 4, to: base + 6, promotion: None });
            }
            if self.castling & (2 << shift) != 0
                && self.board[base as usize] == mine(Rook)
                && empty(&[1, 2, 3])
                && safe(&[3, 2])
            {
                out.push(Move { from: base + 4, to: base + 2, promotion: None });
            }
        }

        fn pseudo_moves(&self) -> Vec<Move> {
            let mut out = Vec::new();
            for from in 0..64u8 {
                let Some(p) = self.board[from as usize].filter(|p| p.color == self.side) else { continue };
                match p.kind {
                    Pawn => self.pawn_moves(from, &mut out),
                    Knight => self.push_targets(from, &KNIGHT, false, &mut out),
                    King => self.push_targets(from, &KING, false, &mut out),
                    Bishop => self.push_targets(from, &[KING[1], KING[3], KING[5], KING[7]], true, &mut out),
                    Rook => self.push_targets(from, &[KING[0], KING[2], KING[4], KING[6]], true, &mut out),
                    Queen => self.push_targets(from, &KING, true, &mut out),
                }
            }
            self.castling_moves(&mut out);
            out
        }

        fn play(&mut self, mv: Move) {
            let (from, to) = (mv.from as usize, mv.to as usize);
            let moving = self.board[from].take();
            let captured = self.board[to];
            let kind = moving.map(|p| p.kind);
            if kind == Some(Pawn) && captured.is_none() && self.en_passant == Some(mv.to) && from % 8 != to % 8 {
                self.board[from / 8 * 8 + to % 8] = None;
            }
            self.board[to] = mv.promotion.map(|kind| Piece { color: self.side, kind }).or(moving);
            if kind == Some(King) && from % 8 == 4 && to % 8 == 6 {
                self.board[to - 1] = self.board[to + 1].take();
            }
            if kind == Some(King) && from % 8 == 4 && to % 8 == 2 {
                self.board[to + 1] = self.board[to - 2].take();
            }
            for (bit, _, squares) in RIGHTS {
                if squares.contains(&mv.from) || squares.contains(&mv.to) {
                    self.castling &= !bit;
                }
            }
            self.en_passant = (kind == Some(Pawn) && mv.from.abs_diff(mv.to) == 16).then(|| (mv.from + mv.to) / 2);
            self.halfmove = if kind == Some(Pawn) || captured.is_some() { 0 } else { self.halfmove.saturating_add(1) };
            if self.side == Color::Black {
                self.fullmove = self.fullmove.saturating_add(1);
            }
            self.side = self.side.other();
        }
    }

    pub fn make_move(pos: &Position, mv: Move) -> Position {
        let mut next = *pos;
        next.play(mv);
        next
    }

    pub fn legal_moves(pos: &Position) -> Vec<Move> {
        pos.pseudo_moves().into_iter().filter(|&m| !make_move(pos, m).king_attacked(pos.side)).collect()
    }

    pub fn is_check(pos: &Position) -> bool {
        pos.king_attacked(pos.side)
    }

    fn insufficient_material(pos: &Position) -> bool {
        let pieces: Vec<(usize, Piece)> = pos
            .board
            .iter()
            .enumerate()
            .filter_map(|(s, p)| p.map(|p| (s, p)))
            .filter(|(_, p)| p.kind != King)
            .collect();
        let bishop_colors: Vec<usize> =
            pieces.iter().filter(|(_, p)| p.kind == Bishop).map(|(s, _)| (s % 8 + s / 8) % 2).collect();
        let knights = pieces.iter().filter(|(_, p)| p.kind == Knight).count();
        match (knights, bishop_colors.len()) {
            _ if pieces.iter().any(|(_, p)| matches!(p.kind, Pawn | Rook | Queen)) => false,
            (0, _) => bishop_colors.windows(2).all(|w| w[0] == w[1]),
            (1, 0) => true,
            _ => false,
        }
    }

    pub fn game_status(pos: &Position) -> Status {
        match () {
            _ if legal_moves(pos).is_empty() && is_check(pos) => Status::Checkmate,
            _ if legal_moves(pos).is_empty() => Status::Stalemate,
            _ if pos.halfmove >= 100 => Status::FiftyMoveRule,
            _ if insufficient_material(pos) => Status::InsufficientMaterial,
            _ => Status::Ongoing,
        }
    }

    pub fn perft(pos: &Position, depth: u32) -> u64 {
        match depth {
            0 => 1,
            1 => legal_moves(pos).len() as u64,
            _ => legal_moves(pos).into_iter().map(|m| perft(&make_move(pos, m), depth - 1)).sum(),
        }
    }

    fn piece_char(p: Piece) -> char {
        let c = ['p', 'n', 'b', 'r', 'q', 'k'][p.kind as usize];
        if p.color == Color::White {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }

    fn square_of(file: u8, rank: u8) -> Option<u8> {
        (matches!(file, b'a'..=b'h') && matches!(rank, b'1'..=b'8')).then(|| (rank - b'1') * 8 + (file - b'a'))
    }

    fn square_name(square: u8) -> String {
        format!("{}{}", (b'a' + square % 8) as char, square / 8 + 1)
    }

    fn parse_board(text: &str) -> Result<[Option<Piece>; 64], FenError> {
        let mut board = [None; 64];
        let mut rows = 0;
        for (i, row) in text.split('/').enumerate() {
            if i >= 8 {
                return Err(FenError::BadRank);
            }
            rows += 1;
            let mut file = 0;
            for c in row.chars() {
                match c {
                    '1'..='8' => file += c as usize - '0' as usize,
                    _ => {
                        let kind = match c.to_ascii_lowercase() {
                            'p' => Pawn,
                            'n' => Knight,
                            'b' => Bishop,
                            'r' => Rook,
                            'q' => Queen,
                            'k' => King,
                            _ => return Err(FenError::BadPiece(c)),
                        };
                        let color = if c.is_ascii_uppercase() { Color::White } else { Color::Black };
                        if file >= 8 {
                            return Err(FenError::BadRank);
                        }
                        board[(7 - i) * 8 + file] = Some(Piece { color, kind });
                        file += 1;
                    }
                }
                if file > 8 {
                    return Err(FenError::BadRank);
                }
            }
            if file != 8 {
                return Err(FenError::BadRank);
            }
        }
        if rows != 8 {
            return Err(FenError::BadRank);
        }
        Ok(board)
    }

    pub fn parse_fen(text: &str) -> Result<Position, FenError> {
        let fields: [&str; 6] =
            text.split_whitespace().collect::<Vec<_>>().try_into().map_err(|_| FenError::FieldCount)?;
        let board = parse_board(fields[0])?;
        let side = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err(FenError::BadSide),
        };
        let mut castling = 0;
        if fields[2] != "-" {
            for c in fields[2].chars() {
                let bit = RIGHTS.iter().find(|r| r.1 == c).ok_or(FenError::BadCastling)?.0;
                if castling & bit != 0 {
                    return Err(FenError::BadCastling);
                }
                castling |= bit;
            }
        }
        let en_passant = match fields[3].as_bytes() {
            b"-" => None,
            &[f, r] => {
                let want = if side == Color::White { b'6' } else { b'3' };
                Some(square_of(f, r).filter(|_| r == want).ok_or(FenError::BadEnPassant)?)
            }
            _ => return Err(FenError::BadEnPassant),
        };
        let halfmove = fields[4].parse().map_err(|_| FenError::BadHalfmove)?;
        let fullmove = fields[5].parse().map_err(|_| FenError::BadFullmove)?;
        let pieces = || board.iter().enumerate().filter_map(|(s, p)| p.map(|p| (s, p)));
        if pieces().any(|(s, p)| p.kind == Pawn && !(8..56).contains(&s)) {
            return Err(FenError::PawnOnBackRank);
        }
        for color in [Color::White, Color::Black] {
            if pieces().filter(|(_, p)| *p == Piece { color, kind: King }).count() != 1 {
                return Err(FenError::KingCount);
            }
        }
        let pos = Position { board, side, castling, en_passant, halfmove, fullmove };
        if pos.king_attacked(side.other()) {
            return Err(FenError::OpponentInCheck);
        }
        Ok(pos)
    }

    pub fn to_fen(pos: &Position) -> String {
        let rows: Vec<String> = pos
            .board
            .chunks(8)
            .rev()
            .map(|row| {
                let mut s = String::new();
                let mut empty = 0;
                for square in row {
                    match square {
                        None => empty += 1,
                        Some(p) => {
                            if empty > 0 {
                                s += &empty.to_string();
                                empty = 0;
                            }
                            s.push(piece_char(*p));
                        }
                    }
                }
                if empty > 0 {
                    s += &empty.to_string();
                }
                s
            })
            .collect();
        let rights: String = RIGHTS.iter().filter(|r| pos.castling & r.0 != 0).map(|r| r.1).collect();
        format!(
            "{} {} {} {} {} {}",
            rows.join("/"),
            if pos.side == Color::White { 'w' } else { 'b' },
            if rights.is_empty() { "-" } else { &rights },
            pos.en_passant.map_or("-".to_string(), square_name),
            pos.halfmove,
            pos.fullmove
        )
    }

    pub fn move_to_uci(mv: Move) -> String {
        let promotion =
            mv.promotion.map_or(String::new(), |k| piece_char(Piece { color: Color::Black, kind: k }).into());
        format!("{}{}{}", square_name(mv.from), square_name(mv.to), promotion)
    }

    pub fn parse_uci(text: &str) -> Result<Move, MoveError> {
        let bytes = text.as_bytes();
        let (&[f1, r1, f2, r2], rest) = (bytes.first_chunk::<4>().ok_or(MoveError::Syntax)?, &bytes[4..]);
        if rest.len() > 1 {
            return Err(MoveError::Syntax);
        }
        let from = square_of(f1, r1).ok_or(MoveError::Syntax)?;
        let to = square_of(f2, r2).ok_or(MoveError::Syntax)?;
        let promotion = match rest {
            [] => None,
            [b'q'] => Some(Queen),
            [b'r'] => Some(Rook),
            [b'b'] => Some(Bishop),
            [b'n'] => Some(Knight),
            _ => return Err(MoveError::BadPromotion),
        };
        Ok(Move { from, to, promotion })
    }

    pub fn play_uci(pos: &Position, text: &str) -> Result<Position, MoveError> {
        let mv = parse_uci(text)?;
        legal_moves(pos).contains(&mv).then(|| make_move(pos, mv)).ok_or(MoveError::Illegal)
    }
}

use chess::{FenError, Move, MoveError, Position, Status};

/// The six positions of the Chess Programming Wiki's "Perft Results" with
/// their published counts from depth 1. The Rust side runs every count; the
/// generated package the shallow ones (`chess_matches_rust`).
const PERFT: &[(&str, &[u64])] = &[
    (chess::START_FEN, &[20, 400, 8_902, 197_281]),
    ("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", &[48, 2_039, 97_862]),
    ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", &[14, 191, 2_812, 43_238, 674_624]),
    ("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", &[6, 264, 9_467, 422_333]),
    ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", &[44, 1_486, 62_379]),
    ("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", &[46, 2_079, 89_890]),
];

/// Endgames the random games below start from as well: promotions on both
/// sides, en passant with the capturer pinned along the rank and not, and
/// bare kings with minor pieces.
const ENDGAMES: &[&str] =
    &["8/PPP4k/8/8/8/8/4Kppp/8 w - - 0 1", "8/8/8/KPp4r/8/8/8/7k w - c6 0 2", "4k3/8/8/2b5/8/8/8/1N2K3 w - - 0 1"];

fn pos(fen: &str) -> Position {
    chess::parse_fen(fen).unwrap_or_else(|e| panic!("{fen}: {e:?}"))
}

fn ucis(moves: &[Move]) -> Vec<String> {
    moves.iter().map(|m| chess::move_to_uci(*m)).collect()
}

fn legal(fen: &str) -> Vec<String> {
    ucis(&chess::legal_moves(&pos(fen)))
}

/// The position after the UCI moves in `line`, each checked as legal.
fn after(fen: &str, line: &str) -> Position {
    line.split_whitespace().fold(pos(fen), |p, m| chess::play_uci(&p, m).unwrap_or_else(|e| panic!("{m}: {e:?}")))
}

#[test]
fn the_published_counts_and_rules_come_out_as_published() {
    for (fen, counts) in PERFT {
        let p = pos(fen);
        assert_eq!(chess::to_fen(&p), *fen);
        for (depth, want) in counts.iter().enumerate() {
            assert_eq!(chess::perft(&p, depth as u32 + 1), *want, "{fen} depth {}", depth + 1);
        }
        assert_eq!(chess::perft(&p, 0), 1);
    }
    assert_eq!(chess::start_position(), pos(chess::START_FEN));

    // UCI text both ways.
    let e2e4 = chess::parse_uci("e2e4").unwrap();
    assert_eq!((e2e4.from(), e2e4.to(), e2e4.promotion()), (12, 28, None));
    assert_eq!(chess::move_to_uci(e2e4), "e2e4");
    assert_eq!(chess::parse_uci("e7e8q"), Move::new(52, 60, Some(chess::PieceKind::Queen)));
    assert_eq!(chess::move_to_uci(chess::parse_uci("a2a1n").unwrap()), "a2a1n");
    for (text, err) in [
        ("e2e", MoveError::Syntax),
        ("e2e4qq", MoveError::Syntax),
        ("i2e4", MoveError::Syntax),
        ("e0e4", MoveError::Syntax),
        ("e7e8k", MoveError::BadPromotion),
        ("e7e8Q", MoveError::BadPromotion),
        ("é2e4", MoveError::Syntax),
    ] {
        assert_eq!(chess::parse_uci(text), Err(err), "{text}");
    }
    assert_eq!(Move::new(64, 0, None), Err(MoveError::SquareOutOfRange));
    assert_eq!(Move::new(8, 0, Some(chess::PieceKind::King)), Err(MoveError::BadPromotion));
    assert_eq!(chess::play_uci(&pos(chess::START_FEN), "e2e5"), Err(MoveError::Illegal));
    assert_eq!(
        chess::to_fen(&after(chess::START_FEN, "e2e4")),
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
    assert_eq!(
        chess::to_fen(&after(chess::START_FEN, "e2e4 c7c5 g1f3")),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
    );

    // Castling: not out of, through, or into check; b1 may be attacked.
    let castles = |fen: &str| {
        let moves = legal(fen);
        (moves.contains(&"e1g1".to_string()), moves.contains(&"e1c1".to_string()))
    };
    assert_eq!(castles("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1"), (true, true));
    assert_eq!(castles("4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1"), (false, true));
    assert_eq!(castles("3rk3/8/8/8/8/8/8/R3K2R w KQ - 0 1"), (true, false));
    assert_eq!(castles("1r2k3/8/8/8/8/8/8/R3K2R w KQ - 0 1"), (true, true));
    assert_eq!(castles("4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1"), (false, true));
    assert_eq!(castles("6r1/4k3/8/8/8/8/8/R3K2R w KQ - 0 1"), (false, true));
    assert_eq!(castles("4r1k1/8/8/8/8/8/8/R3K2R w KQ - 0 1"), (false, false));
    assert_eq!(castles("4k3/8/8/8/8/8/8/R3K2R w - - 0 1"), (false, false));
    assert_eq!(castles("4k3/8/8/8/8/8/8/RN2K1NR w KQ - 0 1"), (false, false));
    assert_eq!(
        chess::to_fen(&after("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1 e8c8")),
        "2kr3r/8/8/8/8/8/8/R4RK1 w - - 2 2"
    );
    // A rook captured on its square takes its right away.
    assert_eq!(
        chess::to_fen(&after("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "a1a8")),
        "R3k2r/8/8/8/8/8/8/4K2R b Kk - 0 1"
    );

    // En passant: only right after the double step; not when the capturer
    // is pinned along the rank; the captured pawn leaves its square.
    assert!(!legal("8/8/8/KPp4r/8/8/8/7k w - c6 0 2").contains(&"b5c6".to_string()));
    assert!(legal("8/8/8/1Pp5/8/8/8/K6k w - c6 0 2").contains(&"b5c6".to_string()));
    assert!(!legal("8/8/8/1Pp5/8/8/8/K6k w - - 0 2").contains(&"b5c6".to_string()));
    assert_eq!(chess::to_fen(&after("8/8/8/1Pp5/8/8/8/K6k w - c6 0 2", "b5c6")), "8/8/2P5/8/8/8/8/K6k b - - 0 2");
    assert_eq!(
        chess::to_fen(&after("4k3/8/8/8/3p4/8/4P3/4K3 w - - 0 1", "e2e4 d4e3")),
        "4k3/8/8/8/8/4p3/8/4K3 w - - 0 2"
    );

    // Promotion to each of four pieces, by moving and by capturing.
    let promotions: Vec<String> =
        legal("1n5k/P7/8/8/8/8/8/K7 w - - 0 1").into_iter().filter(|m| m.len() == 5).collect();
    assert_eq!(promotions, ["a7a8q", "a7a8r", "a7a8b", "a7a8n", "a7b8q", "a7b8r", "a7b8b", "a7b8n"]);
    assert_eq!(chess::to_fen(&after("1n5k/P7/8/8/8/8/8/K7 w - - 0 1", "a7b8n")), "1N5k/8/8/8/8/8/8/K7 b - - 0 1");
    assert_eq!(chess::play_uci(&pos("1n5k/P7/8/8/8/8/8/K7 w - - 0 1"), "a7a8"), Err(MoveError::Illegal));

    // Mate, stalemate, the fifty-move rule, dead positions.
    let fools = after(chess::START_FEN, "f2f3 e7e5 g2g4 d8h4");
    assert!(chess::is_check(&fools));
    assert_eq!(chess::game_status(&fools), Status::Checkmate);
    let status = |fen: &str| chess::game_status(&pos(fen));
    assert_eq!(status("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1"), Status::Stalemate);
    assert_eq!(status("7k/5Q2/6K1/8/8/8/8/8 b - - 100 80"), Status::Stalemate);
    assert_eq!(status("7k/6Q1/6K1/8/8/8/8/8 b - - 100 80"), Status::Checkmate);
    assert_eq!(status("k7/8/8/8/8/8/8/K6R w - - 99 80"), Status::Ongoing);
    assert_eq!(status("k7/8/8/8/8/8/8/K6R w - - 100 80"), Status::FiftyMoveRule);
    assert_eq!(chess::game_status(&after("k7/8/8/8/8/8/8/K6R w - - 99 80", "h1h2")), Status::FiftyMoveRule);
    assert_eq!(chess::game_status(&after("k7/8/8/8/8/8/P7/K6R w - - 99 80", "a2a3")), Status::Ongoing);
    assert_eq!(status("k7/8/8/8/8/8/8/K7 w - - 0 1"), Status::InsufficientMaterial);
    assert_eq!(status("k7/8/8/8/8/8/8/KN6 w - - 0 1"), Status::InsufficientMaterial);
    assert_eq!(status("k7/8/8/8/8/8/8/KB6 w - - 0 1"), Status::InsufficientMaterial);
    assert_eq!(status("k4b2/8/8/8/8/8/8/K1B5 w - - 0 1"), Status::InsufficientMaterial);
    assert_eq!(status("k1b5/8/8/8/8/8/8/K1B5 w - - 0 1"), Status::Ongoing);
    assert_eq!(status("k7/8/8/8/8/8/8/KNN5 w - - 0 1"), Status::Ongoing);
    assert_eq!(status("k7/8/8/8/8/8/8/KNB5 w - - 0 1"), Status::Ongoing);
    assert_eq!(status("k7/8/8/8/8/8/P7/K7 w - - 0 1"), Status::Ongoing);
    assert_eq!(status(chess::START_FEN), Status::Ongoing);

    // Every FEN error.
    for (fen, err) in [
        ("8/8/8/8/8/8/8/8 w - - 0", FenError::FieldCount),
        ("", FenError::FieldCount),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNX w KQkq - 0 1", FenError::BadPiece('X')),
        ("rnbqkbnr/pppppppp/9/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", FenError::BadPiece('9')),
        ("rnbqkbnr/pppppppp/7/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", FenError::BadRank),
        ("rnbqkbnr/pppppppp/8p/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", FenError::BadRank),
        ("rnbqkbnr/pppppppp/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", FenError::BadRank),
        ("rnbqkbnr/pppppppp/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", FenError::BadRank),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1", FenError::BadSide),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KKq - 0 1", FenError::BadCastling),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkx - 0 1", FenError::BadCastling),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e3 0 1", FenError::BadEnPassant),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e9 0 1", FenError::BadEnPassant),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -1 0 1", FenError::BadEnPassant),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - -1 1", FenError::BadHalfmove),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 4294967296", FenError::BadFullmove),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQQBNR w KQkq - 0 1", FenError::KingCount),
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBKKBNR w KQkq - 0 1", FenError::KingCount),
        ("rnbqkbnP/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", FenError::PawnOnBackRank),
        ("4k3/8/8/8/8/8/8/4K2r b - - 0 1", FenError::OpponentInCheck),
    ] {
        assert_eq!(chess::parse_fen(fen), Err(err), "{fen:?}");
    }
    // Four fields are not enough; extra spaces between six are fine, and
    // the clock may read `+5` as Rust's `parse` does.
    assert_eq!(chess::to_fen(&pos("  8/8/8/8/8/8/8/K6k   w - -  0   1 ")), "8/8/8/8/8/8/8/K6k w - - 0 1");
    assert_eq!(chess::to_fen(&pos("8/8/8/8/8/8/8/K6k w - - +5 1")), "8/8/8/8/8/8/8/K6k w - - 5 1");
    // Accessors.
    let p = pos(PERFT[4].0);
    assert_eq!(p.piece_at(4), Some(chess::Piece { color: chess::Color::White, kind: chess::PieceKind::King }));
    assert_eq!((p.piece_at(20), p.piece_at(64), p.piece_at(255)), (None, None, None));
    assert_eq!((p.halfmove_clock(), p.fullmove_number(), p.side_to_move()), (1, 8, chess::Color::White));
    assert!(p.castling().white_kingside && !p.castling().black_queenside);
    assert_eq!(after(chess::START_FEN, "e2e4").en_passant(), Some(20));
}

/// The FEN, the sorted legal moves in UCI, status, and check of a position
/// on each side; equal when the two implementations agree on it.
fn summary(p: &Position) -> (String, Vec<String>, String, bool) {
    let mut moves = ucis(&chess::legal_moves(p));
    moves.sort();
    (chess::to_fen(p), moves, format!("{:?}", chess::game_status(p)), chess::is_check(p))
}

fn reference_summary(p: &idiomatic::Position) -> (String, Vec<String>, String, bool) {
    let mut moves: Vec<String> = idiomatic::legal_moves(p).into_iter().map(idiomatic::move_to_uci).collect();
    moves.sort();
    (idiomatic::to_fen(p), moves, format!("{:?}", idiomatic::game_status(p)), idiomatic::is_check(p))
}

/// The perft positions and the endgames.
fn starts() -> impl Iterator<Item = &'static str> {
    PERFT.iter().map(|(f, _)| *f).chain(ENDGAMES.iter().copied())
}

/// Seeded random games: `games` games of up to `plies` plies from each
/// start, each move drawn from the legal ones. Each game is its positions
/// and the moves between them (one fewer).
fn games(seed: u64, games: usize, plies: usize) -> Vec<(Vec<Position>, Vec<Move>)> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    for fen in starts() {
        for _ in 0..games {
            let (mut line, mut played) = (vec![pos(fen)], Vec::new());
            for _ in 0..plies {
                let last = *line.last().unwrap();
                let moves = chess::legal_moves(&last);
                if moves.is_empty() {
                    break;
                }
                let mv = rng.pick(&moves);
                played.push(mv);
                line.push(chess::make_move(&last, mv));
            }
            out.push((line, played));
        }
    }
    out
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    // Random games, played on both sides move by move.
    let mut positions = 0;
    for (line, played) in games(0xC0FFEE, 6, 160) {
        let mut reference = idiomatic::parse_fen(&chess::to_fen(&line[0])).unwrap();
        for (k, p) in line.iter().enumerate() {
            assert_eq!(summary(p), reference_summary(&reference), "ply {k} of {}", chess::to_fen(&line[0]));
            if k % 16 == 0 {
                assert_eq!(chess::perft(p, 2), idiomatic::perft(&reference, 2), "{}", chess::to_fen(p));
            }
            if let Some(mv) = played.get(k) {
                reference = idiomatic::play_uci(&reference, &chess::move_to_uci(*mv)).unwrap();
            }
            positions += 1;
        }
    }
    assert!(positions > 5_000, "{positions} positions");

    // FEN text with one character changed or dropped.
    let alphabet = "pnbrqkPNBRQK0123456789/ wb-KQkqaeh+x";
    let mut fens = 0;
    for fen in starts() {
        let chars: Vec<char> = fen.chars().collect();
        for i in 0..chars.len() {
            let mut texts = vec![chars.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, c)| c).collect::<String>()];
            for c in alphabet.chars() {
                let mut changed = chars.clone();
                changed[i] = c;
                texts.push(changed.into_iter().collect());
            }
            for text in texts {
                let (a, b) = (chess::parse_fen(&text), idiomatic::parse_fen(&text));
                match (a, b) {
                    (Ok(a), Ok(b)) => assert_eq!(summary(&a), reference_summary(&b), "{text}"),
                    (a, b) => assert_eq!(format!("{:?}", a.map(|_| ())), format!("{:?}", b.map(|_| ())), "{text}"),
                }
                fens += 1;
            }
        }
    }
    assert!(fens > 10_000, "{fens} FEN texts");

    // Every UCI string of length 0 to 5 over a small alphabet, and Move::new.
    let alphabet = ["a", "h", "i", "1", "8", "9", "q", "n", "k", "é"];
    let mut texts = vec![String::new()];
    for len in 1..=5 {
        let shorter: Vec<String> = texts.iter().filter(|t| t.chars().count() == len - 1).cloned().collect();
        for t in shorter {
            for c in alphabet {
                texts.push(format!("{t}{c}"));
            }
        }
    }
    texts.extend(["e2e4", "e7e8q", "e7e8r", "a2a1b", "h7h8n", "e1g1", "e2e4 "].map(String::from));
    for text in &texts {
        let a = chess::parse_uci(text).map(chess::move_to_uci);
        let b = idiomatic::parse_uci(text).map(idiomatic::move_to_uci);
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "{text:?}");
    }
    for from in [0u8, 63, 64, 255] {
        for promotion in
            [None, Some(chess::PieceKind::Pawn), Some(chess::PieceKind::Rook), Some(chess::PieceKind::King)]
        {
            let reference = promotion.map(|k| {
                [
                    idiomatic::PieceKind::Pawn,
                    idiomatic::PieceKind::Knight,
                    idiomatic::PieceKind::Bishop,
                    idiomatic::PieceKind::Rook,
                    idiomatic::PieceKind::Queen,
                    idiomatic::PieceKind::King,
                ][k as usize]
            });
            let a = Move::new(from, 7, promotion).map(chess::move_to_uci);
            let b = idiomatic::Move::new(from, 7, reference).map(idiomatic::move_to_uci);
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
    }
    // Accessors.
    for (line, _) in games(7, 1, 40) {
        for p in line {
            let r = idiomatic::parse_fen(&chess::to_fen(&p)).unwrap();
            for s in 0..=255u8 {
                assert_eq!(format!("{:?}", p.piece_at(s)), format!("{:?}", r.piece_at(s)));
            }
            assert_eq!(format!("{:?}", p.side_to_move()), format!("{:?}", r.side_to_move()));
            assert_eq!(
                (p.en_passant(), p.halfmove_clock(), p.fullmove_number()),
                (r.en_passant(), r.halfmove_clock(), r.fullmove_number())
            );
        }
    }
}

/// `to_fen(make_move(pos, mv))`, the generated functions nested.
fn after_case(p: Position, mv: Move) -> support::Case {
    let call = format!("toFen(pkg.makeMove({}, {}))", p.js(), mv.js());
    support::run("to_fen", call, move || chess::to_fen(&chess::make_move(&p, mv)))
}

/// `play_uci` on text, then `to_fen` of the result if it is legal: the
/// position comes back through `parse_fen`'s printer, as `play_uci`
/// returns the same type.
fn play_case(p: Position, text: String) -> support::Case {
    let call = format!("playUci({}, {})", p.js(), text.js());
    support::run("play_uci", call, move || chess::play_uci(&p, &text))
}

#[test]
fn chess_matches_rust() {
    let cases = support::cases(|cases| {
        // Perft, the shallow counts.
        for (fen, counts) in PERFT {
            let p = pos(fen);
            cases.push(case!(chess::parse_fen(*fen)));
            for depth in 0..=counts.len().min(if *fen == chess::START_FEN { 3 } else { 2 }) as u32 {
                cases.push(case!(chess::perft(&p, depth)));
            }
        }
        cases.push(support::run("parse_fen", "pkg.Result.ok(pkg.startPosition())".to_string(), || {
            Ok::<_, FenError>(chess::start_position())
        }));
        // Positions along random games: moves, status, check, accessors,
        // FEN before and after every legal move.
        for (g, (line, _)) in games(0xBEEF, 2, 120).into_iter().enumerate() {
            for (k, p) in line.into_iter().enumerate() {
                if (k + g) % 16 != 0 {
                    continue;
                }
                cases.push(case!(chess::legal_moves(&p)));
                cases.push(case!(chess::to_fen(&p)));
                cases.push(case!(chess::game_status(&p)));
                cases.push(case!(chess::is_check(&p)));
                cases.push(case!(chess::parse_fen(chess::to_fen(&p).as_str())));
                if k % 24 == 0 {
                    cases.push(case!(chess::position_view(&p)));
                }
                for (i, mv) in chess::legal_moves(&p).into_iter().enumerate() {
                    if i % 3 == k % 3 {
                        cases.push(after_case(p, mv));
                    }
                    if i == 0 {
                        cases.push(case!(chess::move_view(mv)));
                        cases.push(case!(chess::move_to_uci(mv)));
                        cases.push(play_case(p, chess::move_to_uci(mv)));
                    }
                }
                cases.push(play_case(p, "e2e4".to_string()));
            }
        }
        // The rules one by one.
        for fen in [
            "4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1",
            "1r2k3/8/8/8/8/8/8/R3K2R w KQ - 0 1",
            "8/8/8/KPp4r/8/8/8/7k w - c6 0 2",
            "8/8/8/1Pp5/8/8/8/K6k w - c6 0 2",
            "1n5k/P7/8/8/8/8/8/K7 w - - 0 1",
            "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
            "7k/6Q1/6K1/8/8/8/8/8 b - - 100 80",
            "k7/8/8/8/8/8/8/K6R w - - 100 80",
            "k4b2/8/8/8/8/8/8/K1B5 w - - 0 1",
            "k1b5/8/8/8/8/8/8/K1B5 w - - 0 1",
            "k7/8/8/8/8/8/8/KN6 w - - 0 1",
        ] {
            let p = pos(fen);
            cases.push(case!(chess::legal_moves(&p)));
            cases.push(case!(chess::game_status(&p)));
            for mv in chess::legal_moves(&p) {
                cases.push(after_case(p, mv));
            }
        }
        // Errors.
        for fen in [
            "8/8/8/8/8/8/8/8 w - - 0",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNX w KQkq - 0 1",
            "rnbqkbnr/pppppppp/7/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KKq - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e3 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - -1 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 4294967296",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQQBNR w KQkq - 0 1",
            "rnbqkbnP/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "4k3/8/8/8/8/8/8/4K2r b - - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - +5 1",
            "  8/8/8/8/8/8/8/K6k   w - -  0   1 ",
            "8/8/8/8/8/8/8/K6\u{e9} w - - 0 1",
        ] {
            cases.push(case!(chess::parse_fen(fen)));
        }
        for text in ["e2e4", "e7e8q", "a2a1n", "e2e", "e2e4qq", "i2e4", "e7e8k", "e7e8Q", "\u{e9}2e4", "e2\u{e9}"] {
            cases.push(case!(chess::parse_uci(text)));
        }
        for (from, to, promotion) in [
            (0u8, 63u8, None),
            (64, 0, None),
            (8, 0, Some(chess::PieceKind::King)),
            (52, 60, Some(chess::PieceKind::Knight)),
        ] {
            cases.push(support::run(
                "parse_uci",
                format!("pkg.Move.new({}, {}, {})", from.js(), to.js(), promotion.js()),
                move || Move::new(from, to, promotion),
            ));
        }
    });
    assert!((500..3000).contains(&cases.len()), "{} cases", cases.len());
    support::assert_equivalent("chess", chess::SOURCE, &cases);
}
