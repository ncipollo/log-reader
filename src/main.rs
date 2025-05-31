use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];
    
    match fs::read_to_string(file_path) {
        Ok(contents) => println!("{}", contents),
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            process::exit(1);
        }
    }
} 