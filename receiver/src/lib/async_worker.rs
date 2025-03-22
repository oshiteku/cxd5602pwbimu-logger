use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;

use super::serial::{open_serial_port, parse_sensor_data, read_serial_data};
use super::ParquetWriter;
use super::SensorData;

/// Worker for handling file writing in a separate thread
///
/// This struct is responsible for writing sensor data to Parquet files,
/// handling file rotation, and managing the background file writing operations.
pub struct FileWriterWorker {
    writer: ParquetWriter,
    split_minutes: u32,
    last_rotation: DateTime<Utc>,
    output_dir: String,
    prefix: String,
}

impl FileWriterWorker {
    /// Creates a new file writer worker
    ///
    /// # Arguments
    /// * `writer` - The configured Parquet writer
    /// * `split_minutes` - Interval in minutes for file rotation (0 = no splitting)
    /// * `output_dir` - Directory to store Parquet files
    /// * `prefix` - Filename prefix for Parquet files
    ///
    /// # Returns
    /// A new FileWriterWorker instance
    pub fn new(
        writer: ParquetWriter,
        split_minutes: u32,
        output_dir: String,
        prefix: String,
    ) -> Self {
        FileWriterWorker {
            writer,
            split_minutes,
            last_rotation: Utc::now(),
            output_dir,
            prefix,
        }
    }

    /// Check if it's time to rotate the file based on split_minutes
    fn should_rotate_file(&self) -> bool {
        if self.split_minutes == 0 {
            return false; // Never rotate if split_minutes is 0
        }

        let now = Utc::now();
        let rotation_interval = Duration::minutes(self.split_minutes as i64);
        now - self.last_rotation >= rotation_interval
    }

    /// Process incoming sensor data and write it to a Parquet file
    ///
    /// Runs in a loop until signaled to stop. Handles file rotation based on time
    /// intervals and writes incoming data to Parquet files.
    ///
    /// # Arguments
    /// * `rx` - Receiver channel for incoming sensor data
    /// * `running` - Atomic flag indicating whether the process should continue running
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn process_data_loop(
        mut self,
        rx: Receiver<SensorData>,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        println!("File writer thread started");

        // Process incoming data until the running flag is set to false
        while running.load(Ordering::SeqCst) {
            // Check if we need to rotate the file based on time
            if self.should_rotate_file() {
                println!("Rotating file based on time interval");
                self.writer.rotate_file(&self.output_dir, &self.prefix)?;
                self.last_rotation = Utc::now();
            }

            // Try to receive data with a timeout
            match rx.recv_timeout(StdDuration::from_millis(100)) {
                Ok(data) => {
                    // Add the data to the writer
                    self.writer.add_data(data)?;
                }
                Err(RecvTimeoutError::Timeout) => {
                    // No data received within timeout, check if we should continue
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // Sender has been dropped, exit the loop
                    println!("Data producer disconnected, stopping file writer");
                    break;
                }
            }
        }

        // Ensure all data is flushed before exiting
        println!("Closing Parquet writer in file writer thread");
        self.writer.close()?;
        println!("File writer thread shutting down");
        Ok(())
    }
}

/// Worker for reading serial data in a separate thread
///
/// This struct is responsible for reading data from the serial port,
/// parsing it into sensor data structures, and sending that data to the
/// file writer thread. It also provides a simulation mode for testing.
pub struct SerialReaderWorker {
    port_name: String,
    baud_rate: u32,
}

impl SerialReaderWorker {
    /// Creates a new serial reader worker
    ///
    /// # Arguments
    /// * `port_name` - Name of the serial port to read from
    /// * `baud_rate` - Baud rate for the serial connection
    ///
    /// # Returns
    /// A new SerialReaderWorker instance
    pub fn new(port_name: String, baud_rate: u32) -> Self {
        SerialReaderWorker {
            port_name,
            baud_rate,
        }
    }

    /// Read data from the serial port and send it to the writer thread
    pub fn read_serial_loop<F>(self, running: Arc<AtomicBool>, mut data_callback: F) -> Result<()>
    where
        F: FnMut(SensorData) -> Result<()>,
    {
        println!("Serial reader thread started");

        // Open the serial port
        let mut port = open_serial_port(&self.port_name, self.baud_rate)?;

        while running.load(Ordering::SeqCst) {
            // Try to read a line from the serial port
            match read_serial_data(&mut port) {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    // Parse the line into sensor data
                    match parse_sensor_data(&line) {
                        Ok(data) => {
                            // Send the data to the writer thread
                            if let Err(e) = data_callback(data) {
                                eprintln!("Error sending data to writer: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error parsing sensor data: {}", e);
                            // Continue reading even if there's a parse error
                        }
                    }
                }
                Err(e) => {
                    // Log the error but continue trying to read
                    eprintln!("Error reading from serial port: {}", e);
                    thread::sleep(StdDuration::from_millis(100));
                }
            }
        }

        println!("Serial reader thread shutting down");
        Ok(())
    }

    /// Simulate serial data for testing
    pub fn simulate_data_loop<F>(self, running: Arc<AtomicBool>, mut data_callback: F) -> Result<()>
    where
        F: FnMut(SensorData) -> Result<()>,
    {
        println!("Simulated serial reader thread started");

        let mut i = 0;
        // Generate a fixed number of samples in test mode
        let max_samples = if cfg!(test) { 20 } else { u32::MAX };

        while running.load(Ordering::SeqCst) && i < max_samples {
            // Create simulated data
            let data = SensorData {
                timestamp: i,
                temp: 25.0 + (i as f32 * 0.1),
                gx: 0.1 * i as f32,
                gy: 0.2 * i as f32,
                gz: 0.3 * i as f32,
                ax: 1.0 * i as f32,
                ay: 1.1 * i as f32,
                az: 1.2 * i as f32,
                system_timestamp: Utc::now().timestamp_millis(),
            };

            // Send the data to the writer thread
            if let Err(e) = data_callback(data) {
                eprintln!("Error sending data to writer: {}", e);
            }

            // Increment counter and wait
            i += 1;

            // Exit early if we've hit the max samples in test mode
            if i >= max_samples && cfg!(test) {
                println!("Generated {} test samples, stopping simulation", i);
                break;
            }

            thread::sleep(StdDuration::from_millis(100));
        }

        println!("Simulated serial reader thread shutting down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CompressionType;
    use crate::ParquetWriter;
    use std::sync::mpsc;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_file_writer_worker() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap().to_string();

        // Create a channel for passing sensor data
        let (tx, rx) = mpsc::channel();

        // Create a running flag
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Create a ParquetWriter
        let writer = ParquetWriter::new(
            &dir_path,
            "test_log",
            CompressionType::Snappy,
            10, // Small buffer size to ensure writes happen
        )
        .unwrap();

        // Create and start FileWriterWorker in a separate thread
        let worker = FileWriterWorker::new(
            writer,
            0, // No file splitting
            dir_path.clone(),
            "test_log".to_string(),
        );

        let writer_handle = thread::spawn(move || {
            worker.process_data_loop(rx, running_clone).unwrap();
        });

        // Send some test data
        for i in 0..5 {
            let data = SensorData {
                timestamp: i,
                temp: 25.0 + (i as f32 * 0.1),
                gx: 0.1 * i as f32,
                gy: 0.2 * i as f32,
                gz: 0.3 * i as f32,
                ax: 1.0 * i as f32,
                ay: 1.1 * i as f32,
                az: 1.2 * i as f32,
                system_timestamp: Utc::now().timestamp_millis(),
            };
            tx.send(data).unwrap();
        }

        // Wait a bit for processing
        thread::sleep(StdDuration::from_millis(500));

        // Signal the worker to stop
        running.store(false, Ordering::SeqCst);

        // Drop sender to close the channel
        drop(tx);

        // Wait for the worker thread to finish
        writer_handle.join().unwrap();

        // Check that files were created in the temp directory
        let entries = std::fs::read_dir(&dir_path).unwrap();
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
    }

    #[test]
    fn test_simulated_reader_and_writer() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap().to_string();

        // Create a channel for passing sensor data
        let (tx, rx) = mpsc::channel();

        // Create a running flag
        let running = Arc::new(AtomicBool::new(true));
        let running_clone1 = running.clone();
        let running_clone2 = running.clone();

        // Create a ParquetWriter
        let writer = ParquetWriter::new(
            &dir_path,
            "test_integrated",
            CompressionType::Snappy,
            10, // Small buffer size to ensure writes happen
        )
        .unwrap();

        // Create and start FileWriterWorker in a separate thread
        let writer_worker = FileWriterWorker::new(
            writer,
            0, // No file splitting
            dir_path.clone(),
            "test_integrated".to_string(),
        );

        let writer_handle = thread::spawn(move || {
            writer_worker.process_data_loop(rx, running_clone1).unwrap();
        });

        // Create and start SerialReaderWorker (simulation mode) in a separate thread
        let reader_worker = SerialReaderWorker::new("test_port".to_string(), 115200);

        let reader_handle = thread::spawn(move || {
            let tx_clone = tx;
            reader_worker
                .simulate_data_loop(running_clone2, move |data| {
                    tx_clone
                        .send(data)
                        .map_err(|e| anyhow::anyhow!("Channel send error: {}", e))
                })
                .unwrap();
        });

        // Let the threads run for a short time
        thread::sleep(StdDuration::from_millis(500));

        // Signal the workers to stop
        running.store(false, Ordering::SeqCst);

        // Wait for the threads to finish
        reader_handle.join().unwrap();
        writer_handle.join().unwrap();

        // Check that files were created in the temp directory
        let entries = std::fs::read_dir(&dir_path).unwrap();
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
    }
}
