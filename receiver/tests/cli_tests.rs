use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("receiver").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains("Receives sensor data"));
}

#[test]
fn test_cli_missing_required_arg() {
    let mut cmd = Command::cargo_bin("receiver").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error: the following required arguments were not provided"));
}

#[test]
fn test_cli_invalid_compression() {
    let mut cmd = Command::cargo_bin("receiver").unwrap();
    cmd.args(&["-p", "dummy_port", "-c", "invalid"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid compression algorithm"));
}

#[test]
fn test_cli_output_dir_creation() {
    // Create a temporary directory for testing
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("test_output");
    let output_str = output_path.to_string_lossy();

    let mut cmd = Command::cargo_bin("receiver").unwrap();
    cmd.args(&[
        "-p", "dummy_port",
        "-o", &output_str,
        "-s", "0",
        "-f", "test",
        "-c", "snappy",
        "-u", "10",
    ]);

    // Run with timeout to avoid hanging in the simulation loop
    cmd.timeout(std::time::Duration::from_secs(2));
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Simulating data reception"))
        .stdout(predicate::str::contains("Receiver shutdown complete"));

    // Check if the output directory was created
    assert!(output_path.exists(), "Output directory wasn't created");
}