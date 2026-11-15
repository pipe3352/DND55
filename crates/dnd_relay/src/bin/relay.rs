//! `dnd55-relay [--bind 0.0.0.0:7979]` — run on any machine with a public IP address.
fn main() {
    let mut bind = format!("0.0.0.0:{}", dnd_relay::DEFAULT_PORT);
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--bind")
        && let Some(v) = args.get(i + 1)
    {
        bind = v.clone();
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "dnd55-relay [--bind ADDRESS:PORT]   (default 0.0.0.0:{})",
            dnd_relay::DEFAULT_PORT
        );
        return;
    }
    let addr = match bind.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("invalid --bind address {bind}: {e}");
            std::process::exit(2);
        }
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    println!("DND55 relay listening on {bind} (TCP). Players and hosts connect to this address.");
    if let Err(e) = rt.block_on(dnd_relay::run(dnd_relay::RelayConfig::new(addr), None)) {
        eprintln!("relay stopped: {e}");
        std::process::exit(1);
    }
}
