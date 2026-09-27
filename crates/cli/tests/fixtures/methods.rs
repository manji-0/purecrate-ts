pub struct Rank(u8);

impl Rank {
    pub fn value(&self) -> u8 {
        self.0
    }

    pub fn is_face(&self) -> bool {
        self.value() > 10
    }
}

pub enum Suit {
    Hearts,
    Spades,
}

impl Suit {
    pub fn is_red(&self) -> bool {
        match self {
            Suit::Hearts => true,
            Suit::Spades => false,
        }
    }
}

pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: u8, red: bool) -> Self {
        let suit = if red { Suit::Hearts } else { Suit::Spades };
        Self {
            rank: Rank(rank),
            suit,
        }
    }

    pub fn beats(&self, other: &Card) -> bool {
        self.rank.value() > other.rank.value()
    }

    pub fn same_colour(&self, other: &Card) -> bool {
        self.suit.is_red() == other.suit.is_red()
    }

    pub fn score(&self) -> u8 {
        let base = if self.rank.is_face() { 10 } else { self.rank.value() };
        if self.suit.is_red() {
            base + 1
        } else {
            base
        }
    }
}

pub enum Outcome {
    Win,
    Lose,
    Tie,
}

impl Outcome {
    pub fn points(&self) -> i32 {
        match self {
            Outcome::Win => 3,
            Outcome::Lose => 0,
            Outcome::Tie => 1,
        }
    }
}

pub struct Round {
    pub mine: Card,
    pub theirs: Card,
}

impl Round {
    pub fn outcome(&self) -> Outcome {
        if self.mine.beats(&self.theirs) {
            Outcome::Win
        } else if self.theirs.beats(&self.mine) {
            Outcome::Lose
        } else {
            Outcome::Tie
        }
    }
}

pub fn play(a: u8, a_red: bool, b: u8, b_red: bool) -> i32 {
    let round = Round {
        mine: Card::new(a, a_red),
        theirs: Card::new(b, b_red),
    };
    let bonus: i32 = if round.mine.same_colour(&round.theirs) { 1 } else { 0 };
    let factor: i32 = match round.outcome() {
        Outcome::Tie => 1,
        Outcome::Win => 2,
        Outcome::Lose => 2,
    };
    round.outcome().points() * factor + bonus
}

pub fn total(a: u8, b: u8) -> u8 {
    Card::new(a, true).score() + Card::new(b, false).score()
}
