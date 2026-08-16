#[tokio::main]
async fn main() {
    let code = match savemyterminal::app::run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("savemyterminal: {error:#}");
            1
        }
    };
    std::process::exit(code);
}
