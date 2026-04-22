use std::env;

mod scaffold;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        std::process::exit(1);
    }

    match args.remove(0).as_str() {

        "new" => {
            if let Err(err) = scaffold::handle_new(&args) {
                eprintln!("scaffold failed: {err}");
                eprintln!("Usage: pilcrow-cli new <dir>");
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
    eprintln!("  pilcrow-cli new <dir>");
}
