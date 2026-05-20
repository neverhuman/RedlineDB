pub fn normalize_output(value: &str) -> String {
    let mut normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    while normalized.ends_with([' ', '\t', '\n']) {
        normalized.pop();
    }
    normalized
}
