/// Data structure representing a single sensor reading
#[derive(Debug, Clone)]
pub struct SensorData {
    /// Timestamp from the sensor (uint32 from Arduino)
    pub timestamp: u32,
    /// Temperature reading (float)
    pub temp: f32,
    /// Gyroscope X-axis (float)
    pub gx: f32,
    /// Gyroscope Y-axis (float)
    pub gy: f32,
    /// Gyroscope Z-axis (float)
    pub gz: f32,
    /// Accelerometer X-axis (float)
    pub ax: f32,
    /// Accelerometer Y-axis (float)
    pub ay: f32,
    /// Accelerometer Z-axis (float)
    pub az: f32,
    /// System timestamp when the data was received (i64 representation of time)
    pub system_timestamp: i64,
}

/// Compression algorithm options
pub enum CompressionType {
    None,
    Snappy,
    Gzip,
    Lz4,
    Zstd,
}

impl std::str::FromStr for CompressionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(CompressionType::None),
            "snappy" => Ok(CompressionType::Snappy),
            "gzip" => Ok(CompressionType::Gzip),
            "lz4" => Ok(CompressionType::Lz4),
            "zstd" => Ok(CompressionType::Zstd),
            _ => Err(format!("Unknown compression type: {}", s)),
        }
    }
}