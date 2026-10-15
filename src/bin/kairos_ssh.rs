//! KairOS SSH server entry point.

use kairos::hosted::ssh::config::config_from_env;

#[tokio::main]
async fn main() {
    let mut config = config_from_env();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    if let Ok(port) = args[i + 1].parse() {
                        config.port = port;
                    }
                    i += 2;
                    continue;
                }
            }
            "--listen" => {
                if i + 1 < args.len() {
                    config.listen_address = args[i + 1].clone();
                    i += 2;
                    continue;
                }
            }
            "--allow-password" => {
                config.allow_password = true;
            }
            _ => {}
        }
        i += 1;
    }

    match kairos::hosted::ssh::start_server(config).await {
        Ok(msg) => {
            println!("{msg}");
            println!("SSH LISTENER OK");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }
        Err(err) => {
            eprintln!("{}", err.reason());
            std::process::exit(1);
        }
    }
}
