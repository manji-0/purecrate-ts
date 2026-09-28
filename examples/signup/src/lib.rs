// Sign-up input, checked the same way on the server and in the browser.
//
// - `Email`: the WHATWG HTML "valid e-mail address"
//   (<https://html.spec.whatwg.org/multipage/input.html#valid-e-mail-address>).
//   The spec states it as a regular expression; this is the same language,
//   read byte by byte.
// - `Password`: the length rules of NIST SP 800-63B-4 §3.1.1.2. Length counts
//   code points; at least 15, and at most 64 accepted. A short blocklist
//   stands in for the list of known-compromised values.
//
// Both are closed types: the fields are not `pub`, so the value comes only
// from `parse` (design/04 §1.6).

pub struct Email(String);

pub enum EmailError {
    MissingAt,
    BadLocal,
    BadDomain,
}

pub struct Password(String);

pub enum PasswordError {
    TooShort,
    TooLong,
    Blocked,
}

pub struct Signup {
    email: Email,
    password: Password,
}

pub enum SignupError {
    Email(EmailError),
    Password(PasswordError),
}

fn is_alnum(b: u8) -> bool {
    (b >= 48u8 && b <= 57u8) || (b >= 65u8 && b <= 90u8) || (b >= 97u8 && b <= 122u8)
}

/// The local part's bytes besides letters and digits: .!#$%&'*+/=?^_`{|}~-
fn is_local(b: u8) -> bool {
    is_alnum(b)
        || b == 46u8
        || b == 33u8
        || (b >= 35u8 && b <= 39u8)
        || b == 42u8
        || b == 43u8
        || b == 45u8
        || b == 47u8
        || b == 61u8
        || b == 63u8
        || (b >= 94u8 && b <= 96u8)
        || (b >= 123u8 && b <= 126u8)
}

/// The first `@` at or after `i`.
fn find_at(b: &[u8], i: usize) -> Option<usize> {
    if i == b.len() {
        None
    } else if b[i] == 64u8 {
        Some(i)
    } else {
        find_at(b, i + 1usize)
    }
}

fn local_ok(b: &[u8], i: usize, end: usize) -> bool {
    if i == end {
        true
    } else if is_local(b[i]) {
        local_ok(b, i + 1usize, end)
    } else {
        false
    }
}

/// The end of the label that starts at `start` (a `.` or the end of the
/// input), or `None` if the label is not 1 to 63 letters, digits and
/// hyphens with a letter or digit at both ends.
fn label_end(b: &[u8], start: usize, i: usize) -> Option<usize> {
    if i == b.len() || b[i] == 46u8 {
        let n = i - start;
        if n == 0usize || n > 63usize {
            None
        } else if b[start] == 45u8 || b[i - 1usize] == 45u8 {
            None
        } else {
            Some(i)
        }
    } else if is_alnum(b[i]) || b[i] == 45u8 {
        label_end(b, start, i + 1usize)
    } else {
        None
    }
}

fn domain_ok(b: &[u8], start: usize) -> bool {
    match label_end(b, start, start) {
        None => false,
        Some(end) => {
            if end == b.len() {
                true
            } else {
                domain_ok(b, end + 1usize)
            }
        }
    }
}

impl Email {
    pub fn parse(raw: String) -> Result<Email, EmailError> {
        let b = raw.as_bytes();
        let at = match find_at(b, 0usize) {
            Some(i) => i,
            None => return Err(EmailError::MissingAt),
        };
        if at == 0usize || !local_ok(b, 0usize, at) {
            return Err(EmailError::BadLocal);
        }
        if !domain_ok(b, at + 1usize) {
            return Err(EmailError::BadDomain);
        }
        Ok(Email(raw))
    }
}

/// Code points from the UTF-8 bytes: a continuation byte is 0x80..=0xBF.
fn code_points(b: &[u8], i: usize, n: usize) -> usize {
    if i == b.len() {
        n
    } else if b[i] >= 128u8 && b[i] < 192u8 {
        code_points(b, i + 1usize, n)
    } else {
        code_points(b, i + 1usize, n + 1usize)
    }
}

fn blocked(raw: &str) -> bool {
    raw == "passwordpassword" || raw == "123456789012345" || raw == "qwertyuiopasdfgh"
}

impl Password {
    pub fn parse(raw: String) -> Result<Password, PasswordError> {
        let n = code_points(raw.as_bytes(), 0usize, 0usize);
        if n < 15usize {
            return Err(PasswordError::TooShort);
        }
        if n > 64usize {
            return Err(PasswordError::TooLong);
        }
        if blocked(&raw) {
            return Err(PasswordError::Blocked);
        }
        Ok(Password(raw))
    }
}

impl Signup {
    pub fn parse(email: String, password: String) -> Result<Signup, SignupError> {
        let email = match Email::parse(email) {
            Ok(e) => e,
            Err(e) => return Err(SignupError::Email(e)),
        };
        let password = match Password::parse(password) {
            Ok(p) => p,
            Err(e) => return Err(SignupError::Password(e)),
        };
        Ok(Signup { email, password })
    }
}
