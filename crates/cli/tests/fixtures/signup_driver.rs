pub fn parse_email(raw: String) -> Result<Email, EmailError> {
    Email::parse(raw)
}

pub fn parse_password(raw: String) -> Result<Password, PasswordError> {
    Password::parse(raw)
}

pub fn parse_signup(email: String, password: String) -> Result<Signup, SignupError> {
    Signup::parse(email, password)
}

/// The e-mail and the password read back through the accessors.
pub fn signup_fields(email: String, password: String) -> Result<(String, String), SignupError> {
    let signup = Signup::parse(email, password)?;
    Ok((String::from(signup.email().as_str()), String::from(signup.password().as_str())))
}
