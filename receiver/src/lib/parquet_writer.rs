use anyhow::{Context, Result};
use arrow::array::{Float32Array, Int64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::fs::{create_dir_all, File};
use std::path::Path;
use std::sync::Arc;

use super::error::ReceiverError;
use super::types::{CompressionType, SensorData};

/// Writer for saving sensor data to Parquet files
///
/// This struct handles the conversion of sensor data to the Arrow format
/// and writes it to Parquet files. It supports various compression formats,
/// file rotation, and buffered writing for improved performance.
pub struct ParquetWriter {
    schema: Arc<Schema>,
    compression: CompressionType,
    buffer: Vec<SensorData>,
    buffer_size: usize,
    output_path: String,
    writer: Option<ArrowWriter<File>>,
}

impl ParquetWriter {
    /// Creates a new Parquet writer
    ///
    /// # Arguments
    /// * `output_dir` - Directory where Parquet files will be saved
    /// * `prefix` - Filename prefix for Parquet files
    /// * `compression` - Compression type to use
    /// * `buffer_size` - Number of records to buffer before writing
    ///
    /// # Returns
    /// A new ParquetWriter configured with the specified parameters
    pub fn new(
        output_dir: &str,
        prefix: &str,
        compression: CompressionType,
        buffer_size: usize,
    ) -> Result<Self> {
        // Create schema
        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("temp", DataType::Float32, false),
            Field::new("gx", DataType::Float32, false),
            Field::new("gy", DataType::Float32, false),
            Field::new("gz", DataType::Float32, false),
            Field::new("ax", DataType::Float32, false),
            Field::new("ay", DataType::Float32, false),
            Field::new("az", DataType::Float32, false),
            Field::new("system_timestamp", DataType::Int64, false),
        ]));

        // Ensure output directory exists
        create_dir_all(output_dir)
            .with_context(|| format!("Failed to create output directory: {}", output_dir))?;

        // Generate output file path
        let now = chrono::Utc::now();
        let filename = format!("{}_{}.parquet", prefix, now.format("%Y%m%d_%H%M%S"));
        let output_path = Path::new(output_dir).join(filename);
        let output_path_str = output_path.to_string_lossy().to_string();

        // Create a new Parquet writer
        let file = File::create(&output_path)
            .with_context(|| format!("Failed to create file: {}", output_path_str))?;

        // Convert compression type to Parquet compression
        let props = match compression {
            CompressionType::None => WriterProperties::builder()
                .set_compression(Compression::UNCOMPRESSED)
                .build(),
            CompressionType::Snappy => WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
            CompressionType::Gzip => WriterProperties::builder()
                .set_compression(Compression::GZIP(Default::default()))
                .build(),
            CompressionType::Lz4 => WriterProperties::builder()
                .set_compression(Compression::LZ4)
                .build(),
            CompressionType::Zstd => WriterProperties::builder()
                .set_compression(Compression::ZSTD(Default::default()))
                .build(),
        };

        // Initialize the ArrowWriter
        let writer = ArrowWriter::try_new(file, schema.clone(), Some(props))
            .with_context(|| format!("Failed to create Parquet writer for {}", output_path_str))?;

        Ok(ParquetWriter {
            schema,
            compression,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            output_path: output_path_str,
            writer: Some(writer),
        })
    }

    /// Adds a single sensor data record to the buffer
    ///
    /// Automatically flushes the buffer to disk when it reaches the configured buffer size
    ///
    /// # Arguments
    /// * `data` - The sensor data to add
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn add_data(&mut self, data: SensorData) -> Result<()> {
        self.buffer.push(data);

        if self.buffer.len() >= self.buffer_size {
            self.flush()?;
        }

        Ok(())
    }

    /// Flushes buffered data to the Parquet file
    ///
    /// Writes any data in the buffer to the Parquet file.
    /// No-op if buffer is empty.
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Create the RecordBatch from buffered data
        let batch = self._create_record_batch()?;

        // Write the batch to the Parquet file
        if let Some(writer) = &mut self.writer {
            writer
                .write(&batch)
                .with_context(|| format!("Failed to write data to {}", self.output_path))?;

            println!(
                "Wrote {} records to {}",
                self.buffer.len(),
                self.output_path
            );
        } else {
            return Err(
                ReceiverError::ParquetError("Writer is not initialized".to_string()).into(),
            );
        }

        // Clear the buffer
        self.buffer.clear();

        Ok(())
    }

    /// Creates a new file (for file splitting)
    ///
    /// Closes the current file after flushing any remaining data,
    /// then creates a new file with the current timestamp.
    ///
    /// # Arguments
    /// * `output_dir` - Directory to store the new file
    /// * `prefix` - Filename prefix for the new file
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn rotate_file(&mut self, output_dir: &str, prefix: &str) -> Result<()> {
        // Flush any remaining data
        self.flush()?;

        // Close the current writer by taking it and dropping it
        if let Some(writer) = self.writer.take() {
            writer.close().with_context(|| {
                format!("Failed to close Parquet writer for {}", self.output_path)
            })?;
        }

        // Ensure output directory exists
        create_dir_all(output_dir)
            .with_context(|| format!("Failed to create output directory: {}", output_dir))?;

        // Generate new output file path
        let now = chrono::Utc::now();
        let filename = format!("{}_{}.parquet", prefix, now.format("%Y%m%d_%H%M%S"));
        let output_path = Path::new(output_dir).join(filename);
        self.output_path = output_path.to_string_lossy().to_string();

        // Create a new Parquet writer
        let file = File::create(&output_path)
            .with_context(|| format!("Failed to create file: {}", self.output_path))?;

        // Convert compression type to Parquet compression and build properties
        let props = match self.compression {
            CompressionType::None => WriterProperties::builder()
                .set_compression(Compression::UNCOMPRESSED)
                .build(),
            CompressionType::Snappy => WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
            CompressionType::Gzip => WriterProperties::builder()
                .set_compression(Compression::GZIP(Default::default()))
                .build(),
            CompressionType::Lz4 => WriterProperties::builder()
                .set_compression(Compression::LZ4)
                .build(),
            CompressionType::Zstd => WriterProperties::builder()
                .set_compression(Compression::ZSTD(Default::default()))
                .build(),
        };

        // Initialize the ArrowWriter
        let writer = ArrowWriter::try_new(file, self.schema.clone(), Some(props))
            .with_context(|| format!("Failed to create Parquet writer for {}", self.output_path))?;

        self.writer = Some(writer);

        println!("Rotated to new file: {}", self.output_path);

        Ok(())
    }

    // Convert buffer data to Arrow RecordBatch (for actual file writing)
    fn _create_record_batch(&self) -> Result<RecordBatch> {
        // Extract data into columns
        let timestamps: Int64Array = self
            .buffer
            .iter()
            .map(|data| data.timestamp as i64)
            .collect();

        let temps: Float32Array = self.buffer.iter().map(|data| data.temp).collect();

        let gxs: Float32Array = self.buffer.iter().map(|data| data.gx).collect();

        let gys: Float32Array = self.buffer.iter().map(|data| data.gy).collect();

        let gzs: Float32Array = self.buffer.iter().map(|data| data.gz).collect();

        let axs: Float32Array = self.buffer.iter().map(|data| data.ax).collect();

        let ays: Float32Array = self.buffer.iter().map(|data| data.ay).collect();

        let azs: Float32Array = self.buffer.iter().map(|data| data.az).collect();

        let system_timestamps: Int64Array = self
            .buffer
            .iter()
            .map(|data| data.system_timestamp)
            .collect();

        // Create record batch
        RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(timestamps),
                Arc::new(temps),
                Arc::new(gxs),
                Arc::new(gys),
                Arc::new(gzs),
                Arc::new(axs),
                Arc::new(ays),
                Arc::new(azs),
                Arc::new(system_timestamps),
            ],
        )
        .with_context(|| "Failed to create record batch")
    }

    /// Close the writer and finalize the file
    ///
    /// Flushes any remaining data and properly closes the Parquet file.
    /// This should be called when finished with the writer to ensure all data is saved.
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn close(mut self) -> Result<()> {
        // Flush any remaining data
        self.flush()?;

        // Close the writer
        if let Some(writer) = self.writer.take() {
            writer.close().with_context(|| {
                format!("Failed to close Parquet writer for {}", self.output_path)
            })?;
            println!("Closed Parquet file: {}", self.output_path);
        }

        Ok(())
    }
}
