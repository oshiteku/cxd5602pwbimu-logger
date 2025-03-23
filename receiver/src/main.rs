use anyhow::{Context, Result};
use clap::Parser;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use receiver::{CompressionType, FileWriterWorker, ParquetWriter, SerialReaderWorker};

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

    /// Enable simulation mode (generate test data instead of reading from serial port)
    #[arg(short = 'm', long)]
    simulation: bool,
}

fn run() -> Result<()> {
    let cli = Cli::parse();

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
    println!("  Simulation mode: {}", cli.simulation);

    // Set up ctrl-c handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("Received Ctrl-C, shutting down...");
        r.store(false, Ordering::SeqCst);
    })
    .with_context(|| "Error setting Ctrl-C handler")?;

    // Create a channel for communication between threads
    let (tx, rx) = mpsc::channel();

    // Create parquet writer
    let writer = ParquetWriter::new(&cli.output_dir, &cli.prefix, compression, cli.buffer_size)?;

    // Create file writer worker
    let file_writer = FileWriterWorker::new(
        writer,
        cli.split_minutes,
        cli.output_dir.clone(),
        cli.prefix.clone(),
    );

    // Create serial reader worker
    let serial_reader = SerialReaderWorker::new(cli.port.clone(), cli.baud_rate);

    // Start file writer thread
    let running_writer = running.clone();
    let writer_handle = thread::spawn(move || {
        if let Err(e) = file_writer.process_data_loop(rx, running_writer) {
            eprintln!("Error in file writer thread: {}", e);
        }
    });

    // Start serial reader thread
    let running_reader = running.clone();
    let reader_handle = thread::spawn(move || {
        let result = if cli.simulation {
            // Run in simulation mode
            serial_reader.simulate_data_loop(running_reader, move |data| {
                tx.send(data)
                    .map_err(|e| anyhow::anyhow!("Channel send error: {}", e))
            })
        } else {
            // Run with real serial port
            serial_reader.read_serial_loop(running_reader, move |data| {
                tx.send(data)
                    .map_err(|e| anyhow::anyhow!("Channel send error: {}", e))
            })
        };

        if let Err(e) = result {
            eprintln!("Error in serial reader thread: {}", e);
        }
    });

    // Wait for threads to complete
    reader_handle.join().expect("Serial reader thread panicked");
    writer_handle.join().expect("File writer thread panicked");

    println!("Receiver shutdown complete");

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
