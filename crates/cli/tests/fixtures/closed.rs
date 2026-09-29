pub struct Email(String);

pub enum EmailError {
    Empty,
    Reserved,
}

impl Email {
    pub fn parse(raw: String) -> Result<Email, EmailError> {
        if raw == "" {
            Err(EmailError::Empty)
        } else if raw == "admin" {
            Err(EmailError::Reserved)
        } else {
            Ok(Email::unchecked(raw))
        }
    }

    fn unchecked(raw: String) -> Email {
        Email(raw)
    }
}

pub struct Account {
    email: Email,
    pub age: u8,
}

pub enum SignupError {
    BadEmail(EmailError),
    TooYoung,
}

impl Account {
    pub fn open(email: Email, age: u8) -> Result<Account, SignupError> {
        if age < 18 {
            Err(SignupError::TooYoung)
        } else {
            Ok(Account::with_age(email, age))
        }
    }

    fn with_age(email: Email, age: u8) -> Account {
        Account { email, age }
    }

    pub fn birthday(self) -> Account {
        Account { age: self.age + 1, ..self }
    }
}

pub fn raw(code: u8) -> String {
    if code == 0 {
        String::from("")
    } else if code == 1 {
        String::from("admin")
    } else {
        String::from("a@example.com")
    }
}

pub fn signup(code: u8, age: u8) -> Result<u8, SignupError> {
    let email = match Email::parse(raw(code)) {
        Ok(e) => e,
        Err(e) => return Err(SignupError::BadEmail(e)),
    };
    let account = Account::open(email, age)?;
    Ok(account.birthday().age)
}

pub fn adult(email: Email) -> Account {
    Account::with_age(email, 18u8)
}

pub fn adult_after(code: u8) -> Result<u8, EmailError> {
    let email = Email::parse(raw(code))?;
    Ok(adult(email).birthday().age)
}
