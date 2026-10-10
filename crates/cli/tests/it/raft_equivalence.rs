//! `examples/raft`, one Raft node as `step(node, event)` (Ongaro &
//! Ousterhout, "In Search of an Understandable Consensus Algorithm
//! (Extended Version)", Figure 2 and §5.2–§5.4):
//!
//! - each rule of Figure 2 comes out as the paper writes it, one test per
//!   block: the RequestVote receiver (stale term, one vote per term, the
//!   §5.4.1 up-to-date check), the AppendEntries receiver (stale term, the
//!   consistency check, conflict truncation without truncating on a stale
//!   prefix, appending, `min(leaderCommit, index of last new entry)`), All
//!   Servers (a higher term in any of the four messages makes any role a
//!   follower; committed entries are applied in order), Candidates
//!   (self-vote, a majority, stepping down on an AppendEntries of the
//!   current term, a new election on timeout), Leaders (the initial empty
//!   AppendEntries, `nextIndex`/`matchIndex`, back-off, and the §5.4.2
//!   rule that only an entry of the current term commits by counting,
//!   including Figure 8), Figure 7's followers (a)–(f) repaired by the
//!   term-8 leader, and the errors the model's header lists;
//! - a seeded cluster simulation (3 and 5 nodes, 80 seeds of 4000 random
//!   events each; dropped, duplicated and reordered messages; crashes and
//!   restarts) holds Election Safety, Log Matching, Leader Completeness and
//!   State Machine Safety (§5.4.3, Figure 3) after every step, and
//!   converges once the network heals;
//! - the generated package agrees with Rust on `step` for (node, event)
//!   pairs the simulation actually reached, on every event kind after a set
//!   of states built for it (each role, empty and long logs, conflicting
//!   logs, stale and higher terms, an unknown peer, a sender that differs
//!   from `candidate_id`/`leader_id`, proposals at non-leaders, a second
//!   leader in a term, a term at `u64::MAX`), on `Node::restarted`, and on
//!   whole per-node input sequences replayed from `Node::new` inside TS, so
//!   states the TS side built itself feed its next step.
//!
//! No idiomatic reference: a second, conventional Raft would only restate
//! the same rules, and its line count would not compare like for like with
//! the paper's figure (ssh has none either). The rules above stand in for it.
//!
//! No driver fixture: `Node`'s fields are private in Rust, but `fixture!`
//! gives it `Js` inside the module and the `Node` brand exists only in TS
//! types (the driver is not type-checked), so a reached `Node` is passed to
//! the generated `step` as a literal, as `ulid_equivalence` passes `Ulid`.
//! The TS replays are a call expression that loops in the driver itself.

use std::collections::HashMap;

use crate::support::{self, Js, Rng};

use uuid::Uuid;

purecrate_canon::fixture!(mod raft = "../../../examples/raft/src/lib.rs");

use raft::{
    step, AppendEntries, AppendEntriesReply, Applied, ConfigError, Envelope, Event, LogEntry, Message, Node, Outcome,
    Progress, RequestVote, RequestVoteReply, Role, StepError,
};

// ---------------------------------------------------------------- helpers

fn id(i: usize) -> Uuid {
    Uuid::from_u128(0x5eed_cafe_0000_4000_8000_0000_0000_0000 + i as u128)
}

/// Not a member of any cluster here.
fn stranger() -> Uuid {
    id(999)
}

/// Member `i` of the cluster `0..n`.
fn member(i: usize, n: usize) -> Node {
    let peers = (0..n).filter(|j| *j != i).map(id).collect();
    Node::new(id(i), peers).expect("valid cluster")
}

/// The entry at 0-based position `k` of a log whose terms are given: its
/// command derives from (term, position), so logs that agree on a term at a
/// position agree on the entry, as Log Matching needs of seeded logs.
fn entry(term: u64, k: usize) -> LogEntry {
    LogEntry { term, command: term.wrapping_mul(100).wrapping_add(k as u64 + 1) }
}

fn entries(terms: &[u64]) -> Vec<LogEntry> {
    terms.iter().enumerate().map(|(k, t)| entry(*t, k)).collect()
}

fn rv(term: u64, candidate_id: Uuid, last_log_index: usize, last_log_term: u64) -> Message {
    Message::RequestVote(RequestVote { term, candidate_id, last_log_index, last_log_term })
}

fn rvr(term: u64, vote_granted: bool) -> Message {
    Message::RequestVoteReply(RequestVoteReply { term, vote_granted })
}

fn ae(term: u64, leader_id: Uuid, prev: usize, prev_term: u64, es: Vec<LogEntry>, commit: usize) -> Message {
    Message::AppendEntries(AppendEntries {
        term,
        leader_id,
        prev_log_index: prev,
        prev_log_term: prev_term,
        entries: es,
        leader_commit: commit,
    })
}

fn aer(term: u64, success: bool, index: usize) -> Message {
    Message::AppendEntriesReply(AppendEntriesReply { term, success, index })
}

fn recv(node: &Node, from: Uuid, message: Message) -> Result<Outcome, StepError> {
    step(node.clone(), Event::Receive { from, message })
}

fn ok(r: Result<Outcome, StepError>) -> Outcome {
    r.unwrap_or_else(|e| panic!("step refused: {e:?}"))
}

fn on(node: &Node, event: Event) -> Outcome {
    ok(step(node.clone(), event))
}

fn terms(n: &Node) -> Vec<u64> {
    n.log().iter().map(|e| e.term).collect()
}

/// `node` given the log `log_terms` by an AppendEntries from `leader` in
/// `term` (from index 0, nothing committed).
fn seeded(node: Node, leader: Uuid, term: u64, log_terms: &[u64]) -> Node {
    let o = ok(recv(&node, leader, ae(term, leader, 0, 0, entries(log_terms), 0)));
    assert_eq!(terms(&o.node), log_terms);
    o.node
}

/// Times out and collects grants from the peers in order until it leads.
fn elect(node: &Node) -> (Node, Outcome) {
    let mut o = on(node, Event::ElectionTimeout);
    for p in node.peers() {
        if o.node.is_leader() {
            break;
        }
        let term = o.node.current_term();
        o = ok(recv(&o.node, p, rvr(term, true)));
    }
    assert!(o.node.is_leader(), "not elected");
    (o.node.clone(), o)
}

fn progress(n: &Node) -> Vec<Progress> {
    match n.role() {
        Role::Leader { progress } => progress,
        other => panic!("not a leader: {other:?}"),
    }
}

fn peer_progress(n: &Node, peer: Uuid) -> Progress {
    progress(n).into_iter().find(|p| p.peer == peer).expect("a peer")
}

fn sent(to: Uuid, message: Message) -> Envelope {
    Envelope { to, message }
}

fn applied_indices(o: &Outcome) -> Vec<usize> {
    o.applied.iter().map(|a| a.index).collect()
}

// ------------------------------------------------------- Figure 2: rules

/// RequestVote RPC, receiver implementation, 1 and 2, with §5.4.1.
#[test]
fn request_vote_receiver() {
    let (b, c, d) = (id(1), id(2), id(3));
    let n = seeded(member(0, 5), b, 3, &[1, 2]);

    // 1. Reply false if term < currentTerm.
    let o = ok(recv(&n, c, rv(2, c, 9, 9)));
    assert_eq!(o.messages, vec![sent(c, rvr(3, false))]);
    assert_eq!((o.node.current_term(), o.node.voted_for()), (3, None));
    assert!(!o.reset_election_timer);

    // 2. votedFor is null and the log is at least as up-to-date: grant.
    let o = ok(recv(&n, c, rv(3, c, 2, 2)));
    assert_eq!(o.messages, vec![sent(c, rvr(3, true))]);
    assert_eq!(o.node.voted_for(), Some(c));
    assert!(o.reset_election_timer, "granting a vote restarts the timer");
    let voted = o.node;
    // votedFor is candidateId: grant again (a retried request).
    let o = ok(recv(&voted, c, rv(3, c, 2, 2)));
    assert_eq!(o.messages, vec![sent(c, rvr(3, true))]);
    // Another candidate in the same term: refused, however long its log.
    let o = ok(recv(&voted, d, rv(3, d, 50, 3)));
    assert_eq!(o.messages, vec![sent(d, rvr(3, false))]);
    assert_eq!(o.node.voted_for(), Some(c));
    assert!(!o.reset_election_timer);
    // A new term clears votedFor (All Servers), so `d` gets it.
    let o = ok(recv(&voted, d, rv(4, d, 2, 2)));
    assert_eq!(o.messages, vec![sent(d, rvr(4, true))]);
    assert_eq!((o.node.current_term(), o.node.voted_for()), (4, Some(d)));
}

/// §5.4.1: the later last term wins; with equal last terms, the longer log.
#[test]
fn request_vote_up_to_date_check() {
    let (b, c) = (id(1), id(2));
    let n = seeded(member(0, 5), b, 2, &[1, 2, 2]);
    for (last_index, last_term, granted) in [
        (1, 3, true),   // later last term, shorter log
        (3, 2, true),   // same last term, same length
        (4, 2, true),   // same last term, longer
        (2, 2, false),  // same last term, shorter
        (10, 1, false), // earlier last term, longer
        (0, 0, false),  // empty
    ] {
        let o = ok(recv(&n, c, rv(5, c, last_index, last_term)));
        assert_eq!(o.messages, vec![sent(c, rvr(5, granted))], "({last_index}, {last_term})");
        assert_eq!(o.node.current_term(), 5, "the term is taken either way");
        assert_eq!(o.node.voted_for(), if granted { Some(c) } else { None });
        assert_eq!(o.node.role(), Role::Follower { leader: None });
    }
    // An empty log is behind nobody.
    let o = ok(recv(&member(0, 3), c, rv(1, c, 0, 0)));
    assert_eq!(o.messages, vec![sent(c, rvr(1, true))]);
}

/// AppendEntries RPC, receiver implementation, 1 and 2.
#[test]
fn append_entries_term_and_consistency_check() {
    let (b, c) = (id(1), id(2));
    let n = seeded(member(0, 5), b, 3, &[1, 1, 2]);

    // 1. Reply false if term < currentTerm, and do not follow the sender.
    let o = ok(recv(&n, c, ae(2, c, 0, 0, entries(&[2]), 3)));
    assert_eq!(o.messages, vec![sent(c, aer(3, false, 0))]);
    assert_eq!(o.node, n);
    assert!(!o.reset_election_timer, "a stale leader does not restart the timer");

    // 2. Reply false if the log has no entry at prevLogIndex...
    let o = ok(recv(&n, b, ae(3, b, 4, 2, entries(&[1, 1, 2, 2, 3]), 0)));
    assert_eq!(o.messages, vec![sent(b, aer(3, false, 4))]);
    assert_eq!(terms(&o.node), [1, 1, 2]);
    assert!(o.reset_election_timer, "a current leader restarts the timer");
    // ... or one whose term differs from prevLogTerm.
    let o = ok(recv(&n, b, ae(3, b, 3, 1, vec![entry(3, 3)], 0)));
    assert_eq!(o.messages, vec![sent(b, aer(3, false, 3))]);
    assert_eq!(terms(&o.node), [1, 1, 2]);
    // Matching: success, and the reply names the last verified index.
    let o = ok(recv(&n, b, ae(3, b, 3, 2, vec![], 0)));
    assert_eq!(o.messages, vec![sent(b, aer(3, true, 3))]);
    // Index 0 always matches.
    let o = ok(recv(&n, b, ae(3, b, 0, 0, vec![], 0)));
    assert_eq!(o.messages, vec![sent(b, aer(3, true, 0))]);
    assert_eq!(terms(&o.node), [1, 1, 2]);
    // A candidate refusing a stale AppendEntries stays one.
    let cand = on(&n, Event::ElectionTimeout).node;
    let o = ok(recv(&cand, c, ae(3, c, 0, 0, vec![], 0)));
    assert_eq!(o.messages, vec![sent(c, aer(4, false, 0))]);
    assert_eq!(o.node.role(), Role::Candidate { votes: vec![id(0)] });
}

/// AppendEntries receiver 3 and 4: delete a conflicting entry and all that
/// follow, append the entries not already there, and nothing else.
#[test]
fn append_entries_conflict_and_append() {
    let (b, c) = (id(1), id(2));
    let n = seeded(member(0, 5), b, 2, &[1, 1, 2, 2]);
    // The new leader `c` of term 3 has [1, 1, 3]: index 3 conflicts.
    let o = ok(recv(&n, c, ae(3, c, 2, 1, vec![entry(3, 2)], 0)));
    assert_eq!(terms(&o.node), [1, 1, 3]);
    assert_eq!(o.node.log()[2], entry(3, 2));
    assert_eq!(o.node.role(), Role::Follower { leader: Some(c) });
    assert_eq!(o.messages, vec![sent(c, aer(3, true, 3))]);
    // The conflict in the middle of the batch: the agreeing prefix is kept.
    let o = ok(recv(&n, c, ae(3, c, 1, 1, vec![entry(1, 1), entry(3, 2), entry(3, 3)], 0)));
    assert_eq!(o.node.log(), vec![entry(1, 0), entry(1, 1), entry(3, 2), entry(3, 3)]);
    // A stale AppendEntries carrying a prefix of the log truncates nothing.
    let long = seeded(member(0, 5), b, 1, &[1, 1, 1, 1]);
    let o = ok(recv(&long, b, ae(1, b, 1, 1, vec![entry(1, 1)], 0)));
    assert_eq!(o.node.log(), long.log());
    assert_eq!(o.messages, vec![sent(b, aer(1, true, 2))]);
    // New entries past the end are appended after the ones it has.
    let short = seeded(member(0, 5), b, 1, &[1]);
    let o = ok(recv(&short, b, ae(1, b, 1, 1, vec![entry(1, 1), entry(1, 2)], 0)));
    assert_eq!(terms(&o.node), [1, 1, 1]);
    // Some already there, some new.
    let o = ok(recv(&o.node, b, ae(1, b, 0, 0, entries(&[1, 1, 1, 1, 1]), 0)));
    assert_eq!(terms(&o.node), [1, 1, 1, 1, 1]);
}

/// AppendEntries receiver 5, and All Servers' "apply to the state machine".
#[test]
fn append_entries_leader_commit() {
    let b = id(1);
    let fresh = member(0, 3);
    // leaderCommit beyond the last new entry: commit up to the last new one.
    let o = ok(recv(&fresh, b, ae(1, b, 0, 0, entries(&[1, 1, 1]), 10)));
    assert_eq!(o.node.commit_index(), 3);
    assert_eq!(o.node.last_applied(), 3);
    let want: Vec<Applied> = (1..=3).map(|i| Applied { index: i, entry: entry(1, i - 1) }).collect();
    assert_eq!(o.applied, want, "applied in order, each once");
    // leaderCommit below commitIndex: unchanged, nothing applied again.
    let o2 = ok(recv(&o.node, b, ae(1, b, 3, 1, vec![], 2)));
    assert_eq!(o2.node.commit_index(), 3);
    assert!(o2.applied.is_empty());
    // leaderCommit below the last new entry: commit leaderCommit.
    let o = ok(recv(&fresh, b, ae(1, b, 0, 0, entries(&[1, 1, 1, 1]), 2)));
    assert_eq!((o.node.commit_index(), applied_indices(&o)), (2, vec![1, 2]));
    // A heartbeat's last new entry is prevLogIndex.
    let o3 = ok(recv(&o.node, b, ae(1, b, 3, 1, vec![], 4)));
    assert_eq!((o3.node.commit_index(), applied_indices(&o3)), (3, vec![3]));
    let o4 = ok(recv(&o3.node, b, ae(1, b, 4, 1, vec![], 4)));
    assert_eq!((o4.node.commit_index(), applied_indices(&o4)), (4, vec![4]));
    // A reordered, older AppendEntries verifies less than is committed: the
    // model keeps commitIndex instead of moving it back (header, Choices).
    let o5 = ok(recv(&o4.node, b, ae(1, b, 1, 1, vec![entry(1, 1)], 4)));
    assert_eq!((o5.node.commit_index(), o5.applied.len()), (4, 0));
    let o6 = ok(recv(&o4.node, b, ae(1, b, 1, 1, vec![entry(1, 1)], 9)));
    assert_eq!((o6.node.commit_index(), o6.applied.len()), (4, 0));
}

/// Rules for Servers, All Servers: a higher term in any RPC request or
/// response makes any role a follower of that term.
#[test]
fn a_higher_term_makes_any_role_a_follower() {
    let (b, c) = (id(1), id(2));
    let base = seeded(member(0, 5), b, 1, &[1]);
    let follower = seeded(member(0, 5), b, 2, &[1, 2]);
    let candidate = on(&base, Event::ElectionTimeout).node;
    let (leader, _) = elect(&base);
    assert_eq!((candidate.current_term(), leader.current_term()), (2, 2));
    for (name, n) in [("follower", &follower), ("candidate", &candidate), ("leader", &leader)] {
        for m in
            [rv(5, c, 0, 0), rvr(5, false), rvr(5, true), ae(5, c, 0, 0, vec![], 0), aer(5, true, 1), aer(5, false, 0)]
        {
            let o = ok(recv(n, c, m.clone()));
            assert_eq!(o.node.current_term(), 5, "{name} {m:?}");
            assert_eq!(o.node.voted_for(), None, "{name} {m:?}");
            let leader = if matches!(m, Message::AppendEntries(_)) { Some(c) } else { None };
            assert_eq!(o.node.role(), Role::Follower { leader }, "{name} {m:?}");
            assert_eq!(o.node.log(), n.log(), "{name} {m:?}");
        }
    }
    // A higher-term request is then handled as at a follower: the vote is
    // granted to an up-to-date candidate.
    let o = ok(recv(&leader, c, rv(5, c, 1, 1)));
    assert_eq!((o.node.voted_for(), o.messages.clone()), (Some(c), vec![sent(c, rvr(5, true))]));
}

/// Rules for Servers, Followers and Candidates.
#[test]
fn candidates_elect_by_majority() {
    let me = id(0);
    let (b, c, d) = (id(1), id(2), id(3));
    let base = seeded(member(0, 5), b, 1, &[1]);
    // On conversion: increment currentTerm, vote for self, restart the
    // timer, send RequestVote to all other servers.
    let o = on(&base, Event::ElectionTimeout);
    assert_eq!((o.node.current_term(), o.node.voted_for()), (2, Some(me)));
    assert_eq!(o.node.role(), Role::Candidate { votes: vec![me] });
    assert!(o.reset_election_timer);
    let want: Vec<Envelope> = base.peers().into_iter().map(|p| sent(p, rv(2, me, 1, 1))).collect();
    assert_eq!(o.messages, want);
    let cand = o.node;

    // Votes count once each, only granted ones, only of this term.
    let o = ok(recv(&cand, b, rvr(2, true)));
    assert_eq!(o.node.role(), Role::Candidate { votes: vec![me, b] });
    let two = o.node;
    for (from, m) in [(b, rvr(2, true)), (c, rvr(2, false)), (c, rvr(1, true))] {
        let o = ok(recv(&two, from, m));
        assert_eq!(o.node, two);
        assert!(o.messages.is_empty());
    }
    // The third of five is a majority.
    let o = ok(recv(&two, d, rvr(2, true)));
    assert!(o.node.is_leader());
    assert_eq!(o.node.current_term(), 2);

    // Two of three.
    let three = on(&member(0, 3), Event::ElectionTimeout).node;
    assert!(!three.is_leader());
    assert!(ok(recv(&three, b, rvr(1, true))).node.is_leader());
    // A cluster of one leads at once.
    let alone = Node::new(me, vec![]).expect("valid");
    let o = on(&alone, Event::ElectionTimeout);
    assert!(o.node.is_leader());
    assert_eq!(o.node.role(), Role::Leader { progress: vec![] });
    assert!(o.messages.is_empty());

    // AppendEntries from a leader of the current term: become its follower.
    let o = ok(recv(&two, c, ae(2, c, 1, 1, vec![], 0)));
    assert_eq!(o.node.role(), Role::Follower { leader: Some(c) });
    assert_eq!((o.node.current_term(), o.node.voted_for()), (2, Some(me)));
    assert_eq!(o.messages, vec![sent(c, aer(2, true, 1))]);
    // Election timeout elapses: start a new election, votes start over.
    let o = on(&two, Event::ElectionTimeout);
    assert_eq!(o.node.current_term(), 3);
    assert_eq!(o.node.role(), Role::Candidate { votes: vec![me] });
    assert_eq!(o.messages.len(), 4);
    // A heartbeat tick does nothing at a follower or candidate.
    for n in [&base, &two] {
        let o = on(n, Event::HeartbeatTimeout);
        assert_eq!((&o.node, o.messages.len(), o.reset_election_timer), (n, 0, false));
    }
}

/// Rules for Servers, Leaders: the initial heartbeat, proposals,
/// nextIndex and matchIndex, back-off.
#[test]
fn leaders_replicate_and_back_off() {
    let me = id(0);
    let (b, c) = (id(1), id(2));
    let base = seeded(member(0, 3), b, 1, &[1, 1]);
    let (leader, won) = elect(&base);
    assert_eq!(leader.current_term(), 2);
    // nextIndex = last log index + 1, matchIndex = 0.
    assert_eq!(
        progress(&leader),
        vec![Progress { peer: b, next_index: 3, match_index: 0 }, Progress { peer: c, next_index: 3, match_index: 0 }]
    );
    // Upon election: an initial empty AppendEntries to each server.
    let heartbeat = vec![sent(b, ae(2, me, 2, 1, vec![], 0)), sent(c, ae(2, me, 2, 1, vec![], 0))];
    assert_eq!(won.messages, heartbeat);
    // Repeated during idle periods.
    let o = on(&leader, Event::HeartbeatTimeout);
    assert_eq!((o.node.clone(), o.messages), (leader.clone(), heartbeat));
    // An election timeout at a leader changes nothing.
    let o = on(&leader, Event::ElectionTimeout);
    assert_eq!((o.node, o.messages.len(), o.reset_election_timer), (leader.clone(), 0, false));

    // A command: appended to the local log, sent from nextIndex.
    let o = on(&leader, Event::Propose { command: 77 });
    let new = LogEntry { term: 2, command: 77 };
    assert_eq!(o.node.log()[2], new);
    assert_eq!(o.messages, vec![sent(b, ae(2, me, 2, 1, vec![new], 0)), sent(c, ae(2, me, 2, 1, vec![new], 0))]);
    let l3 = o.node;

    // Success: matchIndex and nextIndex rise; a majority of two of three
    // with the entry of term 2 commits it, and the earlier ones with it.
    let o = ok(recv(&l3, b, aer(2, true, 3)));
    assert_eq!(peer_progress(&o.node, b), Progress { peer: b, next_index: 4, match_index: 3 });
    assert_eq!((o.node.commit_index(), applied_indices(&o)), (3, vec![1, 2, 3]));
    assert!(o.messages.is_empty(), "nothing left to send");
    let committed = o.node;
    // A duplicate or older success never lowers matchIndex.
    let o = ok(recv(&committed, b, aer(2, true, 1)));
    assert_eq!(peer_progress(&o.node, b), Progress { peer: b, next_index: 4, match_index: 3 });

    // Failure: decrement nextIndex and retry at once.
    let o = ok(recv(&committed, c, aer(2, false, 2)));
    assert_eq!(peer_progress(&o.node, c).next_index, 2);
    assert_eq!(o.messages, vec![sent(c, ae(2, me, 1, 1, vec![entry(1, 1), new], 3))]);
    let backed = o.node;
    // A duplicate refusal of the same probe is not a second decrement.
    let o = ok(recv(&backed, c, aer(2, false, 2)));
    assert_eq!((peer_progress(&o.node, c).next_index, o.messages.len()), (2, 0));
    let o = ok(recv(&backed, c, aer(2, false, 1)));
    assert_eq!(peer_progress(&o.node, c).next_index, 1);
    assert_eq!(o.messages, vec![sent(c, ae(2, me, 0, 0, vec![entry(1, 0), entry(1, 1), new], 3))]);
    // Never below 1.
    let o = ok(recv(&o.node, c, aer(2, false, 0)));
    assert_eq!((peer_progress(&o.node, c).next_index, o.messages.len()), (1, 0));
    // A success that leaves the peer behind sends the rest.
    let o = ok(recv(&o.node, c, aer(2, true, 1)));
    assert_eq!(peer_progress(&o.node, c), Progress { peer: c, next_index: 2, match_index: 1 });
    assert_eq!(o.messages, vec![sent(c, ae(2, me, 1, 1, vec![entry(1, 1), new], 3))]);
    // A reply of an older term is ignored.
    let o = ok(recv(&committed, c, aer(1, false, 2)));
    assert_eq!((o.node, o.messages.len()), (committed, 0));
}

/// §5.4.2: an entry of an earlier term is not committed by counting its
/// replicas, only together with an entry of the leader's term.
#[test]
fn only_current_term_entries_commit_by_counting() {
    let (b, c, d) = (id(1), id(2), id(3));
    let base = seeded(member(0, 5), b, 2, &[1, 2]);
    let (leader, _) = elect(&base);
    assert_eq!(leader.current_term(), 3);
    let mut n = leader;
    for p in [b, c, d] {
        n = ok(recv(&n, p, aer(3, true, 2))).node;
        assert_eq!(peer_progress(&n, p).match_index, 2);
    }
    assert_eq!(n.commit_index(), 0, "index 2 (term 2) is on four of five, yet not committed");
    let o = on(&n, Event::Propose { command: 3 });
    assert_eq!(o.node.commit_index(), 0);
    let o = ok(recv(&o.node, b, aer(3, true, 3)));
    assert_eq!(o.node.commit_index(), 0, "two of five");
    let o = ok(recv(&o.node, c, aer(3, true, 3)));
    assert_eq!((o.node.commit_index(), applied_indices(&o)), (3, vec![1, 2, 3]));
}

/// The errors the model's header lists, and `Node::new`'s.
#[test]
fn refusals() {
    let me = id(0);
    let (b, c) = (id(1), id(2));
    let n = seeded(member(0, 3), b, 1, &[1]);
    for m in [rv(1, stranger(), 1, 1), rvr(1, true), ae(1, stranger(), 1, 1, vec![], 0), aer(1, true, 1)] {
        assert_eq!(recv(&n, stranger(), m), Err(StepError::UnknownPeer));
    }
    assert_eq!(recv(&n, me, rvr(1, true)), Err(StepError::UnknownPeer), "itself is not a peer");
    assert_eq!(recv(&n, b, rv(1, c, 1, 1)), Err(StepError::SenderMismatch));
    assert_eq!(recv(&n, b, ae(1, c, 0, 0, vec![], 0)), Err(StepError::SenderMismatch));
    let propose = Event::Propose { command: 1 };
    assert_eq!(step(n.clone(), propose.clone()), Err(StepError::NotLeader { leader: Some(b) }));
    assert_eq!(step(member(0, 3), propose.clone()), Err(StepError::NotLeader { leader: None }));
    let cand = on(&n, Event::ElectionTimeout).node;
    assert_eq!(step(cand, propose), Err(StepError::NotLeader { leader: None }));
    let (leader, _) = elect(&n);
    assert_eq!(recv(&leader, c, ae(2, c, 0, 0, vec![], 0)), Err(StepError::SecondLeader));
    assert_eq!(Node::new(me, vec![b, me]), Err(ConfigError::SelfInPeers));
    assert_eq!(Node::new(me, vec![b, c, b]), Err(ConfigError::DuplicatePeer));
}

/// A crash keeps currentTerm, votedFor and the log; commitIndex,
/// lastApplied and the role start over, and committed entries are applied
/// again once a leader says they are committed.
#[test]
fn restart_keeps_persistent_state() {
    let b = id(1);
    let o = ok(recv(&member(0, 3), b, ae(4, b, 0, 0, entries(&[1, 4, 4]), 2)));
    let voted = ok(recv(&o.node, b, rv(4, b, 3, 4))).node;
    let r = voted.clone().restarted();
    assert_eq!((r.current_term(), r.voted_for(), r.log()), (4, Some(b), voted.log()));
    assert_eq!((r.commit_index(), r.last_applied(), r.role()), (0, 0, Role::Follower { leader: None }));
    let o = ok(recv(&r, b, ae(4, b, 3, 4, vec![], 3)));
    assert_eq!(applied_indices(&o), [1, 2, 3]);
}

// ------------------------------------------------------------- the cluster

/// A node's input, as the differential replays it.
#[derive(Clone)]
enum Input {
    Step(Event),
    Restart,
}

struct InFlight {
    from: usize,
    to: usize,
    message: Message,
}

/// Nodes, the network between them, and what the safety properties need
/// to remember: the leader of each term, each committed entry with the
/// term of the node that first committed it, each applied entry.
struct Cluster {
    ids: Vec<Uuid>,
    nodes: Vec<Node>,
    alive: Vec<bool>,
    net: Vec<InFlight>,
    leaders: HashMap<u64, Uuid>,
    committed: HashMap<usize, (LogEntry, u64)>,
    applied: HashMap<usize, LogEntry>,
    steps: usize,
    /// Every (node, event) `fire` stepped, when recording.
    pairs: Option<Vec<(Node, Event)>>,
    /// Each node's inputs from `Node::new`, when recording.
    inputs: Vec<Vec<Input>>,
}

impl Cluster {
    fn new(n: usize) -> Cluster {
        Cluster {
            ids: (0..n).map(id).collect(),
            nodes: (0..n).map(|i| member(i, n)).collect(),
            alive: vec![true; n],
            net: Vec::new(),
            leaders: HashMap::new(),
            committed: HashMap::new(),
            applied: HashMap::new(),
            steps: 0,
            pairs: None,
            inputs: vec![Vec::new(); n],
        }
    }

    fn recording(n: usize) -> Cluster {
        Cluster { pairs: Some(Vec::new()), ..Cluster::new(n) }
    }

    fn index_of(&self, id: Uuid) -> usize {
        self.ids.iter().position(|x| *x == id).expect("a member")
    }

    fn fire(&mut self, i: usize, event: Event) -> Result<(), StepError> {
        assert!(self.alive[i]);
        let before = self.nodes[i].clone();
        if let Some(pairs) = &mut self.pairs {
            pairs.push((before.clone(), event.clone()));
            self.inputs[i].push(Input::Step(event.clone()));
        }
        let out = step(before.clone(), event)?;
        self.steps += 1;
        let after = &out.node;
        assert!(after.commit_index() >= before.commit_index(), "commitIndex went back");
        assert!(after.current_term() >= before.current_term(), "currentTerm went back");
        assert_eq!(after.last_applied(), after.commit_index());
        assert!(after.commit_index() <= after.last_log_index());
        // Applied: exactly (lastApplied, commitIndex], in order; State
        // Machine Safety: no two servers apply different entries at an index.
        let want: Vec<usize> = (before.last_applied() + 1..=after.commit_index()).collect();
        assert_eq!(applied_indices(&out), want);
        for a in &out.applied {
            let first = *self.applied.entry(a.index).or_insert(a.entry);
            assert_eq!(first, a.entry, "State Machine Safety: index {}", a.index);
        }
        self.nodes[i] = out.node;
        for env in out.messages {
            let to = self.index_of(env.to);
            self.net.push(InFlight { from: i, to, message: env.message });
        }
        self.check();
        Ok(())
    }

    /// Election Safety, Log Matching, committed entries never change, and
    /// Leader Completeness.
    fn check(&mut self) {
        for node in &self.nodes {
            if node.is_leader() {
                let leader = *self.leaders.entry(node.current_term()).or_insert(node.id());
                assert_eq!(leader, node.id(), "Election Safety: two leaders in term {}", node.current_term());
            }
        }
        let logs: Vec<Vec<LogEntry>> = self.nodes.iter().map(Node::log).collect();
        for (a, la) in logs.iter().enumerate() {
            for (b, lb) in logs.iter().enumerate().skip(a + 1) {
                let both = la.len().min(lb.len());
                // The last index where the terms agree: everything up to it
                // is identical.
                if let Some(k) = (0..both).rev().find(|k| la[*k].term == lb[*k].term) {
                    assert_eq!(la[..=k], lb[..=k], "Log Matching: nodes {a} and {b} through index {}", k + 1);
                }
            }
        }
        for (node, log) in self.nodes.iter().zip(&logs) {
            for k in 1..=node.commit_index() {
                let (first, _) = *self.committed.entry(k).or_insert((log[k - 1], node.current_term()));
                assert_eq!(first, log[k - 1], "committed entry {k} changed");
            }
        }
        for (node, log) in self.nodes.iter().zip(&logs) {
            if node.is_leader() {
                for (k, (e, t)) in &self.committed {
                    if node.current_term() > *t {
                        assert!(
                            *k <= log.len() && log[*k - 1] == *e,
                            "Leader Completeness: the leader of term {} lacks committed index {k}",
                            node.current_term()
                        );
                    }
                }
            }
        }
    }

    fn deliver(&mut self, k: usize) {
        let m = self.net.remove(k);
        if !self.alive[m.to] {
            return;
        }
        let event = Event::Receive { from: self.ids[m.from], message: m.message };
        self.fire(m.to, event).expect("a member's message is accepted");
    }

    /// Delivers, oldest first, what `keep(from, to)` lets through, and drops
    /// the rest, until the network is empty.
    fn run(&mut self, keep: &dyn Fn(usize, usize) -> bool) {
        let mut guard = 0;
        while let Some(m) = self.net.first() {
            guard += 1;
            assert!(guard < 100_000, "the network does not drain");
            if keep(m.from, m.to) {
                self.deliver(0);
            } else {
                self.net.remove(0);
            }
        }
    }

    fn run_all(&mut self) {
        self.run(&|_, _| true);
    }

    fn drop_all(&mut self) {
        self.net.clear();
    }

    fn crash(&mut self, i: usize) {
        self.alive[i] = false;
        self.net.retain(|m| m.to != i && m.from != i);
    }

    fn restart(&mut self, i: usize) {
        self.nodes[i] = self.nodes[i].clone().restarted();
        self.alive[i] = true;
        if self.pairs.is_some() {
            self.inputs[i].push(Input::Restart);
        }
    }

    /// The live leader of the highest term.
    fn leader(&self) -> Option<usize> {
        (0..self.nodes.len())
            .filter(|i| self.alive[*i] && self.nodes[*i].is_leader())
            .max_by_key(|i| self.nodes[*i].current_term())
    }

    fn terms(&self, i: usize) -> Vec<u64> {
        terms(&self.nodes[i])
    }

    fn all_committed(&self, index: usize) -> bool {
        self.nodes.iter().zip(&self.alive).all(|(n, alive)| !alive || n.commit_index() >= index)
    }

    fn elect(&mut self, i: usize) {
        self.fire(i, Event::ElectionTimeout).expect("timeout");
        self.run_all();
        assert!(self.nodes[i].is_leader(), "node {i} not elected");
    }

    fn propose(&mut self, i: usize, command: u64) {
        self.fire(i, Event::Propose { command }).expect("at the leader");
    }

    fn heartbeat(&mut self, i: usize) {
        self.fire(i, Event::HeartbeatTimeout).expect("heartbeat");
    }

    /// Node `i` given the log `log_terms` by an AppendEntries of `term`
    /// from member `j`, outside the network.
    fn seed(&mut self, i: usize, j: usize, term: u64, log_terms: &[u64]) {
        self.nodes[i] = seeded(self.nodes[i].clone(), self.ids[j], term, log_terms);
    }
}

/// Figure 7: the term-8 leader brings followers (a)–(f) to its log.
#[test]
fn figure_7_followers_are_repaired() {
    let mut c = Cluster::new(7);
    let leader = [1, 1, 1, 4, 4, 5, 5, 6, 6, 6];
    let followers: [&[u64]; 6] = [
        &[1, 1, 1, 4, 4, 5, 5, 6, 6],          // (a) missing one
        &[1, 1, 1, 4],                         // (b) missing many
        &[1, 1, 1, 4, 4, 5, 5, 6, 6, 6, 6],    // (c) one extra, uncommitted
        &[1, 1, 1, 4, 4, 5, 5, 6, 6, 6, 7, 7], // (d) two extra of term 7
        &[1, 1, 1, 4, 4, 4, 4],                // (e) extra of term 4
        &[1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 3],    // (f) terms 2 and 3 never committed
    ];
    c.seed(0, 1, 6, &leader);
    let seen = [6, 4, 6, 7, 4, 3];
    for (k, f) in followers.iter().enumerate() {
        c.seed(k + 1, 0, seen[k], f);
    }
    c.check();
    // Term 6, then two timeouts whose requests are lost: term 8.
    c.fire(0, Event::ElectionTimeout).expect("timeout");
    c.drop_all();
    c.fire(0, Event::ElectionTimeout).expect("timeout");
    assert_eq!(c.nodes[0].current_term(), 8);
    c.run_all();
    assert!(c.nodes[0].is_leader(), "(a), (b), (e), (f) vote for it");
    c.propose(0, 800);
    c.run_all();
    for _ in 0..3 {
        c.heartbeat(0);
        c.run_all();
    }
    let mut want = leader.to_vec();
    want.push(8);
    for i in 0..7 {
        assert_eq!(c.terms(i), want, "follower {i} not repaired");
        assert_eq!(c.nodes[i].commit_index(), 11);
    }
}

/// Figure 8 to (c): S1 leads term 2 and has index 2 (term 2) on S2; S5
/// leads term 3; S1 leads term 4 and puts index 2 on S3: a majority, but of
/// term 2, so not committed.
fn figure_8_to_c() -> Cluster {
    let mut c = Cluster::new(5);
    let (s1, s2, s3, s5) = (0, 1, 2, 4);
    c.elect(s1);
    c.propose(s1, 1);
    c.run_all();
    c.heartbeat(s1);
    c.run_all();
    assert!(c.all_committed(1));
    c.crash(s1);
    c.restart(s1);
    c.elect(s1);
    assert_eq!(c.nodes[s1].current_term(), 2);
    // (a)
    c.propose(s1, 2);
    c.run(&|f, t| (f == s1 && t == s2) || (f == s2 && t == s1));
    assert_eq!(c.terms(s2), [1, 2]);
    c.crash(s1);
    // (b): S5 by S3, S4 and itself.
    c.elect(s5);
    assert_eq!(c.nodes[s5].current_term(), 3);
    c.propose(s5, 3);
    c.drop_all();
    c.crash(s5);
    // (c)
    c.restart(s1);
    c.fire(s1, Event::ElectionTimeout).expect("timeout");
    c.drop_all();
    c.fire(s1, Event::ElectionTimeout).expect("timeout");
    assert_eq!(c.nodes[s1].current_term(), 4);
    c.run_all();
    assert!(c.nodes[s1].is_leader());
    for _ in 0..3 {
        c.heartbeat(s1);
        c.run_all();
    }
    for s in [s1, s2, s3] {
        assert_eq!(c.terms(s), [1, 2]);
    }
    assert!(c.nodes[s1].commit_index() < 2, "§5.4.2: index 2 of term 2 is not committed by counting");
    c
}

/// Figure 8 (d): S1 crashes, S5 is elected with S2, S3, S4 and overwrites
/// index 2, which was never committed.
#[test]
fn figure_8_an_old_entry_on_a_majority_is_overwritten() {
    let mut c = figure_8_to_c();
    let (s1, s2, s3, s4, s5) = (0, 1, 2, 3, 4);
    c.crash(s1);
    c.restart(s5);
    c.fire(s5, Event::ElectionTimeout).expect("timeout");
    c.drop_all();
    c.fire(s5, Event::ElectionTimeout).expect("timeout");
    c.run_all();
    assert!(c.nodes[s5].is_leader());
    assert_eq!(c.nodes[s5].current_term(), 5);
    c.propose(s5, 5);
    c.run_all();
    for _ in 0..3 {
        c.heartbeat(s5);
        c.run_all();
    }
    for s in [s2, s3, s4, s5] {
        assert_eq!(c.terms(s), [1, 3, 5]);
        assert_eq!(c.nodes[s].commit_index(), 3);
    }
}

/// Figure 8 (e): S1 replicates an entry of term 4 to a majority, which
/// commits index 2 with it; S5 can no longer be elected.
#[test]
fn figure_8_a_current_entry_commits_the_old_one() {
    let mut c = figure_8_to_c();
    let (s1, s4, s5) = (0, 3, 4);
    c.propose(s1, 4);
    c.run(&|f, t| f != s4 && t != s4);
    assert_eq!(c.nodes[s1].commit_index(), 3);
    assert_eq!(c.terms(s1), [1, 2, 4]);
    c.crash(s1);
    c.restart(s5);
    for _ in 0..3 {
        c.fire(s5, Event::ElectionTimeout).expect("timeout");
        c.run_all();
        assert!(!c.nodes[s5].is_leader(), "S5 lacks committed entries");
    }
}

/// Seeded random events: timeouts, ticks, proposals at random nodes,
/// crashes that keep a majority alive, restarts, and messages delivered in
/// random order, a twentieth dropped and a twentieth duplicated. Then the
/// network heals and the cluster must converge.
fn random_run(c: &mut Cluster, seed: u64, events: usize) {
    let n = c.nodes.len();
    let mut rng = Rng::new(seed);
    let mut command = 0u64;
    for _ in 0..events {
        let roll = rng.below(100);
        let i = rng.below(n as u64) as usize;
        if roll < 4 {
            if c.alive[i] {
                c.fire(i, Event::ElectionTimeout).expect("timeout");
            }
        } else if roll < 12 {
            if c.alive[i] {
                c.heartbeat(i);
            }
        } else if roll < 20 {
            if c.alive[i] {
                command += 1;
                match c.fire(i, Event::Propose { command }) {
                    Ok(()) | Err(StepError::NotLeader { .. }) => {}
                    Err(e) => panic!("seed {seed}: {e:?}"),
                }
            }
        } else if roll < 22 {
            let live = c.alive.iter().filter(|a| **a).count();
            if c.alive[i] && (live - 1) * 2 > n {
                c.crash(i);
            }
        } else if roll < 26 {
            if !c.alive[i] {
                c.restart(i);
            }
        } else if !c.net.is_empty() {
            let k = rng.below(c.net.len() as u64) as usize;
            match rng.below(20) {
                0 => {
                    c.net.remove(k);
                }
                1 => {
                    let m = &c.net[k];
                    let copy = InFlight { from: m.from, to: m.to, message: m.message.clone() };
                    c.net.push(copy);
                    c.deliver(k);
                }
                _ => c.deliver(k),
            }
        }
    }
    for i in 0..n {
        if !c.alive[i] {
            c.restart(i);
        }
    }
    c.run_all();
    let mut t = 0;
    loop {
        if let Some(l) = c.leader() {
            let term = c.nodes[l].current_term();
            if c.nodes.iter().all(|x| x.current_term() == term) {
                break;
            }
        }
        c.fire(t % n, Event::ElectionTimeout).expect("timeout");
        c.run_all();
        t += 1;
        assert!(t < 50, "seed {seed}: no leader after healing");
    }
    let l = c.leader().expect("a leader");
    c.propose(l, 1 << 40);
    c.run_all();
    for _ in 0..4 {
        c.heartbeat(l);
        c.run_all();
    }
    let last = c.nodes[l].last_log_index();
    assert!(c.all_committed(last), "seed {seed}: did not converge");
    for i in 0..n {
        assert_eq!(c.terms(i), c.terms(l), "seed {seed}: node {i}");
    }
}

#[test]
fn simulated_clusters_stay_safe() {
    for n in [3, 5] {
        let (mut applied, mut leaders, mut steps) = (0, 0, 0);
        for seed in 1..=80 {
            let mut c = Cluster::new(n);
            random_run(&mut c, seed * 7919 + n as u64, 4000);
            applied += c.applied.len();
            leaders += c.leaders.len();
            steps += c.steps;
        }
        // The runs elect, replicate and commit, not only time out (3
        // nodes: about 4200 leader terms, 5400 applied indices, 118000
        // steps; 5 nodes: 3500, 2500, 143000).
        assert!(leaders > 2000, "{n} nodes: {leaders} leader terms");
        assert!(applied > 1500, "{n} nodes: {applied} applied indices");
        assert!(steps > 80_000, "{n} nodes: {steps} steps");
    }
}

// ------------------------------------------------------- Rust against TS

fn step_case(node: Node, event: Event) -> support::Case {
    case!(raft::step(node, event))
}

/// `Node::restarted`, then the event.
fn restarted_case(node: Node, event: Event) -> support::Case {
    let call = format!("step(pkg.Node.restarted({}), {})", node.js(), event.js());
    support::run("step", call, move || step(node.restarted(), event))
}

/// Node `i` of `n` from `Node::new` through `inputs`, inside TS; the last
/// step's result (a refused step leaves the node as it was).
fn replay_case(i: usize, n: usize, inputs: Vec<Input>) -> support::Case {
    let me = id(i);
    let peers: Vec<Uuid> = (0..n).filter(|j| *j != i).map(id).collect();
    let script: Vec<String> = inputs
        .iter()
        .map(|input| match input {
            Input::Step(e) => e.js(),
            Input::Restart => "null".to_string(),
        })
        .collect();
    let call = format!(
        "(() => {{ let n = pkg.Node.new({}, {}).value; let r = null; \
         for (const e of [{}]) {{ if (e === null) {{ n = pkg.Node.restarted(n); continue; }} \
         r = step(n, e); if (r.kind === \"Ok\") n = r.value.node; }} return r; }})()",
        me.js(),
        peers.js(),
        script.join(", ")
    );
    support::run("step", call, move || {
        let mut node = Node::new(me, peers).expect("valid");
        let mut last = None;
        for input in inputs {
            match input {
                Input::Restart => node = node.restarted(),
                Input::Step(e) => {
                    let r = step(node.clone(), e);
                    if let Ok(o) = &r {
                        node = o.node.clone();
                    }
                    last = Some(r);
                }
            }
        }
        last.expect("a step")
    })
}

/// States built for the grid, by name.
fn grid_states() -> Vec<(&'static str, Node)> {
    let (b, c, d) = (id(1), id(2), id(3));
    let mut out: Vec<(&'static str, Node)> = Vec::new();
    out.push(("fresh follower", member(0, 5)));
    let known = ok(recv(&member(0, 3), b, ae(2, b, 0, 0, entries(&[1, 1, 2]), 2))).node;
    out.push(("follower with a leader, 2 committed", known.clone()));
    let long: Vec<u64> = (0..30).map(|k| 1 + k / 7).collect();
    out.push(("follower with a long log", ok(recv(&member(0, 5), c, ae(5, c, 0, 0, entries(&long), 17))).node));
    out.push(("follower with Figure 7 (f)'s log", seeded(member(0, 5), b, 3, &[1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 3])));
    out.push(("follower that voted", ok(recv(&known, c, rv(3, c, 3, 2))).node));
    out.push(("restarted follower", known.clone().restarted()));
    let base = seeded(member(0, 5), b, 1, &[1, 1]);
    let cand = on(&base, Event::ElectionTimeout).node;
    out.push(("candidate, one vote of five", cand.clone()));
    out.push(("candidate, two votes of five", ok(recv(&cand, b, rvr(2, true))).node));
    out.push(("candidate, empty log", on(&member(0, 3), Event::ElectionTimeout).node));
    out.push(("leader, empty log", elect(&member(0, 3)).0));
    let (l5, _) = elect(&base);
    let mut l5 = on(&l5, Event::Propose { command: 9 }).node;
    l5 = on(&l5, Event::Propose { command: u64::MAX }).node;
    l5 = ok(recv(&l5, b, aer(2, true, 4))).node;
    l5 = ok(recv(&l5, c, aer(2, false, 2))).node;
    l5 = ok(recv(&l5, d, aer(2, true, 2))).node;
    out.push(("leader of five, peers apart", l5));
    out.push(("leader of one", on(&Node::new(id(0), vec![]).expect("valid"), Event::ElectionTimeout).node));
    let f7: Cluster = {
        let mut c = Cluster::new(3);
        c.seed(0, 1, 6, &[1, 1, 1, 4, 4, 5, 5, 6, 6, 6]);
        c.fire(0, Event::ElectionTimeout).expect("timeout");
        c.drop_all();
        c.fire(0, Event::ElectionTimeout).expect("timeout");
        c.run_all();
        c
    };
    out.push(("leader of term 8 with Figure 7's log", f7.nodes[0].clone()));
    out.push(("follower at the largest term", seeded(member(0, 3), b, u64::MAX, &[1, u64::MAX - 1, u64::MAX])));
    out
}

/// Every event kind, against a state's term and log.
fn grid_events(n: &Node) -> Vec<Event> {
    let (b, c) = (id(1), id(2));
    let t = n.current_term();
    let len = n.last_log_index();
    let last = n.log().last().map_or(0, |e| e.term);
    let mut out = vec![
        Event::ElectionTimeout,
        Event::HeartbeatTimeout,
        Event::Propose { command: 42 },
        Event::Propose { command: u64::MAX },
    ];
    let receive = |from: Uuid, message: Message| Event::Receive { from, message };
    let near: Vec<u64> = [t.checked_sub(1), Some(t), t.checked_add(1)].into_iter().flatten().collect();
    for term in near {
        out.extend([
            receive(b, rv(term, b, len, last)),
            receive(b, rv(term, b, 0, 0)),
            receive(b, rv(term, b, len + 1, last.max(term))),
            receive(b, rvr(term, true)),
            receive(b, rvr(term, false)),
            receive(b, ae(term, b, len, last, vec![], len + 5)),
            receive(b, ae(term, b, 0, 0, vec![entry(term, 0), entry(term, 1)], 1)),
            receive(b, ae(term, b, 1, 1, vec![entry(1, 1), entry(term, 2)], 1 << 32)),
            receive(b, ae(term, b, len + 3, term, vec![], 0)),
            receive(b, ae(term, b, len, last, vec![entry(term, len)], len + 1)),
            receive(b, aer(term, true, len)),
            receive(b, aer(term, true, 0)),
            receive(b, aer(term, false, len.saturating_sub(1))),
            receive(b, aer(term, false, 0)),
            receive(c, aer(term, true, len + 2)),
        ]);
    }
    out.extend([
        receive(stranger(), rv(t, stranger(), 0, 0)),
        receive(stranger(), aer(t, true, 0)),
        receive(b, rv(t, c, len, last)),
        receive(b, ae(t, c, 0, 0, vec![], 0)),
        receive(id(0), rvr(t, true)),
    ]);
    out
}

/// A recorded run: its size, the (node, event) pairs `fire` stepped, and
/// each node's inputs from `Node::new`.
struct Recorded {
    n: usize,
    pairs: Vec<(Node, Event)>,
    inputs: Vec<Vec<Input>>,
}

/// Seeded runs, and the end of Figure 8.
fn recorded_runs() -> Vec<Recorded> {
    let mut out = Vec::new();
    for (n, seed) in [(3, 11), (3, 12), (5, 21), (5, 22)] {
        let mut c = Cluster::recording(n);
        random_run(&mut c, seed, 500);
        out.push(Recorded { n, pairs: c.pairs.take().expect("recording"), inputs: std::mem::take(&mut c.inputs) });
    }
    // The figure scenarios reach states the random runs rarely do.
    let mut c = figure_8_to_c();
    c.pairs = Some(Vec::new());
    c.crash(0);
    c.restart(4);
    c.fire(4, Event::ElectionTimeout).expect("timeout");
    c.run_all();
    c.fire(4, Event::ElectionTimeout).expect("timeout");
    c.run_all();
    out.push(Recorded { n: 5, pairs: c.pairs.take().expect("recording"), inputs: Vec::new() });
    out
}

#[test]
fn raft_matches_rust() {
    let runs = recorded_runs();
    let states = grid_states();
    let cases = support::cases(|cases| {
        for run in &runs {
            for (node, event) in &run.pairs {
                cases.push(step_case(node.clone(), event.clone()));
            }
        }
        for (_, node) in &states {
            for event in grid_events(node) {
                cases.push(step_case(node.clone(), event));
            }
            for event in [Event::HeartbeatTimeout, Event::ElectionTimeout, Event::Propose { command: 1 }] {
                cases.push(restarted_case(node.clone(), event));
            }
        }
        for run in &runs {
            for (i, mine) in run.inputs.iter().enumerate() {
                if mine.iter().any(|x| matches!(x, Input::Step(_))) {
                    cases.push(replay_case(i, run.n, mine.clone()));
                }
            }
        }
    });
    let reached: usize = runs.iter().map(|r| r.pairs.len()).sum();
    assert!(reached > 1000, "only {reached} reached pairs");
    assert!(cases.len() > 2000, "only {} cases", cases.len());
    // The grid reaches every outcome: each error, and Ok.
    let kinds = |text: &str| cases.iter().filter(|c| c.rust.contains(text)).count();
    for text in ["UnknownPeer", "SenderMismatch", "NotLeader", "SecondLeader", "Ok(", "panic("] {
        assert!(kinds(text) > 0, "no case gives {text}");
    }
    support::assert_equivalent("raft", raft::SOURCE, &cases);
}
