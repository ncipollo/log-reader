use log_reader::watch_log;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Watch a log file and emit lines as Vec<String>
    let mut stream = watch_log("fixtures/simple_append.log", None).await?;

    println!("Watching log file - emitting Vec<String>...");

    let mut count = 0;
    while let Some(lines_result) = stream.next().await {
        match lines_result {
            Ok(lines) => {
                println!("Received batch #{} with {} lines:", count + 1, lines.len());
                for (i, line) in lines.iter().enumerate() {
                    println!("  [{}]: {}", i + 1, line);
                }
                println!("---");
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }

        count += 1;
        if count >= 2 {
            // Only show first couple of batches for demo
            break;
        }
    }

    Ok(())
}
