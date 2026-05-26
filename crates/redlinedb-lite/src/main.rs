fn main() {
    let code = redlinedb_lite::run(std::env::args_os());
    std::process::exit(code);
}
