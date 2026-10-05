//! `examples/ssh`, an SSH client from the server's identification to the
//! end of user authentication (RFC 4253, RFC 4252, RFC 8308, OpenSSH's
//! strict key exchange):
//!
//! - each rule comes out as the specifications write it: the version line,
//!   negotiation (first of the client's the server lists; no MAC under an
//!   AEAD cipher), guessed packets, strict key exchange and its sequence
//!   numbers, known_hosts, publickey and password authentication,
//!   re-exchange, packet framing, and the keys to derive;
//! - the generated package agrees with Rust on all of those runs, and on
//!   every two events from a pool appended to a run that stops at each
//!   point of the handshake, whole outcome compared.

use crate::support;

purecrate_canon::fixture!(mod ssh = "../../../examples/ssh/src/lib.rs", "fixtures/ssh_driver.rs");

use ssh::{
    Action, Cipher, Config, ConfigError, Connection, Direction, Event, Failure, FramingError, HostKey, HostKeyPolicy,
    Identity, KexInit, KexMethod, KexReply, KeyType, KnownHost, Mac, Message, Outcome, Phase, SignatureAlgorithm,
};

const OUR_ID: &str = "SSH-2.0-purecrate_0.1";
const SERVER_ID: &str = "SSH-2.0-OpenSSH_9.9p1 Debian-3";
const ED25519: &str = "SHA256:ok5QnjeGYTkI1hEu4ZyFPZYjqILjHbUjL21Ve8Ha2wk";
const OTHER: &str = "SHA256:2bWhXa1yTJ8l0cF8gEj6+vF9o7qkCq9ANtZK6sWbE5o";

fn s(x: &str) -> String {
    x.to_string()
}

fn config(policy: HostKeyPolicy, known: Vec<KnownHost>, guess: bool) -> Config {
    Config {
        identification: s(OUR_ID),
        host: s("example.com"),
        port: 22,
        kex: vec![KexMethod::Mlkem768X25519Sha256, KexMethod::Curve25519Sha256, KexMethod::EcdhSha2Nistp256],
        host_keys: vec![
            SignatureAlgorithm::Ed25519,
            SignatureAlgorithm::EcdsaSha2Nistp256,
            SignatureAlgorithm::RsaSha512,
        ],
        ciphers: vec![Cipher::Chacha20Poly1305, Cipher::Aes256Gcm, Cipher::Aes128Ctr],
        macs: vec![Mac::HmacSha256Etm, Mac::HmacSha256],
        guess,
        policy,
        known_hosts: known,
        identities: vec![
            Identity { key_type: KeyType::Rsa, public_key: s(RSA_BLOB) },
            Identity { key_type: KeyType::Ed25519, public_key: s(ED_BLOB) },
        ],
        password: true,
        password_prompts: 2,
    }
}

fn known(hosts: &str, fingerprint: &str, revoked: bool) -> KnownHost {
    KnownHost { hosts: s(hosts), key_type: s("ssh-ed25519"), fingerprint: s(fingerprint), revoked }
}

/// The usual client: ed25519 key on file, guessing.
fn usual() -> Config {
    config(HostKeyPolicy::Strict, vec![known("example.com,192.0.2.1", ED25519, false)], true)
}

/// OpenSSH 9.9's server KEXINIT (`sshd -T`), with what a test changes.
fn openssh(kex: &str, host_keys: &str, ciphers: &str, macs: &str) -> KexInit {
    KexInit {
        kex: s(kex),
        host_keys: s(host_keys),
        ciphers_client_to_server: s(ciphers),
        ciphers_server_to_client: s(ciphers),
        macs_client_to_server: s(macs),
        macs_server_to_client: s(macs),
        compression_client_to_server: s("none,zlib@openssh.com"),
        compression_server_to_client: s("none,zlib@openssh.com"),
        first_kex_packet_follows: false,
    }
}

const KEX: &str = "mlkem768x25519-sha256,sntrup761x25519-sha512,curve25519-sha256,curve25519-sha256@libssh.org,\
ecdh-sha2-nistp256,ecdh-sha2-nistp384,ecdh-sha2-nistp521,ext-info-s,kex-strict-s-v00@openssh.com";
const HOST_KEYS: &str = "ssh-ed25519,ecdsa-sha2-nistp256,rsa-sha2-512,rsa-sha2-256";
const CIPHERS: &str = "chacha20-poly1305@openssh.com,aes128-ctr,aes192-ctr,aes256-ctr,aes128-gcm@openssh.com,\
aes256-gcm@openssh.com";
const MACS: &str = "umac-64-etm@openssh.com,umac-128-etm@openssh.com,hmac-sha2-256-etm@openssh.com,\
hmac-sha2-512-etm@openssh.com,hmac-sha1-etm@openssh.com,umac-64@openssh.com,hmac-sha2-256,hmac-sha2-512,hmac-sha1";

fn server() -> KexInit {
    openssh(KEX, HOST_KEYS, CIPHERS, MACS)
}

fn reply(fingerprint: &str, signature_valid: bool) -> KexReply {
    KexReply {
        host_key: HostKey { key_type: s("ssh-ed25519"), fingerprint: s(fingerprint) },
        x25519_secret: vec![7; 32],
        exchange_hash: (0..32).collect(),
        signature_valid,
    }
}

fn line(x: &str) -> Event {
    Event::Line(s(x))
}

fn packet(m: Message) -> Event {
    Event::Packet(m)
}

fn failure(methods: &str, partial: bool) -> Event {
    packet(Message::UserauthFailure { can_continue: s(methods), partial_success: partial })
}

const RSA_BLOB: &str = "AAAAB3NzaC1yc2EAAAADAQABAAABAQ";
const ED_BLOB: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIG";

/// PK_OK for the key the algorithm signs with.
fn pk_ok(algorithm: &str) -> Event {
    let blob = if algorithm.starts_with("rsa") { RSA_BLOB } else { ED_BLOB };
    packet(Message::Userauth60 { first: s(algorithm), second: s(blob) })
}

/// The events of a handshake through `upto` of: identification, KEXINIT,
/// reply, NEWKEYS, EXT_INFO, SERVICE_ACCEPT, the "none" failure, PK_OK,
/// success.
fn handshake(upto: usize) -> Vec<Event> {
    let all = vec![
        line(SERVER_ID),
        packet(Message::KexInit(server())),
        packet(Message::KexReply(reply(ED25519, true))),
        packet(Message::NewKeys),
        packet(Message::ExtInfo { server_sig_algs: Some(s("ssh-ed25519,rsa-sha2-256")) }),
        packet(Message::ServiceAccept(s("ssh-userauth"))),
        failure("publickey,password", false),
        pk_ok("rsa-sha2-256"),
        packet(Message::UserauthSuccess),
    ];
    all.into_iter().take(upto).collect()
}

fn with(mut events: Vec<Event>, more: Vec<Event>) -> Vec<Event> {
    events.extend(more);
    events
}

fn ran(o: &Outcome) -> (&Connection, &Vec<Action>) {
    match o {
        Outcome::Ran { connection, actions } => (connection, actions),
        other => panic!("expected a run, got {other:?}"),
    }
}

fn failed(o: &Outcome) -> (usize, &Failure, u32) {
    match o {
        Outcome::Failed { index, failure, code } => (*index, failure, *code),
        other => panic!("expected a failure, got {other:?}"),
    }
}

/// The runs the spec checks below make, for the TS comparison too.
fn scenarios() -> Vec<(Config, Vec<Event>)> {
    let mut out = Vec::new();
    for upto in 0..=9 {
        out.push((usual(), handshake(upto)));
    }
    // Lines before the identification, and identifications refused.
    for id in [SERVER_ID, "SSH-1.99-Cisco-1.25", "SSH-1.5-old", "SSH-2.0-", "SSH-2.0-a b", "SSH-2.0x"] {
        out.push((usual(), vec![line("Welcome"), line(""), line(id)]));
    }
    out.push((usual(), vec![line(&format!("SSH-2.0-{}", "x".repeat(245)))]));
    out.push((usual(), vec![line(&format!("SSH-2.0-{}", "x".repeat(246)))]));
    out.push((usual(), vec![packet(Message::Ignore)]));
    // Negotiation: no MAC under AEAD; MAC needed under CTR; nothing common.
    for (kex, host_keys, ciphers, macs) in [
        (KEX, HOST_KEYS, "aes256-gcm@openssh.com", ""),
        (KEX, HOST_KEYS, "aes128-ctr", "hmac-sha2-256,hmac-sha2-256-etm@openssh.com"),
        (KEX, HOST_KEYS, "aes128-ctr", "hmac-sha1"),
        (KEX, HOST_KEYS, "3des-cbc", MACS),
        ("diffie-hellman-group14-sha256", HOST_KEYS, CIPHERS, MACS),
        (KEX, "ssh-dss", CIPHERS, MACS),
        ("curve25519-sha256,,x", HOST_KEYS, CIPHERS, MACS),
        ("ecdh-sha2-nistp256", "ssh-ed25519", "aes128-ctr,chacha20-poly1305@openssh.com", "hmac-sha2-256"),
    ] {
        out.push((usual(), vec![line(SERVER_ID), packet(Message::KexInit(openssh(kex, host_keys, ciphers, macs)))]));
    }
    // Guesses: ours wrong when the server prefers another method; the
    // server's wrong, so its next kex packet is dropped.
    let curve_first = openssh("curve25519-sha256,mlkem768x25519-sha256", HOST_KEYS, CIPHERS, MACS);
    out.push((usual(), vec![line(SERVER_ID), packet(Message::KexInit(curve_first.clone()))]));
    let server_guess = KexInit { first_kex_packet_follows: true, ..curve_first };
    out.push((
        usual(),
        vec![
            line(SERVER_ID),
            packet(Message::KexInit(server_guess)),
            packet(Message::KexReply(reply(OTHER, false))),
            packet(Message::KexReply(reply(ED25519, true))),
        ],
    ));
    // Strict key exchange: an IGNORE before KEXINIT, or during it.
    out.push((usual(), vec![line(SERVER_ID), packet(Message::Ignore), packet(Message::KexInit(server()))]));
    out.push((usual(), with(handshake(2), vec![packet(Message::Ignore)])));
    let lax = openssh("curve25519-sha256", HOST_KEYS, CIPHERS, MACS);
    out.push((
        usual(),
        vec![line(SERVER_ID), packet(Message::Ignore), packet(Message::KexInit(lax.clone())), packet(Message::Debug)],
    ));
    // The reply: wrong key type, an all-zero X25519 secret, a bad signature.
    let rsa =
        KexReply { host_key: HostKey { key_type: s("ssh-rsa"), fingerprint: s(ED25519) }, ..reply(ED25519, true) };
    let zero = KexReply { x25519_secret: vec![0; 32], ..reply(ED25519, true) };
    for r in [rsa, zero, reply(ED25519, false)] {
        out.push((usual(), with(handshake(2), vec![packet(Message::KexReply(r))])));
    }
    // known_hosts under each policy: known, unknown, changed, revoked, and
    // the `[host]:port` form.
    let entries = [
        vec![known("example.com", ED25519, false)],
        vec![],
        vec![known("example.com", OTHER, false)],
        vec![known("example.com", ED25519, false), known("*", ED25519, true)],
        vec![known("[example.com]:2222", ED25519, false)],
    ];
    for policy in [HostKeyPolicy::Strict, HostKeyPolicy::AcceptNew, HostKeyPolicy::Ask] {
        for entry in &entries {
            for port in [22, 2222] {
                let c = Config { port, ..config(policy, entry.clone(), false) };
                out.push((c, handshake(3)));
            }
        }
    }
    // Asking: yes before and after the server's NEWKEYS, and no.
    let ask = config(HostKeyPolicy::Ask, vec![], false);
    let yes = Event::HostKeyAnswer(true);
    out.push((ask.clone(), with(handshake(3), vec![yes.clone(), packet(Message::NewKeys)])));
    out.push((ask.clone(), with(handshake(4), vec![yes.clone()])));
    out.push((ask.clone(), with(handshake(4), vec![packet(Message::ExtInfo { server_sig_algs: None }), yes])));
    out.push((ask, with(handshake(3), vec![Event::HostKeyAnswer(false)])));
    // Service and authentication.
    out.push((usual(), with(handshake(5), vec![packet(Message::ServiceAccept(s("ssh-connection")))])));
    let rsa_skipped = vec![packet(Message::ExtInfo { server_sig_algs: Some(s("ssh-ed25519")) })];
    out.push((usual(), with(with(handshake(4), rsa_skipped), handshake(9).split_off(5))));
    let tries = vec![
        failure("publickey,password", false),
        failure("publickey,password", false),
        failure("password", true),
        packet(Message::Userauth60 { first: s("Password expired"), second: s("en") }),
        failure("password", false),
        failure("password", false),
    ];
    out.push((usual(), with(handshake(7), tries)));
    out.push((usual(), with(handshake(7), vec![pk_ok("ssh-ed25519")])));
    out.push((usual(), with(handshake(6), vec![packet(Message::UserauthBanner(s("Authorized use only")))])));
    out.push((usual(), with(handshake(6), vec![failure("hostbased", false)])));
    out.push((usual(), with(handshake(6), vec![failure("publickey,,password", false)])));
    // Unknown and connection-layer messages.
    for n in [42, 60, 94] {
        out.push((usual(), with(handshake(6), vec![packet(Message::Other(n))])));
        out.push((usual(), with(handshake(9), vec![packet(Message::Other(n))])));
        out.push((usual(), with(handshake(9), vec![packet(Message::KexInit(server())), packet(Message::Other(n))])));
    }
    // Re-exchange, by the server and by the client, with the same key and
    // another.
    for fingerprint in [ED25519, OTHER] {
        let again = vec![packet(Message::KexReply(reply(fingerprint, true))), packet(Message::NewKeys)];
        out.push((usual(), with(handshake(9), with(vec![packet(Message::KexInit(server()))], again.clone()))));
        out.push((usual(), with(handshake(9), with(vec![Event::Rekey, packet(Message::KexInit(server()))], again))));
    }
    out.push((usual(), with(handshake(7), vec![packet(Message::KexInit(server()))])));
    out.push((usual(), vec![Event::Rekey]));
    out.push((usual(), with(handshake(9), vec![packet(Message::Disconnect { code: 11, description: s("bye") })])));
    // From the independent review: the server's wrong guess drops the next
    // packet of any kind; a later EXT_INFO replaces the first; known_hosts
    // names as OpenSSH writes them; the standard strict marker; PK_OK's key;
    // RSA tried with each algorithm when the server lists none.
    let server_guess =
        KexInit { first_kex_packet_follows: true, ..openssh("ecdh-sha2-nistp256", HOST_KEYS, CIPHERS, MACS) };
    for between in [Message::Ignore, Message::Other(30)] {
        out.push((
            usual(),
            vec![
                line(SERVER_ID),
                packet(Message::KexInit(server_guess.clone())),
                packet(between),
                packet(Message::KexReply(reply(ED25519, true))),
            ],
        ));
    }
    out.push((
        usual(),
        with(
            handshake(5),
            vec![packet(Message::ExtInfo { server_sig_algs: None }), packet(Message::ServiceAccept(s("ssh-userauth")))],
        ),
    ));
    for hosts in ["[example.com]:22", "[example.com]:022", "[example.com]:+2222", "EXAMPLE.com", "[Example.COM]:2222"] {
        for port in [22, 2222] {
            out.push((
                Config { port, ..config(HostKeyPolicy::Strict, vec![known(hosts, ED25519, false)], false) },
                handshake(3),
            ));
        }
    }
    let standard = openssh("curve25519-sha256,kex-strict-s", HOST_KEYS, CIPHERS, MACS);
    out.push((usual(), vec![line(SERVER_ID), packet(Message::Ignore), packet(Message::KexInit(standard))]));
    out.push((
        usual(),
        with(handshake(7), vec![packet(Message::Userauth60 { first: s("rsa-sha2-256"), second: s(ED_BLOB) })]),
    ));
    let no_sig_algs = with(
        handshake(4),
        vec![
            packet(Message::ServiceAccept(s("ssh-userauth"))),
            failure("publickey", false),
            failure("publickey", false),
            failure("publickey", false),
        ],
    );
    out.push((usual(), no_sig_algs));
    // Configurations refused.
    out.push((Config { identification: s("SSH-2.0-bad-name"), ..usual() }, vec![]));
    out.push((Config { identification: s("SSH-1.99-x"), ..usual() }, vec![]));
    out.push((Config { kex: vec![], ..usual() }, vec![]));
    out.push((Config { ciphers: vec![], ..usual() }, vec![]));
    out
}

#[test]
fn the_handshake_follows_the_specifications() {
    // A whole handshake: version, KEXINIT (guessed right, so no second
    // init), NEWKEYS once the key is known, the service, then publickey
    // with the RSA algorithm server-sig-algs names, queried then signed.
    let o = ssh::connect(usual(), handshake(9));
    let (c, actions) = ran(&o);
    assert!(matches!(c.phase, Phase::Established));
    assert_eq!(c.session_id, Some((0..32).collect::<Vec<u8>>()));
    assert!(c.strict, "both sides offered strict key exchange");
    let kinds: Vec<String> =
        actions.iter().map(|a| format!("{a:?}").split(['(', ' ']).next().unwrap().to_string()).collect();
    assert_eq!(
        kinds,
        [
            "Identify",
            "KexInit",
            "KexEcdhInit",
            "NewKeys",
            "ServiceRequest",
            "AuthNone",
            "AuthPublicKey",
            "AuthPublicKey"
        ]
    );
    assert_eq!(actions[2], Action::KexEcdhInit(KexMethod::Mlkem768X25519Sha256));
    assert_eq!(
        actions[7],
        Action::AuthPublicKey { key: 0, algorithm: SignatureAlgorithm::RsaSha256, signed: true },
        "PK_OK, then the same key signed"
    );
    // Strict: the count restarts after each NEWKEYS. Sent after ours:
    // SERVICE_REQUEST, "none", query, signed; received after the server's:
    // EXT_INFO, SERVICE_ACCEPT, FAILURE, PK_OK, SUCCESS.
    assert_eq!((c.send_seq, c.recv_seq), (4, 5));
    let a = c.algorithms.expect("negotiated");
    assert_eq!(a.client_to_server.cipher, Cipher::Chacha20Poly1305);
    assert_eq!(a.client_to_server.mac, None, "an AEAD cipher's MAC is not negotiated");
    // Our offer carries the pseudo-algorithms in the initial KEXINIT only.
    let Action::KexInit(offer) = &actions[1] else { panic!("KEXINIT second") };
    assert_eq!(offer.kex[offer.kex.len() - 2..], [s("ext-info-c"), s("kex-strict-c-v00@openssh.com")]);
    assert!(offer.first_kex_packet_follows);

    let all = scenarios();
    let outcome = |i: usize| ssh::connect(all[i].0.clone(), all[i].1.clone());
    let find = |events: &[Event], config: &Config| {
        all.iter().position(|(c, e)| e == events && c == config).map(outcome).expect("a scenario")
    };

    // RFC 4253 §4.2: lines before the identification; 255 with CR LF.
    let pre = |id: &str| find(&[line("Welcome"), line(""), line(id)], &usual());
    assert!(matches!(ran(&pre(SERVER_ID)).0.phase, Phase::KeyExchange(_)));
    assert!(matches!(ran(&pre("SSH-1.99-Cisco-1.25")).0.phase, Phase::KeyExchange(_)), "1.99 is 2.0 (§5.1)");
    assert_eq!(failed(&pre("SSH-1.5-old")).1, &Failure::UnsupportedVersion);
    assert_eq!(failed(&pre("SSH-1.5-old")).2, 8);
    assert_eq!(failed(&pre("SSH-2.0-")).1, &Failure::BadIdentification);
    assert!(matches!(ran(&pre("SSH-2.0-a b")).0.phase, Phase::KeyExchange(_)), "comments follow a space");
    let long = |n: usize| ssh::connect(usual(), vec![line(&format!("SSH-2.0-{}", "x".repeat(n)))]);
    assert!(matches!(ran(&long(245)).0.phase, Phase::KeyExchange(_)));
    assert_eq!(failed(&long(246)).1, &Failure::IdentificationTooLong);
    assert_eq!(failed(&ssh::connect(usual(), vec![packet(Message::Ignore)])).1, &Failure::UnexpectedLine);

    // §7.1: the client's first the server lists, per direction.
    let negotiated = |kex: &str, host_keys: &str, ciphers: &str, macs: &str| {
        ssh::connect(usual(), vec![line(SERVER_ID), packet(Message::KexInit(openssh(kex, host_keys, ciphers, macs)))])
    };
    let algorithms = |o: &Outcome| match &ran(o).0.phase {
        Phase::KeyExchange(kx) => match &kx.stage {
            ssh::KexStage::AwaitingReply { algorithms, .. } => *algorithms,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let gcm = algorithms(&negotiated(KEX, HOST_KEYS, "aes256-gcm@openssh.com", ""));
    assert_eq!((gcm.client_to_server.cipher, gcm.client_to_server.mac), (Cipher::Aes256Gcm, None));
    let ctr = algorithms(&negotiated(KEX, HOST_KEYS, "aes128-ctr", "hmac-sha2-256,hmac-sha2-256-etm@openssh.com"));
    assert_eq!((ctr.server_to_client.cipher, ctr.server_to_client.mac), (Cipher::Aes128Ctr, Some(Mac::HmacSha256Etm)));
    let fails = |kex, hk, ci, ma| failed(&negotiated(kex, hk, ci, ma)).1.clone();
    assert_eq!(
        fails(KEX, HOST_KEYS, "aes128-ctr", "hmac-sha1"),
        Failure::NoCommonAlgorithm(ssh::Category::Mac(Direction::ClientToServer))
    );
    assert_eq!(
        fails(KEX, HOST_KEYS, "3des-cbc", MACS),
        Failure::NoCommonAlgorithm(ssh::Category::Cipher(Direction::ClientToServer))
    );
    assert_eq!(
        fails("diffie-hellman-group14-sha256", HOST_KEYS, CIPHERS, MACS),
        Failure::NoCommonAlgorithm(ssh::Category::Kex)
    );
    assert_eq!(fails(KEX, "ssh-dss", CIPHERS, MACS), Failure::NoCommonAlgorithm(ssh::Category::HostKey));
    assert_eq!(
        fails("curve25519-sha256,,x", HOST_KEYS, CIPHERS, MACS),
        Failure::BadNameList,
        "RFC 4251 §5: no empty name"
    );
    let ecdh = algorithms(&negotiated(
        "ecdh-sha2-nistp256",
        "ssh-ed25519",
        "aes128-ctr,chacha20-poly1305@openssh.com",
        "hmac-sha2-256",
    ));
    assert_eq!(ecdh.kex, KexMethod::EcdhSha2Nistp256);
    assert_eq!(ecdh.client_to_server.cipher, Cipher::Chacha20Poly1305, "the client's order, not the server's");

    // §7 guesses.
    let curve_first = openssh("curve25519-sha256,mlkem768x25519-sha256", HOST_KEYS, CIPHERS, MACS);
    let o = ssh::connect(usual(), vec![line(SERVER_ID), packet(Message::KexInit(curve_first.clone()))]);
    assert_eq!(
        ran(&o).1[1..],
        [
            Action::KexInit(ssh::our_offer(&ran(&o).0.clone(), true, true)),
            Action::KexEcdhInit(KexMethod::Mlkem768X25519Sha256),
            Action::KexEcdhInit(KexMethod::Mlkem768X25519Sha256),
        ],
        "a wrong guess is followed by the init for what was negotiated"
    );
    let o = find(
        &[
            line(SERVER_ID),
            packet(Message::KexInit(KexInit { first_kex_packet_follows: true, ..curve_first })),
            packet(Message::KexReply(reply(OTHER, false))),
            packet(Message::KexReply(reply(ED25519, true))),
        ],
        &usual(),
    );
    assert_eq!(ran(&o).1.last(), Some(&Action::NewKeys), "the server's wrong guess is dropped unread");

    // Strict key exchange (OpenSSH PROTOCOL, CVE-2023-48795).
    let o = find(&[line(SERVER_ID), packet(Message::Ignore), packet(Message::KexInit(server()))], &usual());
    assert_eq!(failed(&o).1, &Failure::StrictKexViolation(20), "KEXINIT must be the first packet");
    assert_eq!(
        failed(&find(&with(handshake(2), vec![packet(Message::Ignore)]), &usual())).1,
        &Failure::StrictKexViolation(2)
    );
    let lax = openssh("curve25519-sha256", HOST_KEYS, CIPHERS, MACS);
    let o = find(
        &[line(SERVER_ID), packet(Message::Ignore), packet(Message::KexInit(lax)), packet(Message::Debug)],
        &usual(),
    );
    assert!(!ran(&o).0.strict, "the server did not offer it");

    // The reply.
    let reply_fails =
        |r: KexReply| failed(&ssh::connect(usual(), with(handshake(2), vec![packet(Message::KexReply(r))]))).1.clone();
    let rsa =
        KexReply { host_key: HostKey { key_type: s("ssh-rsa"), fingerprint: s(ED25519) }, ..reply(ED25519, true) };
    assert_eq!(reply_fails(rsa), Failure::WrongHostKeyType);
    assert_eq!(reply_fails(KexReply { x25519_secret: vec![0; 32], ..reply(ED25519, true) }), Failure::ZeroSharedSecret);
    assert_eq!(reply_fails(reply(ED25519, false)), Failure::BadSignature);

    // known_hosts.
    let host = |policy, entry: Vec<KnownHost>, port| {
        ssh::connect(Config { port, ..config(policy, entry, false) }, handshake(3))
    };
    let ok = known("example.com", ED25519, false);
    assert_eq!(ran(&host(HostKeyPolicy::Strict, vec![ok.clone()], 22)).1.last(), Some(&Action::NewKeys));
    assert_eq!(failed(&host(HostKeyPolicy::Strict, vec![ok.clone()], 2222)).1, &Failure::HostKeyUnknown);
    let bracketed = known("[example.com]:2222", ED25519, false);
    assert_eq!(ran(&host(HostKeyPolicy::Strict, vec![bracketed], 2222)).1.last(), Some(&Action::NewKeys));
    let learned = ran(&host(HostKeyPolicy::AcceptNew, vec![], 22)).1.clone();
    assert!(matches!(learned[learned.len() - 2], Action::Learn(_)));
    let changed = vec![known("example.com", OTHER, false)];
    for policy in [HostKeyPolicy::Strict, HostKeyPolicy::AcceptNew, HostKeyPolicy::Ask] {
        assert_eq!(failed(&host(policy, changed.clone(), 22)).1, &Failure::HostKeyChanged);
        let revoked = vec![ok.clone(), known("*", ED25519, true)];
        assert_eq!(failed(&host(policy, revoked, 22)).1, &Failure::HostKeyRevoked);
    }
    assert_eq!(failed(&host(HostKeyPolicy::Strict, changed, 22)).2, 9);
    // Asked: the server's NEWKEYS may come before the answer.
    let ask = config(HostKeyPolicy::Ask, vec![], false);
    let o = ssh::connect(ask.clone(), with(handshake(4), vec![Event::HostKeyAnswer(true)]));
    assert!(matches!(ran(&o).0.phase, Phase::Service));
    assert_eq!(
        ran(&o).1[ran(&o).1.len() - 3..],
        [Action::Learn(reply(ED25519, true).host_key), Action::NewKeys, Action::ServiceRequest]
    );
    assert_eq!(
        failed(&ssh::connect(ask, with(handshake(3), vec![Event::HostKeyAnswer(false)]))).1,
        &Failure::HostKeyRejected
    );

    // RFC 4252.
    let o = ssh::connect(usual(), with(handshake(5), vec![packet(Message::ServiceAccept(s("ssh-connection")))]));
    assert_eq!(failed(&o).1, &Failure::WrongService);
    // server-sig-algs without an RSA algorithm: the RSA key is skipped.
    let skipped = with(
        with(handshake(4), vec![packet(Message::ExtInfo { server_sig_algs: Some(s("ssh-ed25519")) })]),
        handshake(9).split_off(5),
    );
    let o = ssh::connect(usual(), skipped);
    assert_eq!(failed(&o).1, &Failure::UnexpectedMessage(60), "PK_OK names the algorithm queried, ssh-ed25519");
    let tries = vec![
        failure("publickey,password", false),
        failure("publickey,password", false),
        failure("password", true),
        packet(Message::Userauth60 { first: s("Password expired"), second: s("en") }),
        failure("password", false),
        failure("password", false),
    ];
    let o = ssh::connect(usual(), with(handshake(7), tries));
    let (index, f, code) = failed(&o);
    // The RSA key (queried by the handshake), the ed25519 key, then two
    // password prompts; PASSWD_CHANGEREQ asks for a new one in between.
    assert_eq!((index, f, code), (11, &Failure::NoMoreAuthMethods, 14));
    let o = ssh::connect(usual(), with(handshake(6), vec![packet(Message::UserauthBanner(s("Authorized use only")))]));
    assert_eq!(ran(&o).1.last(), Some(&Action::Show(s("Authorized use only"))));

    // RFC 4253 §11.4, §7.1: an unknown number is answered with its sequence
    // number; connection messages wait for authentication.
    let o = ssh::connect(usual(), with(handshake(6), vec![packet(Message::Other(42))]));
    assert_eq!(ran(&o).1.last(), Some(&Action::Unimplemented(2)), "EXT_INFO 0, SERVICE_ACCEPT 1, then this");
    assert_eq!(
        failed(&ssh::connect(usual(), with(handshake(6), vec![packet(Message::Other(94))]))).1,
        &Failure::UnexpectedMessage(94)
    );
    assert!(ran(&ssh::connect(usual(), with(handshake(9), vec![packet(Message::Other(94))]))).1.len() == 8);
    let mid_kex = with(handshake(9), vec![packet(Message::KexInit(server())), packet(Message::Other(94))]);
    assert_eq!(failed(&ssh::connect(usual(), mid_kex)).1, &Failure::UnexpectedMessage(94));

    // §9: the session identifier stays; the host key must not change.
    let again = |fingerprint| {
        vec![
            packet(Message::KexReply(KexReply { exchange_hash: vec![9; 32], ..reply(fingerprint, true) })),
            packet(Message::NewKeys),
        ]
    };
    let o = ssh::connect(usual(), with(handshake(9), with(vec![packet(Message::KexInit(server()))], again(ED25519))));
    let c = ran(&o).0;
    assert!(matches!(c.phase, Phase::Established));
    assert_eq!(c.session_id, Some((0..32).collect::<Vec<u8>>()));
    let o = ssh::connect(usual(), with(handshake(9), with(vec![packet(Message::KexInit(server()))], again(OTHER))));
    assert_eq!(failed(&o).1, &Failure::HostKeyChangedOnRekey);
    assert_eq!(failed(&ssh::connect(usual(), vec![Event::Rekey])).1, &Failure::Misuse);

    // Configurations.
    let refused = |c: Config| match ssh::connect(c, vec![]) {
        Outcome::Refused(e) => e,
        other => panic!("{other:?}"),
    };
    assert_eq!(refused(Config { identification: s("SSH-2.0-bad-name"), ..usual() }), ConfigError::BadIdentification);
    assert_eq!(refused(Config { kex: vec![], ..usual() }), ConfigError::NoKex);
}

#[test]
fn the_independent_review_findings_hold() {
    // RFC 4253 §7.1: after the server's wrong guess, the next packet of any
    // kind is dropped; the reply after it is read.
    let server_guess =
        KexInit { first_kex_packet_follows: true, ..openssh("ecdh-sha2-nistp256", HOST_KEYS, CIPHERS, MACS) };
    for between in [Message::Ignore, Message::Other(30)] {
        let o = ssh::connect(
            usual(),
            vec![
                line(SERVER_ID),
                packet(Message::KexInit(server_guess.clone())),
                packet(between),
                packet(Message::KexReply(reply(ED25519, true))),
            ],
        );
        assert_eq!(ran(&o).1.last(), Some(&Action::NewKeys), "the reply after the dropped packet is read");
        assert!(!ran(&o).1.iter().any(|a| matches!(a, Action::Unimplemented(_))), "dropped silently");
    }

    // RFC 8308 §2.4: a later EXT_INFO replaces the first.
    let o = ssh::connect(usual(), with(handshake(5), vec![packet(Message::ExtInfo { server_sig_algs: None })]));
    assert_eq!(ran(&o).0.server_sig_algs, None);

    // known_hosts as OpenSSH writes names: bare for port 22, `[host]:port`
    // in plain decimal otherwise; host names without ASCII case.
    let trust = |hosts: &str, port: u16| {
        let key = reply(ED25519, true).host_key;
        ssh::judge_host_key(&vec![known(hosts, ED25519, false)], "example.com", port, &key)
    };
    assert_eq!(trust("[example.com]:22", 22), ssh::Trust::Unknown);
    assert_eq!(trust("[example.com]:022", 22), ssh::Trust::Unknown);
    assert_eq!(trust("[example.com]:+2222", 2222), ssh::Trust::Unknown);
    assert_eq!(trust("[example.com]:02222", 2222), ssh::Trust::Unknown);
    assert_eq!(trust("[Example.COM]:2222", 2222), ssh::Trust::Known);
    assert_eq!(trust("EXAMPLE.com", 22), ssh::Trust::Known);

    // The standard strict marker is the v00 one (draft §5.1).
    let standard = openssh("curve25519-sha256,kex-strict-s", HOST_KEYS, CIPHERS, MACS);
    let o = ssh::connect(usual(), vec![line(SERVER_ID), packet(Message::KexInit(standard.clone()))]);
    assert!(ran(&o).0.strict);
    let o = ssh::connect(usual(), vec![line(SERVER_ID), packet(Message::Ignore), packet(Message::KexInit(standard))]);
    assert_eq!(failed(&o).1, &Failure::StrictKexViolation(20));

    // RFC 4252 §7: PK_OK echoes the key queried, not another.
    let wrong_key = vec![packet(Message::Userauth60 { first: s("rsa-sha2-256"), second: s(ED_BLOB) })];
    assert_eq!(failed(&ssh::connect(usual(), with(handshake(7), wrong_key))).1, &Failure::UnexpectedMessage(60));

    // RFC 8308 §3.1: with no server-sig-algs, an RSA key refused with
    // rsa-sha2-512 is tried with rsa-sha2-256, then the next key.
    let attempts = with(
        handshake(4),
        vec![
            packet(Message::ServiceAccept(s("ssh-userauth"))),
            failure("publickey", false),
            failure("publickey", false),
            failure("publickey", false),
        ],
    );
    let o = ssh::connect(usual(), attempts);
    let queries: Vec<(usize, SignatureAlgorithm)> = ran(&o)
        .1
        .iter()
        .filter_map(|a| match a {
            Action::AuthPublicKey { key, algorithm, signed: false } => Some((*key, *algorithm)),
            _ => None,
        })
        .collect();
    assert_eq!(
        queries,
        [(0, SignatureAlgorithm::RsaSha512), (0, SignatureAlgorithm::RsaSha256), (1, SignatureAlgorithm::Ed25519)]
    );
}

#[test]
fn framing_and_keys_follow_the_specifications() {
    let (opened, _) = ssh::start(usual()).expect("starts");
    // RFC 4253 §6: before NEWKEYS, blocks of 8 over the whole packet.
    assert_eq!(ssh::check_packet(&opened, Direction::ServerToClient, 12, 4), Ok(7));
    assert_eq!(ssh::check_packet(&opened, Direction::ServerToClient, 12, 3), Err(FramingError::PaddingTooShort));
    assert_eq!(ssh::check_packet(&opened, Direction::ServerToClient, 13, 4), Err(FramingError::Misaligned));
    assert_eq!(ssh::check_packet(&opened, Direction::ServerToClient, 5, 4), Err(FramingError::TooShort));
    assert_eq!(ssh::check_packet(&opened, Direction::ServerToClient, 262_148, 4), Err(FramingError::TooLong));
    // Under chacha20-poly1305 the length is not counted.
    let o = ssh::connect(usual(), handshake(4));
    let keyed = ran(&o).0;
    assert_eq!(ssh::check_packet(keyed, Direction::ServerToClient, 16, 4), Ok(11));
    assert_eq!(ssh::check_packet(keyed, Direction::ServerToClient, 12, 4), Err(FramingError::Misaligned));

    // §7.2: chacha20-poly1305 under mlkem768x25519-sha256 needs two 64-byte
    // keys, two SHA-256 rounds each, and nothing else.
    let plan = ssh::key_plan(&keyed.algorithms.expect("negotiated"));
    let lengths: Vec<(char, usize, usize)> = plan.iter().map(|k| (k.letter, k.length, k.rounds)).collect();
    assert_eq!(lengths, [('A', 0, 0), ('B', 0, 0), ('C', 64, 2), ('D', 64, 2), ('E', 0, 0), ('F', 0, 0)]);
}

/// Events any point of the handshake may meet.
fn pool() -> Vec<Event> {
    vec![
        line(SERVER_ID),
        line("noise"),
        packet(Message::KexInit(server())),
        packet(Message::KexReply(reply(ED25519, true))),
        packet(Message::KexReply(reply(OTHER, true))),
        packet(Message::NewKeys),
        packet(Message::Ignore),
        packet(Message::Other(42)),
        packet(Message::Other(94)),
        packet(Message::ServiceAccept(s("ssh-userauth"))),
        packet(Message::ExtInfo { server_sig_algs: Some(s("rsa-sha2-512")) }),
        failure("publickey,password", true),
        pk_ok("rsa-sha2-512"),
        packet(Message::UserauthSuccess),
        packet(Message::UserauthBanner(s("hi"))),
        Event::HostKeyAnswer(true),
        Event::Rekey,
        packet(Message::Disconnect { code: 11, description: s("bye") }),
    ]
}

#[test]
fn ssh_matches_rust() {
    let pool = pool();
    let ask = config(HostKeyPolicy::Ask, vec![], false);
    support::equivalence("ssh", ssh::SOURCE, |cases| {
        for (config, events) in scenarios() {
            cases.push(case!(ssh::connect(config, events)));
        }
        for upto in 0..=9 {
            for (i, a) in pool.iter().enumerate() {
                for b in &pool {
                    // The asking client on every other first event, so both
                    // reach each point.
                    let config = if i % 2 == 0 { usual() } else { ask.clone() };
                    let events = with(handshake(upto), vec![a.clone(), b.clone()]);
                    cases.push(case!(ssh::connect(config, events)));
                }
            }
        }
        let (opened, _) = ssh::start(usual()).expect("starts");
        let keyed = match ssh::connect(usual(), handshake(4)) {
            Outcome::Ran { connection, .. } => connection,
            other => panic!("{other:?}"),
        };
        for c in [&opened, &keyed] {
            for d in [Direction::ClientToServer, Direction::ServerToClient] {
                for (length, padding) in [(12, 4), (12, 3), (13, 4), (16, 4), (5, 4), (262_144, 15), (262_148, 4)] {
                    cases.push(case!(ssh::check_packet(c, d, length, padding)));
                }
            }
        }
        cases.push(case!(ssh::key_plan(&keyed.algorithms.expect("negotiated"))));
        for line in [SERVER_ID, "SSH-1.99-Cisco-1.25", "SSH-2.0-x\u{0}", "SSH-2.0-", "ssh-2.0-x", "SSH-3.0-x y"] {
            cases.push(case!(ssh::parse_identification(line)));
        }
        for list in ["", "a,b", "a,,b", ",", "a@b@c", "x y", "ext-info-c"] {
            cases.push(case!(ssh::name_list(list)));
        }
    });
}
