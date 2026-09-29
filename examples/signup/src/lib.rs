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
// from `parse` (design/01 §4).

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
    matches!(b, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}

/// The local part's bytes besides letters and digits: .!#$%&'*+/=?^_`{|}~-
fn is_local(b: u8) -> bool {
    is_alnum(b)
        || matches!(b, b'.' | b'!' | b'#'..=b'\'' | b'*' | b'+' | b'-' | b'/' | b'=' | b'?' | b'^'..=b'`' | b'{'..=b'~')
}

impl Email {
    pub fn parse(raw: String) -> Result<Email, EmailError> {
        let b = raw.as_bytes();
        let mut at = b.len();
        for i in 0..b.len() {
            if b[i] == b'@' && at == b.len() {
                at = i;
            }
        }
        if at == b.len() {
            return Err(EmailError::MissingAt);
        }
        if at == 0 {
            return Err(EmailError::BadLocal);
        }
        for i in 0..at {
            if !is_local(b[i]) {
                return Err(EmailError::BadLocal);
            }
        }
        // Labels of 1 to 63 letters, digits and hyphens, with a letter or
        // digit at both ends, separated by `.`. `i == b.len()` ends the last.
        let mut start = at + 1;
        for i in (at + 1)..(b.len() + 1) {
            if i == b.len() || b[i] == b'.' {
                let n = i - start;
                if n == 0 || n > 63 || b[start] == b'-' || b[i - 1] == b'-' {
                    return Err(EmailError::BadDomain);
                }
                start = i + 1;
            } else if !is_alnum(b[i]) && b[i] != b'-' {
                return Err(EmailError::BadDomain);
            }
        }
        Ok(Email(raw))
    }
}

fn blocked(raw: &str) -> bool {
    raw == "passwordpassword" || raw == "123456789012345" || raw == "qwertyuiopasdfgh"
}

impl Password {
    pub fn parse(raw: String) -> Result<Password, PasswordError> {
        // Code points: every byte but a UTF-8 continuation byte (0x80..=0xBF).
        let b = raw.as_bytes();
        let mut n = 0usize;
        for i in 0..b.len() {
            if !matches!(b[i], 0x80..=0xBF) {
                n += 1;
            }
        }
        if n < 15 {
            return Err(PasswordError::TooShort);
        }
        if n > 64 {
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
