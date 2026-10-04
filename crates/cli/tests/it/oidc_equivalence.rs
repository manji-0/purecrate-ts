//! `examples/oidc`, an OpenID Provider's authorization code login with a
//! TOTP second factor (OIDC Core 1.0, RFC 6749, RFC 7636, RFC 4226,
//! RFC 6238, RFC 8176):
//!
//! - the published vectors come out as printed: RFC 4226 Appendix D (the
//!   ten HMAC-SHA-1 values to HOTP codes), RFC 6238 Appendix B (the SHA-1
//!   rows, eight digits), RFC 7636 Appendix B (verifier and challenge);
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against) agree on every request below, under each client,
//!   session and consent, on every sequence of four events from each start
//!   and every run of up to five, and on each code redemption;
//! - the generated package agrees with Rust on the vectors, on every request
//!   below, and on every run of up to five events, whole `Flow` compared.

use crate::support;

purecrate_canon::fixture!(mod oidc = "../../../examples/oidc/src/lib.rs", "fixtures/oidc_driver.rs");

const SOURCE: &str = oidc::SOURCE;

/// The same flow as one would write it without the subset's constraints:
/// iterators, `from_be_bytes`, a tuple `match` with guards. Not
/// converted; the reference only.
mod idiomatic {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum ErrorCode {
        InvalidRequest,
        UnsupportedResponseType,
        InvalidScope,
        AccessDenied,
        LoginRequired,
        ConsentRequired,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum DisplayError {
        UnknownClient,
        MissingRedirectUri,
        UnregisteredRedirectUri,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct ErrorRedirect {
        pub redirect_uri: String,
        pub error: ErrorCode,
        pub state: Option<String>,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub enum AuthorizationError {
        Display(DisplayError),
        Redirect(ErrorRedirect),
    }

    pub struct Client {
        pub client_id: String,
        pub redirect_uris: Vec<String>,
        pub require_pkce: bool,
        pub allow_plain_pkce: bool,
    }
    #[derive(Default)]
    pub struct AuthorizationParams {
        pub client_id: Option<String>,
        pub response_type: Option<String>,
        pub redirect_uri: Option<String>,
        pub scope: Option<String>,
        pub state: Option<String>,
        pub nonce: Option<String>,
        pub code_challenge: Option<String>,
        pub code_challenge_method: Option<String>,
        pub prompt: Option<String>,
        pub max_age: Option<String>,
        pub acr_values: Option<String>,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum AuthStrength {
        PasswordOnly,
        PasswordAndTotp,
    }
    pub struct Session {
        pub subject: String,
        pub auth_time: i64,
        pub strength: AuthStrength,
    }
    pub struct Policy {
        pub max_password_failures: u32,
        pub max_otp_failures: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum PkceMethod {
        S256,
        Plain,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct Pkce {
        pub challenge: String,
        pub method: PkceMethod,
    }
    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct Prompt {
        pub no_interaction: bool,
        pub login: bool,
        pub consent: bool,
        pub select_account: bool,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct AuthorizationRequest {
        client_id: String,
        redirect_uri: String,
        scope: String,
        state: String,
        nonce: Option<String>,
        pkce: Option<Pkce>,
        prompt: Prompt,
        max_age: Option<i64>,
        wants_mfa: bool,
    }

    pub const ACR_MFA: &str = "urn:example:acr:mfa";
    pub const ACR_PASSWORD: &str = "urn:example:acr:pwd";

    pub fn has_token(list: &str, word: &str) -> bool {
        list.split(' ').any(|t| t == word)
    }

    fn state_is_valid(s: &str) -> bool {
        (1..=512).contains(&s.len()) && s.bytes().all(|b| (0x20..=0x7e).contains(&b))
    }

    pub fn pkce_string_is_valid(s: &str) -> bool {
        (43..=128).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    }

    fn parse_seconds(s: &str) -> Option<i64> {
        (!s.is_empty() && s.len() <= 18 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
    }

    fn parse_prompt(s: &str) -> Option<Prompt> {
        let mut p = Prompt::default();
        for token in s.split(' ').filter(|t| !t.is_empty()) {
            match token {
                "none" => p.no_interaction = true,
                "login" => p.login = true,
                "consent" => p.consent = true,
                "select_account" => p.select_account = true,
                _ => return None,
            }
        }
        (!(p.no_interaction && (p.login || p.consent || p.select_account))).then_some(p)
    }

    pub fn validate_request(
        params: &AuthorizationParams,
        client: Option<&Client>,
    ) -> Result<AuthorizationRequest, AuthorizationError> {
        use AuthorizationError::Display;
        let client = client
            .filter(|c| params.client_id.as_deref() == Some(c.client_id.as_str()))
            .ok_or(Display(DisplayError::UnknownClient))?;
        let redirect_uri = params.redirect_uri.as_deref().ok_or(Display(DisplayError::MissingRedirectUri))?;
        if !client.redirect_uris.iter().any(|u| u == redirect_uri) {
            return Err(Display(DisplayError::UnregisteredRedirectUri));
        }
        let echoed = params.state.as_deref().filter(|s| state_is_valid(s));
        let fail = |error| {
            AuthorizationError::Redirect(ErrorRedirect {
                redirect_uri: redirect_uri.to_owned(),
                error,
                state: echoed.map(str::to_owned),
            })
        };
        match params.response_type.as_deref() {
            Some("code") => {}
            Some(_) => return Err(fail(ErrorCode::UnsupportedResponseType)),
            None => return Err(fail(ErrorCode::InvalidRequest)),
        }
        let scope =
            params.scope.as_deref().filter(|s| has_token(s, "openid")).ok_or_else(|| fail(ErrorCode::InvalidScope))?;
        let state = echoed.ok_or_else(|| fail(ErrorCode::InvalidRequest))?;
        let nonce = match params.nonce.as_deref() {
            Some(n) if !state_is_valid(n) => return Err(fail(ErrorCode::InvalidRequest)),
            n => n.map(str::to_owned),
        };
        let pkce = match (params.code_challenge.as_deref(), params.code_challenge_method.as_deref()) {
            (Some(c), _) if !pkce_string_is_valid(c) => return Err(fail(ErrorCode::InvalidRequest)),
            (Some(c), m) => {
                let method = match m {
                    Some("S256") => PkceMethod::S256,
                    Some("plain") | None => PkceMethod::Plain,
                    Some(_) => return Err(fail(ErrorCode::InvalidRequest)),
                };
                if method == PkceMethod::Plain && !client.allow_plain_pkce {
                    return Err(fail(ErrorCode::InvalidRequest));
                }
                Some(Pkce { challenge: c.to_owned(), method })
            }
            (None, Some(_)) => return Err(fail(ErrorCode::InvalidRequest)),
            (None, None) if client.require_pkce => return Err(fail(ErrorCode::InvalidRequest)),
            (None, None) => None,
        };
        let prompt = match params.prompt.as_deref() {
            Some(p) => parse_prompt(p).ok_or_else(|| fail(ErrorCode::InvalidRequest))?,
            None => Prompt::default(),
        };
        let max_age = params
            .max_age
            .as_deref()
            .map(|m| parse_seconds(m).ok_or_else(|| fail(ErrorCode::InvalidRequest)))
            .transpose()?;
        let wants_mfa = params.acr_values.as_deref().is_some_and(|a| has_token(a, ACR_MFA));
        Ok(AuthorizationRequest {
            client_id: client.client_id.clone(),
            redirect_uri: redirect_uri.to_owned(),
            scope: scope.to_owned(),
            state: state.to_owned(),
            nonce,
            pkce,
            prompt,
            max_age,
            wants_mfa,
        })
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum OtpDigits {
        Six,
        Seven,
        Eight,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct TotpEnrollment {
        pub t0: i64,
        pub period: i64,
        pub digits: OtpDigits,
        pub last_used_step: Option<i64>,
        pub failures: u32,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct StepMac {
        pub step: i64,
        pub mac: Vec<u8>,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum OtpCheck {
        Accepted(i64),
        Replayed,
        Mismatch,
        Malformed,
        ClockBeforeEpoch,
    }

    impl OtpDigits {
        fn count(self) -> usize {
            match self {
                Self::Six => 6,
                Self::Seven => 7,
                Self::Eight => 8,
            }
        }
    }

    pub fn totp_step(now: i64, t0: i64, period: i64) -> Option<i64> {
        (period > 0 && now >= t0).then(|| (now - t0) / period)
    }

    /// RFC 4226 §5.3, as the RFC writes it.
    pub fn truncate_mac(mac: &[u8], digits: OtpDigits) -> Option<u32> {
        let offset = usize::from(mac.get(19..)?.last()? & 0x0f);
        let word: [u8; 4] = mac[offset..offset + 4].try_into().ok()?;
        Some((u32::from_be_bytes(word) & 0x7fff_ffff) % 10u32.pow(digits.count() as u32))
    }

    fn parse_otp(code: &str, digits: OtpDigits) -> Option<u32> {
        (code.len() == digits.count() && code.bytes().all(|b| b.is_ascii_digit())).then(|| code.parse().ok()).flatten()
    }

    pub fn check_totp(code: &str, now: i64, enrollment: &TotpEnrollment, candidates: &[StepMac]) -> OtpCheck {
        let Some(current) = totp_step(now, enrollment.t0, enrollment.period) else { return OtpCheck::ClockBeforeEpoch };
        let Some(submitted) = parse_otp(code, enrollment.digits) else { return OtpCheck::Malformed };
        let mut matching = candidates
            .iter()
            .filter(|c| {
                (current - 1..=current + 1).contains(&c.step)
                    && truncate_mac(&c.mac, enrollment.digits) == Some(submitted)
            })
            .map(|c| c.step)
            .peekable();
        if matching.peek().is_none() {
            return OtpCheck::Mismatch;
        }
        match matching.find(|&s| enrollment.last_used_step.is_none_or(|last| s > last)) {
            Some(step) => OtpCheck::Accepted(step),
            None => OtpCheck::Replayed,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Notice {
        Clear,
        WrongPassword,
        WrongOtp,
        OtpReplayed,
        MalformedOtp,
        OtpUnavailable,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct Authentication {
        pub subject: String,
        pub auth_time: i64,
        pub strength: AuthStrength,
        pub totp_step: Option<i64>,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub struct CodeGrant {
        pub client_id: String,
        pub redirect_uri: String,
        pub scope: String,
        pub state: String,
        pub nonce: Option<String>,
        pub pkce: Option<Pkce>,
        pub subject: String,
        pub auth_time: i64,
        pub amr: Vec<String>,
        pub acr: String,
    }
    #[derive(Debug, Clone, PartialEq)]
    pub enum Flow {
        AwaitingPassword {
            request: AuthorizationRequest,
            needs_consent: bool,
            failures: u32,
            notice: Notice,
        },
        AwaitingOtp {
            request: AuthorizationRequest,
            needs_consent: bool,
            subject: String,
            enrollment: TotpEnrollment,
            notice: Notice,
        },
        AwaitingConsent {
            request: AuthorizationRequest,
            auth: Authentication,
        },
        CodeIssued(CodeGrant),
        Rejected(ErrorRedirect),
        Locked {
            subject: String,
        },
    }
    pub enum SecondFactor {
        Totp(TotpEnrollment),
        NotEnrolled,
    }
    pub enum Event {
        PasswordChecked { subject: String, verified: bool, second_factor: SecondFactor, now: i64 },
        OtpSubmitted { code: String, now: i64, candidates: Vec<StepMac> },
        ConsentGranted,
        ConsentDenied,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum FlowError {
        InvalidTransition,
    }

    fn authenticated(request: AuthorizationRequest, needs_consent: bool, auth: Authentication) -> Flow {
        if needs_consent {
            Flow::AwaitingConsent { request, auth }
        } else {
            issue(request, auth)
        }
    }

    fn issue(request: AuthorizationRequest, auth: Authentication) -> Flow {
        let (amr, acr): (&[&str], _) = match auth.strength {
            AuthStrength::PasswordOnly => (&["pwd"], ACR_PASSWORD),
            AuthStrength::PasswordAndTotp => (&["pwd", "otp", "mfa"], ACR_MFA),
        };
        Flow::CodeIssued(CodeGrant {
            client_id: request.client_id,
            redirect_uri: request.redirect_uri,
            scope: request.scope,
            state: request.state,
            nonce: request.nonce,
            pkce: request.pkce,
            subject: auth.subject,
            auth_time: auth.auth_time,
            amr: amr.iter().map(|s| s.to_string()).collect(),
            acr: acr.to_owned(),
        })
    }

    pub fn session_is_usable(request: &AuthorizationRequest, session: &Session, now: i64) -> bool {
        !request.prompt.login
            && !request.prompt.select_account
            && request.max_age.is_none_or(|m| now - session.auth_time <= m)
            && (!request.wants_mfa || session.strength == AuthStrength::PasswordAndTotp)
    }

    pub fn begin(
        params: &AuthorizationParams,
        client: Option<&Client>,
        session: Option<&Session>,
        consent_on_file: bool,
        now: i64,
    ) -> Result<Flow, AuthorizationError> {
        let request = validate_request(params, client)?;
        let reusable = session.filter(|s| session_is_usable(&request, s, now)).map(|s| Authentication {
            subject: s.subject.clone(),
            auth_time: s.auth_time,
            strength: s.strength,
            totp_step: None,
        });
        let needs_consent = request.prompt.consent || !consent_on_file;
        let redirect = |error| {
            AuthorizationError::Redirect(ErrorRedirect {
                redirect_uri: request.redirect_uri.clone(),
                error,
                state: Some(request.state.clone()),
            })
        };
        match (request.prompt.no_interaction, reusable, needs_consent) {
            (true, None, _) => Err(redirect(ErrorCode::LoginRequired)),
            (true, Some(_), true) => Err(redirect(ErrorCode::ConsentRequired)),
            (_, Some(auth), _) => Ok(authenticated(request, needs_consent, auth)),
            (false, None, _) => {
                Ok(Flow::AwaitingPassword { request, needs_consent, failures: 0, notice: Notice::Clear })
            }
        }
    }

    pub fn step(flow: Flow, event: Event, policy: &Policy) -> Result<Flow, FlowError> {
        use Event::*;
        use Flow::*;
        Ok(match (flow, event) {
            (AwaitingPassword { failures, .. }, PasswordChecked { subject, verified: false, .. })
                if failures + 1 >= policy.max_password_failures =>
            {
                Locked { subject }
            }
            (AwaitingPassword { request, needs_consent, failures, .. }, PasswordChecked { verified: false, .. }) => {
                AwaitingPassword { request, needs_consent, failures: failures + 1, notice: Notice::WrongPassword }
            }
            (AwaitingPassword { .. }, PasswordChecked { subject, second_factor: SecondFactor::Totp(e), .. })
                if e.failures >= policy.max_otp_failures =>
            {
                Locked { subject }
            }
            (
                AwaitingPassword { request, needs_consent, .. },
                PasswordChecked { subject, second_factor: SecondFactor::Totp(enrollment), .. },
            ) => AwaitingOtp { request, needs_consent, subject, enrollment, notice: Notice::Clear },
            (
                AwaitingPassword { request, needs_consent, .. },
                PasswordChecked { subject, second_factor: SecondFactor::NotEnrolled, now, .. },
            ) => authenticated(
                request,
                needs_consent,
                Authentication { subject, auth_time: now, strength: AuthStrength::PasswordOnly, totp_step: None },
            ),
            (
                AwaitingOtp { request, needs_consent, subject, enrollment, .. },
                OtpSubmitted { code, now, candidates },
            ) => {
                let notice = match check_totp(&code, now, &enrollment, &candidates) {
                    OtpCheck::Accepted(step) => {
                        let auth = Authentication {
                            subject,
                            auth_time: now,
                            strength: AuthStrength::PasswordAndTotp,
                            totp_step: Some(step),
                        };
                        return Ok(authenticated(request, needs_consent, auth));
                    }
                    OtpCheck::ClockBeforeEpoch => {
                        return Ok(AwaitingOtp {
                            request,
                            needs_consent,
                            subject,
                            enrollment,
                            notice: Notice::OtpUnavailable,
                        });
                    }
                    OtpCheck::Replayed => Notice::OtpReplayed,
                    OtpCheck::Malformed => Notice::MalformedOtp,
                    OtpCheck::Mismatch => Notice::WrongOtp,
                };
                let failures = enrollment.failures + 1;
                if failures >= policy.max_otp_failures {
                    Locked { subject }
                } else {
                    AwaitingOtp {
                        request,
                        needs_consent,
                        subject,
                        enrollment: TotpEnrollment { failures, ..enrollment },
                        notice,
                    }
                }
            }
            (AwaitingConsent { request, auth }, ConsentGranted) => issue(request, auth),
            (AwaitingConsent { request, .. }, ConsentDenied) => Rejected(ErrorRedirect {
                redirect_uri: request.redirect_uri,
                error: ErrorCode::AccessDenied,
                state: Some(request.state),
            }),
            _ => return Err(FlowError::InvalidTransition),
        })
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum TokenError {
        InvalidRequest,
        InvalidGrant,
    }

    pub fn check_redemption(
        grant: &CodeGrant,
        client_id: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
        verifier_s256: Option<&str>,
    ) -> Result<(), TokenError> {
        if grant.client_id != client_id || grant.redirect_uri != redirect_uri {
            return Err(TokenError::InvalidGrant);
        }
        match (&grant.pkce, code_verifier) {
            (None, None) => Ok(()),
            (None, Some(_)) | (Some(_), None) => Err(TokenError::InvalidRequest),
            (Some(_), Some(v)) if !pkce_string_is_valid(v) => Err(TokenError::InvalidRequest),
            (Some(p), Some(v)) => {
                let ok = match p.method {
                    PkceMethod::Plain => v == p.challenge,
                    PkceMethod::S256 => verifier_s256 == Some(p.challenge.as_str()),
                };
                ok.then_some(()).ok_or(TokenError::InvalidGrant)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Published vectors
// ---------------------------------------------------------------------------

/// RFC 4226 Appendix D: the secret, HMAC-SHA-1(secret, count) for counts
/// 0..=9, the 31-bit truncated value, and the 6-digit HOTP.
const SECRET: &[u8] = b"12345678901234567890";
const APPENDIX_D: [(&str, u32, u32); 10] = [
    ("cc93cf18508d94934c64b65d8ba7667fb7cde4b0", 1284755224, 755224),
    ("75a48a19d4cbe100644e8ac1397eea747a2d33ab", 1094287082, 287082),
    ("0bacb7fa082fef30782211938bc1c5e70416ff44", 137359152, 359152),
    ("66c28227d03a2d5529262ff016a1e6ef76557ece", 1726969429, 969429),
    ("a904c900a64b35909874b33e61c5938a8e15ed1c", 1640338314, 338314),
    ("a37e783d7b7233c083d4f62926c7a25f238d0316", 868254676, 254676),
    ("bc9cd28561042c83f219324d3c607256c03272ae", 1918287922, 287922),
    ("a4fb960c0bc06e1eabb804e5b397cdc4b45596fa", 82162583, 162583),
    ("1b3c89f65e6c9e883012052823443f048b4332db", 673399871, 399871),
    ("1637409809a679dc698207310c8c7fc07290d9e5", 645520489, 520489),
];

/// RFC 6238 Appendix B, the SHA-1 rows: Unix time and the 8-digit TOTP
/// (T0 = 0, X = 30, the Appendix D secret).
const APPENDIX_B: [(i64, &str); 6] = [
    (59, "94287082"),
    (1111111109, "07081804"),
    (1111111111, "14050471"),
    (1234567890, "89005924"),
    (2000000000, "69279037"),
    (20000000000, "65353130"),
];

/// RFC 7636 Appendix B: code_verifier and its S256 code_challenge.
const VERIFIER: &str = "dBjftJeZ4CVP-mJ0DXYvF5Bi6MpSsTr0ArHHf4vk4Yw";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
}

/// Test-only SHA-1 (FIPS 180-4) and HMAC (RFC 2104): what the caller
/// computes for each candidate step. Held against Appendix D below.
fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0];
    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&(msg.len() as u64 * 8).to_be_bytes());
    for chunk in data.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..80 {
            w[i] = if i < 16 {
                u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().expect("4 bytes"))
            } else {
                (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1)
            };
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

/// HMAC-SHA-1(secret, step as 8-byte big-endian) (RFC 6238 §4, RFC 4226 §5.2).
fn mac_for_step(step: i64) -> Vec<u8> {
    let mut key = SECRET.to_vec();
    key.resize(64, 0);
    let inner: Vec<u8> = key.iter().map(|b| b ^ 0x36).chain((step as u64).to_be_bytes()).collect();
    let outer: Vec<u8> = key.iter().map(|b| b ^ 0x5c).chain(sha1(&inner)).collect();
    sha1(&outer).to_vec()
}

use oidc::{OtpCheck, OtpDigits, StepMac, TotpEnrollment};

fn enrollment(digits: OtpDigits, last_used_step: Option<i64>) -> TotpEnrollment {
    TotpEnrollment { t0: 0, period: 30, digits, last_used_step, failures: 0 }
}

fn candidates(steps: std::ops::RangeInclusive<i64>) -> Vec<StepMac> {
    steps.map(|step| StepMac { step, mac: mac_for_step(step) }).collect()
}

#[test]
fn the_published_vectors_come_out_as_printed() {
    // RFC 4226 §5.4's worked example.
    let example = unhex("1f8698690e02ca16618550ef7f19da8e945b555a");
    assert_eq!(oidc::truncate_mac(&example, OtpDigits::Six), Some(872921));
    assert_eq!(idiomatic::truncate_mac(&example, idiomatic::OtpDigits::Six), Some(872921));

    // RFC 4226 Appendix D: the test SHA-1 reproduces the published HMACs,
    // and each HMAC truncates to the published value and HOTP.
    for (count, (hex, truncated, hotp)) in APPENDIX_D.iter().enumerate() {
        let mac = unhex(hex);
        assert_eq!(mac_for_step(count as i64), mac, "HMAC for count {count}");
        assert_eq!(oidc::truncate_mac(&mac, OtpDigits::Six), Some(*hotp), "HOTP for count {count}");
        assert_eq!(oidc::truncate_mac(&mac, OtpDigits::Seven), Some(truncated % 10_000_000));
        assert_eq!(oidc::truncate_mac(&mac, OtpDigits::Eight), Some(truncated % 100_000_000));
        assert_eq!(idiomatic::truncate_mac(&mac, idiomatic::OtpDigits::Six), Some(*hotp));
    }

    // RFC 6238 Appendix B (SHA-1): the code for T is accepted with T-1..=T+1
    // supplied, refused once T is recorded as used (§5.2), and malformed
    // without its leading digit.
    for (now, code) in APPENDIX_B {
        let t = now / 30;
        assert_eq!(oidc::totp_step(now, 0, 30), Some(t));
        let window = candidates(t - 1..=t + 1);
        let code = code.to_string();
        assert_eq!(
            oidc::check_totp(&code, now, &enrollment(OtpDigits::Eight, None), &window),
            OtpCheck::Accepted(t),
            "at {now}"
        );
        assert_eq!(
            oidc::check_totp(&code, now, &enrollment(OtpDigits::Eight, Some(t - 1)), &window),
            OtpCheck::Accepted(t)
        );
        assert_eq!(oidc::check_totp(&code, now, &enrollment(OtpDigits::Eight, Some(t)), &window), OtpCheck::Replayed);
        assert_eq!(
            oidc::check_totp(&code[1..].to_string(), now, &enrollment(OtpDigits::Eight, None), &window),
            OtpCheck::Malformed
        );
        // Only the candidate for T carries this code: dropping it is a mismatch.
        let others: Vec<StepMac> = candidates(t - 1..=t + 1).into_iter().filter(|c| c.step != t).collect();
        assert_eq!(oidc::check_totp(&code, now, &enrollment(OtpDigits::Eight, None), &others), OtpCheck::Mismatch);
    }

    // RFC 7636 Appendix B: both strings are well formed (§4.1, §4.2), and a
    // grant with the S256 challenge redeems with that verifier (§4.6).
    assert!(oidc::pkce_string_is_valid(&VERIFIER.to_string()));
    assert!(oidc::pkce_string_is_valid(&CHALLENGE.to_string()));
    let grant = grant(&strict(), base(), Start::Fresh);
    let verifier = Some(VERIFIER.to_string());
    assert_eq!(
        oidc::check_redemption(&grant, &"app".to_string(), &CB.to_string(), &verifier, &Some(CHALLENGE.to_string())),
        Ok(())
    );
    assert_eq!(
        oidc::check_redemption(&grant, &"app".to_string(), &CB.to_string(), &verifier, &Some(VERIFIER.to_string())),
        Err(oidc::TokenError::InvalidGrant),
        "S256 compares the hash, not the verifier"
    );
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

use oidc::{AuthStrength, AuthorizationParams, Client, Event, Policy, Script, SecondFactor, Session};

const CB: &str = "https://app.example/cb";
/// `begin`'s clock in the request matrix.
const NOW: i64 = 1000;

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

fn base() -> AuthorizationParams {
    AuthorizationParams {
        client_id: some("app"),
        response_type: some("code"),
        redirect_uri: some(CB),
        scope: some("openid profile"),
        state: some("xyz"),
        nonce: some("n-0S6_WzA2Mj"),
        code_challenge: some(CHALLENGE),
        code_challenge_method: some("S256"),
        prompt: None,
        max_age: None,
        acr_values: None,
    }
}

/// A public client: PKCE required, S256 only.
fn strict() -> Option<Client> {
    Some(Client {
        client_id: "app".into(),
        redirect_uris: vec![CB.into(), "https://app.example/cb2".into()],
        require_pkce: true,
        allow_plain_pkce: false,
    })
}

/// A confidential client: PKCE optional, `plain` allowed.
fn lax() -> Option<Client> {
    Some(Client { require_pkce: false, allow_plain_pkce: true, ..strict().unwrap() })
}

fn session(auth_time: i64, strength: AuthStrength) -> Option<Session> {
    Some(Session { subject: "alice".into(), auth_time, strength })
}

/// No session; a password-only one and a two-factor one 100 s old; a
/// two-factor one from the epoch.
fn sessions() -> [Option<Session>; 4] {
    [
        None,
        session(NOW - 100, AuthStrength::PasswordOnly),
        session(NOW - 100, AuthStrength::PasswordAndTotp),
        session(0, AuthStrength::PasswordAndTotp),
    ]
}

/// The base request, then one change per field (or a pair, where the order
/// of the checks matters), covering each error of §3.1.2.2.
fn requests() -> Vec<AuthorizationParams> {
    type Edit = fn(&mut AuthorizationParams);
    let edits: &[Edit] = &[
        |_| {},
        |p| p.client_id = some("other"),
        |p| p.client_id = None,
        |p| p.redirect_uri = None,
        |p| p.redirect_uri = some("https://app.example/cb/"),
        |p| p.redirect_uri = some("https://APP.example/cb"),
        |p| p.redirect_uri = some(""),
        |p| p.redirect_uri = some("https://app.example/cb2"),
        |p| {
            p.redirect_uri = some("https://evil.example/");
            p.response_type = some("token");
        },
        |p| p.response_type = some("token"),
        |p| p.response_type = some("code id_token"),
        |p| p.response_type = some("Code"),
        |p| p.response_type = None,
        |p| {
            p.response_type = some("token");
            p.scope = None;
        },
        |p| p.scope = some("profile"),
        |p| p.scope = some("openidx profile"),
        |p| p.scope = some("OPENID"),
        |p| p.scope = some("profile openid"),
        |p| p.scope = some(" openid "),
        |p| p.scope = some("openid"),
        |p| p.scope = some(""),
        |p| p.scope = None,
        |p| {
            p.scope = None;
            p.state = None;
        },
        |p| p.state = None,
        |p| p.state = some(""),
        |p| p.state = some("a\u{7f}"),
        |p| p.state = some("caf\u{e9}"),
        |p| p.state = some("~ ok ~"),
        |p| p.state = Some("s".repeat(512)),
        |p| p.state = Some("s".repeat(513)),
        |p| {
            p.state = some("caf\u{e9}");
            p.response_type = some("token");
        },
        |p| p.nonce = None,
        |p| p.nonce = some("bad\nnonce"),
        |p| p.nonce = some(""),
        |p| p.nonce = Some("n".repeat(513)),
        |p| {
            p.code_challenge = None;
            p.code_challenge_method = None;
        },
        |p| p.code_challenge = None,
        |p| p.code_challenge = some("short"),
        |p| p.code_challenge = Some("a".repeat(42)),
        |p| p.code_challenge = Some("a".repeat(128)),
        |p| p.code_challenge = Some("a".repeat(129)),
        |p| p.code_challenge = some("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw+cM"),
        |p| p.code_challenge = some("\u{e9}E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-c"),
        |p| p.code_challenge_method = some("plain"),
        |p| p.code_challenge_method = some("s256"),
        |p| p.code_challenge_method = None,
        |p| {
            p.code_challenge = some(VERIFIER);
            p.code_challenge_method = some("plain");
        },
        |p| p.prompt = some("none"),
        |p| p.prompt = some("login"),
        |p| p.prompt = some("consent"),
        |p| p.prompt = some("select_account"),
        |p| p.prompt = some("login  consent"),
        |p| p.prompt = some(" "),
        |p| p.prompt = some(""),
        |p| p.prompt = some("none none"),
        |p| p.prompt = some("none login"),
        |p| p.prompt = some("none consent"),
        |p| p.prompt = some("bogus"),
        |p| p.max_age = some("0"),
        |p| p.max_age = some("99"),
        |p| p.max_age = some("100"),
        |p| p.max_age = some("999999999999999999"),
        |p| p.max_age = some("9999999999999999999"),
        |p| p.max_age = some("-1"),
        |p| p.max_age = some("+5"),
        |p| p.max_age = some(""),
        |p| {
            p.max_age = some("99");
            p.prompt = some("none");
        },
        |p| p.acr_values = some("urn:example:acr:mfa"),
        |p| p.acr_values = some("urn:example:acr:pwd urn:example:acr:mfa"),
        |p| p.acr_values = some("urn:example:acr:mfax"),
        |p| {
            p.acr_values = some("urn:example:acr:mfa");
            p.prompt = some("none");
        },
    ];
    edits
        .iter()
        .map(|edit| {
            let mut p = base();
            edit(&mut p);
            p
        })
        .collect()
}

#[test]
fn each_request_error_is_redirected_or_shown() {
    use oidc::{AuthorizationError as A, DisplayError as D, ErrorCode as E};
    let with = |edit: fn(&mut AuthorizationParams)| {
        let mut p = base();
        edit(&mut p);
        oidc::validate_request(&p, &strict()).map(|_| ())
    };
    let redirect = |error, state: Option<&str>| {
        Err(A::Redirect(oidc::ErrorRedirect { redirect_uri: CB.into(), error, state: state.map(String::from) }))
    };

    assert_eq!(with(|_| {}), Ok(()));
    assert_eq!(oidc::validate_request(&base(), &None).map(|_| ()), Err(A::Display(D::UnknownClient)));
    // About the client or its redirect_uri: shown, never redirected.
    assert_eq!(with(|p| p.client_id = some("other")), Err(A::Display(D::UnknownClient)));
    assert_eq!(with(|p| p.redirect_uri = None), Err(A::Display(D::MissingRedirectUri)));
    assert_eq!(with(|p| p.redirect_uri = some("https://app.example/cb/")), Err(A::Display(D::UnregisteredRedirectUri)));
    assert_eq!(
        with(|p| {
            p.redirect_uri = some("https://evil.example/");
            p.response_type = some("token");
        }),
        Err(A::Display(D::UnregisteredRedirectUri))
    );
    // Everything after: redirected, with state echoed when well formed.
    assert_eq!(with(|p| p.response_type = some("token")), redirect(E::UnsupportedResponseType, Some("xyz")));
    assert_eq!(with(|p| p.response_type = None), redirect(E::InvalidRequest, Some("xyz")));
    assert_eq!(with(|p| p.scope = some("profile")), redirect(E::InvalidScope, Some("xyz")));
    assert_eq!(with(|p| p.state = None), redirect(E::InvalidRequest, None));
    assert_eq!(
        with(|p| {
            p.state = some("caf\u{e9}");
            p.response_type = some("token");
        }),
        redirect(E::UnsupportedResponseType, None)
    );
    assert_eq!(with(|p| p.nonce = some("bad\nnonce")), redirect(E::InvalidRequest, Some("xyz")));
    assert_eq!(with(|p| p.code_challenge = None), redirect(E::InvalidRequest, Some("xyz")));
    assert_eq!(with(|p| p.code_challenge_method = some("plain")), redirect(E::InvalidRequest, Some("xyz")));
    assert_eq!(with(|p| p.prompt = some("none login")), redirect(E::InvalidRequest, Some("xyz")));
    assert_eq!(with(|p| p.max_age = some("-1")), redirect(E::InvalidRequest, Some("xyz")));
    // A confidential client may omit PKCE, or use plain.
    let mut p = base();
    p.code_challenge = None;
    p.code_challenge_method = None;
    assert!(oidc::validate_request(&p, &lax()).is_ok());
    p.code_challenge_method = some("S256");
    assert_eq!(
        oidc::validate_request(&p, &lax()).map(|_| ()),
        redirect(E::InvalidRequest, Some("xyz")),
        "a method without a challenge"
    );
}

#[test]
fn prompt_and_max_age_decide_on_the_session() {
    use oidc::{AuthorizationError as A, ErrorCode as E, Flow};
    let begin = |edit: fn(&mut AuthorizationParams), s: Option<Session>, consent: bool| {
        let mut p = base();
        edit(&mut p);
        oidc::begin(&p, &strict(), &s, consent, NOW)
    };
    let mfa = || session(NOW - 100, AuthStrength::PasswordAndTotp);
    let pwd = || session(NOW - 100, AuthStrength::PasswordOnly);
    let error = |r: Result<Flow, A>| match r {
        Err(A::Redirect(e)) => Some(e.error),
        _ => None,
    };
    let is = |r: &Result<Flow, A>, name: &str| format!("{r:?}").starts_with(&format!("Ok({name}"));

    assert!(is(&begin(|_| {}, None, true), "AwaitingPassword"));
    assert!(is(&begin(|_| {}, mfa(), true), "CodeIssued"));
    assert!(is(&begin(|_| {}, mfa(), false), "AwaitingConsent"));
    assert!(is(&begin(|p| p.prompt = some("consent"), mfa(), true), "AwaitingConsent"));
    assert!(is(&begin(|p| p.prompt = some("login"), mfa(), true), "AwaitingPassword"));
    assert!(is(&begin(|p| p.prompt = some("select_account"), mfa(), true), "AwaitingPassword"));
    // prompt=none never shows a page (OIDC Core §3.1.2.6).
    assert_eq!(error(begin(|p| p.prompt = some("none"), None, true)), Some(E::LoginRequired));
    assert_eq!(error(begin(|p| p.prompt = some("none"), mfa(), false)), Some(E::ConsentRequired));
    assert!(is(&begin(|p| p.prompt = some("none"), mfa(), true), "CodeIssued"));
    // max_age: the session is 100 s old.
    assert!(is(&begin(|p| p.max_age = some("100"), mfa(), true), "CodeIssued"));
    assert!(is(&begin(|p| p.max_age = some("99"), mfa(), true), "AwaitingPassword"));
    assert!(is(&begin(|p| p.max_age = some("0"), mfa(), true), "AwaitingPassword"));
    assert_eq!(
        error(begin(
            |p| {
                p.max_age = some("99");
                p.prompt = some("none");
            },
            mfa(),
            true
        )),
        Some(E::LoginRequired)
    );
    // acr_values asking for MFA passes over a password-only session.
    assert!(is(&begin(|p| p.acr_values = some("urn:example:acr:mfa"), pwd(), true), "AwaitingPassword"));
    assert!(is(&begin(|p| p.acr_values = some("urn:example:acr:mfa"), mfa(), true), "CodeIssued"));
    // A reused session keeps its auth_time and strength.
    match begin(|_| {}, pwd(), true) {
        Ok(Flow::CodeIssued(g)) => {
            assert_eq!((g.auth_time, g.acr.as_str()), (NOW - 100, "urn:example:acr:pwd"));
            assert_eq!(g.amr, oidc::amr_values(AuthStrength::PasswordOnly));
        }
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Runs of events
// ---------------------------------------------------------------------------

/// The flow's clock: T0 = 0 and X = 30, so the OTP events at 59 are in step
/// 1, and the Appendix D MACs are those of steps 0..=3.
const PASSWORD_AT: i64 = 45;
const OTP_AT: i64 = 59;

/// Each event of the alphabet: passwords wrong and right with each second
/// factor; codes for T, T-1, T+1, and T+2 (outside the window although its
/// MAC is supplied), malformed, and before T0; consent both ways.
const EVENTS: u8 = 12;

fn password(verified: bool, second_factor: SecondFactor) -> Event {
    Event::PasswordChecked { subject: "alice".into(), verified, second_factor, now: PASSWORD_AT }
}

fn otp(code: &str, now: i64) -> Event {
    Event::OtpSubmitted {
        code: code.into(),
        now,
        candidates: (0..4).map(|s| StepMac { step: s as i64, mac: unhex(APPENDIX_D[s].0) }).collect(),
    }
}

fn event(code: u8) -> Event {
    let totp = |last| SecondFactor::Totp(enrollment(OtpDigits::Six, last));
    match code {
        0 => password(false, totp(None)),
        1 => password(true, totp(None)),
        2 => password(true, totp(Some(1))),
        3 => password(true, SecondFactor::NotEnrolled),
        4 => otp("287082", OTP_AT),
        5 => otp("755224", OTP_AT),
        6 => otp("359152", OTP_AT),
        7 => otp("969429", OTP_AT),
        8 => otp("12a456", OTP_AT),
        9 => otp("287082", -1),
        10 => Event::ConsentGranted,
        _ => Event::ConsentDenied,
    }
}

fn script(codes: &[u8]) -> Script {
    codes.iter().rev().fold(Script::End, |rest, &c| Script::Then(event(c), Box::new(rest)))
}

/// Where a run starts: a fresh login under the strict client, the same
/// with a policy of one password and two OTP attempts, and a two-factor
/// session reused under `prompt=consent`.
#[derive(Clone, Copy)]
enum Start {
    Fresh,
    Tight,
    Reused,
}

const STARTS: [Start; 3] = [Start::Fresh, Start::Tight, Start::Reused];

struct Setup {
    params: AuthorizationParams,
    client: Option<Client>,
    session: Option<Session>,
    consent_on_file: bool,
    now: i64,
    policy: Policy,
}

fn setup(client: &Option<Client>, params: AuthorizationParams, start: Start) -> Setup {
    let policy = |p, o| Policy { max_password_failures: p, max_otp_failures: o };
    let (params, session, policy) = match start {
        Start::Fresh => (params, None, policy(3, 3)),
        Start::Tight => (params, None, policy(1, 2)),
        Start::Reused => (
            AuthorizationParams { prompt: some("consent"), ..params },
            session(30, AuthStrength::PasswordAndTotp),
            policy(3, 3),
        ),
    };
    // No consent on file, so each login ends at the consent screen.
    Setup { params, client: client.clone(), session, consent_on_file: false, now: 40, policy }
}

fn trace(s: &Setup, codes: &[u8]) -> Result<oidc::Flow, oidc::RunError> {
    oidc::trace(&s.params, &s.client, &s.session, s.consent_on_file, s.now, &s.policy, script(codes))
}

/// The grant of a full two-factor login from `start`.
fn grant(client: &Option<Client>, params: AuthorizationParams, start: Start) -> oidc::CodeGrant {
    let codes: &[u8] = match start {
        Start::Reused => &[10],
        _ => &[1, 4, 10],
    };
    match trace(&setup(client, params, start), codes) {
        Ok(oidc::Flow::CodeIssued(g)) => g,
        other => panic!("no code: {other:?}"),
    }
}

#[test]
fn logins_run_as_the_rfcs_say() {
    use oidc::{Flow, Notice, RunError};
    let fresh = setup(&strict(), base(), Start::Fresh);
    let run = |codes: &[u8]| trace(&fresh, codes);

    // Password, then the code for T, then consent: a two-factor grant (RFC 8176 §2).
    let g = grant(&strict(), base(), Start::Fresh);
    assert_eq!(
        (g.subject.as_str(), g.auth_time, g.acr.as_str(), g.state.as_str()),
        ("alice", OTP_AT, "urn:example:acr:mfa", "xyz")
    );
    assert_eq!(g.amr, oidc::amr_values(AuthStrength::PasswordAndTotp));
    assert_eq!(g.amr, ["pwd", "otp", "mfa"]);
    assert_eq!(
        (g.nonce.as_deref(), g.pkce.as_ref().map(|p| p.challenge.as_str())),
        (Some("n-0S6_WzA2Mj"), Some(CHALLENGE))
    );
    // The step accepted is what the caller records.
    let accepted = |codes: &[u8]| match trace(&fresh, codes) {
        Ok(Flow::AwaitingConsent { auth, .. }) => auth.totp_step,
        other => panic!("{other:?}"),
    };
    assert_eq!(accepted(&[1, 4]), Some(1));
    assert_eq!(accepted(&[1, 5]), Some(0), "T-1 is in the window");
    assert_eq!(accepted(&[1, 6]), Some(2), "T+1 is in the window");
    assert_eq!(accepted(&[2, 6]), Some(2), "a step after the last used one");
    let notice = |codes: &[u8]| match trace(&fresh, codes) {
        Ok(Flow::AwaitingOtp { notice, enrollment, .. }) => (notice, enrollment.failures),
        other => panic!("{other:?}"),
    };
    assert_eq!(notice(&[1, 7]), (Notice::WrongOtp, 1), "T+2 is outside, though its MAC was supplied");
    assert_eq!(notice(&[2, 4]), (Notice::OtpReplayed, 1), "step 1 was used already");
    assert_eq!(notice(&[2, 5]), (Notice::OtpReplayed, 1), "step 0 is before the last used");
    assert_eq!(notice(&[1, 8]), (Notice::MalformedOtp, 1));
    assert_eq!(
        notice(&[1, 9]),
        (Notice::OtpUnavailable, 0),
        "before T0: the server's clock, not the End-User, so not counted"
    );
    // Attempt limits (RFC 4226 §7.3), then nothing more is accepted.
    let locked = Ok(Flow::Locked { subject: "alice".into() });
    assert!(matches!(run(&[1, 7, 8]), Ok(Flow::AwaitingOtp { enrollment: TotpEnrollment { failures: 2, .. }, .. })));
    assert_eq!(run(&[1, 7, 8, 7]), locked);
    assert_eq!(run(&[1, 7, 8, 7, 4]), Err(RunError::Step { index: 4, error: oidc::FlowError::InvalidTransition }));
    assert_eq!(run(&[0, 0, 0]), locked);
    assert!(matches!(run(&[0, 0, 1]), Ok(Flow::AwaitingOtp { .. })), "failures below the limit do not lock");
    assert_eq!(trace(&setup(&strict(), base(), Start::Tight), &[0]), locked);
    // Failures stored from an earlier flow count: a new flow does not reset them.
    let stored = |failures, wrong_code: bool| {
        let e = TotpEnrollment { failures, ..enrollment(OtpDigits::Six, None) };
        let then = |ev, rest| oidc::Script::Then(ev, Box::new(rest));
        let rest = if wrong_code { then(event(7), oidc::Script::End) } else { oidc::Script::End };
        let s = setup(&strict(), base(), Start::Fresh);
        oidc::trace(
            &s.params,
            &s.client,
            &s.session,
            false,
            s.now,
            &s.policy,
            then(password(true, SecondFactor::Totp(e)), rest),
        )
    };
    let stored = |failures| stored(failures, failures < 3);
    assert_eq!(stored(3), locked, "at the limit already, before any code");
    assert_eq!(stored(2), locked, "one short, then a wrong code");
    assert!(matches!(stored(1), Ok(Flow::AwaitingOtp { enrollment: TotpEnrollment { failures: 2, .. }, .. })));
    // Consent on file: the code is issued as soon as both factors pass.
    let on_file = Setup { consent_on_file: true, ..setup(&strict(), base(), Start::Fresh) };
    assert!(matches!(trace(&on_file, &[1, 4]), Ok(Flow::CodeIssued(_))));
    // Without a second factor: password-only acr and amr, even when MFA was asked for.
    let mut asks_mfa = base();
    asks_mfa.acr_values = some("urn:example:acr:mfa");
    match trace(&setup(&strict(), asks_mfa, Start::Fresh), &[3, 10]) {
        Ok(Flow::CodeIssued(g)) => {
            assert_eq!((g.auth_time, g.acr.as_str()), (PASSWORD_AT, "urn:example:acr:pwd"));
            assert_eq!(g.amr, oidc::amr_values(AuthStrength::PasswordOnly));
        }
        other => panic!("{other:?}"),
    }
    // Consent denied: access_denied to the client, state echoed.
    assert_eq!(
        run(&[3, 11]),
        Ok(Flow::Rejected(oidc::ErrorRedirect {
            redirect_uri: CB.into(),
            error: oidc::ErrorCode::AccessDenied,
            state: some("xyz")
        }))
    );
    // Events out of order.
    assert_eq!(run(&[10]), Err(RunError::Step { index: 0, error: oidc::FlowError::InvalidTransition }));
    assert_eq!(run(&[1, 1]), Err(RunError::Step { index: 1, error: oidc::FlowError::InvalidTransition }));
}

/// Token endpoint cases (RFC 6749 §4.1.3, RFC 7636 §4.5-§4.6): each grant
/// with the client, redirect_uri, verifier and hash the caller passes.
/// A grant, then client_id, redirect_uri, code_verifier, and the hash.
type Redemption = (oidc::CodeGrant, String, String, Option<String>, Option<String>);

fn redemptions() -> Vec<Redemption> {
    let mut plain = base();
    plain.code_challenge = some(VERIFIER);
    plain.code_challenge_method = None;
    let mut without = base();
    without.code_challenge = None;
    without.code_challenge_method = None;
    let grants = [
        grant(&strict(), base(), Start::Fresh),
        grant(&strict(), base(), Start::Reused),
        grant(&lax(), plain, Start::Fresh),
        grant(&lax(), without, Start::Fresh),
    ];
    let callers = [
        ("app", CB, Some(VERIFIER), Some(CHALLENGE)),
        ("app", CB, Some(VERIFIER), Some(VERIFIER)),
        ("app", CB, Some(VERIFIER), None),
        ("app", CB, Some(CHALLENGE), Some(CHALLENGE)),
        ("app", CB, Some("short"), Some(CHALLENGE)),
        ("app", CB, None, Some(CHALLENGE)),
        ("app", CB, None, None),
        ("other", CB, Some(VERIFIER), Some(CHALLENGE)),
        ("app", "https://app.example/cb2", Some(VERIFIER), Some(CHALLENGE)),
    ];
    let mut out = Vec::new();
    for g in &grants {
        for (id, uri, v, h) in callers {
            out.push((g.clone(), id.to_string(), uri.to_string(), v.map(String::from), h.map(String::from)));
        }
    }
    out
}

#[test]
fn codes_redeem_only_with_their_verifier() {
    use oidc::TokenError::{InvalidGrant, InvalidRequest};
    let verdicts: Vec<Result<(), oidc::TokenError>> =
        redemptions().iter().map(|(g, id, uri, v, h)| oidc::check_redemption(g, id, uri, v, h)).collect();
    let (s256, reused, plain, without) = (&verdicts[0..9], &verdicts[9..18], &verdicts[18..27], &verdicts[27..36]);
    let expected_s256 = [
        Ok(()),
        Err(InvalidGrant),
        Err(InvalidGrant),
        Ok(()),
        Err(InvalidRequest),
        Err(InvalidRequest),
        Err(InvalidRequest),
        Err(InvalidGrant),
        Err(InvalidGrant),
    ];
    assert_eq!(s256, expected_s256, "row 3: the S256 check trusts the hash the caller computed");
    assert_eq!(reused, expected_s256);
    assert_eq!(
        plain,
        [
            Ok(()),
            Ok(()),
            Ok(()),
            Err(InvalidGrant),
            Err(InvalidRequest),
            Err(InvalidRequest),
            Err(InvalidRequest),
            Err(InvalidGrant),
            Err(InvalidGrant)
        ]
    );
    assert_eq!(
        without,
        [
            Err(InvalidRequest),
            Err(InvalidRequest),
            Err(InvalidRequest),
            Err(InvalidRequest),
            Err(InvalidRequest),
            Ok(()),
            Ok(()),
            Err(InvalidGrant),
            Err(InvalidGrant)
        ]
    );
}

// ---------------------------------------------------------------------------
// The idiomatic reference
// ---------------------------------------------------------------------------

fn to_params(p: &AuthorizationParams) -> idiomatic::AuthorizationParams {
    idiomatic::AuthorizationParams {
        client_id: p.client_id.clone(),
        response_type: p.response_type.clone(),
        redirect_uri: p.redirect_uri.clone(),
        scope: p.scope.clone(),
        state: p.state.clone(),
        nonce: p.nonce.clone(),
        code_challenge: p.code_challenge.clone(),
        code_challenge_method: p.code_challenge_method.clone(),
        prompt: p.prompt.clone(),
        max_age: p.max_age.clone(),
        acr_values: p.acr_values.clone(),
    }
}

fn to_client(c: &Client) -> idiomatic::Client {
    idiomatic::Client {
        client_id: c.client_id.clone(),
        redirect_uris: c.redirect_uris.clone(),
        require_pkce: c.require_pkce,
        allow_plain_pkce: c.allow_plain_pkce,
    }
}

fn to_strength(s: AuthStrength) -> idiomatic::AuthStrength {
    match s {
        AuthStrength::PasswordOnly => idiomatic::AuthStrength::PasswordOnly,
        AuthStrength::PasswordAndTotp => idiomatic::AuthStrength::PasswordAndTotp,
    }
}

fn to_session(s: &Session) -> idiomatic::Session {
    idiomatic::Session { subject: s.subject.clone(), auth_time: s.auth_time, strength: to_strength(s.strength) }
}

fn to_enrollment(e: &TotpEnrollment) -> idiomatic::TotpEnrollment {
    let digits = match e.digits {
        OtpDigits::Six => idiomatic::OtpDigits::Six,
        OtpDigits::Seven => idiomatic::OtpDigits::Seven,
        OtpDigits::Eight => idiomatic::OtpDigits::Eight,
    };
    idiomatic::TotpEnrollment {
        t0: e.t0,
        period: e.period,
        digits,
        last_used_step: e.last_used_step,
        failures: e.failures,
    }
}

fn to_event(e: Event) -> idiomatic::Event {
    match e {
        Event::PasswordChecked { subject, verified, second_factor, now } => idiomatic::Event::PasswordChecked {
            subject,
            verified,
            second_factor: match second_factor {
                SecondFactor::Totp(e) => idiomatic::SecondFactor::Totp(to_enrollment(&e)),
                SecondFactor::NotEnrolled => idiomatic::SecondFactor::NotEnrolled,
            },
            now,
        },
        Event::OtpSubmitted { code, now, candidates } => idiomatic::Event::OtpSubmitted {
            code,
            now,
            candidates: candidates.into_iter().map(|c| idiomatic::StepMac { step: c.step, mac: c.mac }).collect(),
        },
        Event::ConsentGranted => idiomatic::Event::ConsentGranted,
        Event::ConsentDenied => idiomatic::Event::ConsentDenied,
    }
}

fn idiomatic_begin(s: &Setup) -> Result<idiomatic::Flow, idiomatic::AuthorizationError> {
    let client = s.client.as_ref().map(to_client);
    let session = s.session.as_ref().map(to_session);
    idiomatic::begin(&to_params(&s.params), client.as_ref(), session.as_ref(), s.consent_on_file, s.now)
}

/// The driver's `RunError`, over the reference's types.
#[derive(Debug)]
#[allow(dead_code)]
enum RunError {
    Refused(idiomatic::AuthorizationError),
    Step { index: u32, error: idiomatic::FlowError },
}

fn idiomatic_trace(s: &Setup, codes: &[u8]) -> Result<idiomatic::Flow, RunError> {
    let policy = idiomatic::Policy {
        max_password_failures: s.policy.max_password_failures,
        max_otp_failures: s.policy.max_otp_failures,
    };
    let mut flow = idiomatic_begin(s).map_err(RunError::Refused)?;
    for (index, &c) in codes.iter().enumerate() {
        flow = idiomatic::step(flow, to_event(event(c)), &policy)
            .map_err(|error| RunError::Step { index: index as u32, error })?;
    }
    Ok(flow)
}

/// Both sides as `Debug` text.
fn same(constrained: &impl std::fmt::Debug, reference: &impl std::fmt::Debug) -> bool {
    format!("{constrained:?}") == format!("{reference:?}")
}

/// Every request under each client (strict, lax, unknown), session and
/// consent on file.
fn begins() -> Vec<Setup> {
    let mut out = Vec::new();
    for params in requests() {
        for client in [strict(), lax(), None] {
            for session in sessions() {
                for consent_on_file in [false, true] {
                    out.push(Setup {
                        params: params.clone(),
                        client: client.clone(),
                        session: session.clone(),
                        consent_on_file,
                        now: NOW,
                        policy: Policy { max_password_failures: 3, max_otp_failures: 3 },
                    });
                }
            }
        }
    }
    out
}

/// Every sequence of `n` events over the alphabet.
fn sequences(n: u32) -> impl Iterator<Item = Vec<u8>> {
    (0..u32::from(EVENTS).pow(n)).map(move |mut k| {
        (0..n)
            .map(|_| {
                let c = (k % u32::from(EVENTS)) as u8;
                k /= u32::from(EVENTS);
                c
            })
            .collect()
    })
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let mut differ: Vec<String> = Vec::new();
    for s in begins() {
        let constrained = oidc::begin(&s.params, &s.client, &s.session, s.consent_on_file, s.now);
        let reference = idiomatic_begin(&s);
        if !same(&constrained, &reference) {
            differ.push(format!("begin {:?}: idiomatic {reference:?} / constrained {constrained:?}", s.params));
        }
    }
    for start in STARTS {
        let s = setup(&strict(), base(), start);
        for codes in sequences(4).chain(runs(&s, 5)) {
            let constrained = trace(&s, &codes);
            let reference = idiomatic_trace(&s, &codes);
            if !same(&constrained, &reference) {
                differ.push(format!("{codes:?}: idiomatic {reference:?} / constrained {constrained:?}"));
            }
        }
    }
    for (g, id, uri, v, h) in redemptions() {
        let constrained = oidc::check_redemption(&g, &id, &uri, &v, &h);
        let pkce = g.pkce.as_ref().map(|p| idiomatic::Pkce {
            challenge: p.challenge.clone(),
            method: if matches!(p.method, oidc::PkceMethod::S256) {
                idiomatic::PkceMethod::S256
            } else {
                idiomatic::PkceMethod::Plain
            },
        });
        let ig = idiomatic::CodeGrant {
            client_id: g.client_id.clone(),
            redirect_uri: g.redirect_uri.clone(),
            scope: g.scope.clone(),
            state: g.state.clone(),
            nonce: g.nonce.clone(),
            pkce,
            subject: g.subject.clone(),
            auth_time: g.auth_time,
            amr: g.amr.clone(),
            acr: g.acr.clone(),
        };
        let r = idiomatic::check_redemption(&ig, &id, &uri, v.as_deref(), h.as_deref());
        if !same(&constrained, &r) {
            differ.push(format!("redeem {g:?} {id} {uri} {v:?} {h:?}: idiomatic {r:?} / constrained {constrained:?}"));
        }
    }
    for (now, _) in APPENDIX_B {
        let t = now / 30;
        for step in t - 2..=t + 2 {
            let mac = mac_for_step(step);
            for (d, i) in [
                (OtpDigits::Six, idiomatic::OtpDigits::Six),
                (OtpDigits::Seven, idiomatic::OtpDigits::Seven),
                (OtpDigits::Eight, idiomatic::OtpDigits::Eight),
            ] {
                if oidc::truncate_mac(&mac, d) != idiomatic::truncate_mac(&mac, i) {
                    differ.push(format!("truncate step {step} {d:?}"));
                }
            }
        }
    }
    assert!(differ.is_empty(), "{} differ, first:\n{}", differ.len(), differ[..differ.len().min(10)].join("\n"));
}

// ---------------------------------------------------------------------------
// The generated package
// ---------------------------------------------------------------------------

/// Every run of up to `depth` events, cut at the first refused one: `play`
/// never reads a script past it, so the longer sequences add nothing.
fn runs(s: &Setup, depth: usize) -> Vec<Vec<u8>> {
    fn grow(s: &Setup, prefix: &mut Vec<u8>, depth: usize, out: &mut Vec<Vec<u8>>) {
        for c in 0..EVENTS {
            prefix.push(c);
            if prefix.len() == depth || trace(s, prefix).is_err() {
                out.push(prefix.clone());
            } else {
                grow(s, prefix, depth, out);
            }
            prefix.pop();
        }
    }
    let mut out = vec![vec![]];
    grow(s, &mut Vec::new(), depth, &mut out);
    out
}

#[test]
fn generated_oidc_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        // Truncation and the TOTP check on the published vectors.
        let example = unhex("1f8698690e02ca16618550ef7f19da8e945b555a");
        let short = example[..19].to_vec();
        let high = [0xffu8; 20].to_vec();
        for mac in APPENDIX_D.iter().map(|(hex, _, _)| unhex(hex)).chain([example, short, high]) {
            for d in [OtpDigits::Six, OtpDigits::Seven, OtpDigits::Eight] {
                cases.push(case!(oidc::truncate_mac(&mac, d)));
            }
        }
        for (now, code) in APPENDIX_B {
            let t = now / 30;
            let code = code.to_string();
            let window = candidates(t - 2..=t + 2);
            for last in [None, Some(t - 1), Some(t), Some(t + 1)] {
                cases.push(case!(oidc::check_totp(&code, now, &enrollment(OtpDigits::Eight, last), &window)));
            }
            for other in [code[1..].to_string(), format!("{code}0"), code.replace('0', "O")] {
                cases.push(case!(oidc::check_totp(&other, now, &enrollment(OtpDigits::Eight, None), &window)));
            }
            cases.push(case!(oidc::check_totp(
                &code,
                now,
                &enrollment(OtpDigits::Eight, None),
                &candidates(t + 1..=t + 2)
            )));
            cases.push(case!(oidc::check_totp(&code, now, &enrollment(OtpDigits::Six, None), &window)));
            cases.push(case!(oidc::check_totp(
                &code[2..].to_string(),
                now,
                &enrollment(OtpDigits::Six, None),
                &window
            )));
        }
        let before = TotpEnrollment { t0: 60, ..enrollment(OtpDigits::Six, None) };
        let stalled = TotpEnrollment { period: 0, ..enrollment(OtpDigits::Six, None) };
        for (now, e) in [(59i64, &before), (60, &before), (59, &stalled)] {
            cases.push(case!(oidc::check_totp(&"287082".to_string(), now, e, &candidates(0..=3))));
        }
        for (now, t0, period) in [
            (0i64, 0i64, 30i64),
            (29, 0, 30),
            (30, 0, 30),
            (-1, 0, 30),
            (100, 100, 30),
            (99, 100, 30),
            (59, 0, 0),
            (59, 0, -30),
            (i64::MAX, 0, 1),
            (i64::MAX, i64::MIN, 30),
        ] {
            cases.push(case!(oidc::totp_step(now, t0, period)));
        }
        // Every request under each client, session and consent on file.
        for s in begins() {
            cases.push(case!(oidc::begin(&s.params, &s.client, &s.session, s.consent_on_file, s.now)));
        }
        // Every run of up to five events from each start.
        for start in STARTS {
            let s = setup(&strict(), base(), start);
            for codes in runs(&s, 5) {
                cases.push(case!(oidc::trace(
                    &s.params,
                    &s.client,
                    &s.session,
                    s.consent_on_file,
                    s.now,
                    &s.policy,
                    script(&codes)
                )));
            }
        }
        for (g, id, uri, v, h) in redemptions() {
            cases.push(case!(oidc::check_redemption(&g, &id, &uri, &v, &h)));
        }
        for list in [
            "openid",
            "openid profile",
            "profile openid",
            " openid",
            "openid ",
            "openidx",
            "xopenid",
            "",
            " ",
            "open id",
        ] {
            cases.push(case!(oidc::has_token(&list.to_string(), "openid")));
        }
        for code in [
            oidc::ErrorCode::InvalidRequest,
            oidc::ErrorCode::UnsupportedResponseType,
            oidc::ErrorCode::InvalidScope,
            oidc::ErrorCode::AccessDenied,
            oidc::ErrorCode::LoginRequired,
            oidc::ErrorCode::ConsentRequired,
        ] {
            cases.push(case!(oidc::error_code_wire(code)));
        }
        cases
    });
    for reached in [
        "Ok(Flow::AwaitingPassword {",
        "Ok(Flow::AwaitingOtp {",
        "notice: Notice::OtpReplayed",
        "notice: Notice::MalformedOtp",
        "Ok(Flow::AwaitingConsent {",
        "totp_step: Some(0)",
        "totp_step: Some(2)",
        r#"amr: ["pwd", "otp", "mfa"]"#,
        r#"amr: ["pwd"]"#,
        "Ok(Flow::Rejected(",
        "Ok(Flow::Locked {",
        "Err(RunError::Step { index: 4, error: FlowError::InvalidTransition })",
        "Err(AuthorizationError::Display(DisplayError::UnknownClient))",
        "Err(AuthorizationError::Display(DisplayError::MissingRedirectUri))",
        "Err(AuthorizationError::Display(DisplayError::UnregisteredRedirectUri))",
        "error: ErrorCode::UnsupportedResponseType, state: None",
        "error: ErrorCode::InvalidScope",
        "error: ErrorCode::LoginRequired",
        "error: ErrorCode::ConsentRequired",
        "OtpCheck::Accepted(",
        "OtpCheck::Replayed",
        "OtpCheck::Mismatch",
        "OtpCheck::Malformed",
        "OtpCheck::ClockBeforeEpoch",
        "Err(TokenError::InvalidGrant)",
    ] {
        assert!(cases.iter().any(|c| c.rust.contains(reached)), "no case reaches {reached}");
    }
    support::assert_equivalent("oidc", SOURCE, &cases);
}
