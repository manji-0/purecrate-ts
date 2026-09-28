pub fn parse_email(raw: String) -> Result<Email, EmailError> {
    Email::parse(raw)
}

pub fn parse_password(raw: String) -> Result<Password, PasswordError> {
    Password::parse(raw)
}

pub fn parse_signup(email: String, password: String) -> Result<Signup, SignupError> {
    Signup::parse(email, password)
}
