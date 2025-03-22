use anyhow::{Context, Result};
use clap::Parser;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod lib;
use lib::{CompressionType, ParquetWriter, SensorData};

#[derive(Parser, Debug)]
#[command(name = "receiver")]
#[command(about = "Receives sensor data over UART and stores it in Parquet format")]
#[command(version)]
struct Cli {
    /// Serial port to connect to (e.g. /dev/ttyUSB0, COM3)
    #[arg(short, long)]
    port: String,

    /// Baud rate for serial connection
    #[arg(short, long, default_value = "115200")]
    baud_rate: u32,

    /// Output directory for Parquet files
    #[arg(short, long, default_value = "./logs")]
    output_dir: String,

    /// File split interval in minutes (0 = no splitting)
    #[arg(short, long, default_value = "0")]
    split_minutes: u32,

    /// Output file name prefix
    #[arg(short = 'f', long, default_value = "sensor_log")]
    prefix: String,

    /// Compression algorithm (none, snappy, gzip, lz4, zstd)
    #[arg(short, long, default_value = "snappy")]
    compression: String,

    /// Buffer size (how many records to accumulate before writing)
    #[arg(short = 'u', long, default_value = "100")]
    buffer_size: usize,
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    
    // Validate port exists (disabled for testing)
    // if !std::path::Path::new(&cli.port).exists() {
    //    anyhow::bail!("Serial port {} does not exist", cli.port);
    // }
    
    // Parse compression type
    let compression = CompressionType::from_str(&cli.compression)
        .map_err(|e| anyhow::anyhow!("Invalid compression algorithm: {}", e))?;
    
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&cli.output_dir)
        .with_context(|| format!("Failed to create output directory: {}", cli.output_dir))?;
    
    println!("Starting receiver with the following configuration:");
    println!("  Port: {}", cli.port);
    println!("  Baud rate: {}", cli.baud_rate);
    println!("  Output directory: {}", cli.output_dir);
    println!("  Split interval: {} minutes", cli.split_minutes);
    println!("  File prefix: {}", cli.prefix);
    println!("  Compression: {}", cli.compression);
    println!("  Buffer size: {}", cli.buffer_size);
    
    // Set up ctrl-c handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        println!("Received Ctrl-C, shutting down...");
        r.store(false, Ordering::SeqCst);
    })
    .with_context(|| "Error setting Ctrl-C handler")?;
    
    // Create parquet writer
    let mut writer = ParquetWriter::new(
        &cli.output_dir,
        &cli.prefix,
        compression,
        cli.buffer_size,
    )?;
    
    // TODO: In a real implementation, open the serial port and read data
    
    // For demonstration, we'll just simulate receiving a few records and exit
    println!("Simulating data reception for demo purposes...");
    
    for i in 0..10 {
        // Simulate receiving data
        let data = SensorData {
            timestamp: i,
            temp: 25.0 + (i as f32 * 0.1),
            gx: 0.1 * i as f32,
            gy: 0.2 * i as f32,
            gz: 0.3 * i as f32,
            ax: 1.0 * i as f32,
            ay: 1.1 * i as f32,
            az: 1.2 * i as f32,
            system_timestamp: chrono::Utc::now().timestamp_millis(),
        };
        
        writer.add_data(data)?;
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Check if we should exit
        if !running.load(Ordering::SeqCst) {
            break;
        }
    }
    
    // Ensure all data is flushed before exit
    writer.flush()?;
    
    println!("Receiver shutdown complete");
    
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}