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

/// Read all available sensor data lines from a serial port
///
/// This improved version uses a fixed buffer to read multiple bytes at once
/// and maintains state between calls to handle incomplete lines.
/// It processes all complete lines in the buffer at once to avoid data loss.
pub fn read_serial_data(port: &mut Box<dyn SerialPort>) -> Result<Vec<String>> {
    let mut buf = [0u8; 4096]; // Large buffer to read multiple lines at once
    let mut complete_lines = Vec::new();

    // Read available data into buffer
    let n = match port.read(&mut buf) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    if n == 0 {
        return Ok(Vec::new());
    }

    // Convert received bytes to string
    let data = String::from_utf8_lossy(&buf[..n]).to_string();

    // Process the data with our line buffer
    LINE_BUFFER.with(|buffer| {
        let mut line_buffer = buffer.borrow_mut();

        // Append new data to existing buffer
        line_buffer.push_str(&data);

        // Process all complete lines in the buffer
        while let Some(pos) = line_buffer.find('\n') {
            // Extract the complete line
            let complete_line = line_buffer[..pos].to_string();
            complete_lines.push(complete_line);

            // Remove the processed line from the buffer
            *line_buffer = line_buffer[pos + 1..].to_string();
        }

        // Check for CR line endings as well
        while let Some(pos) = line_buffer.find('\r') {
            // Handle carriage return line endings
            let complete_line = line_buffer[..pos].to_string();
            complete_lines.push(complete_line);

            // Remove the processed line from the buffer
            *line_buffer = line_buffer[pos + 1..].to_string();
        }

        Ok(complete_lines)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    // MockSerialPort to simulate serial port behavior in tests
    struct MockSerialPort {
        cursor: Cursor<Vec<u8>>,
    }

    impl MockSerialPort {
        fn new(data: &[u8]) -> Self {
            Self {
                cursor: Cursor::new(data.to_vec()),
            }
        }
    }

    impl Read for MockSerialPort {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.cursor.read(buf)
        }
    }

    impl std::io::Write for MockSerialPort {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            // Just pretend we wrote everything
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl serialport::SerialPort for MockSerialPort {
        fn name(&self) -> Option<String> {
            Some("mock".to_string())
        }

        fn baud_rate(&self) -> serialport::Result<u32> {
            Ok(115200)
        }

        fn data_bits(&self) -> serialport::Result<serialport::DataBits> {
            Ok(serialport::DataBits::Eight)
        }

        fn flow_control(&self) -> serialport::Result<serialport::FlowControl> {
            Ok(serialport::FlowControl::None)
        }

        fn parity(&self) -> serialport::Result<serialport::Parity> {
            Ok(serialport::Parity::None)
        }

        fn stop_bits(&self) -> serialport::Result<serialport::StopBits> {
            Ok(serialport::StopBits::One)
        }

        fn timeout(&self) -> std::time::Duration {
            std::time::Duration::from_millis(100)
        }

        fn set_baud_rate(&mut self, _: u32) -> serialport::Result<()> {
            Ok(())
        }

        fn set_data_bits(&mut self, _: serialport::DataBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_flow_control(&mut self, _: serialport::FlowControl) -> serialport::Result<()> {
            Ok(())
        }

        fn set_parity(&mut self, _: serialport::Parity) -> serialport::Result<()> {
            Ok(())
        }

        fn set_stop_bits(&mut self, _: serialport::StopBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_timeout(&mut self, _: std::time::Duration) -> serialport::Result<()> {
            Ok(())
        }

        fn write_request_to_send(&mut self, _: bool) -> serialport::Result<()> {
            Ok(())
        }

        fn write_data_terminal_ready(&mut self, _: bool) -> serialport::Result<()> {
            Ok(())
        }

        fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
            Ok(true)
        }

        fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
            Ok(true)
        }

        fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }

        fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
            Ok(true)
        }

        fn bytes_to_read(&self) -> serialport::Result<u32> {
            Ok(0)
        }

        fn bytes_to_write(&self) -> serialport::Result<u32> {
            Ok(0)
        }

        fn clear(&self, _: serialport::ClearBuffer) -> serialport::Result<()> {
            Ok(())
        }

        fn try_clone(&self) -> serialport::Result<Box<dyn serialport::SerialPort>> {
            Err(serialport::Error::new(
                serialport::ErrorKind::Io(std::io::ErrorKind::Other),
                "Cannot clone mock serial port",
            ))
        }

        fn set_break(&self) -> serialport::Result<()> {
            Ok(())
        }

        fn clear_break(&self) -> serialport::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_read_serial_data_multiple_lines() {
        // Initialize a mock serial port with multiple lines of data
        let data = "00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n\
                   00000124,41300000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n\
                   00000125,41400000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n";

        let mut port = Box::new(MockSerialPort::new(data.as_bytes())) as Box<dyn SerialPort>;

        // Clear any existing line buffer
        LINE_BUFFER.with(|buffer| {
            *buffer.borrow_mut() = String::new();
        });

        // Read the data
        let result = read_serial_data(&mut port).unwrap();

        // Verify that all three lines were read
        assert_eq!(result.len(), 3, "Should have read 3 complete lines");
        assert_eq!(
            result[0],
            "00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000"
        );
        assert_eq!(
            result[1],
            "00000124,41300000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000"
        );
        assert_eq!(
            result[2],
            "00000125,41400000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000"
        );
    }

    #[test]
    fn test_read_serial_data_partial_line() {
        // Initialize a mock serial port with an incomplete line
        let data = "00000123,41200000,3F800000,3F800000,3F800000,3F80";
        let mut port = Box::new(MockSerialPort::new(data.as_bytes())) as Box<dyn SerialPort>;

        // Clear any existing line buffer
        LINE_BUFFER.with(|buffer| {
            *buffer.borrow_mut() = String::new();
        });

        // Read the data (should not find any complete lines)
        let result = read_serial_data(&mut port).unwrap();
        assert_eq!(result.len(), 0, "Should not have any complete lines");

        // Check that the data is in the buffer
        LINE_BUFFER.with(|buffer| {
            let line_buffer = buffer.borrow();
            assert_eq!(*line_buffer, data, "Data should be stored in buffer");
        });

        // Now add the rest of the line
        let data2 = "0000,3F800000,3F800000\n";
        let mut port = Box::new(MockSerialPort::new(data2.as_bytes())) as Box<dyn SerialPort>;

        // Read the data (should find the complete line now)
        let result = read_serial_data(&mut port).unwrap();
        assert_eq!(result.len(), 1, "Should now have one complete line");
        assert_eq!(
            result[0],
            "00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000"
        );
    }

    #[test]
    fn test_read_serial_data_multiple_reads() {
        // First read: two complete lines and start of a third
        let data1 = "00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n\
                   00000124,41300000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n\
                   00000125,414";
        let mut port1 = Box::new(MockSerialPort::new(data1.as_bytes())) as Box<dyn SerialPort>;

        // Clear any existing line buffer
        LINE_BUFFER.with(|buffer| {
            *buffer.borrow_mut() = String::new();
        });

        // First read
        let result1 = read_serial_data(&mut port1).unwrap();
        assert_eq!(result1.len(), 2, "Should have read 2 complete lines");

        // Second read: rest of the third line and a fourth line
        let data2 = "00000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n\
                    00000126,41500000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000\n";
        let mut port2 = Box::new(MockSerialPort::new(data2.as_bytes())) as Box<dyn SerialPort>;

        // Second read
        let result2 = read_serial_data(&mut port2).unwrap();
        assert_eq!(result2.len(), 2, "Should have read 2 more complete lines");
        assert_eq!(
            result2[0],
            "00000125,41400000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000"
        );
        assert_eq!(
            result2[1],
            "00000126,41500000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000"
        );
    }

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
