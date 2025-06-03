use log_reader::watch_log;
use std::env;
use std::process;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];

    match watch_log(file_path, None).await {
        Ok(mut stream) => {
            println!("Watching file: {}", file_path);
            while let Some(line_result) = stream.next().await {
                match line_result {
                    Ok(content) => println!("{}", content),
                    Err(e) => {
                        eprintln!("Error reading file: {}", e);
                        process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error setting up file watcher: {}", e);
            process::exit(1);
        }
    }
}
