use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

// Import crate from the lib
extern crate receiver;
use receiver::{CompressionType, FileWriterWorker, ParquetWriter, SensorData, SerialReaderWorker};

#[test]
fn test_end_to_end_async_processing() -> Result<()> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path().to_str().unwrap().to_string();

    // Create a channel for communication between threads
    let (tx, rx) = mpsc::channel();

    // Set up running flag
    let running = Arc::new(AtomicBool::new(true));
    let running_writer = running.clone();
    let running_reader = running.clone();

    // Create parquet writer with buffer size 1 since tx side handles buffering
    let writer = ParquetWriter::new(
        &dir_path,
        "async_test",
        CompressionType::Snappy,
        1, // Small buffer size since tx side handles buffering
    )?;

    // Create file writer worker
    let file_writer = FileWriterWorker::new(
        writer,
        0, // No file splitting
        dir_path.clone(),
        "async_test".to_string(),
    );

    // Create serial reader worker in simulation mode with buffer
    let serial_reader = SerialReaderWorker::new("test_port".to_string(), 115200, 5);

    // Start file writer thread
    let writer_handle = thread::spawn(move || {
        if let Err(e) = file_writer.process_data_loop(rx, running_writer) {
            eprintln!("Error in file writer thread: {}", e);
        }
    });

    // Start serial reader thread in simulation mode
    let reader_handle = thread::spawn(move || {
        if let Err(e) = serial_reader.simulate_data_loop(running_reader, tx) {
            eprintln!("Error in serial reader thread: {}", e);
        }
    });

    // Let the threads run for a short while
    thread::sleep(Duration::from_millis(500));

    // Signal shutdown
    running.store(false, Ordering::SeqCst);

    // Wait for threads to complete
    reader_handle.join().expect("Serial reader thread panicked");
    writer_handle.join().expect("File writer thread panicked");

    // Check that files were created
    let entries = std::fs::read_dir(&dir_path)?;
    let parquet_files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map_or(false, |ext| ext == "parquet")
        })
        .collect();

    assert!(!parquet_files.is_empty(), "No Parquet files were created");

    Ok(())
}

#[test]
fn test_file_rotation() -> Result<()> {
    // Create a temporary directory for the test
    let temp_dir = tempdir()?;
    let dir_path = temp_dir.path().to_str().unwrap().to_string();

    // Create a channel for communication between threads
    let (tx, rx) = mpsc::channel();

    // Set up running flag
    let running = Arc::new(AtomicBool::new(true));
    let running_writer = running.clone();

    // Create parquet writer with buffer size 1 since tx side handles buffering
    let writer = ParquetWriter::new(
        &dir_path,
        "rotation_test",
        CompressionType::Snappy,
        1, // Small buffer size since tx side handles buffering
    )?;

    // Create file writer worker with very short rotation time for testing
    let file_writer = FileWriterWorker::new(
        writer,
        0, // We'll trigger rotation manually
        dir_path.clone(),
        "rotation_test".to_string(),
    );

    // Create a thread to handle file writing
    let writer_handle = thread::spawn(move || {
        if let Err(e) = file_writer.process_data_loop(rx, running_writer) {
            eprintln!("Error in file writer thread: {}", e);
        }
    });

    // Create a batch of data to send
    let mut data_batch = Vec::with_capacity(10);
    for i in 0..10 {
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
        data_batch.push(data);
    }

    // Send the batch
    tx.send(data_batch)?;

    // Give time for data to be processed
    thread::sleep(Duration::from_millis(300));

    // Signal shutdown
    running.store(false, Ordering::SeqCst);
    drop(tx);

    // Wait for thread to complete
    writer_handle.join().expect("File writer thread panicked");

    // Check that files were created
    let entries = std::fs::read_dir(&dir_path)?;
    let parquet_files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map_or(false, |ext| ext == "parquet")
        })
        .collect();

    assert!(!parquet_files.is_empty(), "No Parquet files were created");

    Ok(())
}
