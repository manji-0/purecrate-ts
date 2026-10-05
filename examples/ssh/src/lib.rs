// SSH client handshake, from the server's identification line to the end of
// user authentication: which message the client sends next, and which it
// must refuse, at each point. The UI (a host key prompt) and the transport
// share this state machine.
//
// Specifications:
// - RFC 4253 (transport) §4.2 protocol version exchange, §5.1 "1.99",
//   §6 binary packet (lengths, padding, alignment), §6.4 sequence numbers,
//   §7 key exchange (§7.1 algorithm negotiation and guessed packets,
//   §7.2 key derivation, §7.3 NEWKEYS), §9 re-exchange, §10 service
//   request, §11.1 DISCONNECT reason codes, §11.4 UNIMPLEMENTED.
// - RFC 4251 §5 name-list, §6 algorithm names.
// - RFC 4252 (user authentication) §5 requests and responses, §5.1
//   partial success, §5.2 the "none" method, §5.4 banner, §7 publickey
//   (query, PK_OK, signed request), §8 password and PASSWD_CHANGEREQ.
// - RFC 5656 §4 and RFC 8731 ECDH; RFC 8731 §3 the all-zero shared secret.
// - mlkem768x25519-sha256 (draft-ietf-sshm-mlkem-hybrid-kex) and
//   sntrup761x25519-sha512: hybrid methods whose K is encoded as a string.
// - RFC 8308 §2 ext-info-c, §2.4 when EXT_INFO may come, §3.1
//   server-sig-algs.
// - RFC 8332 rsa-sha2-256 / rsa-sha2-512 over "ssh-rsa" keys.
// - Strict key exchange (the Terrapin fix, CVE-2023-48795):
//   draft-ietf-sshm-strict-kex-02 §3 and OpenSSH PROTOCOL. §3.1: the client
//   offers both "kex-strict-c" and "kex-strict-c-v00@openssh.com", and
//   strict mode is on only for a matching pair (standard with standard,
//   v00 with v00), so offering both makes either server name enough.
// - OpenSSH PROTOCOL: chacha20-poly1305 and aes-gcm, whose MAC is not
//   negotiated.
// - sshd(8) SSH_KNOWN_HOSTS FILE FORMAT: comma-separated patterns with '*'
//   and '?', '!' negation (a negated match means the line does not apply),
//   "[host]:port", hashed names ("|1|salt|hash", HMAC-SHA1 of the name
//   keyed by the salt, as OpenSSH's hostfile.c computes it), @revoked.
//   As OpenSSH does, a pattern is matched, without ASCII case, against the
//   name as written for the port: "host" for 22, "[host]:port" otherwise,
//   so "*" names every port and "*.com" only port 22.
//
// Everything impure is outside: the caller reads and writes the socket,
// strips CR LF from the lines before the identification, decodes and
// encodes packets, generates the ephemeral key pair for the method an
// `Action::KexEcdhInit` names, computes the shared secret, the exchange hash
// H and the host key's fingerprint, verifies the server's signature over H,
// derives the keys (`key_plan` says how much of each), encrypts, signs the
// publickey request, prompts for the password, and appends the host key an
// `Action::Learn` names to known_hosts.
//
// Policy choices, not the specifications':
// - the client offers ext-info-c and strict key exchange in the initial
//   KEXINIT only, as OpenSSH does;
// - host key algorithms with a known_hosts key for this host come first in
//   the initial offer (as OpenSSH orders them); a re-exchange offers only
//   the algorithm in use, and the server's key must not change;
// - the signature and the shared secret are checked before the host key is
//   looked up, so the user is never asked about a key the server did not
//   prove it holds (OpenSSH asks first);
// - a user key is tried with the first of its signature algorithms the
//   server lists in server-sig-algs, or, when the server sent none, with
//   each of them in turn (RFC 8308 §3.1); a key with no listed algorithm is
//   skipped. A later EXT_INFO replaces an earlier one;
// - @revoked refuses the key for every host, whatever the line's host list
//   (stricter than OpenSSH, which scopes it to the hosts the line names);
// - a hashed known_hosts line that cannot be read (not "|1|", bad base64,
//   a salt or hash that is not 20 bytes) for the key's type makes a key
//   no readable line names undecidable: refused under StrictHostKeyChecking
//   yes and accept-new (never learned), asked about under ask. OpenSSH
//   skips such a line, and accept-new would then learn a key the line may
//   have recorded otherwise;
// - strict key exchange lasts, for the client, until both NEWKEYS are
//   through: a non-KEX message after the server's NEWKEYS while the user
//   is still asked about the host key is refused, except EXT_INFO as the
//   packet right after that NEWKEYS (RFC 8308 §2.4 puts it there; it
//   arrives under the new keys and asks for no answer). OpenSSH's client
//   never reaches this point: it asks before sending its NEWKEYS;
// - EXT_INFO is accepted in two places only: as the first packet after
//   the server's first NEWKEYS, and during authentication. RFC 8308 §2.4's
//   second opportunity is "immediately preceding" USERAUTH_SUCCESS, which a
//   client cannot see coming, so any EXT_INFO while authenticating is
//   taken (a later one replaces an earlier one); in the service request,
//   after a re-exchange's NEWKEYS, or once established, it is refused;
// - a server's empty name-list is read as offering nothing (RFC 4253 §7.1
//   asks the sender for at least one name; an empty MAC list beside an
//   AEAD cipher costs nothing); the client never sends one: the
//   configuration must name at least one of each;
// - packets under 16 bytes with the length field are refused (RFC 4253
//   §6), under EtM and AEAD too (OpenSSH accepts 12 there);
// - at most 1024 lines before the identification (OpenSSH's limit) and
//   packets of at most 256 KiB (OpenSSH's PACKET_MAX_SIZE).
//
// Left out on purpose:
// - the connection protocol (RFC 4254): a message numbered 80 or more after
//   authentication is the connection layer's, and the state machine only
//   counts it;
// - @cert-authority, CheckHostIP; the caller lower-cases the host name;
// - compression other than "none", Diffie-Hellman group exchange, GSSAPI,
//   keyboard-interactive, host-based authentication;
// - re-exchange started by the client before authentication, and data or
//   time limits for re-exchange (§9): the caller sends `Event::Rekey`.

// ---------------------------------------------------------------------------
// Limits and names
// ---------------------------------------------------------------------------

/// RFC 4253 §4.2: 255 characters including the CR LF the caller strips.
pub const MAX_IDENTIFICATION_LEN: usize = 253;
/// Lines a server may send before its identification (RFC 4253 §4.2 sets
/// no limit; OpenSSH stops reading at 1024).
pub const MAX_PREAMBLE_LINES: u32 = 1024;
/// RFC 4251 §6.
pub const MAX_NAME_LEN: usize = 64;
/// RFC 4253 §6.1 asks for at least 35000; OpenSSH's PACKET_MAX_SIZE.
pub const MAX_PACKET_LEN: u32 = 256 * 1024;
/// RFC 4253 §6.
pub const MIN_PADDING: u32 = 4;
pub const MIN_BLOCK: u32 = 8;
/// RFC 4253 §6: the smallest packet, its length field included.
pub const MIN_PACKET: u32 = 16;

/// The pre-standard strict markers OpenSSH sends.
pub const STRICT_KEX_CLIENT: &str = "kex-strict-c-v00@openssh.com";
pub const STRICT_KEX_SERVER: &str = "kex-strict-s-v00@openssh.com";
/// The standard names (draft-ietf-sshm-strict-kex-02 §3.1). Each pairs only
/// with its own kind: the standard name on one side and only the v00 name
/// on the other MUST NOT enable strict key exchange.
pub const STRICT_KEX_CLIENT_STANDARD: &str = "kex-strict-c";
pub const STRICT_KEX_SERVER_STANDARD: &str = "kex-strict-s";
pub const EXT_INFO_CLIENT: &str = "ext-info-c";
pub const USERAUTH_SERVICE: &str = "ssh-userauth";

// ---------------------------------------------------------------------------
// Algorithms
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KexMethod {
    Mlkem768X25519Sha256,
    Sntrup761X25519Sha512,
    Curve25519Sha256,
    EcdhSha2Nistp256,
    EcdhSha2Nistp384,
    EcdhSha2Nistp521,
}

pub fn kex_name(k: KexMethod) -> String {
    String::from(match k {
        KexMethod::Mlkem768X25519Sha256 => "mlkem768x25519-sha256",
        KexMethod::Sntrup761X25519Sha512 => "sntrup761x25519-sha512",
        KexMethod::Curve25519Sha256 => "curve25519-sha256",
        KexMethod::EcdhSha2Nistp256 => "ecdh-sha2-nistp256",
        KexMethod::EcdhSha2Nistp384 => "ecdh-sha2-nistp384",
        KexMethod::EcdhSha2Nistp521 => "ecdh-sha2-nistp521",
    })
}

/// The hash of the exchange hash and of the key derivation (RFC 4253 §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

pub fn kex_hash(k: KexMethod) -> HashAlgorithm {
    match k {
        KexMethod::Mlkem768X25519Sha256 | KexMethod::Curve25519Sha256 | KexMethod::EcdhSha2Nistp256 => {
            HashAlgorithm::Sha256
        }
        KexMethod::EcdhSha2Nistp384 => HashAlgorithm::Sha384,
        KexMethod::Sntrup761X25519Sha512 | KexMethod::EcdhSha2Nistp521 => HashAlgorithm::Sha512,
    }
}

pub fn hash_len(h: HashAlgorithm) -> usize {
    match h {
        HashAlgorithm::Sha256 => 32,
        HashAlgorithm::Sha384 => 48,
        HashAlgorithm::Sha512 => 64,
    }
}

/// How the shared secret K enters the exchange hash and the key
/// derivation: an mpint for the classical methods (RFC 5656 §4, RFC 8731),
/// a string for the hybrid ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretEncoding {
    Mpint,
    Bytes,
}

pub fn secret_encoding_for(k: KexMethod) -> SecretEncoding {
    match k {
        KexMethod::Mlkem768X25519Sha256 | KexMethod::Sntrup761X25519Sha512 => SecretEncoding::Bytes,
        _ => SecretEncoding::Mpint,
    }
}

/// Methods with an X25519 part, whose shared secret must not be all zeros
/// (RFC 8731 §3).
fn uses_x25519(k: KexMethod) -> bool {
    matches!(k, KexMethod::Mlkem768X25519Sha256 | KexMethod::Sntrup761X25519Sha512 | KexMethod::Curve25519Sha256)
}

/// Host key and user key signature algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    Ed25519,
    EcdsaSha2Nistp256,
    RsaSha512,
    RsaSha256,
}

pub fn signature_name(a: SignatureAlgorithm) -> String {
    String::from(match a {
        SignatureAlgorithm::Ed25519 => "ssh-ed25519",
        SignatureAlgorithm::EcdsaSha2Nistp256 => "ecdsa-sha2-nistp256",
        SignatureAlgorithm::RsaSha512 => "rsa-sha2-512",
        SignatureAlgorithm::RsaSha256 => "rsa-sha2-256",
    })
}

/// The key type a signature algorithm signs with, as known_hosts and the
/// key blob name it: both RSA algorithms use "ssh-rsa" keys (RFC 8332 §3).
pub fn key_type_name(a: SignatureAlgorithm) -> String {
    String::from(match a {
        SignatureAlgorithm::Ed25519 => "ssh-ed25519",
        SignatureAlgorithm::EcdsaSha2Nistp256 => "ecdsa-sha2-nistp256",
        SignatureAlgorithm::RsaSha512 | SignatureAlgorithm::RsaSha256 => "ssh-rsa",
    })
}

/// The type of a user key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    Ed25519,
    EcdsaNistp256,
    Rsa,
}

/// A user key: its type, and its public key blob as the caller encodes it
/// in a publickey request (PK_OK echoes it, RFC 4252 §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub key_type: KeyType,
    pub public_key: String,
}

/// The signature algorithms a user key can sign with, preferred first.
pub fn signature_algorithms(t: KeyType) -> Vec<SignatureAlgorithm> {
    match t {
        KeyType::Ed25519 => vec![SignatureAlgorithm::Ed25519],
        KeyType::EcdsaNistp256 => vec![SignatureAlgorithm::EcdsaSha2Nistp256],
        KeyType::Rsa => vec![SignatureAlgorithm::RsaSha512, SignatureAlgorithm::RsaSha256],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cipher {
    Chacha20Poly1305,
    Aes256Gcm,
    Aes128Gcm,
    Aes256Ctr,
    Aes128Ctr,
}

pub fn cipher_name(c: Cipher) -> String {
    String::from(match c {
        Cipher::Chacha20Poly1305 => "chacha20-poly1305@openssh.com",
        Cipher::Aes256Gcm => "aes256-gcm@openssh.com",
        Cipher::Aes128Gcm => "aes128-gcm@openssh.com",
        Cipher::Aes256Ctr => "aes256-ctr",
        Cipher::Aes128Ctr => "aes128-ctr",
    })
}

/// Key and IV lengths in bytes, block size, and whether the cipher
/// authenticates the packet itself (OpenSSH's cipher table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CipherSpec {
    pub key_len: usize,
    pub iv_len: usize,
    pub block: u32,
    pub aead: bool,
}

pub fn cipher_params(c: Cipher) -> CipherSpec {
    match c {
        Cipher::Chacha20Poly1305 => CipherSpec { key_len: 64, iv_len: 0, block: 8, aead: true },
        Cipher::Aes256Gcm => CipherSpec { key_len: 32, iv_len: 12, block: 16, aead: true },
        Cipher::Aes128Gcm => CipherSpec { key_len: 16, iv_len: 12, block: 16, aead: true },
        Cipher::Aes256Ctr => CipherSpec { key_len: 32, iv_len: 16, block: 16, aead: false },
        Cipher::Aes128Ctr => CipherSpec { key_len: 16, iv_len: 16, block: 16, aead: false },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mac {
    HmacSha256Etm,
    HmacSha512Etm,
    HmacSha256,
    HmacSha512,
}

pub fn mac_name(m: Mac) -> String {
    String::from(match m {
        Mac::HmacSha256Etm => "hmac-sha2-256-etm@openssh.com",
        Mac::HmacSha512Etm => "hmac-sha2-512-etm@openssh.com",
        Mac::HmacSha256 => "hmac-sha2-256",
        Mac::HmacSha512 => "hmac-sha2-512",
    })
}

pub fn mac_key_len(m: Mac) -> usize {
    match m {
        Mac::HmacSha256Etm | Mac::HmacSha256 => 32,
        Mac::HmacSha512Etm | Mac::HmacSha512 => 64,
    }
}

/// Encrypt-then-MAC: the packet length travels in the clear.
pub fn mac_is_etm(m: Mac) -> bool {
    matches!(m, Mac::HmacSha256Etm | Mac::HmacSha512Etm)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}

/// What protects one direction. `mac` is `None` under an AEAD cipher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transform {
    pub cipher: Cipher,
    pub mac: Option<Mac>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Algorithms {
    pub kex: KexMethod,
    pub host_key: SignatureAlgorithm,
    pub client_to_server: Transform,
    pub server_to_client: Transform,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// RFC 4253 §11.1.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    HostNotAllowedToConnect = 1,
    ProtocolError = 2,
    KeyExchangeFailed = 3,
    Reserved = 4,
    MacError = 5,
    CompressionError = 6,
    ServiceNotAvailable = 7,
    ProtocolVersionNotSupported = 8,
    HostKeyNotVerifiable = 9,
    ConnectionLost = 10,
    ByApplication = 11,
    TooManyConnections = 12,
    AuthCancelledByUser = 13,
    NoMoreAuthMethodsAvailable = 14,
    IllegalUserName = 15,
}

/// The code a DISCONNECT carries.
pub fn reason_code(r: DisconnectReason) -> u32 {
    r as u32
}

/// The list negotiation found nothing in common for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Kex,
    HostKey,
    Cipher(Direction),
    Mac(Direction),
    Compression(Direction),
}

/// Why the client gives up. The caller sends DISCONNECT with
/// `reason_for` and closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    IdentificationTooLong,
    BadIdentification,
    UnsupportedVersion,
    TooManyPreambleLines,
    /// A line after the identification, or a packet before it.
    UnexpectedLine,
    UnexpectedMessage(u8),
    /// A message strict key exchange forbids during the initial exchange,
    /// or a KEXINIT that was not the first packet.
    StrictKexViolation(u8),
    BadNameList,
    NoCommonAlgorithm(Category),
    WrongHostKeyType,
    ZeroSharedSecret,
    BadSignature,
    HostKeyUnknown,
    HostKeyChanged,
    HostKeyRevoked,
    /// A hashed known_hosts line for the key's type could not be read, and
    /// no other line decides (see `Trust::Undecidable`).
    KnownHostsUnreadable,
    HostKeyRejected,
    HostKeyChangedOnRekey,
    WrongService,
    NoMoreAuthMethods,
    /// An event the caller should not have sent in this phase.
    Misuse,
}

pub fn reason_for(f: &Failure) -> DisconnectReason {
    match f {
        Failure::UnsupportedVersion => DisconnectReason::ProtocolVersionNotSupported,
        Failure::NoCommonAlgorithm(_)
        | Failure::WrongHostKeyType
        | Failure::ZeroSharedSecret
        | Failure::BadSignature => DisconnectReason::KeyExchangeFailed,
        Failure::HostKeyUnknown
        | Failure::HostKeyChanged
        | Failure::HostKeyRevoked
        | Failure::KnownHostsUnreadable
        | Failure::HostKeyRejected
        | Failure::HostKeyChangedOnRekey => DisconnectReason::HostKeyNotVerifiable,
        Failure::WrongService => DisconnectReason::ServiceNotAvailable,
        Failure::NoMoreAuthMethods => DisconnectReason::NoMoreAuthMethodsAvailable,
        Failure::Misuse => DisconnectReason::ByApplication,
        _ => DisconnectReason::ProtocolError,
    }
}

// ---------------------------------------------------------------------------
// Version exchange (RFC 4253 §4.2)
// ---------------------------------------------------------------------------

/// `SSH-protoversion-softwareversion SP comments`, without the CR LF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identification {
    pub proto_version: String,
    pub software_version: String,
    pub comments: Option<String>,
}

pub fn parse_identification(line: &str) -> Result<Identification, Failure> {
    if line.len() > MAX_IDENTIFICATION_LEN {
        return Err(Failure::IdentificationTooLong);
    }
    let rest = line.strip_prefix("SSH-").ok_or(Failure::BadIdentification)?;
    let (proto, rest) = rest.split_once('-').ok_or(Failure::BadIdentification)?;
    // "1.99" is a server that also speaks version 1; a version 2 client
    // reads it as "2.0" (§5.1).
    if proto != "2.0" && proto != "1.99" {
        return Err(Failure::UnsupportedVersion);
    }
    let (software, comments) = match rest.split_once(' ') {
        Some((s, c)) => (s, Some(String::from(c))),
        None => (rest, None),
    };
    // Printable US-ASCII without whitespace; no NUL anywhere. The '-' the
    // RFC also rules out is accepted, as OpenSSH does ("SSH-1.99-Cisco-1.25").
    let software_ok = !software.is_empty() && software.bytes().all(|b| matches!(b, 0x21..=0x7e));
    if !software_ok || line.bytes().any(|b| b == 0) {
        return Err(Failure::BadIdentification);
    }
    Ok(Identification { proto_version: String::from(proto), software_version: String::from(software), comments })
}

// ---------------------------------------------------------------------------
// Name-lists and negotiation (RFC 4251 §5, §6; RFC 4253 §7.1)
// ---------------------------------------------------------------------------

/// A comma-separated name-list; "" is the empty list.
pub fn name_list(s: &str) -> Result<Vec<String>, Failure> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    s.split(',').map(algorithm_name).collect()
}

/// Non-empty, at most 64 printable US-ASCII characters without spaces, and
/// at most one '@' (`name@domain`).
fn algorithm_name(s: &str) -> Result<String, Failure> {
    let ok = !s.is_empty()
        && s.len() <= MAX_NAME_LEN
        && s.bytes().all(|b| matches!(b, 0x21..=0x7e))
        && s.bytes().filter(|b| *b == b'@').count() <= 1;
    if ok {
        Ok(String::from(s))
    } else {
        Err(Failure::BadNameList)
    }
}

fn listed(list: &Vec<String>, name: &str) -> bool {
    list.iter().any(|n| n.as_str() == name)
}

/// The server's KEXINIT, its name-lists as received (the cookie and the
/// language lists play no part).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KexInit {
    pub kex: String,
    pub host_keys: String,
    pub ciphers_client_to_server: String,
    pub ciphers_server_to_client: String,
    pub macs_client_to_server: String,
    pub macs_server_to_client: String,
    pub compression_client_to_server: String,
    pub compression_server_to_client: String,
    pub first_kex_packet_follows: bool,
}

/// The client's KEXINIT: the same lists in both directions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub kex: Vec<String>,
    pub host_keys: Vec<String>,
    pub ciphers: Vec<String>,
    pub macs: Vec<String>,
    pub compression: Vec<String>,
    pub first_kex_packet_follows: bool,
}

/// The host key algorithms offered: in the initial exchange, those with a
/// known_hosts key for this host first; in a re-exchange, the one in use.
fn host_key_order(c: &Connection, initial: bool) -> Vec<SignatureAlgorithm> {
    if !initial {
        if let Some(a) = &c.algorithms {
            return vec![a.host_key];
        }
    }
    let mut known: Vec<SignatureAlgorithm> = Vec::new();
    let mut rest: Vec<SignatureAlgorithm> = Vec::new();
    for a in c.config.host_keys.iter() {
        let key_type = key_type_name(*a);
        let has_key = c.config.known_hosts.iter().any(|k| {
            !k.revoked
                && k.key_type == key_type
                && matches!(names_host(&k.hosts, &c.config.host, c.config.port), LineMatch::Names)
        });
        if has_key {
            known.push(*a);
        } else {
            rest.push(*a);
        }
    }
    for a in rest.iter() {
        known.push(*a);
    }
    known
}

/// The strict markers the initial KEXINIT offers: both, for the widest
/// reach (draft-ietf-sshm-strict-kex-02 §3.1).
fn strict_markers() -> Vec<String> {
    vec![String::from(STRICT_KEX_CLIENT_STANDARD), String::from(STRICT_KEX_CLIENT)]
}

/// `guess`: a guessed KEX_ECDH_INIT follows.
pub fn our_offer(c: &Connection, initial: bool, guess: bool) -> Offer {
    let mut kex: Vec<String> = c.config.kex.iter().map(|k| kex_name(*k)).collect();
    if initial {
        kex.push(String::from(EXT_INFO_CLIENT));
        for marker in strict_markers() {
            kex.push(marker);
        }
    }
    Offer {
        kex,
        host_keys: host_key_order(c, initial).iter().map(|a| signature_name(*a)).collect(),
        ciphers: c.config.ciphers.iter().map(|x| cipher_name(*x)).collect(),
        macs: c.config.macs.iter().map(|m| mac_name(*m)).collect(),
        compression: vec![String::from("none")],
        first_kex_packet_follows: guess,
    }
}

fn pick_kex(ours: &Vec<KexMethod>, theirs: &Vec<String>) -> Option<KexMethod> {
    for k in ours.iter() {
        if listed(theirs, &kex_name(*k)) {
            return Some(*k);
        }
    }
    None
}

fn pick_signature(ours: &Vec<SignatureAlgorithm>, theirs: &Vec<String>) -> Option<SignatureAlgorithm> {
    for a in ours.iter() {
        if listed(theirs, &signature_name(*a)) {
            return Some(*a);
        }
    }
    None
}

fn pick_cipher(ours: &Vec<Cipher>, theirs: &Vec<String>) -> Option<Cipher> {
    for x in ours.iter() {
        if listed(theirs, &cipher_name(*x)) {
            return Some(*x);
        }
    }
    None
}

fn pick_mac(ours: &Vec<Mac>, theirs: &Vec<String>) -> Option<Mac> {
    for m in ours.iter() {
        if listed(theirs, &mac_name(*m)) {
            return Some(*m);
        }
    }
    None
}

fn pick_transform(
    config: &Config,
    ciphers: &str,
    macs: &str,
    compression: &str,
    d: Direction,
) -> Result<Transform, Failure> {
    let cipher =
        pick_cipher(&config.ciphers, &name_list(ciphers)?).ok_or(Failure::NoCommonAlgorithm(Category::Cipher(d)))?;
    let their_macs = name_list(macs)?;
    // An AEAD cipher authenticates the packet; the MAC list is not consulted.
    let mac = match (cipher_params(cipher).aead, pick_mac(&config.macs, &their_macs)) {
        (true, _) => None,
        (false, Some(m)) => Some(m),
        (false, None) => return Err(Failure::NoCommonAlgorithm(Category::Mac(d))),
    };
    if !listed(&name_list(compression)?, "none") {
        return Err(Failure::NoCommonAlgorithm(Category::Compression(d)));
    }
    Ok(Transform { cipher, mac })
}

/// RFC 4253 §7.1: for each list, the first of the client's algorithms the
/// server also lists. Every method here needs a signature-capable host key,
/// and every host key algorithm here is one.
pub fn negotiate(c: &Connection, initial: bool, peer: &KexInit) -> Result<Algorithms, Failure> {
    let host_key = pick_signature(&host_key_order(c, initial), &name_list(&peer.host_keys)?)
        .ok_or(Failure::NoCommonAlgorithm(Category::HostKey))?;
    let kex = pick_kex(&c.config.kex, &name_list(&peer.kex)?).ok_or(Failure::NoCommonAlgorithm(Category::Kex))?;
    let client_to_server = pick_transform(
        &c.config,
        &peer.ciphers_client_to_server,
        &peer.macs_client_to_server,
        &peer.compression_client_to_server,
        Direction::ClientToServer,
    )?;
    let server_to_client = pick_transform(
        &c.config,
        &peer.ciphers_server_to_client,
        &peer.macs_server_to_client,
        &peer.compression_server_to_client,
        Direction::ServerToClient,
    )?;
    Ok(Algorithms { kex, host_key, client_to_server, server_to_client })
}

fn first_is(list: &Vec<String>, name: &str) -> bool {
    !list.is_empty() && list[0].as_str() == name
}

/// RFC 4253 §7: a guessed packet is right when both sides prefer the same
/// kex method and the same host key algorithm (the other lists must agree
/// anyway, or the exchange fails).
fn same_preference(c: &Connection, initial: bool, peer: &KexInit) -> Result<bool, Failure> {
    let host_keys = host_key_order(c, initial);
    let kex = name_list(&peer.kex)?;
    let theirs = name_list(&peer.host_keys)?;
    Ok(first_is(&kex, &kex_name(c.config.kex[0])) && first_is(&theirs, &signature_name(host_keys[0])))
}

// ---------------------------------------------------------------------------
// Host keys (known_hosts)
// ---------------------------------------------------------------------------

/// The server's host key: its type and the fingerprint the caller computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKey {
    pub key_type: String,
    pub fingerprint: String,
}

/// One known_hosts line: `hosts` is the comma-separated host list, each
/// `host` for port 22 or `[host]:port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHost {
    pub hosts: String,
    pub key_type: String,
    pub fingerprint: String,
    pub revoked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Known,
    Unknown,
    /// A key of the same type is listed for this host, and it is another.
    Changed,
    Revoked,
    /// No readable line names this host with a key of this type, and a
    /// hashed line for the type could not be read.
    Undecidable,
}

/// StrictHostKeyChecking yes, accept-new, ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyPolicy {
    Strict,
    AcceptNew,
    Ask,
}

/// Whether a known_hosts line's host list names the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineMatch {
    Names,
    Other,
    /// A hashed name that is not "|1|salt|hash" with a 20-byte salt and
    /// hash in base64.
    Unreadable,
}

/// sshd(8): each pattern in turn; a negated one that matches means the
/// line does not apply, whatever else matched (OpenSSH `match_hostname`).
/// A hashed name is the whole field and takes no operators.
fn names_host(hosts: &str, host: &str, port: u16) -> LineMatch {
    let name = lookup_name(host, port);
    if hosts.starts_with("|") {
        return hashed_names(hosts, &name);
    }
    let mut positive = false;
    for pattern in hosts.split(',') {
        match pattern.strip_prefix("!") {
            Some(negated) => {
                if glob(&lowered(negated), &name) {
                    return LineMatch::Other;
                }
            }
            None => {
                if glob(&lowered(pattern), &name) {
                    positive = true;
                }
            }
        }
    }
    if positive {
        LineMatch::Names
    } else {
        LineMatch::Other
    }
}

fn lower(b: u32) -> u32 {
    if matches!(b, 65..=90) {
        b + 32
    } else {
        b
    }
}

fn lowered(s: &str) -> Vec<u32> {
    s.bytes().map(|b| lower(u32::from(b))).collect()
}

/// The name as OpenSSH writes and looks it up (`misc.c` put_host_port):
/// the host for port 22, `[host]:port` in plain decimal for any other; its
/// bytes, ASCII lower-cased.
fn lookup_name(host: &str, port: u16) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    if port != 22 {
        out.push(u32::from(b'['));
    }
    for b in host.bytes() {
        out.push(lower(u32::from(b)));
    }
    if port != 22 {
        out.push(u32::from(b']'));
        out.push(u32::from(b':'));
        let p = u32::from(port);
        let mut div: u32 = 1;
        while div * 10 <= p {
            div *= 10;
        }
        while div > 0 {
            out.push(u32::from(b'0') + (p / div) % 10);
            div /= 10;
        }
    }
    out
}

/// `*` any run of bytes, `?` one byte, anything else itself (OpenSSH
/// `match_pattern`), with the last `*` backtracked to.
fn glob(pattern: &Vec<u32>, name: &Vec<u32>) -> bool {
    let star = u32::from(b'*');
    let mut p: usize = 0;
    let mut n: usize = 0;
    let mut last_star: Option<usize> = None;
    let mut resume: usize = 0;
    while n < name.len() {
        if p < pattern.len() && pattern[p] == star {
            last_star = Some(p);
            resume = n;
            p += 1;
        } else if matches_one(pattern, p, name[n]) {
            p += 1;
            n += 1;
        } else {
            match last_star {
                Some(s) => {
                    p = s + 1;
                    resume += 1;
                    n = resume;
                }
                None => return false,
            }
        }
    }
    while p < pattern.len() && pattern[p] == star {
        p += 1;
    }
    p == pattern.len()
}

/// Whether `pattern[p]` exists and matches the byte `b` alone.
fn matches_one(pattern: &Vec<u32>, p: usize, b: u32) -> bool {
    if p >= pattern.len() {
        return false;
    }
    let any = u32::from(b'?');
    pattern[p] == any || pattern[p] == b
}

/// "|1|" base64(salt) "|" base64(HMAC-SHA1(salt, name)) (OpenSSH
/// hostfile.c `host_hash`, salt and hash 20 bytes each).
fn hashed_names(field: &str, name: &Vec<u32>) -> LineMatch {
    let parts = match field.strip_prefix("|1|") {
        Some(rest) => rest.split_once('|'),
        None => None,
    };
    let decoded = match parts {
        Some((s, h)) => (base64_decode(s), base64_decode(h)),
        None => return LineMatch::Unreadable,
    };
    match decoded {
        (Some(salt), Some(hash)) => {
            if salt.len() != 20 || hash.len() != 20 {
                return LineMatch::Unreadable;
            }
            let mac = hmac_sha1(&salt, name);
            for i in 0..20usize {
                if mac[i] != hash[i] {
                    return LineMatch::Other;
                }
            }
            LineMatch::Names
        }
        _ => LineMatch::Unreadable,
    }
}

/// The base64 alphabet (RFC 4648 §4).
fn base64_value(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some(u32::from(b - b'A')),
        b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Base64 with padding (RFC 4648 §4) to bytes, one per u32; None when the
/// length is not a multiple of 4 or a character is out of place.
fn base64_decode(s: &str) -> Option<Vec<u32>> {
    if s.len() % 4 != 0 {
        return None;
    }
    let mut out: Vec<u32> = Vec::new();
    let mut acc: u32 = 0;
    let mut held: u32 = 0;
    let mut padding: usize = 0;
    for b in s.bytes() {
        if b == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return None;
        }
        let v = base64_value(b)?;
        acc = (acc << 6) | v;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((acc >> held) & 0xff);
            acc &= (1 << held) - 1;
        }
    }
    if padding > 2 {
        return None;
    }
    Some(out)
}

/// ROTL^n(x) (FIPS 180-4 §3.2), for 0 < n < 32.
fn rotl(x: u32, n: u32) -> u32 {
    (x << n) | (x >> (32 - n))
}

/// SHA-1 (FIPS 180-4 §5.1.1, §6.1) of bytes held one per u32, as its 20
/// bytes. The bit length is a u32: the messages here are under 400 bytes.
fn sha1(message: &Vec<u32>) -> Vec<u32> {
    let mut bytes: Vec<u32> = Vec::new();
    let mut bits: u32 = 0;
    for b in message.iter() {
        bytes.push(*b);
        bits += 8;
    }
    bytes.push(0x80);
    while bytes.len() % 64 != 56 {
        bytes.push(0);
    }
    bytes.push(0);
    bytes.push(0);
    bytes.push(0);
    bytes.push(0);
    bytes.push(bits >> 24);
    bytes.push((bits >> 16) & 0xff);
    bytes.push((bits >> 8) & 0xff);
    bytes.push(bits & 0xff);
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xefcdab89;
    let mut h2: u32 = 0x98badcfe;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xc3d2e1f0;
    let mut block: usize = 0;
    while block < bytes.len() {
        let mut w: Vec<u32> = Vec::new();
        for t in 0..80usize {
            if t < 16 {
                let i = block + 4 * t;
                w.push((bytes[i] << 24) | (bytes[i + 1] << 16) | (bytes[i + 2] << 8) | bytes[i + 3]);
            } else {
                w.push(rotl(w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16], 1));
            }
        }
        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        for t in 0..80usize {
            let f = if t < 20 {
                (b & c) | (!b & d)
            } else if t < 40 || t >= 60 {
                b ^ c ^ d
            } else {
                (b & c) | (b & d) | (c & d)
            };
            let k: u32 = if t < 20 {
                0x5a827999
            } else if t < 40 {
                0x6ed9eba1
            } else if t < 60 {
                0x8f1bbcdc
            } else {
                0xca62c1d6
            };
            let temp = rotl(a, 5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[t]);
            e = d;
            d = c;
            c = rotl(b, 30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
        block += 64;
    }
    let mut out: Vec<u32> = Vec::new();
    for h in vec![h0, h1, h2, h3, h4] {
        out.push(h >> 24);
        out.push((h >> 16) & 0xff);
        out.push((h >> 8) & 0xff);
        out.push(h & 0xff);
    }
    out
}

/// HMAC-SHA1 (RFC 2104) with a key of at most 64 bytes.
fn hmac_sha1(key: &Vec<u32>, message: &Vec<u32>) -> Vec<u32> {
    let mut inner: Vec<u32> = Vec::new();
    let mut outer: Vec<u32> = Vec::new();
    for i in 0..64usize {
        let k = if i < key.len() { key[i] } else { 0 };
        inner.push(k ^ 0x36);
        outer.push(k ^ 0x5c);
    }
    for b in message.iter() {
        inner.push(*b);
    }
    for b in sha1(&inner) {
        outer.push(b);
    }
    sha1(&outer)
}

/// A revoked key is refused for every host (sshd(8), @revoked).
pub fn judge_host_key(known: &Vec<KnownHost>, host: &str, port: u16, key: &HostKey) -> Trust {
    let revoked = known.iter().any(|k| k.revoked && k.key_type == key.key_type && k.fingerprint == key.fingerprint);
    if revoked {
        return Trust::Revoked;
    }
    let mut trust = Trust::Unknown;
    let mut unreadable = false;
    for k in known.iter() {
        if !k.revoked && k.key_type == key.key_type {
            match names_host(&k.hosts, host, port) {
                LineMatch::Names => {
                    if k.fingerprint == key.fingerprint {
                        return Trust::Known;
                    }
                    trust = Trust::Changed;
                }
                LineMatch::Unreadable => unreadable = true,
                LineMatch::Other => {}
            }
        }
    }
    if unreadable && matches!(trust, Trust::Unknown) {
        return Trust::Undecidable;
    }
    trust
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The client's identification line, without CR LF.
    pub identification: String,
    pub host: String,
    pub port: u16,
    pub kex: Vec<KexMethod>,
    pub host_keys: Vec<SignatureAlgorithm>,
    pub ciphers: Vec<Cipher>,
    pub macs: Vec<Mac>,
    /// Send KEX_ECDH_INIT for the preferred method with the KEXINIT.
    pub guess: bool,
    pub policy: HostKeyPolicy,
    pub known_hosts: Vec<KnownHost>,
    /// The user's keys, tried in order.
    pub identities: Vec<Identity>,
    pub password: bool,
    pub password_prompts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    BadIdentification,
    NoKex,
    NoHostKeys,
    NoCiphers,
    /// RFC 4253 §7.1: each name-list holds at least one name, the MAC
    /// list included (it is sent even when every cipher is AEAD).
    NoMacs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KexStage {
    /// Waiting for the server's KEXINIT. `guessed`: our KEX_ECDH_INIT for
    /// the preferred method went out with our KEXINIT.
    AwaitingKexInit { guessed: bool },
    /// `ignore_next`: the server guessed wrong, and its next kex packet is
    /// dropped (§7).
    AwaitingReply { algorithms: Algorithms, ignore_next: bool },
    /// The user is asked about the host key; the server's NEWKEYS may come
    /// in meanwhile.
    AwaitingApproval { algorithms: Algorithms, host_key: HostKey, peer_new_keys: bool },
    /// Our NEWKEYS is sent.
    AwaitingNewKeys { algorithms: Algorithms },
}

/// Where a key exchange returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resume {
    Service,
    Authenticating(Auth),
    Established,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KexState {
    pub stage: KexStage,
    pub resume: Resume,
    pub initial: bool,
}

/// The request whose answer is awaited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// The "none" method, which lists the methods that can continue.
    Probe,
    KeyQuery {
        key: usize,
        algorithm: SignatureAlgorithm,
    },
    KeySigned {
        key: usize,
        algorithm: SignatureAlgorithm,
    },
    Password,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auth {
    pub pending: Pending,
    /// The first identity not yet tried, and the first of its signature
    /// algorithms not yet tried with it.
    pub next_key: usize,
    pub next_alg: usize,
    pub password_tries: u32,
    pub partial_successes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Version {
        preamble_lines: u32,
    },
    KeyExchange(KexState),
    /// SERVICE_REQUEST "ssh-userauth" sent.
    Service,
    Authenticating(Auth),
    Established,
    Closed {
        code: u32,
        description: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub config: Config,
    pub phase: Phase,
    /// RFC 4253 §6.4, per direction; strict key exchange resets them to 0
    /// after each NEWKEYS.
    pub send_seq: u32,
    pub recv_seq: u32,
    pub strict: bool,
    /// V_S for the exchange hash.
    pub server_identification: Option<String>,
    /// The first exchange hash; unchanged by re-exchange (§7.2).
    pub session_id: Option<Vec<u8>>,
    pub host_key: Option<HostKey>,
    pub algorithms: Option<Algorithms>,
    /// What protects each direction now; `None` before the first NEWKEYS.
    pub outbound: Option<Transform>,
    pub inbound: Option<Transform>,
    /// RFC 8308 §3.1, once the server sends EXT_INFO.
    pub server_sig_algs: Option<Vec<String>>,
    /// The next packet is the first after the server's first NEWKEYS, where
    /// EXT_INFO may come (RFC 8308 §2.4).
    pub ext_info_next: bool,
}

// ---------------------------------------------------------------------------
// Events and actions
// ---------------------------------------------------------------------------

/// The server's KEX_ECDH_REPLY (or the hybrid reply), with what the caller
/// computed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KexReply {
    pub host_key: HostKey,
    /// The X25519 shared secret, for the methods with an X25519 part.
    pub x25519_secret: Vec<u8>,
    pub exchange_hash: Vec<u8>,
    /// The server's signature over the exchange hash verifies with the host
    /// key under the negotiated host key algorithm.
    pub signature_valid: bool,
}

/// A decoded packet from the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Disconnect {
        code: u32,
        description: String,
    },
    Ignore,
    Unimplemented {
        seq: u32,
    },
    Debug,
    ServiceAccept(String),
    ExtInfo {
        server_sig_algs: Option<String>,
    },
    KexInit(KexInit),
    NewKeys,
    KexReply(KexReply),
    UserauthFailure {
        can_continue: String,
        partial_success: bool,
    },
    UserauthSuccess,
    UserauthBanner(String),
    /// Number 60 means what the method in flight says (RFC 4252 §7, §8):
    /// PK_OK (algorithm, key blob) or PASSWD_CHANGEREQ (prompt, language).
    /// Both begin with two strings.
    Userauth60 {
        first: String,
        second: String,
    },
    /// Any other message number.
    Other(u8),
}

pub fn message_number(m: &Message) -> u8 {
    match m {
        Message::Disconnect { .. } => 1,
        Message::Ignore => 2,
        Message::Unimplemented { .. } => 3,
        Message::Debug => 4,
        Message::ServiceAccept(_) => 6,
        Message::ExtInfo { .. } => 7,
        Message::KexInit(_) => 20,
        Message::NewKeys => 21,
        Message::KexReply(_) => 31,
        Message::UserauthFailure { .. } => 51,
        Message::UserauthSuccess => 52,
        Message::UserauthBanner(_) => 53,
        Message::Userauth60 { .. } => 60,
        Message::Other(n) => *n,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A line before the server's identification, or the identification.
    Line(String),
    Packet(Message),
    /// The user's answer to `Action::AskHostKey`.
    HostKeyAnswer(bool),
    /// Start a re-exchange (§9), once established.
    Rekey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send this identification line, then CR LF.
    Identify(String),
    KexInit(Offer),
    /// Generate an ephemeral key for this method and send its public part.
    KexEcdhInit(KexMethod),
    NewKeys,
    ServiceRequest,
    AuthNone,
    /// A publickey request for `identities[key]`; `signed` asks the caller
    /// to sign it over the session identifier (RFC 4252 §7).
    AuthPublicKey {
        key: usize,
        algorithm: SignatureAlgorithm,
        signed: bool,
    },
    AuthPassword,
    /// The server asks for a new password with this prompt.
    ChangePassword(String),
    Unimplemented(u32),
    /// Show the user this host key and ask whether to connect.
    AskHostKey(HostKey),
    /// Append this host key to known_hosts for the configured host.
    Learn(HostKey),
    /// Show the user this banner.
    Show(String),
}

fn is_packet(a: &Action) -> bool {
    !matches!(a, Action::Identify(_) | Action::AskHostKey(_) | Action::Learn(_) | Action::Show(_))
}

// ---------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------

/// Validates the configuration and sends the identification.
pub fn start(config: Config) -> Result<(Connection, Vec<Action>), ConfigError> {
    let id = parse_identification(&config.identification).map_err(|_| ConfigError::BadIdentification)?;
    // What the client sends is held to the RFC: "2.0", no '-' in the
    // software version.
    if id.proto_version.as_str() != "2.0" || id.software_version.contains("-") {
        return Err(ConfigError::BadIdentification);
    }
    if config.kex.is_empty() {
        return Err(ConfigError::NoKex);
    }
    if config.host_keys.is_empty() {
        return Err(ConfigError::NoHostKeys);
    }
    if config.ciphers.is_empty() {
        return Err(ConfigError::NoCiphers);
    }
    if config.macs.is_empty() {
        return Err(ConfigError::NoMacs);
    }
    let line = config.identification.clone();
    let c = Connection {
        config,
        phase: Phase::Version { preamble_lines: 0 },
        send_seq: 0,
        recv_seq: 0,
        strict: false,
        server_identification: None,
        session_id: None,
        host_key: None,
        algorithms: None,
        outbound: None,
        inbound: None,
        server_sig_algs: None,
        ext_info_next: false,
    };
    Ok((c, vec![Action::Identify(line)]))
}

pub fn step(c: Connection, event: Event) -> Result<(Connection, Vec<Action>), Failure> {
    let result = match event {
        Event::Line(line) => on_line(c, &line),
        Event::Packet(m) => on_packet(c, m),
        Event::HostKeyAnswer(yes) => on_answer(c, yes),
        Event::Rekey => on_rekey(c),
    };
    let (next, actions) = result?;
    Ok(sent(next, actions))
}

/// Counts the packets sent; under strict key exchange the count restarts
/// after NEWKEYS.
fn sent(c: Connection, actions: Vec<Action>) -> (Connection, Vec<Action>) {
    let mut seq = c.send_seq;
    for a in actions.iter() {
        if is_packet(a) {
            seq = seq.wrapping_add(1);
        }
        if matches!(a, Action::NewKeys) && c.strict {
            seq = 0;
        }
    }
    (Connection { send_seq: seq, ..c }, actions)
}

fn on_line(c: Connection, line: &str) -> Result<(Connection, Vec<Action>), Failure> {
    let lines = match &c.phase {
        Phase::Version { preamble_lines } => *preamble_lines,
        _ => return Err(Failure::UnexpectedLine),
    };
    if !line.starts_with("SSH-") {
        if lines == MAX_PREAMBLE_LINES {
            return Err(Failure::TooManyPreambleLines);
        }
        return Ok((Connection { phase: Phase::Version { preamble_lines: lines + 1 }, ..c }, Vec::new()));
    }
    parse_identification(line)?;
    let c = Connection { server_identification: Some(String::from(line)), ..c };
    let (stage, actions) = open_kex(&c, true);
    Ok((
        Connection { phase: Phase::KeyExchange(KexState { stage, resume: Resume::Service, initial: true }), ..c },
        actions,
    ))
}

/// Our KEXINIT, and with it our guess when configured.
fn open_kex(c: &Connection, initial: bool) -> (KexStage, Vec<Action>) {
    let offer = our_offer(c, initial, c.config.guess);
    if c.config.guess {
        (
            KexStage::AwaitingKexInit { guessed: true },
            vec![Action::KexInit(offer), Action::KexEcdhInit(c.config.kex[0])],
        )
    } else {
        (KexStage::AwaitingKexInit { guessed: false }, vec![Action::KexInit(offer)])
    }
}

fn on_rekey(c: Connection) -> Result<(Connection, Vec<Action>), Failure> {
    if !matches!(c.phase, Phase::Established) {
        return Err(Failure::Misuse);
    }
    let (stage, actions) = open_kex(&c, false);
    Ok((
        Connection { phase: Phase::KeyExchange(KexState { stage, resume: Resume::Established, initial: false }), ..c },
        actions,
    ))
}

fn on_packet(c: Connection, m: Message) -> Result<(Connection, Vec<Action>), Failure> {
    match c.phase {
        Phase::Closed { .. } => return Ok((c, Vec::new())),
        Phase::Version { .. } => return Err(Failure::UnexpectedLine),
        _ => {}
    }
    let seq = c.recv_seq;
    let window = c.ext_info_next;
    let c = Connection { recv_seq: seq.wrapping_add(1), ext_info_next: false, ..c };
    match m {
        Message::Disconnect { code, description } => {
            Ok((Connection { phase: Phase::Closed { code, description }, ..c }, Vec::new()))
        }
        // RFC 8308 §2.4's first opportunity; the second, during
        // authentication, is `in_auth`'s.
        Message::ExtInfo { server_sig_algs } if window => ext_info(c, server_sig_algs),
        _ => in_phase(c, m, seq),
    }
}

fn in_phase(c: Connection, m: Message, seq: u32) -> Result<(Connection, Vec<Action>), Failure> {
    match c.phase.clone() {
        Phase::KeyExchange(kx) => in_kex(c, kx, m, seq),
        Phase::Service => in_service(c, m, seq),
        Phase::Authenticating(auth) => in_auth(c, auth, m, seq),
        Phase::Established => match m {
            Message::KexInit(peer) => peer_kex(c, Resume::Established, peer, seq),
            Message::Ignore | Message::Debug | Message::Unimplemented { .. } => Ok((c, Vec::new())),
            Message::Other(n) => other(c, n, seq, false, true),
            _ => Err(Failure::UnexpectedMessage(message_number(&m))),
        },
        _ => Ok((c, Vec::new())),
    }
}

/// A message the handshake does not decode (RFC 4253 §7.1, §11.4): numbers
/// 50 and up are refused while the server is in a key exchange, numbers 80
/// and up belong to the connection layer once authenticated and are
/// refused before, and the rest are answered with UNIMPLEMENTED.
fn other(
    c: Connection,
    n: u8,
    seq: u32,
    peer_in_kex: bool,
    established: bool,
) -> Result<(Connection, Vec<Action>), Failure> {
    if n >= 50 && peer_in_kex {
        return Err(Failure::UnexpectedMessage(n));
    }
    if n >= 80 {
        if established {
            return Ok((c, Vec::new()));
        }
        return Err(Failure::UnexpectedMessage(n));
    }
    Ok((c, vec![Action::Unimplemented(seq)]))
}

/// The server starts a re-exchange: our KEXINIT answers its own.
fn peer_kex(c: Connection, resume: Resume, peer: KexInit, seq: u32) -> Result<(Connection, Vec<Action>), Failure> {
    let actions = vec![Action::KexInit(our_offer(&c, false, false))];
    on_kex_init(
        c,
        KexState { stage: KexStage::AwaitingKexInit { guessed: false }, resume, initial: false },
        false,
        peer,
        seq,
        actions,
    )
}

fn on_kex_init(
    c: Connection,
    kx: KexState,
    guessed: bool,
    peer: KexInit,
    seq: u32,
    sent_before: Vec<Action>,
) -> Result<(Connection, Vec<Action>), Failure> {
    let strict = if kx.initial {
        let theirs = name_list(&peer.kex)?;
        let ours = strict_markers();
        // draft-ietf-sshm-strict-kex-02 §3.1: a matching pair only.
        let standard = listed(&ours, STRICT_KEX_CLIENT_STANDARD) && listed(&theirs, STRICT_KEX_SERVER_STANDARD);
        let v00 = listed(&ours, STRICT_KEX_CLIENT) && listed(&theirs, STRICT_KEX_SERVER);
        standard || v00
    } else {
        c.strict
    };
    // Strict key exchange: KEXINIT is the first packet received.
    if strict && kx.initial && seq != 0 {
        return Err(Failure::StrictKexViolation(20));
    }
    let algorithms = negotiate(&c, kx.initial, &peer)?;
    let same = same_preference(&c, kx.initial, &peer)?;
    let mut actions = sent_before;
    if !(guessed && same) {
        actions.push(Action::KexEcdhInit(algorithms.kex));
    }
    let stage = KexStage::AwaitingReply { algorithms, ignore_next: peer.first_kex_packet_follows && !same };
    Ok((Connection { strict, phase: Phase::KeyExchange(KexState { stage, ..kx }), ..c }, actions))
}

fn in_kex(c: Connection, kx: KexState, m: Message, seq: u32) -> Result<(Connection, Vec<Action>), Failure> {
    // The server guessed wrong and sent a guessed packet: the next packet,
    // whatever it is, is dropped unread (RFC 4253 §7.1).
    match kx.stage.clone() {
        KexStage::AwaitingReply { algorithms, ignore_next: true } => {
            let stage = KexStage::AwaitingReply { algorithms, ignore_next: false };
            return Ok((Connection { phase: Phase::KeyExchange(KexState { stage, ..kx }), ..c }, Vec::new()));
        }
        _ => {}
    }
    let server_keyed = matches!(kx.stage, KexStage::AwaitingApproval { peer_new_keys: true, .. });
    let kex_message = matches!(m, Message::KexInit(_) | Message::KexReply(_) | Message::NewKeys);
    // Until our own NEWKEYS too (draft-ietf-sshm-strict-kex-02 §3.2): the
    // server's NEWKEYS does not end the initial exchange while the user is
    // asked. The EXT_INFO allowed right after it is taken in `on_packet`.
    if c.strict && kx.initial && !kex_message {
        return Err(Failure::StrictKexViolation(message_number(&m)));
    }
    // The server is between its KEXINIT and its NEWKEYS.
    let peer_in_kex = !server_keyed && !matches!(kx.stage, KexStage::AwaitingKexInit { .. });
    let established = matches!(kx.resume, Resume::Established);
    match (kx.stage.clone(), m) {
        (KexStage::AwaitingKexInit { guessed }, Message::KexInit(peer)) => {
            on_kex_init(c, kx, guessed, peer, seq, Vec::new())
        }
        (KexStage::AwaitingReply { algorithms, ignore_next: false }, Message::KexReply(reply)) => {
            on_reply(c, kx, algorithms, reply)
        }
        (KexStage::AwaitingApproval { algorithms, host_key, peer_new_keys: false }, Message::NewKeys) => {
            let c = peer_keyed(c, &algorithms);
            let stage = KexStage::AwaitingApproval { algorithms, host_key, peer_new_keys: true };
            Ok((Connection { phase: Phase::KeyExchange(KexState { stage, ..kx }), ..c }, Vec::new()))
        }
        (KexStage::AwaitingNewKeys { algorithms }, Message::NewKeys) => {
            Ok(finish_kex(peer_keyed(c, &algorithms), kx, Vec::new()))
        }
        (_, Message::Ignore | Message::Debug | Message::Unimplemented { .. }) => Ok((c, Vec::new())),
        (_, Message::Other(n)) => other(c, n, seq, peer_in_kex, established && !peer_in_kex),
        (_, m) => Err(Failure::UnexpectedMessage(message_number(&m))),
    }
}

/// The server's NEWKEYS: what it sends next is under the new keys.
fn peer_keyed(c: Connection, algorithms: &Algorithms) -> Connection {
    let recv_seq: u32 = if c.strict { 0 } else { c.recv_seq };
    let first = c.inbound.is_none();
    Connection { inbound: Some(algorithms.server_to_client), recv_seq, ext_info_next: first, ..c }
}

fn on_reply(
    c: Connection,
    kx: KexState,
    algorithms: Algorithms,
    reply: KexReply,
) -> Result<(Connection, Vec<Action>), Failure> {
    if reply.host_key.key_type != key_type_name(algorithms.host_key) {
        return Err(Failure::WrongHostKeyType);
    }
    if uses_x25519(algorithms.kex) && reply.x25519_secret.iter().all(|b| *b == 0) {
        return Err(Failure::ZeroSharedSecret);
    }
    if !reply.signature_valid {
        return Err(Failure::BadSignature);
    }
    let session_id = match &c.session_id {
        Some(id) => id.clone(),
        None => reply.exchange_hash.clone(),
    };
    let c = Connection { session_id: Some(session_id), algorithms: Some(algorithms), ..c };
    let key = reply.host_key;
    if !kx.initial {
        let same = match &c.host_key {
            Some(k) => k.key_type == key.key_type && k.fingerprint == key.fingerprint,
            None => false,
        };
        if !same {
            return Err(Failure::HostKeyChangedOnRekey);
        }
        return Ok(keyed(c, kx, algorithms, Vec::new()));
    }
    let c = Connection { host_key: Some(key.clone()), ..c };
    match judge_host_key(&c.config.known_hosts, &c.config.host, c.config.port, &key) {
        Trust::Known => Ok(keyed(c, kx, algorithms, Vec::new())),
        Trust::Revoked => Err(Failure::HostKeyRevoked),
        Trust::Changed => Err(Failure::HostKeyChanged),
        Trust::Unknown => match c.config.policy {
            HostKeyPolicy::Strict => Err(Failure::HostKeyUnknown),
            HostKeyPolicy::AcceptNew => Ok(keyed(c, kx, algorithms, vec![Action::Learn(key)])),
            HostKeyPolicy::Ask => Ok(ask(c, kx, algorithms, key)),
        },
        Trust::Undecidable => match c.config.policy {
            HostKeyPolicy::Strict | HostKeyPolicy::AcceptNew => Err(Failure::KnownHostsUnreadable),
            HostKeyPolicy::Ask => Ok(ask(c, kx, algorithms, key)),
        },
    }
}

fn ask(c: Connection, kx: KexState, algorithms: Algorithms, key: HostKey) -> (Connection, Vec<Action>) {
    let stage = KexStage::AwaitingApproval { algorithms, host_key: key.clone(), peer_new_keys: false };
    (Connection { phase: Phase::KeyExchange(KexState { stage, ..kx }), ..c }, vec![Action::AskHostKey(key)])
}

/// Our NEWKEYS: what we send next is under the new keys.
fn keyed(c: Connection, kx: KexState, algorithms: Algorithms, before: Vec<Action>) -> (Connection, Vec<Action>) {
    let mut actions = before;
    actions.push(Action::NewKeys);
    let stage = KexStage::AwaitingNewKeys { algorithms };
    let c = Connection {
        outbound: Some(algorithms.client_to_server),
        phase: Phase::KeyExchange(KexState { stage, ..kx }),
        ..c
    };
    (c, actions)
}

/// Both NEWKEYS are through; the initial exchange goes on to ask for user
/// authentication.
fn finish_kex(c: Connection, kx: KexState, before: Vec<Action>) -> (Connection, Vec<Action>) {
    let mut actions = before;
    let phase = match kx.resume {
        Resume::Service => {
            if kx.initial {
                actions.push(Action::ServiceRequest);
            }
            Phase::Service
        }
        Resume::Authenticating(auth) => Phase::Authenticating(auth),
        Resume::Established => Phase::Established,
    };
    (Connection { phase, ..c }, actions)
}

fn on_answer(c: Connection, yes: bool) -> Result<(Connection, Vec<Action>), Failure> {
    let kx = match &c.phase {
        Phase::KeyExchange(kx) => kx.clone(),
        _ => return Err(Failure::Misuse),
    };
    match kx.stage.clone() {
        KexStage::AwaitingApproval { algorithms, host_key, peer_new_keys } => {
            if !yes {
                return Err(Failure::HostKeyRejected);
            }
            let (c, actions) = keyed(c, kx.clone(), algorithms, vec![Action::Learn(host_key)]);
            if peer_new_keys {
                Ok(finish_kex(c, kx, actions))
            } else {
                Ok((c, actions))
            }
        }
        _ => Err(Failure::Misuse),
    }
}

fn ext_info(c: Connection, server_sig_algs: Option<String>) -> Result<(Connection, Vec<Action>), Failure> {
    // A later EXT_INFO replaces what an earlier one said (RFC 8308 §2.4).
    let algs = match server_sig_algs {
        Some(s) => Some(name_list(&s)?),
        None => None,
    };
    Ok((Connection { server_sig_algs: algs, ..c }, Vec::new()))
}

fn in_service(c: Connection, m: Message, seq: u32) -> Result<(Connection, Vec<Action>), Failure> {
    match m {
        Message::ServiceAccept(name) => {
            if name.as_str() != USERAUTH_SERVICE {
                return Err(Failure::WrongService);
            }
            let auth =
                Auth { pending: Pending::Probe, next_key: 0, next_alg: 0, password_tries: 0, partial_successes: 0 };
            Ok((Connection { phase: Phase::Authenticating(auth), ..c }, vec![Action::AuthNone]))
        }
        Message::KexInit(peer) => peer_kex(c, Resume::Service, peer, seq),
        Message::Ignore | Message::Debug | Message::Unimplemented { .. } => Ok((c, Vec::new())),
        Message::Other(n) => other(c, n, seq, false, false),
        _ => Err(Failure::UnexpectedMessage(message_number(&m))),
    }
}

/// The next (identity, algorithm) to try from `auth`: with server-sig-algs,
/// the first of each key's algorithms the server lists, once per key; with
/// none, each of the key's algorithms in turn (RFC 8308 §3.1's trial and
/// error, as an RSA key may be refused with rsa-sha2-512 and taken with
/// rsa-sha2-256). The next position to try after it comes with it.
fn next_key_attempt(c: &Connection, auth: &Auth) -> Option<(usize, SignatureAlgorithm, usize, usize)> {
    let mut i = auth.next_key;
    let mut j = auth.next_alg;
    while i < c.config.identities.len() {
        let algs = signature_algorithms(c.config.identities[i].key_type);
        while j < algs.len() {
            let a = algs[j];
            match &c.server_sig_algs {
                None => return Some((i, a, i, j + 1)),
                Some(list) => {
                    if listed(list, &signature_name(a)) {
                        return Some((i, a, i + 1, 0));
                    }
                }
            }
            j += 1;
        }
        i += 1;
        j = 0;
    }
    None
}

/// After a failure: the next identity the server may accept, then the
/// password, among the methods that can continue (RFC 4252 §5.1).
fn next_attempt(c: &Connection, auth: Auth, methods: &Vec<String>) -> Result<(Auth, Action), Failure> {
    if listed(methods, "publickey") {
        if let Some((key, algorithm, next_key, next_alg)) = next_key_attempt(c, &auth) {
            let pending = Pending::KeyQuery { key, algorithm };
            let action = Action::AuthPublicKey { key, algorithm, signed: false };
            return Ok((Auth { pending, next_key, next_alg, ..auth }, action));
        }
    }
    if listed(methods, "password") && c.config.password && auth.password_tries < c.config.password_prompts {
        let tries = auth.password_tries + 1;
        return Ok((Auth { pending: Pending::Password, password_tries: tries, ..auth }, Action::AuthPassword));
    }
    Err(Failure::NoMoreAuthMethods)
}

fn in_auth(c: Connection, auth: Auth, m: Message, seq: u32) -> Result<(Connection, Vec<Action>), Failure> {
    match (auth.pending, m) {
        (_, Message::UserauthSuccess) => Ok((Connection { phase: Phase::Established, ..c }, Vec::new())),
        (_, Message::UserauthFailure { can_continue, partial_success }) => {
            let partial = if partial_success { auth.partial_successes + 1 } else { auth.partial_successes };
            let (auth, action) =
                next_attempt(&c, Auth { partial_successes: partial, ..auth }, &name_list(&can_continue)?)?;
            Ok((Connection { phase: Phase::Authenticating(auth), ..c }, vec![action]))
        }
        (Pending::KeyQuery { key, algorithm }, Message::Userauth60 { first, second }) => {
            // PK_OK echoes the algorithm and the key of the query it answers.
            if first != signature_name(algorithm) || second != c.config.identities[key].public_key {
                return Err(Failure::UnexpectedMessage(60));
            }
            let auth = Auth { pending: Pending::KeySigned { key, algorithm }, ..auth };
            Ok((
                Connection { phase: Phase::Authenticating(auth), ..c },
                vec![Action::AuthPublicKey { key, algorithm, signed: true }],
            ))
        }
        (Pending::Password, Message::Userauth60 { first, .. }) => Ok((c, vec![Action::ChangePassword(first)])),
        (_, Message::UserauthBanner(text)) => Ok((c, vec![Action::Show(text)])),
        (_, Message::ExtInfo { server_sig_algs }) => ext_info(c, server_sig_algs),
        (_, Message::KexInit(peer)) => peer_kex(c, Resume::Authenticating(auth), peer, seq),
        (_, Message::Ignore | Message::Debug | Message::Unimplemented { .. }) => Ok((c, Vec::new())),
        (_, Message::Other(n)) => other(c, n, seq, false, false),
        (_, m) => Err(Failure::UnexpectedMessage(message_number(&m))),
    }
}

// ---------------------------------------------------------------------------
// Packets and keys
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramingError {
    TooLong,
    TooShort,
    PaddingTooShort,
    Misaligned,
}

/// RFC 4253 §6 for a packet in direction `d` under what protects it now:
/// at least 4 bytes of padding, room for the message number, and the
/// encrypted part a multiple of the cipher's block (at least 8). Under EtM
/// and AEAD the length field is not encrypted and not counted. Returns the
/// payload length.
pub fn check_packet(c: &Connection, d: Direction, packet_length: u32, padding_length: u8) -> Result<u32, FramingError> {
    let transform = match d {
        Direction::ClientToServer => c.outbound,
        Direction::ServerToClient => c.inbound,
    };
    let (block, length_in_clear) = match transform {
        None => (MIN_BLOCK, false),
        Some(t) => {
            let spec = cipher_params(t.cipher);
            (spec.block.max(MIN_BLOCK), spec.aead || t.mac.map(mac_is_etm).unwrap_or(false))
        }
    };
    let padding = u32::from(padding_length);
    if packet_length > MAX_PACKET_LEN {
        return Err(FramingError::TooLong);
    }
    if padding < MIN_PADDING {
        return Err(FramingError::PaddingTooShort);
    }
    // The whole packet, length field included, is at least 16 bytes or a
    // block, whichever is larger (§6); under EtM and AEAD too, where the
    // alignment alone would let 12 through.
    if packet_length < padding + 2 || packet_length + 4 < block.max(MIN_PACKET) {
        return Err(FramingError::TooShort);
    }
    let covered = if length_in_clear { packet_length } else { packet_length + 4 };
    if covered % block != 0 {
        return Err(FramingError::Misaligned);
    }
    Ok(packet_length - padding - 1)
}

/// One key of RFC 4253 §7.2: HASH(K || H || letter || session_id), extended
/// by HASH(K || H || K1 || ..) for `rounds` hashes in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyNeed {
    pub letter: char,
    pub length: usize,
    pub rounds: usize,
}

fn need(letter: char, length: usize, hash: usize) -> KeyNeed {
    KeyNeed { letter, length, rounds: (length + hash - 1) / hash }
}

fn transform_mac_len(t: &Transform) -> usize {
    match t.mac {
        Some(m) => mac_key_len(m),
        None => 0,
    }
}

/// The six keys, A to F: IVs, encryption keys, integrity keys, client to
/// server first. A length of 0 is not derived.
pub fn key_plan(a: &Algorithms) -> Vec<KeyNeed> {
    let hash = hash_len(kex_hash(a.kex));
    let up = cipher_params(a.client_to_server.cipher);
    let down = cipher_params(a.server_to_client.cipher);
    vec![
        need('A', up.iv_len, hash),
        need('B', down.iv_len, hash),
        need('C', up.key_len, hash),
        need('D', down.key_len, hash),
        need('E', transform_mac_len(&a.client_to_server), hash),
        need('F', transform_mac_len(&a.server_to_client), hash),
    ]
}

/// Whether the connection layer may send now: authenticated, and not
/// between our KEXINIT and our NEWKEYS (RFC 4253 §7.1).
pub fn can_send_data(c: &Connection) -> bool {
    match &c.phase {
        Phase::Established => true,
        Phase::KeyExchange(kx) => {
            matches!(kx.resume, Resume::Established) && matches!(kx.stage, KexStage::AwaitingNewKeys { .. })
        }
        _ => false,
    }
}
