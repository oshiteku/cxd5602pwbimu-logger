use anyhow::{Context, Result};
use arrow::array::{Float32Array, Int64Array};
use arrow::record_batch::RecordBatch;
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use parquet::basic::Compression;
use std::fs::{File, create_dir_all};
use std::path::Path;
use std::sync::Arc;

use super::error::ReceiverError;
use super::types::{CompressionType, SensorData};

pub struct ParquetWriter {
    schema: Arc<Schema>,
    compression: CompressionType,
    buffer: Vec<SensorData>,
    buffer_size: usize,
    output_path: String,
    writer: Option<ArrowWriter<File>>,
}

impl ParquetWriter {
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

    pub fn add_data(&mut self, data: SensorData) -> Result<()> {
        self.buffer.push(data);
        
        if self.buffer.len() >= self.buffer_size {
            self.flush()?;
        }
        
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Create the RecordBatch from buffered data
        let batch = self._create_record_batch()?;
        
        // Write the batch to the Parquet file
        if let Some(writer) = &mut self.writer {
            writer.write(&batch)
                .with_context(|| format!("Failed to write data to {}", self.output_path))?;
            
            println!("Wrote {} records to {}", self.buffer.len(), self.output_path);
        } else {
            return Err(ReceiverError::ParquetError(
                "Writer is not initialized".to_string()).into());
        }
        
        // Clear the buffer
        self.buffer.clear();
        
        Ok(())
    }
    
    // Creates a new file (for file splitting)
    pub fn rotate_file(&mut self, output_dir: &str, prefix: &str) -> Result<()> {
        // Flush any remaining data
        self.flush()?;
        
        // Close the current writer by taking it and dropping it
        if let Some(writer) = self.writer.take() {
            writer.close()
                .with_context(|| format!("Failed to close Parquet writer for {}", self.output_path))?;
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
        let timestamps: Int64Array = self.buffer.iter()
            .map(|data| data.timestamp as i64)
            .collect();
        
        let temps: Float32Array = self.buffer.iter()
            .map(|data| data.temp)
            .collect();
        
        let gxs: Float32Array = self.buffer.iter()
            .map(|data| data.gx)
            .collect();
        
        let gys: Float32Array = self.buffer.iter()
            .map(|data| data.gy)
            .collect();
        
        let gzs: Float32Array = self.buffer.iter()
            .map(|data| data.gz)
            .collect();
        
        let axs: Float32Array = self.buffer.iter()
            .map(|data| data.ax)
            .collect();
        
        let ays: Float32Array = self.buffer.iter()
            .map(|data| data.ay)
            .collect();
        
        let azs: Float32Array = self.buffer.iter()
            .map(|data| data.az)
            .collect();
        
        let system_timestamps: Int64Array = self.buffer.iter()
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
    
    // Close the writer and finalize the file
    pub fn close(mut self) -> Result<()> {
        // Flush any remaining data
        self.flush()?;
        
        // Close the writer
        if let Some(writer) = self.writer.take() {
            writer.close()
                .with_context(|| format!("Failed to close Parquet writer for {}", self.output_path))?;
            println!("Closed Parquet file: {}", self.output_path);
        }
        
        Ok(())
    }
}