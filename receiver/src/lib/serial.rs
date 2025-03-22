use anyhow::{Context, Result};
use chrono::Utc;
use serialport::SerialPort;
use std::cell::RefCell;
use std::time::Duration;

use super::error::ReceiverError;
use super::types::SensorData;

// Buffer to hold incomplete lines between reads
thread_local! {
    static LINE_BUFFER: RefCell<String> = RefCell::new(String::with_capacity(4096));
}

/// Opens a serial port with the specified settings
pub fn open_serial_port(port: &str, baud_rate: u32) -> Result<Box<dyn SerialPort>> {
    serialport::new(port, baud_rate)
        .timeout(Duration::from_millis(100)) // Increased timeout for high-speed data
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

    // Parse each hex string into u32, then bit-cast to f32 for the float values
    let timestamp = u32::from_str_radix(parts[0], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid timestamp: {}, error: {}", parts[0], e))
    })?;

    // Parse temperature (hex as u32 to f32 bit pattern)
    let temp_bits = u32::from_str_radix(parts[1], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid temperature: {}, error: {}", parts[1], e))
    })?;
    let temp = f32::from_bits(temp_bits);

    // Parse gyroscope values
    let gx_bits = u32::from_str_radix(parts[2], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid gx: {}, error: {}", parts[2], e))
    })?;
    let gx = f32::from_bits(gx_bits);

    let gy_bits = u32::from_str_radix(parts[3], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid gy: {}, error: {}", parts[3], e))
    })?;
    let gy = f32::from_bits(gy_bits);

    let gz_bits = u32::from_str_radix(parts[4], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid gz: {}, error: {}", parts[4], e))
    })?;
    let gz = f32::from_bits(gz_bits);

    // Parse accelerometer values
    let ax_bits = u32::from_str_radix(parts[5], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid ax: {}, error: {}", parts[5], e))
    })?;
    let ax = f32::from_bits(ax_bits);

    let ay_bits = u32::from_str_radix(parts[6], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid ay: {}, error: {}", parts[6], e))
    })?;
    let ay = f32::from_bits(ay_bits);

    let az_bits = u32::from_str_radix(parts[7], 16).map_err(|e| {
        ReceiverError::ParseError(format!("Invalid az: {}, error: {}", parts[7], e))
    })?;
    let az = f32::from_bits(az_bits);

    let system_ts = Utc::now().timestamp_millis();

    Ok(SensorData {
        timestamp,
        temp,
        gx,
        gy,
        gz,
        ax,
        ay,
        az,
        system_timestamp: system_ts,
    })
}

/// Read sensor data from a serial port
///
/// This improved version uses a fixed buffer to read multiple bytes at once
/// and maintains state between calls to handle incomplete lines.
pub fn read_serial_data(port: &mut Box<dyn SerialPort>) -> Result<String> {
    let mut buf = [0u8; 4096]; // Large buffer to read multiple lines at once

    // Read available data into buffer
    let n = match port.read(&mut buf) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Ok(String::new()),
        Err(e) => return Err(e.into()),
    };

    if n == 0 {
        return Ok(String::new());
    }

    // Convert received bytes to string
    let data = String::from_utf8_lossy(&buf[..n]).to_string();

    // Process the data with our line buffer
    LINE_BUFFER.with(|buffer| {
        let mut line_buffer = buffer.borrow_mut();

        // Append new data to existing buffer
        line_buffer.push_str(&data);

        // Look for a complete line
        if let Some(pos) = line_buffer.find('\n') {
            // Extract the complete line
            let complete_line = line_buffer[..pos].to_string();

            // Remove the processed line from the buffer
            *line_buffer = line_buffer[pos + 1..].to_string();

            Ok(complete_line)
        } else if let Some(pos) = line_buffer.find('\r') {
            // Handle carriage return line endings
            let complete_line = line_buffer[..pos].to_string();
            *line_buffer = line_buffer[pos + 1..].to_string();
            Ok(complete_line)
        } else {
            // No complete line yet, return empty string
            Ok(String::new())
        }
    })
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

        // Test float bit conversions
        // 41200000 in hex is 10.0 in float
        assert!(
            (data.temp - 10.0).abs() < f32::EPSILON,
            "Temperature should be 10.0"
        );

        // 3F800000 in hex is 1.0 in float
        assert!((data.gx - 1.0).abs() < f32::EPSILON, "gx should be 1.0");
        assert!((data.gy - 1.0).abs() < f32::EPSILON, "gy should be 1.0");
        assert!((data.gz - 1.0).abs() < f32::EPSILON, "gz should be 1.0");
        assert!((data.ax - 1.0).abs() < f32::EPSILON, "ax should be 1.0");
        assert!((data.ay - 1.0).abs() < f32::EPSILON, "ay should be 1.0");
        assert!((data.az - 1.0).abs() < f32::EPSILON, "az should be 1.0");
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

    #[test]
    fn test_bit_conversion() {
        // Test specific known bit patterns
        let line = "00000001,40A00000,40400000,C0000000,00000000,3F800000,BF800000,80000000";
        let result = parse_sensor_data(line).unwrap();

        assert_eq!(result.timestamp, 1); // 0x1
        assert!(
            (result.temp - 5.0).abs() < f32::EPSILON,
            "40A00000 should be 5.0"
        );
        assert!(
            (result.gx - 3.0).abs() < f32::EPSILON,
            "40400000 should be 3.0"
        );
        assert!(
            (result.gy - (-2.0)).abs() < f32::EPSILON,
            "C0000000 should be -2.0"
        );
        assert!(
            (result.gz - 0.0).abs() < f32::EPSILON,
            "00000000 should be 0.0"
        );
        assert!(
            (result.ax - 1.0).abs() < f32::EPSILON,
            "3F800000 should be 1.0"
        );
        assert!(
            (result.ay - (-1.0)).abs() < f32::EPSILON,
            "BF800000 should be -1.0"
        );
        assert!(
            (result.az - (-0.0)).abs() < f32::EPSILON,
            "80000000 should be -0.0"
        );
    }
}
