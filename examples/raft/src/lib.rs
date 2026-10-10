// Raft: one node's state machine, as a pure function.
//
// This is the consensus core of "In Search of an Understandable Consensus
// Algorithm (Extended Version)" (Ongaro & Ousterhout), written from Figure 2
// ("The Raft consensus algorithm: a condensed summary") and the rules of
// §5.2 (leader election), §5.3 (log replication) and §5.4 (safety, with the
// commitment restriction of §5.4.2).
//
// `step(node, event)` takes a node and one input and returns the next node,
// the messages to send, and the log entries that became applied. There is
// no I/O, clock, or randomness: an election timeout and a heartbeat tick are
// events the caller delivers (the caller owns the randomized timer, and
// `Outcome::reset_election_timer` tells it when Figure 2 says to restart
// it). A message from another node arrives as `Event::Receive { from, .. }`.
//
// Representation
//
// - A node is identified by a `Uuid`. The cluster is fixed at construction:
//   `Node::new(id, peers)` takes the other members (not itself), and a
//   majority is more than half of `peers.len() + 1`.
// - The log is 1-based as in the paper: index `i` (1..=log.len()) is
//   `log[i - 1]`, index 0 is "before the first entry" with term 0. Indices
//   are `usize`, terms are `u64`. A command is an opaque `u64`.
// - Persistent state: `current_term`, `voted_for`, `log`. Volatile state:
//   `commit_index`, `last_applied`. Leader state: `next_index` and
//   `match_index` per peer, held in `Role::Leader { progress }`, so it exists
//   only while the node leads and is rebuilt on each election. A candidate's
//   granted votes (itself included) are `Role::Candidate { votes }`.
// - `Node` has private fields: the only ways to get one are `Node::new`,
//   `step`, and `Node::restarted` (a crash and recovery, which keeps the
//   persistent state and resets the volatile state as Figure 2 says).
//
// What the model checks
//
// - All servers: a message with a higher term sets `current_term`, clears
//   `voted_for`, and makes the node a follower before the message is handled
//   (this is how a stale leader or candidate steps down). Whenever
//   `commit_index > last_applied` the entries in between are applied, in
//   order, and returned in `Outcome::applied`.
// - RequestVote: refused for a lower term; granted when `voted_for` is empty
//   or already the candidate and the candidate's log is at least as
//   up-to-date (§5.4.1: higher last term, or the same last term and a last
//   index at least as large).
// - AppendEntries: refused for a lower term; otherwise the receiver follows
//   the sender, checks that it has an entry at `prev_log_index` with
//   `prev_log_term`, deletes an existing entry that conflicts with a new one
//   and everything after it, appends the entries it does not have, and sets
//   `commit_index = min(leader_commit, index of last new entry)` when
//   `leader_commit > commit_index`.
// - Candidates: an election timeout increments the term, votes for itself,
//   and sends RequestVote to every peer; a majority of votes makes it the
//   leader; AppendEntries from a leader of the same term makes it a follower;
//   another timeout starts a new election.
// - Leaders: on election, `next_index = last log index + 1` and
//   `match_index = 0` for every peer, and an AppendEntries (empty unless the
//   peer is behind) goes to each; a heartbeat tick repeats it; a client
//   proposal is appended to the leader's log and replicated; a successful
//   reply raises `match_index`/`next_index`; a refusal decrements
//   `next_index` and retries at once; `commit_index` moves to the largest N
//   replicated on a majority by `match_index` with `log[N].term ==
//   current_term` (§5.4.2: an entry from an earlier term is never committed
//   by counting replicas, only together with a later entry of the leader's
//   term).
//
// Choices Figure 2 leaves to the implementation
//
// - An AppendEntries reply carries `index`: on success the index of the last
//   entry the message verified (`prev_log_index + entries.len()`), on a
//   refusal the `prev_log_index` that did not match. The leader uses it to
//   set `match_index` without remembering what it sent, and ignores a
//   refusal that is not about its current `next_index` (a duplicate or
//   reordered reply), so `next_index` is decremented once per probe and
//   never below 1.
// - A stale AppendEntries (reordered on the network) can verify fewer
//   entries than the follower already has from the same leader; then
//   `min(leader_commit, last new index)` could be below `commit_index`, and
//   `commit_index` is kept instead of moving backwards.
// - A leader sends every entry from `next_index` on (no batching limit) and
//   sends the rest after a success that left the peer behind.
// - A message whose `from` is not a configured peer is an error
//   (`StepError::UnknownPeer`); so is a RequestVote whose `candidate_id` or
//   an AppendEntries whose `leader_id` differs from `from`, and an
//   AppendEntries for the current term at a leader (two leaders in one term,
//   which Election Safety rules out).
// - A proposal at a follower or candidate is `StepError::NotLeader` with the
//   leader the follower last heard from, for the caller to redirect.
//
// What the model leaves out
//
// - Cluster membership changes (§6, joint consensus): the peer list is fixed.
// - Log compaction and snapshots (§7, InstallSnapshot).
// - Client interaction beyond appending (§8): no linearizable reads, no
//   deduplication of retried commands, and no no-op entry at the start of a
//   term (so a new leader commits earlier entries only when a client
//   proposes something in its term).
// - Batching or size limits on AppendEntries, flow control, pipelining.
// - Pre-vote, leadership transfer, check-quorum, leases.
// - Timers themselves: randomized election timeouts and the heartbeat
//   interval are the caller's; the model only reacts to their events.
// - Persistence: the caller must store `current_term`, `voted_for`, and the
//   log before sending the returned messages.
// - The fast `next_index` back-off of §5.3 (conflict term hints).

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry {
    pub term: u64,
    pub command: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub peer: Uuid,
    pub next_index: usize,
    pub match_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Follower { leader: Option<Uuid> },
    Candidate { votes: Vec<Uuid> },
    Leader { progress: Vec<Progress> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    id: Uuid,
    peers: Vec<Uuid>,
    current_term: u64,
    voted_for: Option<Uuid>,
    log: Vec<LogEntry>,
    commit_index: usize,
    last_applied: usize,
    role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestVote {
    pub term: u64,
    pub candidate_id: Uuid,
    pub last_log_index: usize,
    pub last_log_term: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestVoteReply {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendEntries {
    pub term: u64,
    pub leader_id: Uuid,
    pub prev_log_index: usize,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendEntriesReply {
    pub term: u64,
    pub success: bool,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    RequestVote(RequestVote),
    RequestVoteReply(RequestVoteReply),
    AppendEntries(AppendEntries),
    AppendEntriesReply(AppendEntriesReply),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub to: Uuid,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ElectionTimeout,
    HeartbeatTimeout,
    Receive { from: Uuid, message: Message },
    Propose { command: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub index: usize,
    pub entry: LogEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub node: Node,
    pub messages: Vec<Envelope>,
    pub applied: Vec<Applied>,
    pub reset_election_timer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    SelfInPeers,
    DuplicatePeer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepError {
    UnknownPeer,
    SenderMismatch,
    NotLeader { leader: Option<Uuid> },
    SecondLeader,
}

impl Node {
    pub fn new(id: Uuid, peers: Vec<Uuid>) -> Result<Node, ConfigError> {
        for (i, p) in peers.iter().enumerate() {
            if *p == id {
                return Err(ConfigError::SelfInPeers);
            }
            for j in 0..i {
                if peers[j] == *p {
                    return Err(ConfigError::DuplicatePeer);
                }
            }
        }
        Ok(Node {
            id,
            peers,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            role: Role::Follower { leader: None },
        })
    }

    // A crash and restart: persistent state survives, volatile state and
    // the role start over (Figure 2, "reinitialized after election" for the
    // leader state, "initialized to 0" for commit_index and last_applied).
    pub fn restarted(self) -> Node {
        Node {
            commit_index: 0,
            last_applied: 0,
            role: Role::Follower { leader: None },
            ..self
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn peers(&self) -> Vec<Uuid> {
        self.peers.clone()
    }

    pub fn current_term(&self) -> u64 {
        self.current_term
    }

    pub fn voted_for(&self) -> Option<Uuid> {
        self.voted_for
    }

    pub fn log(&self) -> Vec<LogEntry> {
        self.log.clone()
    }

    pub fn last_log_index(&self) -> usize {
        self.log.len()
    }

    pub fn commit_index(&self) -> usize {
        self.commit_index
    }

    pub fn last_applied(&self) -> usize {
        self.last_applied
    }

    pub fn role(&self) -> Role {
        self.role.clone()
    }

    pub fn is_leader(&self) -> bool {
        matches!(self.role, Role::Leader { .. })
    }
}

impl Message {
    pub fn term(&self) -> u64 {
        match self {
            Message::RequestVote(m) => m.term,
            Message::RequestVoteReply(m) => m.term,
            Message::AppendEntries(m) => m.term,
            Message::AppendEntriesReply(m) => m.term,
        }
    }
}

pub fn step(node: Node, event: Event) -> Result<Outcome, StepError> {
    match event {
        Event::ElectionTimeout => {
            if node.is_leader() {
                return Ok(finish(node, Vec::new(), false));
            }
            let (next, sent) = start_election(node);
            Ok(finish(next, sent, true))
        }
        Event::HeartbeatTimeout => {
            let sent = heartbeat(&node);
            Ok(finish(node, sent, false))
        }
        Event::Propose { command } => propose(node, command),
        Event::Receive { from, message } => receive(node, from, message),
    }
}

fn receive(node: Node, from: Uuid, message: Message) -> Result<Outcome, StepError> {
    if !node.peers.iter().any(|p| *p == from) {
        return Err(StepError::UnknownPeer);
    }
    let term = message.term();
    let node = if term > node.current_term {
        Node {
            current_term: term,
            voted_for: None,
            role: Role::Follower { leader: None },
            ..node
        }
    } else {
        node
    };
    match message {
        Message::RequestVote(m) => on_request_vote(node, from, m),
        Message::RequestVoteReply(m) => Ok(on_vote_reply(node, from, m)),
        Message::AppendEntries(m) => on_append_entries(node, from, m),
        Message::AppendEntriesReply(m) => Ok(on_append_reply(node, from, m)),
    }
}

fn term_at(log: &[LogEntry], index: usize) -> u64 {
    if index == 0 {
        0
    } else {
        log[index - 1].term
    }
}

fn is_majority(count: usize, peers: usize) -> bool {
    count * 2 > peers + 1
}

fn finish(node: Node, messages: Vec<Envelope>, reset: bool) -> Outcome {
    let mut applied: Vec<Applied> = Vec::new();
    for i in node.last_applied + 1..node.commit_index + 1 {
        applied.push(Applied {
            index: i,
            entry: node.log[i - 1],
        });
    }
    let commit = node.commit_index;
    Outcome {
        node: Node {
            last_applied: commit,
            ..node
        },
        messages,
        applied,
        reset_election_timer: reset,
    }
}

fn start_election(node: Node) -> (Node, Vec<Envelope>) {
    let term = node.current_term + 1;
    let me = node.id;
    let candidate = Node {
        current_term: term,
        voted_for: Some(me),
        role: Role::Candidate { votes: vec![me] },
        ..node
    };
    if is_majority(1, candidate.peers.len()) {
        return become_leader(candidate);
    }
    let last_index = candidate.log.len();
    let last_term = term_at(&candidate.log, last_index);
    let mut sent: Vec<Envelope> = Vec::new();
    for p in &candidate.peers {
        sent.push(Envelope {
            to: *p,
            message: Message::RequestVote(RequestVote {
                term,
                candidate_id: me,
                last_log_index: last_index,
                last_log_term: last_term,
            }),
        });
    }
    (candidate, sent)
}

fn become_leader(node: Node) -> (Node, Vec<Envelope>) {
    let next = node.log.len() + 1;
    let mut progress: Vec<Progress> = Vec::new();
    for p in &node.peers {
        progress.push(Progress {
            peer: *p,
            next_index: next,
            match_index: 0,
        });
    }
    let leader = Node {
        role: Role::Leader { progress },
        ..node
    };
    let sent = heartbeat(&leader);
    (leader, sent)
}

fn append_to(node: &Node, peer: Uuid, next_index: usize) -> Envelope {
    let prev = next_index - 1;
    let mut entries: Vec<LogEntry> = Vec::new();
    for i in prev..node.log.len() {
        entries.push(node.log[i]);
    }
    Envelope {
        to: peer,
        message: Message::AppendEntries(AppendEntries {
            term: node.current_term,
            leader_id: node.id,
            prev_log_index: prev,
            prev_log_term: term_at(&node.log, prev),
            entries,
            leader_commit: node.commit_index,
        }),
    }
}

fn heartbeat(node: &Node) -> Vec<Envelope> {
    let mut sent: Vec<Envelope> = Vec::new();
    match node.role.clone() {
        Role::Leader { progress } => {
            for p in &progress {
                sent.push(append_to(node, p.peer, p.next_index));
            }
        }
        Role::Follower { leader: _ } => {}
        Role::Candidate { votes: _ } => {}
    }
    sent
}

fn propose(node: Node, command: u64) -> Result<Outcome, StepError> {
    match node.role.clone() {
        Role::Leader { progress } => {
            let mut log = node.log.clone();
            log.push(LogEntry {
                term: node.current_term,
                command,
            });
            let leader = advance_commit(Node { log, ..node }, &progress);
            let sent = heartbeat(&leader);
            Ok(finish(leader, sent, false))
        }
        Role::Follower { leader } => Err(StepError::NotLeader { leader }),
        Role::Candidate { votes: _ } => Err(StepError::NotLeader { leader: None }),
    }
}

// Figure 2, Rules for Servers, Leaders, last bullet, with §5.4.2: the
// largest N > commit_index with a majority of match_index >= N and
// log[N].term == current_term.
fn advance_commit(node: Node, progress: &[Progress]) -> Node {
    let mut n = node.log.len();
    let mut commit = node.commit_index;
    while n > node.commit_index {
        if node.log[n - 1].term == node.current_term {
            let mut count: usize = 1;
            for p in progress {
                if p.match_index >= n {
                    count += 1;
                }
            }
            if is_majority(count, node.peers.len()) {
                commit = n;
                break;
            }
        }
        n -= 1;
    }
    Node {
        commit_index: commit,
        ..node
    }
}

fn reply_vote(node: Node, to: Uuid, granted: bool) -> Outcome {
    let reply = Envelope {
        to,
        message: Message::RequestVoteReply(RequestVoteReply {
            term: node.current_term,
            vote_granted: granted,
        }),
    };
    finish(node, vec![reply], granted)
}

fn on_request_vote(node: Node, from: Uuid, m: RequestVote) -> Result<Outcome, StepError> {
    if m.candidate_id != from {
        return Err(StepError::SenderMismatch);
    }
    if m.term < node.current_term {
        return Ok(reply_vote(node, from, false));
    }
    let free = match node.voted_for {
        None => true,
        Some(v) => v == from,
    };
    let my_last_index = node.log.len();
    let my_last_term = term_at(&node.log, my_last_index);
    let up_to_date = m.last_log_term > my_last_term
        || (m.last_log_term == my_last_term && m.last_log_index >= my_last_index);
    if free && up_to_date {
        let voted = Node {
            voted_for: Some(from),
            ..node
        };
        Ok(reply_vote(voted, from, true))
    } else {
        Ok(reply_vote(node, from, false))
    }
}

fn on_vote_reply(node: Node, from: Uuid, m: RequestVoteReply) -> Outcome {
    match node.role.clone() {
        Role::Candidate { votes } => {
            if m.term != node.current_term || !m.vote_granted {
                return finish(node, Vec::new(), false);
            }
            if votes.iter().any(|v| *v == from) {
                return finish(node, Vec::new(), false);
            }
            let mut more = votes.clone();
            more.push(from);
            if is_majority(more.len(), node.peers.len()) {
                let (leader, sent) = become_leader(node);
                return finish(leader, sent, false);
            }
            finish(
                Node {
                    role: Role::Candidate { votes: more },
                    ..node
                },
                Vec::new(),
                false,
            )
        }
        Role::Follower { leader: _ } => finish(node, Vec::new(), false),
        Role::Leader { progress: _ } => finish(node, Vec::new(), false),
    }
}

fn reply_append(node: Node, to: Uuid, success: bool, index: usize, reset: bool) -> Outcome {
    let reply = Envelope {
        to,
        message: Message::AppendEntriesReply(AppendEntriesReply {
            term: node.current_term,
            success,
            index,
        }),
    };
    finish(node, vec![reply], reset)
}

fn on_append_entries(node: Node, from: Uuid, m: AppendEntries) -> Result<Outcome, StepError> {
    if m.leader_id != from {
        return Err(StepError::SenderMismatch);
    }
    if m.term < node.current_term {
        return Ok(reply_append(node, from, false, m.prev_log_index, false));
    }
    if node.is_leader() {
        return Err(StepError::SecondLeader);
    }
    let node = Node {
        role: Role::Follower { leader: Some(from) },
        ..node
    };
    let prev = m.prev_log_index;
    if prev > node.log.len() || term_at(&node.log, prev) != m.prev_log_term {
        return Ok(reply_append(node, from, false, prev, true));
    }
    let count = m.entries.len();
    let mut first = count;
    for k in 0..count {
        if first == count {
            let pos = prev + k;
            if pos >= node.log.len() || node.log[pos].term != m.entries[k].term {
                first = k;
            }
        }
    }
    let log = if first < count {
        let mut rebuilt: Vec<LogEntry> = Vec::new();
        for i in 0..prev + first {
            rebuilt.push(node.log[i]);
        }
        for k in first..count {
            rebuilt.push(m.entries[k]);
        }
        rebuilt
    } else {
        node.log.clone()
    };
    let last_new = prev + count;
    let commit = if m.leader_commit > node.commit_index {
        node.commit_index.max(m.leader_commit.min(last_new))
    } else {
        node.commit_index
    };
    let next = Node {
        log,
        commit_index: commit,
        ..node
    };
    Ok(reply_append(next, from, true, last_new, true))
}

fn on_append_reply(node: Node, from: Uuid, m: AppendEntriesReply) -> Outcome {
    let progress = match node.role.clone() {
        Role::Leader { progress } => progress,
        Role::Follower { leader: _ } => return finish(node, Vec::new(), false),
        Role::Candidate { votes: _ } => return finish(node, Vec::new(), false),
    };
    if m.term != node.current_term {
        return finish(node, Vec::new(), false);
    }
    let last = node.log.len();
    let mut updated: Vec<Progress> = Vec::new();
    let mut resend: Option<usize> = None;
    for p in &progress {
        if p.peer == from {
            if m.success {
                let matched = p.match_index.max(m.index);
                let next = p.next_index.max(matched + 1);
                updated.push(Progress {
                    peer: p.peer,
                    next_index: next,
                    match_index: matched,
                });
                if next <= last {
                    resend = Some(next);
                }
            } else if m.index + 1 == p.next_index && p.next_index > 1 {
                let next = p.next_index - 1;
                updated.push(Progress {
                    peer: p.peer,
                    next_index: next,
                    match_index: p.match_index,
                });
                resend = Some(next);
            } else {
                updated.push(*p);
            }
        } else {
            updated.push(*p);
        }
    }
    let leader = advance_commit(
        Node {
            role: Role::Leader {
                progress: updated.clone(),
            },
            ..node
        },
        &updated,
    );
    let sent = match resend {
        Some(next) => vec![append_to(&leader, from, next)],
        None => Vec::new(),
    };
    finish(leader, sent, false)
}
