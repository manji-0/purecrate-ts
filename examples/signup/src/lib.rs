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

impl Email {
    pub fn parse(raw: String) -> Result<Email, EmailError> {
        let b = raw.as_bytes();
        let mut at = b.len();
        for i in 0..b.len() {
            if b[i] == 64u8 && at == b.len() {
                at = i;
            }
        }
        if at == b.len() {
            return Err(EmailError::MissingAt);
        }
        if at == 0usize {
            return Err(EmailError::BadLocal);
        }
        for i in 0..at {
            if !is_local(b[i]) {
                return Err(EmailError::BadLocal);
            }
        }
        // Labels of 1 to 63 letters, digits and hyphens, with a letter or
        // digit at both ends, separated by `.`. `i == b.len()` ends the last.
        let mut start = at + 1usize;
        for i in (at + 1usize)..(b.len() + 1usize) {
            if i == b.len() || b[i] == 46u8 {
                let n = i - start;
                if n == 0usize || n > 63usize || b[start] == 45u8 || b[i - 1usize] == 45u8 {
                    return Err(EmailError::BadDomain);
                }
                start = i + 1usize;
            } else if !is_alnum(b[i]) && b[i] != 45u8 {
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
            if b[i] < 128u8 || b[i] >= 192u8 {
                n += 1usize;
            }
        }
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
