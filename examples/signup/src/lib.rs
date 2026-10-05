// Sign-up input, checked the same way on the server and in the browser.
//
// - `Email`: the WHATWG HTML "valid e-mail address"
//   (<https://html.spec.whatwg.org/multipage/input.html#valid-e-mail-address>).
//   The spec states it as a regular expression; this is the same language,
//   split at the first `@` and at each `.` of the domain.
// - `Password`: the length rules of NIST SP 800-63B-4 §3.1.1.2 for a password
//   that is the only factor. Length counts code points; at least 15, and at
//   most 64 accepted (as one factor of several, the minimum would be 8). A
//   short blocklist stands in for the list of known-compromised values.
//   The same section says the verifier SHOULD normalize Unicode passwords
//   to NFC (rev. 3, §5.1.1.2, said NFKC or NFKD). Rust's std has no Unicode
//   normalization, so this does not normalize: the length is counted on
//   the password as given, and a caller that wants the SHOULD normalizes
//   before calling, and does the same before hashing or comparing it later.
//   An `é` written as `e` and a combining accent is two code points here.
// - `Signup`: both, and a password that is not the e-mail address. The
//   e-mail is the username here, and §3.1.1.2 lists the username among the
//   context-specific words a blocklist may hold. The comparison ignores
//   ASCII case: the domain is case-insensitive, RFC 5321 lets a server
//   treat the local part's case as it likes and most ignore it, and
//   someone guessing from the address would try its case variants. An
//   address is ASCII (the WHATWG grammar), so ASCII case is all the case it
//   has. Only equality is checked, not a password that contains the
//   address or its local part. A password that is too short or blocked
//   gets that error first.
//
// All three are closed types: the fields are not `pub`, so the value comes only
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
    /// The password is the e-mail address, ignoring ASCII case.
    PasswordIsEmail,
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
        let (local, domain) = raw.split_once('@').ok_or(EmailError::MissingAt)?;
        if local.is_empty() || !local.bytes().all(is_local) {
            return Err(EmailError::BadLocal);
        }
        // Labels of 1 to 63 letters, digits and hyphens, with a letter or
        // digit at both ends.
        for label in domain.split('.') {
            if label.is_empty()
                || label.len() > 63
                || label.starts_with("-")
                || label.ends_with("-")
                || !label.bytes().all(|b| is_alnum(b) || b == b'-')
            {
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
        let n = raw.chars().count();
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
        let email = Email::parse(email).map_err(SignupError::Email)?;
        let password = Password::parse(password).map_err(SignupError::Password)?;
        if password.0.eq_ignore_ascii_case(&email.0) {
            return Err(SignupError::PasswordIsEmail);
        }
        Ok(Signup { email, password })
    }
}
