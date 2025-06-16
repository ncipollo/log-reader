use log_reader::watch_log;
use std::time::Duration;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Log Reader Comprehensive Example ===\n");

    // Example 1: Basic usage with default newline separator
    println!("1. Basic usage - reading with newline separator:");
    basic_usage().await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 2: Custom separator
    println!("2. Custom separator - reading with pipe '|' separator:");
    custom_separator_usage().await?;

    println!("\n{}\n", "=".repeat(50));

    // Example 3: Processing batches
    println!("3. Batch processing - demonstrating Vec<String> handling:");
    batch_processing().await?;

    Ok(())
}

async fn basic_usage() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = watch_log("fixtures/simple_append.log", None).await?;

    let mut batch_count = 0;
    while let Some(lines_result) = stream.next().await {
        match lines_result {
            Ok(lines) => {
                batch_count += 1;
                println!(
                    "  📦 Batch #{} received {} lines:",
                    batch_count,
                    lines.len()
                );

                // Show first few lines as example
                for (i, line) in lines.iter().take(3).enumerate() {
                    println!("    [{}]: {}", i + 1, line);
                }

                if lines.len() > 3 {
                    println!("    ... and {} more lines", lines.len() - 3);
                }

                break; // Just show the first batch for demo
            }
            Err(e) => {
                eprintln!("  ❌ Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn custom_separator_usage() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = watch_log("fixtures/different_separators.log", Some("|".to_string())).await?;

    // Set a timeout to avoid waiting indefinitely
    let timeout_duration = Duration::from_millis(100);

    match tokio::time::timeout(timeout_duration, stream.next()).await {
        Ok(Some(lines_result)) => match lines_result {
            Ok(lines) => {
                println!("  📦 Received {} parts split by '|':", lines.len());
                for (i, part) in lines.iter().enumerate() {
                    println!("    Part {}: {}", i + 1, part.trim());
                }
            }
            Err(e) => eprintln!("  ❌ Error: {}", e),
        },
        Ok(None) => println!("  ℹ️  Stream ended"),
        Err(_) => println!("  ⏰ Timeout - no new content in file"),
    }

    Ok(())
}

async fn batch_processing() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = watch_log("fixtures/simple_append.log", None).await?;

    let timeout_duration = Duration::from_millis(100);

    match tokio::time::timeout(timeout_duration, stream.next()).await {
        Ok(Some(lines_result)) => {
            match lines_result {
                Ok(lines) => {
                    println!("  📊 Processing batch of {} lines:", lines.len());

                    // Example: Count lines by log level
                    let mut counts = std::collections::HashMap::new();
                    for line in &lines {
                        if line.contains("INFO") {
                            *counts.entry("INFO").or_insert(0) += 1;
                        } else if line.contains("ERROR") {
                            *counts.entry("ERROR").or_insert(0) += 1;
                        } else if line.contains("WARN") {
                            *counts.entry("WARN").or_insert(0) += 1;
                        } else if line.contains("DEBUG") {
                            *counts.entry("DEBUG").or_insert(0) += 1;
                        }
                    }

                    println!("  📈 Log level statistics:");
                    for (level, count) in counts {
                        println!("    {}: {} lines", level, count);
                    }

                    // Example: Find lines with specific keywords
                    let error_lines: Vec<_> = lines
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| line.contains("ERROR"))
                        .collect();

                    if !error_lines.is_empty() {
                        println!("  🚨 Error lines found:");
                        for (index, line) in error_lines {
                            println!("    Line {}: {}", index + 1, line);
                        }
                    }
                }
                Err(e) => eprintln!("  ❌ Error: {}", e),
            }
        }
        Ok(None) => println!("  ℹ️  Stream ended"),
        Err(_) => println!("  ⏰ Timeout - no new content in file"),
    }

    Ok(())
}
