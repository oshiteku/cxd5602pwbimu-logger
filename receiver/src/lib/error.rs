use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReceiverError {
    #[error("Serial port error: {0}")]
    SerialError(#[from] serialport::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Parquet error: {0}")]
    ParquetError(String),
    
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}