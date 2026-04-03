use std::env;

mod check;
mod scaffold;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        std::process::exit(1);
    }

    match args.remove(0).as_str() {
        "check-arch" => {
            if let Err(err) = check::check_arch(&env::current_dir().expect("read current dir")) {
                eprintln!("architecture check failed: {err}");
                std::process::exit(1);
            }
            println!("architecture check passed");
        }
        "new" => {
            if let Err(err) = scaffold::handle_new(&args) {
                eprintln!("scaffold failed: {err}");
                std::process::exit(1);
            }
        }
        _ => {
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  pilcrow-cli check-arch");
    eprintln!("  pilcrow-cli new --convention web-backend <dir>");
}
