# CXD5602PWBIMU Data Logger

A high-performance Rust application for logging sensor data from the Sony CXD5602 PWBIMU sensor module to Parquet files.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

This application receives sensor data via serial/UART connection from Arduino or other microcontrollers connected to a CXD5602 PWBIMU sensor, and efficiently logs this data to Parquet files for later analysis. It features multi-threaded processing, configurable compression, and automatic file rotation.

## Features

- **Serial Data Capture**: Read sensor data from UART/serial connections
- **Efficient Storage**: Save data in columnar Parquet format with multiple compression options
- **Concurrent Processing**: Multi-threaded architecture for optimal performance
- **Time-based File Rotation**: Automatically create new files at specified intervals
- **Flexible Configuration**: Adjust parameters via command-line arguments
- **Robust Error Handling**: Graceful handling of communication errors and data corruption
- **Simulation Mode**: Test functionality without physical hardware connection

## Installation

### Prerequisites

- Rust toolchain (1.67.0 or newer)
- Cargo package manager

### Building from Source

```bash
# Clone the repository
git clone https://github.com/oshiteku/cxd5602pwbimu-logger.git
cd cxd5602pwbimu-logger/receiver

# Build the release version
cargo build --release

# The binary will be available at target/release/receiver
```

## Usage

Run the application with the following command:

```bash
./target/release/receiver -p <SERIAL_PORT> [OPTIONS]
```

Or directly with Cargo:

```bash
cargo run --release -- -p <SERIAL_PORT> [OPTIONS]
```

### Command-line Options

| Option | Description | Default |
|--------|-------------|---------|
| `-p, --port` | Serial port (e.g., `/dev/ttyUSB0`, `COM3`) | (Required) |
| `-b, --baud_rate` | Serial communication speed | 921600 |
| `-o, --output_dir` | Directory for storing Parquet files | `./logs` |
| `-s, --split_minutes` | Minutes between file rotations (0 = no rotation) | 0 |
| `-f, --prefix` | Filename prefix for the output files | `sensor_log` |
| `-c, --compression` | Compression algorithm (none, snappy, gzip, lz4, zstd) | `snappy` |
| `-u, --buffer_size` | Number of data points to buffer before writing | 100 |
| `-m, --simulation` | Run in simulation mode (no hardware needed) | Off |

### Example

```bash
# Capture data from /dev/ttyUSB0 with 60-minute file rotation
./target/release/receiver -p /dev/ttyUSB0 -b 921600 -o ./data -s 60 -c zstd

# Run in simulation mode for testing
./target/release/receiver -p dummy -m
```

## Input Data Format

The application expects sensor data in the following format over the serial connection:

```
%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x\n
```

Where each field represents:
1. Timestamp (uint32)
2. Temperature (float as hex-encoded uint32)
3. Gyroscope X (float as hex-encoded uint32)
4. Gyroscope Y (float as hex-encoded uint32)
5. Gyroscope Z (float as hex-encoded uint32)
6. Accelerometer X (float as hex-encoded uint32)
7. Accelerometer Y (float as hex-encoded uint32)
8. Accelerometer Z (float as hex-encoded uint32)

Example: `00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000`

## Output Format

Data is stored in Parquet files with the following schema:

```
message schema {
    required INT64 timestamp;
    required FLOAT temp;
    required FLOAT gx;
    required FLOAT gy;
    required FLOAT gz;
    required FLOAT ax;
    required FLOAT ay;
    required FLOAT az;
    required INT64 system_timestamp;
}
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name
```

### Code Formatting and Linting

```bash
# Format code
cargo fmt

# Run linter
cargo clippy
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Apache Arrow](https://arrow.apache.org/) and [Parquet](https://parquet.apache.org/)
- Serial communication via [serialport-rs](https://github.com/serialport/serialport-rs)