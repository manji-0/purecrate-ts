use uuid::Uuid;

pub fn parse(s: &str) -> Result<Uuid, uuid::Error> {
    Uuid::parse_str(s)
}

pub fn try_parse(s: String) -> Option<Uuid> {
    match Uuid::try_parse(&s) {
        Ok(id) => Some(id),
        Err(_) => None,
    }
}

pub enum Order {
    Less,
    Equal,
    Greater,
}

/// `==` and `<` on `Uuid` compare the 16 bytes.
pub fn compare(a: Uuid, b: Uuid) -> Order {
    if a == b {
        Order::Equal
    } else if a < b {
        Order::Less
    } else {
        Order::Greater
    }
}

pub fn is_nil(u: Uuid) -> bool {
    u == Uuid::nil()
}

pub struct Disk {
    pub id: Uuid,
    pub parent: Option<Uuid>,
}

pub enum NameOrId {
    Id(Uuid),
    Name(String),
}

/// As omicron reads a path segment: a UUID if it parses, else a name.
pub fn read_segment(s: String) -> NameOrId {
    match Uuid::parse_str(&s) {
        Ok(id) => NameOrId::Id(id),
        Err(_) => NameOrId::Name(s),
    }
}

pub fn reparent(d: Disk, s: &str) -> Result<Disk, uuid::Error> {
    let parent = Uuid::parse_str(s)?;
    Ok(Disk { parent: Some(parent), ..d })
}
