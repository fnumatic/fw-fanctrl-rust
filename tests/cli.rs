use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_no_command() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No command provided"));
}

#[test]
fn test_run_help() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("run")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--config"));
}

#[test]
fn test_run_requires_config() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("run")
        .arg("--config")
        .arg("/nonexistent/path/config.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read config"));
}

#[test]
fn test_use_requires_strategy() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("use").assert().failure();
}

#[test]
fn test_sanity_check_non_framework() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("sanity-check")
        .assert()
        .code(0)
        .stderr(predicate::str::contains("Not a Framework Laptop"));
}

#[test]
fn test_output_format_json() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("--output-format=json")
        .arg("print")
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to connect"));
}

#[test]
fn test_output_format_natural() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("--output-format=natural")
        .arg("print")
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to connect"));
}

#[test]
fn test_use_help() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("use").arg("--help").assert().success();
}

#[test]
fn test_print_help() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("print").arg("--help").assert().success();
}

#[test]
fn test_reset_is_subcommand() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("reset").assert().failure();
}

#[test]
fn test_reload_is_subcommand() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("reload").assert().failure();
}

#[test]
fn test_pause_is_subcommand() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("pause").assert().failure();
}

#[test]
fn test_resume_is_subcommand() {
    let mut cmd = Command::cargo_bin("fw-fanctrl").unwrap();
    cmd.arg("resume").assert().failure();
}
