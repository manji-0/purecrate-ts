// OpenID Provider login flow for the authorization code flow, with a TOTP
// second factor. The login UI and the server share this state machine.
//
// Specifications:
// - OpenID Connect Core 1.0 (OIDC) §3.1.2.1 authentication request,
//   §3.1.2.2 request validation, §3.1.2.3 authentication, §3.1.2.4 consent,
//   §3.1.2.6 error responses.
// - RFC 6749 §4.1 authorization code grant, §4.1.2.1 error response,
//   §10.12 CSRF (`state`), Appendix A.5 (`state` syntax).
// - RFC 7636 PKCE §4.1-§4.6.
// - RFC 4226 HOTP §5.3 dynamic truncation, §7.3 throttling.
// - RFC 6238 TOTP §4.2 time step, §5.2 validation window and replay.
// - RFC 8176 `amr` values.
//
// Everything impure is outside: the caller looks up the client and the
// session, verifies the password hash, computes HMAC-SHA-1 over each
// candidate time step, computes BASE64URL(SHA-256(code_verifier)), supplies
// `now` (seconds since the Unix epoch), generates the code, and stores the
// returned values (lockout counters, last used TOTP step, the code grant).
//
// Left out on purpose:
// - response types other than `code` (implicit, hybrid), `response_mode`;
// - `request` / `request_uri` objects, `claims`, `display`, `ui_locales`,
//   `id_token_hint`, `login_hint`, `registration`;
// - duplicate-parameter detection (the caller's query parser owns that);
// - `acr_values` beyond "is the MFA class requested" (acr is voluntary,
//   OIDC §5.5.1.1 essential acr claims are not supported);
// - multiple accounts per browser (`prompt=select_account` just forces
//   the login form);
// - HOTP counter-based resynchronisation (RFC 4226 §7.4) and TOTP drift
//   tracking (RFC 6238 §6); only the ±1 step window is accepted;
// - the ID token itself, token endpoint client authentication, code expiry.

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error codes returned to the client via redirect
/// (RFC 6749 §4.1.2.1, OIDC Core §3.1.2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedResponseType,
    InvalidScope,
    AccessDenied,
    LoginRequired,
    ConsentRequired,
}

/// The wire value of an error code.
pub fn error_code_wire(code: ErrorCode) -> String {
    match code {
        ErrorCode::InvalidRequest => String::from("invalid_request"),
        ErrorCode::UnsupportedResponseType => String::from("unsupported_response_type"),
        ErrorCode::InvalidScope => String::from("invalid_scope"),
        ErrorCode::AccessDenied => String::from("access_denied"),
        ErrorCode::LoginRequired => String::from("login_required"),
        ErrorCode::ConsentRequired => String::from("consent_required"),
    }
}

/// Errors that MUST NOT be redirected to the client: the redirect target
/// itself is not trustworthy, so the OP shows them to the End-User
/// (RFC 6749 §4.1.2.1 first paragraph, OIDC Core §3.1.2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayError {
    UnknownClient,
    MissingRedirectUri,
    UnregisteredRedirectUri,
}

/// An error response sent to the client's redirect_uri. `state` is echoed
/// exactly when the request carried one (RFC 6749 §4.1.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorRedirect {
    pub redirect_uri: String,
    pub error: ErrorCode,
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationError {
    Display(DisplayError),
    Redirect(ErrorRedirect),
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// Client registration, looked up by the caller from `client_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub client_id: String,
    pub redirect_uris: Vec<String>,
    /// Public clients must send a code_challenge.
    pub require_pkce: bool,
    /// RFC 7636 §4.2: `plain` only for clients that cannot do S256.
    pub allow_plain_pkce: bool,
}

/// Raw authorization request parameters (OIDC Core §3.1.2.1), as strings
/// from the query or form. Absent parameters are `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// How strongly the End-User was authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStrength {
    PasswordOnly,
    PasswordAndTotp,
}

/// An existing OP session for this browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub subject: String,
    pub auth_time: i64,
    pub strength: AuthStrength,
}

/// Limits the OP applies (RFC 4226 §7.3 throttling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub max_password_failures: u32,
    pub max_otp_failures: u32,
}

// ---------------------------------------------------------------------------
// Validated request
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    S256,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    pub challenge: String,
    pub method: PkceMethod,
}

/// The `prompt` parameter (OIDC Core §3.1.2.1), a space-delimited set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prompt {
    pub no_interaction: bool,
    pub login: bool,
    pub consent: bool,
    pub select_account: bool,
}

/// A request that passed §3.1.2.2 validation. Only `validate_request`
/// builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Read access for the consent screen and the server. Fields stay private
/// so that only `validate_request` can construct a request.
impl AuthorizationRequest {
    pub fn client_id(&self) -> &String {
        &self.client_id
    }

    pub fn redirect_uri(&self) -> &String {
        &self.redirect_uri
    }

    pub fn scope(&self) -> &String {
        &self.scope
    }

    pub fn state(&self) -> &String {
        &self.state
    }

    pub fn wants_mfa(&self) -> bool {
        self.wants_mfa
    }
}

/// The acr value this OP asserts for two-factor logins.
pub const ACR_MFA: &str = "urn:example:acr:mfa";

/// The acr value this OP asserts for password-only logins.
pub const ACR_PASSWORD: &str = "urn:example:acr:pwd";

/// The OP bounds `state` so it cannot be used to bloat redirects.
const MAX_STATE_LEN: usize = 512;

/// RFC 7636 §4.1: a code verifier (and a plain challenge) is 43..=128
/// characters.
const PKCE_MIN_LEN: usize = 43;
const PKCE_MAX_LEN: usize = 128;

/// Decimal digits that always fit in i64.
const MAX_SECONDS_DIGITS: usize = 18;

/// An HMAC-SHA-1 output, the shortest MAC RFC 4226 truncates.
const SHA1_LEN: usize = 20;

// ---------------------------------------------------------------------------
// Lexical helpers
// ---------------------------------------------------------------------------

/// Whether the space-delimited list `list` contains `word` as a whole token
/// (scope: RFC 6749 §3.3; prompt and acr_values: OIDC Core §3.1.2.1).
pub fn has_token(list: &String, word: &str) -> bool {
    list.split(' ').any(|token| token == word)
}

/// RFC 6749 Appendix A.5: state = 1*VSCHAR, VSCHAR = %x20-7E, at most
/// `MAX_STATE_LEN` bytes.
fn state_is_valid(state: &String) -> bool {
    !state.is_empty() && state.len() <= MAX_STATE_LEN && state.bytes().all(|b| matches!(b, 0x20..=0x7e))
}

/// RFC 7636 §4.1 / §4.2: 43..=128 characters of
/// unreserved = ALPHA / DIGIT / "-" / "." / "_" / "~".
pub fn pkce_string_is_valid(s: &String) -> bool {
    s.len() >= PKCE_MIN_LEN
        && s.len() <= PKCE_MAX_LEN
        && s.bytes()
            .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'))
}

/// A non-negative decimal integer such as `max_age` (OIDC Core §3.1.2.1).
/// At most `MAX_SECONDS_DIGITS` digits so the value fits in i64.
fn parse_seconds(s: &String) -> Option<i64> {
    if s.is_empty() || s.len() > MAX_SECONDS_DIGITS || !s.bytes().all(|b| matches!(b, b'0'..=b'9')) {
        return None;
    }
    let mut value: i64 = 0;
    for b in s.bytes() {
        value = value * 10 + i64::from(b - b'0');
    }
    Some(value)
}

/// Parses `prompt`. Unknown values and `none` combined with anything else
/// are invalid_request (OIDC Core §3.1.2.1).
fn parse_prompt(s: &String) -> Option<Prompt> {
    let mut no_interaction = false;
    let mut login = false;
    let mut consent = false;
    let mut select_account = false;
    for token in s.split(' ') {
        match token {
            "none" => no_interaction = true,
            "login" => login = true,
            "consent" => consent = true,
            "select_account" => select_account = true,
            "" => {}
            _ => return None,
        }
    }
    if no_interaction && (login || consent || select_account) {
        return None;
    }
    Some(Prompt {
        no_interaction,
        login,
        consent,
        select_account,
    })
}

// ---------------------------------------------------------------------------
// Request validation (OIDC Core §3.1.2.2)
// ---------------------------------------------------------------------------

fn redirect_error(redirect_uri: &String, error: ErrorCode, state: &Option<String>) -> AuthorizationError {
    AuthorizationError::Redirect(ErrorRedirect {
        redirect_uri: String::from(redirect_uri),
        error,
        state: copy_optional(state),
    })
}

/// Rust's `clone` is outside the subset; strings are rebuilt with
/// `String::from`.
fn copy_optional(s: &Option<String>) -> Option<String> {
    match s {
        Some(v) => Some(String::from(v)),
        None => None,
    }
}

fn redirect_uri_registered(client: &Client, uri: &String) -> bool {
    // OIDC Core §3.1.2.1: exact match using simple string comparison.
    client.redirect_uris.iter().any(|registered| registered == uri)
}

/// Validates an authorization request against the client registration.
///
/// Errors about the client or its redirect_uri are shown to the End-User;
/// everything after that is redirected with `error` and the echoed `state`.
pub fn validate_request(
    params: &AuthorizationParams,
    client: &Option<Client>,
) -> Result<AuthorizationRequest, AuthorizationError> {
    let client = match client {
        Some(c) => c,
        None => return Err(AuthorizationError::Display(DisplayError::UnknownClient)),
    };
    match &params.client_id {
        Some(id) if *id == client.client_id => {}
        _ => return Err(AuthorizationError::Display(DisplayError::UnknownClient)),
    }
    // redirect_uri is REQUIRED in OIDC (§3.1.2.1), unlike RFC 6749 §4.1.1.
    let redirect_uri = match &params.redirect_uri {
        Some(u) => u,
        None => return Err(AuthorizationError::Display(DisplayError::MissingRedirectUri)),
    };
    if !redirect_uri_registered(client, redirect_uri) {
        return Err(AuthorizationError::Display(DisplayError::UnregisteredRedirectUri));
    }
    // From here on the redirect target is trusted. Echo state only if it
    // is well formed; a malformed state is not reflected.
    let echoed = match &params.state {
        Some(s) if state_is_valid(s) => Some(String::from(s)),
        _ => None,
    };
    let fail = |error: ErrorCode| redirect_error(redirect_uri, error, &echoed);
    match &params.response_type {
        Some(rt) if rt == "code" => {}
        Some(_) => return Err(fail(ErrorCode::UnsupportedResponseType)),
        None => return Err(fail(ErrorCode::InvalidRequest)),
    }
    // §3.1.2.1: scope MUST contain openid; without it this is not an OIDC
    // request, and this OP serves only OIDC.
    let scope = match &params.scope {
        Some(s) if has_token(s, "openid") => s,
        _ => return Err(fail(ErrorCode::InvalidScope)),
    };
    // RFC 6749 §10.12: this OP requires state from every client.
    let state = match &echoed {
        Some(s) => String::from(s),
        None => return Err(fail(ErrorCode::InvalidRequest)),
    };
    let nonce = match &params.nonce {
        Some(n) if !state_is_valid(n) => return Err(fail(ErrorCode::InvalidRequest)),
        Some(n) => Some(String::from(n)),
        None => None,
    };
    let pkce = match (&params.code_challenge, &params.code_challenge_method) {
        (Some(challenge), _) if !pkce_string_is_valid(challenge) => return Err(fail(ErrorCode::InvalidRequest)),
        (Some(challenge), method) => {
            // RFC 7636 §4.3: absent method means plain.
            let method = match method {
                Some(m) if m == "S256" => PkceMethod::S256,
                Some(m) if m == "plain" => PkceMethod::Plain,
                None => PkceMethod::Plain,
                Some(_) => return Err(fail(ErrorCode::InvalidRequest)),
            };
            if matches!(method, PkceMethod::Plain) && !client.allow_plain_pkce {
                return Err(fail(ErrorCode::InvalidRequest));
            }
            Some(Pkce {
                challenge: String::from(challenge),
                method,
            })
        }
        // RFC 7636 §4.4.1: a required challenge that is missing, or a method
        // without a challenge.
        (None, Some(_)) => return Err(fail(ErrorCode::InvalidRequest)),
        (None, None) if client.require_pkce => return Err(fail(ErrorCode::InvalidRequest)),
        (None, None) => None,
    };
    let prompt = match &params.prompt {
        Some(p) => parse_prompt(p).ok_or(fail(ErrorCode::InvalidRequest))?,
        None => Prompt {
            no_interaction: false,
            login: false,
            consent: false,
            select_account: false,
        },
    };
    let max_age = match &params.max_age {
        Some(m) => Some(parse_seconds(m).ok_or(fail(ErrorCode::InvalidRequest))?),
        None => None,
    };
    let wants_mfa = match &params.acr_values {
        Some(a) => has_token(a, ACR_MFA),
        None => false,
    };
    Ok(AuthorizationRequest {
        client_id: String::from(&client.client_id),
        redirect_uri: String::from(redirect_uri),
        scope: String::from(scope),
        state,
        nonce,
        pkce,
        prompt,
        max_age,
        wants_mfa,
    })
}

// ---------------------------------------------------------------------------
// TOTP (RFC 6238) over HOTP truncation (RFC 4226)
// ---------------------------------------------------------------------------

/// Number of decimal digits in an OTP (RFC 4226 §5.3 requires at least 6);
/// the discriminant is the count.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpDigits {
    Six = 6,
    Seven = 7,
    Eight = 8,
}

fn digit_count(d: OtpDigits) -> usize {
    usize::from(d as u8)
}

fn digit_modulus(d: OtpDigits) -> u32 {
    10u32.pow(u32::from(d as u8))
}

/// A user's TOTP enrollment, loaded by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpEnrollment {
    /// T0, Unix time to start counting steps (RFC 6238 §4.1).
    pub t0: i64,
    /// X, the time step in seconds (RFC 6238 §4.1).
    pub period: i64,
    pub digits: OtpDigits,
    /// The step of the last accepted OTP (RFC 6238 §5.2 replay rule).
    pub last_used_step: Option<i64>,
}

/// T = floor((now - T0) / X) (RFC 6238 §4.2). None before T0 or with a
/// non-positive period.
pub fn totp_step(now: i64, t0: i64, period: i64) -> Option<i64> {
    if period <= 0 || now < t0 {
        return None;
    }
    Some((now - t0) / period)
}

/// HMAC output for one candidate time step, computed by the caller as
/// HMAC-SHA-1(K, T as 8-byte big-endian).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepMac {
    pub step: i64,
    pub mac: Vec<u8>,
}

/// Dynamic truncation and reduction (RFC 4226 §5.3):
/// offset = low 4 bits of the last byte, take 31 bits at offset,
/// then mod 10^digits. None for a MAC shorter than SHA-1's 20 bytes.
pub fn truncate_mac(mac: &Vec<u8>, digits: OtpDigits) -> Option<u32> {
    let n = mac.len();
    if n < SHA1_LEN {
        return None;
    }
    // As RFC 4226 §5.4 writes it; the top bit is masked off.
    let offset = usize::from(mac[n - 1] & 0x0f);
    let bin = (u32::from(mac[offset] & 0x7f) << 24)
        | (u32::from(mac[offset + 1]) << 16)
        | (u32::from(mac[offset + 2]) << 8)
        | u32::from(mac[offset + 3]);
    Some(bin % digit_modulus(digits))
}

/// Parses a submitted OTP: exactly `digits` ASCII digits, leading zeros
/// included.
fn parse_otp(code: &String, digits: OtpDigits) -> Option<u32> {
    if code.len() != digit_count(digits) || !code.bytes().all(|b| matches!(b, b'0'..=b'9')) {
        return None;
    }
    let mut value: u32 = 0;
    for b in code.bytes() {
        value = value * 10 + u32::from(b - b'0');
    }
    Some(value)
}

/// Result of checking a submitted OTP against the candidate MACs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpCheck {
    Accepted(i64),
    Replayed,
    Mismatch,
    Malformed,
    ClockBeforeEpoch,
}

/// RFC 6238 §5.2: accept steps T-1, T, T+1, but never a step at or before
/// the last accepted one. Candidates outside that window are ignored, so
/// the caller cannot widen it by passing more MACs.
pub fn check_totp(code: &String, now: i64, enrollment: &TotpEnrollment, candidates: &Vec<StepMac>) -> OtpCheck {
    let current = match totp_step(now, enrollment.t0, enrollment.period) {
        Some(t) => t,
        None => return OtpCheck::ClockBeforeEpoch,
    };
    let submitted = match parse_otp(code, enrollment.digits) {
        Some(v) => v,
        None => return OtpCheck::Malformed,
    };
    let mut replayed = false;
    for c in candidates {
        if c.step >= current - 1 && c.step <= current + 1 {
            if matches!(truncate_mac(&c.mac, enrollment.digits), Some(value) if value == submitted) {
                let fresh = enrollment.last_used_step.map(|last| c.step > last).unwrap_or(true);
                if fresh {
                    return OtpCheck::Accepted(c.step);
                }
                replayed = true;
            }
        }
    }
    if replayed {
        OtpCheck::Replayed
    } else {
        OtpCheck::Mismatch
    }
}

// ---------------------------------------------------------------------------
// The login state machine
// ---------------------------------------------------------------------------

/// What the login UI should tell the End-User about the last attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Clear,
    WrongPassword,
    WrongOtp,
    OtpReplayed,
    MalformedOtp,
}

/// A completed authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authentication {
    pub subject: String,
    pub auth_time: i64,
    pub strength: AuthStrength,
    /// The TOTP step accepted in this flow; the caller stores it as the new
    /// `last_used_step`.
    pub totp_step: Option<i64>,
}

/// What the token endpoint needs to redeem the code (RFC 6749 §4.1.3) and
/// to build the ID token (OIDC Core §2, §3.1.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flow {
    AwaitingPassword {
        request: AuthorizationRequest,
        failures: u32,
        notice: Notice,
    },
    AwaitingOtp {
        request: AuthorizationRequest,
        subject: String,
        enrollment: TotpEnrollment,
        failures: u32,
        notice: Notice,
    },
    AwaitingConsent {
        request: AuthorizationRequest,
        auth: Authentication,
    },
    CodeIssued(CodeGrant),
    /// Terminal: redirect to the client with an error (e.g. consent denied).
    Rejected(ErrorRedirect),
    /// Terminal: too many failures; shown to the End-User, not redirected.
    Locked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecondFactor {
    Totp(TotpEnrollment),
    NotEnrolled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The caller verified the password hash for `subject`.
    PasswordChecked {
        subject: String,
        verified: bool,
        second_factor: SecondFactor,
        now: i64,
    },
    OtpSubmitted {
        code: String,
        now: i64,
        candidates: Vec<StepMac>,
    },
    ConsentGranted,
    ConsentDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowError {
    InvalidTransition,
}

/// RFC 8176 §2: pwd, otp, and mfa when more than one factor was used.
pub fn amr_values(strength: AuthStrength) -> Vec<String> {
    match strength {
        AuthStrength::PasswordOnly => vec![String::from("pwd")],
        AuthStrength::PasswordAndTotp => vec![String::from("pwd"), String::from("otp"), String::from("mfa")],
    }
}

pub fn acr_value(strength: AuthStrength) -> String {
    match strength {
        AuthStrength::PasswordOnly => String::from(ACR_PASSWORD),
        AuthStrength::PasswordAndTotp => String::from(ACR_MFA),
    }
}

fn issue(request: AuthorizationRequest, auth: Authentication) -> Flow {
    Flow::CodeIssued(CodeGrant {
        client_id: request.client_id,
        redirect_uri: request.redirect_uri,
        scope: request.scope,
        state: request.state,
        nonce: request.nonce,
        pkce: request.pkce,
        subject: auth.subject,
        auth_time: auth.auth_time,
        amr: amr_values(auth.strength),
        acr: acr_value(auth.strength),
    })
}

/// Whether an existing session satisfies the request without asking the
/// End-User to log in again (OIDC Core §3.1.2.1 prompt, max_age,
/// acr_values; §3.1.2.3).
pub fn session_is_usable(request: &AuthorizationRequest, session: &Session, now: i64) -> bool {
    if request.prompt.login || request.prompt.select_account {
        return false;
    }
    let fresh = request
        .max_age
        .map(|max_age| now - session.auth_time <= max_age)
        .unwrap_or(true);
    let strong_enough = !request.wants_mfa || matches!(session.strength, AuthStrength::PasswordAndTotp);
    fresh && strong_enough
}

/// Starts a flow for a request. `consent_on_file` says whether the End-User
/// already consented to this client and scope.
pub fn begin(
    params: &AuthorizationParams,
    client: &Option<Client>,
    session: &Option<Session>,
    consent_on_file: bool,
    now: i64,
) -> Result<Flow, AuthorizationError> {
    let request = validate_request(params, client)?;
    let reusable = match session {
        Some(s) if session_is_usable(&request, s, now) => Some(Authentication {
            subject: String::from(&s.subject),
            auth_time: s.auth_time,
            strength: s.strength,
            totp_step: None,
        }),
        _ => None,
    };
    let needs_consent = request.prompt.consent || !consent_on_file;
    // OIDC Core §3.1.2.1 prompt=none and §3.1.2.6 error codes.
    let state = Some(String::from(&request.state));
    let refuse = |error: ErrorCode| redirect_error(&request.redirect_uri, error, &state);
    match reusable {
        None if request.prompt.no_interaction => Err(refuse(ErrorCode::LoginRequired)),
        Some(_) if request.prompt.no_interaction && needs_consent => Err(refuse(ErrorCode::ConsentRequired)),
        Some(auth) if !needs_consent => Ok(issue(request, auth)),
        Some(auth) => Ok(Flow::AwaitingConsent { request, auth }),
        None => Ok(Flow::AwaitingPassword {
            request,
            failures: 0,
            notice: Notice::Clear,
        }),
    }
}

/// One transition of the login flow.
pub fn step(flow: Flow, event: Event, policy: &Policy) -> Result<Flow, FlowError> {
    match (flow, event) {
        (Flow::AwaitingPassword { request, failures, .. }, Event::PasswordChecked { verified: false, .. }) => {
            let failures = failures + 1;
            if failures >= policy.max_password_failures {
                return Ok(Flow::Locked);
            }
            Ok(Flow::AwaitingPassword {
                request,
                failures,
                notice: Notice::WrongPassword,
            })
        }
        (
            Flow::AwaitingPassword { request, .. },
            Event::PasswordChecked {
                subject,
                second_factor,
                now,
                ..
            },
        ) => match second_factor {
            SecondFactor::Totp(enrollment) => Ok(Flow::AwaitingOtp {
                request,
                subject,
                enrollment,
                failures: 0,
                notice: Notice::Clear,
            }),
            SecondFactor::NotEnrolled => {
                // acr_values is a voluntary claim (§3.1.2.1): proceed and
                // report the weaker acr rather than fail.
                let auth = Authentication {
                    subject,
                    auth_time: now,
                    strength: AuthStrength::PasswordOnly,
                    totp_step: None,
                };
                Ok(Flow::AwaitingConsent { request, auth })
            }
        },
        (
            Flow::AwaitingOtp {
                request,
                subject,
                enrollment,
                failures,
                ..
            },
            Event::OtpSubmitted { code, now, candidates },
        ) => {
            let notice = match check_totp(&code, now, &enrollment, &candidates) {
                OtpCheck::Accepted(step) => {
                    let auth = Authentication {
                        subject,
                        auth_time: now,
                        strength: AuthStrength::PasswordAndTotp,
                        totp_step: Some(step),
                    };
                    return Ok(Flow::AwaitingConsent { request, auth });
                }
                OtpCheck::Replayed => Notice::OtpReplayed,
                OtpCheck::Malformed => Notice::MalformedOtp,
                OtpCheck::Mismatch | OtpCheck::ClockBeforeEpoch => Notice::WrongOtp,
            };
            // RFC 4226 §7.3: every rejected value counts toward the limit.
            let failures = failures + 1;
            if failures >= policy.max_otp_failures {
                return Ok(Flow::Locked);
            }
            Ok(Flow::AwaitingOtp {
                request,
                subject,
                enrollment,
                failures,
                notice,
            })
        }
        (Flow::AwaitingConsent { request, auth }, Event::ConsentGranted) => Ok(issue(request, auth)),
        (Flow::AwaitingConsent { request, .. }, Event::ConsentDenied) => Ok(Flow::Rejected(ErrorRedirect {
            redirect_uri: request.redirect_uri,
            error: ErrorCode::AccessDenied,
            state: Some(request.state),
        })),
        _ => Err(FlowError::InvalidTransition),
    }
}

// ---------------------------------------------------------------------------
// Token endpoint checks (RFC 6749 §4.1.3, RFC 7636 §4.5-§4.6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    InvalidRequest,
    InvalidGrant,
}

/// Checks a code redemption against its grant. `verifier_s256` is
/// BASE64URL(SHA256(ASCII(code_verifier))), computed by the caller.
pub fn check_redemption(
    grant: &CodeGrant,
    client_id: &String,
    redirect_uri: &String,
    code_verifier: &Option<String>,
    verifier_s256: &Option<String>,
) -> Result<(), TokenError> {
    if grant.client_id != *client_id || grant.redirect_uri != *redirect_uri {
        return Err(TokenError::InvalidGrant);
    }
    match (&grant.pkce, code_verifier) {
        (None, None) => Ok(()),
        (None, Some(_)) | (Some(_), None) => Err(TokenError::InvalidRequest),
        (Some(_), Some(verifier)) if !pkce_string_is_valid(verifier) => Err(TokenError::InvalidRequest),
        (Some(pkce), Some(verifier)) => {
            let matches = match pkce.method {
                PkceMethod::Plain => *verifier == pkce.challenge,
                PkceMethod::S256 => matches!(verifier_s256, Some(h) if *h == pkce.challenge),
            };
            if matches {
                Ok(())
            } else {
                Err(TokenError::InvalidGrant)
            }
        }
    }
}
