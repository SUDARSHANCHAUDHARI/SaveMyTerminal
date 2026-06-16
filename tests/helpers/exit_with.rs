fn main() {
    let mut args = std::env::args().skip(1);
    let code: i32 = args.next().unwrap().parse().unwrap();
    for arg in args {
        if let Some(milliseconds) = arg.strip_prefix("sleep=") {
            std::thread::sleep(std::time::Duration::from_millis(
                milliseconds.parse().unwrap(),
            ));
        }
        println!("arg={arg}");
    }
    std::process::exit(code);
}
