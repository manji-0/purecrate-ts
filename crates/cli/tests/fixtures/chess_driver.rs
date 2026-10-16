/// What `Position`'s accessors return: each square from 0 to 65 (the last
/// two off the board), then the side, rights, en-passant square, and clocks.
pub fn position_view(pos: &Position) -> (Vec<Option<Piece>>, Color, CastlingRights, Option<u8>, (u32, u32)) {
    let squares: Vec<Option<Piece>> = (0..66u8).map(|s| pos.piece_at(s)).collect();
    let clocks = (pos.halfmove_clock(), pos.fullmove_number());
    (squares, pos.side_to_move(), pos.castling(), pos.en_passant(), clocks)
}

/// What `Move`'s accessors return.
pub fn move_view(mv: Move) -> (u8, u8, Option<PieceKind>) {
    (mv.from(), mv.to(), mv.promotion())
}
