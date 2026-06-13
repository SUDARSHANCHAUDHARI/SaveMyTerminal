fn main() {
    let mut args = std::env::args().skip(1);
    let code: i32 = args.next().unwrap().parse().unwrap();
    for arg in args {
        println!("arg={arg}");
    }
    std::process::exit(code);
}
