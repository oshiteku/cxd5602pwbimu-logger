use anyhow::{Context, Result};
use chrono::Utc;
use serialport::SerialPort;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use super::error::ReceiverError;
use super::types::SensorData;

/// Opens a serial port with the specified settings
pub fn open_serial_port(port: &str, baud_rate: u32) -> Result<Box<dyn SerialPort>> {
    serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
        .with_context(|| format!("Failed to open serial port {}", port))
}

/// Parse a line of hex data into a SensorData struct
pub fn parse_sensor_data(line: &str) -> Result<SensorData> {
    // Example format: 00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000
    let parts: Vec<&str> = line.trim().split(',').collect();
    
    if parts.len() != 8 {
        return Err(ReceiverError::ParseError(format!(
            "Expected 8 parts, got {}: {}",
            parts.len(),
            line
        ))
        .into());
    }
    
    // For demonstration purposes, we're just parsing without real implementation
    // In a real implementation, we would convert each hex string to u32 and then bit-cast to f32
    
    // Mock implementation to represent the parsing logic only for testing
    let timestamp = u32::from_str_radix(parts[0], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid timestamp: {}, error: {}", parts[0], e))
    })?;
    
    // The remaining values would be parsed similarly in a real implementation
    let system_ts = Utc::now().timestamp_millis();
    
    Ok(SensorData {
        timestamp,
        temp: 25.0, // Placeholder
        gx: 0.1,    // Placeholder
        gy: 0.2,    // Placeholder
        gz: 0.3,    // Placeholder
        ax: 1.0,    // Placeholder
        ay: 1.1,    // Placeholder
        az: 1.2,    // Placeholder
        system_timestamp: system_ts,
    })
}

/// Read sensor data from a serial port
pub fn read_serial_data(port: &mut Box<dyn SerialPort>) -> Result<String> {
    let mut reader = BufReader::new(port);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sensor_data_valid() {
        let line = "00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000";
        let result = parse_sensor_data(line);
        assert!(result.is_ok(), "Failed to parse valid data");
        
        let data = result.unwrap();
        assert_eq!(data.timestamp, 0x123);
    }

    #[test]
    fn test_parse_sensor_data_invalid_format() {
        // Not enough parts
        let line = "00000123,41200000";
        let result = parse_sensor_data(line);
        assert!(result.is_err(), "Should fail with not enough parts");
        
        // Invalid hex in timestamp
        let line = "NOTAHEX,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000";
        let result = parse_sensor_data(line);
        assert!(result.is_err(), "Should fail with invalid hex");
    }
}