//! KairOS hosted runtime entry point.

use kairos::Runtime;
use std::io::{self, BufRead, IsTerminal};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut live_terminal = false;

    let mut runtime = if args.is_empty() {
        if io::stdin().is_terminal() {
            live_terminal = true;
            Runtime::interactive()
        } else {
            let stdin = io::stdin();
            let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
            if lines.is_empty() {
                Runtime::with_input(["ps", "mem", "exit"])
            } else {
                Runtime::with_input(lines)
            }
        }
    } else if args.len() == 1 && args[0] == "--demo" {
        Runtime::with_input(["ps", "mem", "log", "ls /tmp", "exit"])
    } else if args.len() == 1 && args[0] == "--interactive" {
        live_terminal = true;
        Runtime::interactive()
    } else {
        Runtime::with_input(args)
    };

    if let Err(err) = runtime.boot() {
        eprintln!("boot failed: {err}");
        std::process::exit(1);
    }

    if let Err(err) = runtime.run() {
        eprintln!("runtime failed: {err}");
        std::process::exit(1);
    }

    if !live_terminal {
        for line in runtime.output_lines() {
            println!("{line}");
        }
    }
}
