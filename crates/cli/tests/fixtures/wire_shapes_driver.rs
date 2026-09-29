pub fn id_of(n: u64) -> Id {
    Id(n)
}

pub fn label_of(s: String) -> Label {
    Label(s)
}

pub fn sealed_of(code: i32, hint: Option<String>) -> Sealed {
    Sealed { code, hint }
}
