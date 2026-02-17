use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use fw_fanctrl::config::{Config, CurvePoint, Strategy};

fn create_temp_config(content: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let uuid = uuid::Uuid::new_v4();
    let path = dir.join(format!("fw-fanctrl-test-config-{}.json", uuid));
    let mut file = File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path
}

fn create_valid_config() -> (PathBuf, Config) {
    let content = r#"{
        "defaultStrategy": "performance",
        "strategyOnDischarging": "balanced",
        "strategies": {
            "performance": {
                "fanSpeedUpdateFrequency": 2,
                "movingAverageInterval": 30,
                "speedCurve": [
                    {"temp": 0, "speed": 0},
                    {"temp": 50, "speed": 30},
                    {"temp": 70, "speed": 60},
                    {"temp": 90, "speed": 100}
                ]
            },
            "balanced": {
                "fanSpeedUpdateFrequency": 5,
                "movingAverageInterval": 60,
                "speedCurve": [
                    {"temp": 0, "speed": 0},
                    {"temp": 60, "speed": 0},
                    {"temp": 80, "speed": 50},
                    {"temp": 95, "speed": 100}
                ]
            }
        }
    }"#;
    let path = create_temp_config(content);

    let mut strategies = HashMap::new();
    strategies.insert(
        "performance".to_string(),
        Strategy {
            fan_speed_update_frequency: 2,
            moving_average_interval: 30,
            speed_curve: vec![
                CurvePoint { temp: 0, speed: 0 },
                CurvePoint {
                    temp: 50,
                    speed: 30,
                },
                CurvePoint {
                    temp: 70,
                    speed: 60,
                },
                CurvePoint {
                    temp: 90,
                    speed: 100,
                },
            ],
        },
    );
    strategies.insert(
        "balanced".to_string(),
        Strategy {
            fan_speed_update_frequency: 5,
            moving_average_interval: 60,
            speed_curve: vec![
                CurvePoint { temp: 0, speed: 0 },
                CurvePoint { temp: 60, speed: 0 },
                CurvePoint {
                    temp: 80,
                    speed: 50,
                },
                CurvePoint {
                    temp: 95,
                    speed: 100,
                },
            ],
        },
    );
    let config = Config {
        default_strategy: "performance".to_string(),
        strategy_on_discharging: "balanced".to_string(),
        strategies,
    };

    (path, config)
}

#[test]
fn test_load_valid_config() {
    let (path, expected) = create_valid_config();
    let config = Config::load(&path).unwrap();
    assert_eq!(config.default_strategy, expected.default_strategy);
    assert_eq!(
        config.strategy_on_discharging,
        expected.strategy_on_discharging
    );
    assert!(config.strategies.contains_key("performance"));
    assert!(config.strategies.contains_key("balanced"));
}

#[test]
fn test_config_missing_default_strategy() {
    let content = r#"{
        "defaultStrategy": "missing",
        "strategyOnDischarging": "",
        "strategies": {
            "performance": {
                "fanSpeedUpdateFrequency": 2,
                "movingAverageInterval": 30,
                "speedCurve": [{"temp": 0, "speed": 0}]
            }
        }
    }"#;
    let path = create_temp_config(content);
    let result = Config::load(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("not a valid strategy"));
}

#[test]
fn test_config_empty_speed_curve() {
    let content = r#"{
        "defaultStrategy": "empty",
        "strategyOnDischarging": "",
        "strategies": {
            "empty": {
                "fanSpeedUpdateFrequency": 2,
                "movingAverageInterval": 30,
                "speedCurve": []
            }
        }
    }"#;
    let path = create_temp_config(content);
    let result = Config::load(&path);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("has an empty speed curve"));
}

#[test]
fn test_config_invalid_json() {
    let content = "{ invalid json }";
    let path = create_temp_config(content);
    let result = Config::load(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to parse"));
}

#[test]
fn test_config_missing_file() {
    let path = PathBuf::from("/nonexistent/config.json");
    let result = Config::load(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}

#[test]
fn test_strategy_names() {
    let (path, _) = create_valid_config();
    let config = Config::load(&path).unwrap();
    let names = config.strategy_names();
    assert!(names.contains(&&"performance".to_string()));
    assert!(names.contains(&&"balanced".to_string()));
    assert_eq!(names.len(), 2);
}

#[test]
fn test_get_strategy() {
    let (path, _) = create_valid_config();
    let config = Config::load(&path).unwrap();
    let strategy = config.get_strategy("performance");
    assert!(strategy.is_some());
    assert_eq!(strategy.unwrap().fan_speed_update_frequency, 2);
}

#[test]
fn test_get_default_strategy() {
    let (path, _) = create_valid_config();
    let config = Config::load(&path).unwrap();
    let strategy = config.get_default_strategy();
    assert_eq!(strategy.fan_speed_update_frequency, 2);
}

#[test]
fn test_get_discharging_strategy() {
    let (path, _) = create_valid_config();
    let config = Config::load(&path).unwrap();
    let strategy = config.get_discharging_strategy();
    assert_eq!(strategy.fan_speed_update_frequency, 5);
}

#[test]
fn test_discharging_fallback_to_default() {
    let content = r#"{
        "defaultStrategy": "performance",
        "strategyOnDischarging": "",
        "strategies": {
            "performance": {
                "fanSpeedUpdateFrequency": 2,
                "movingAverageInterval": 30,
                "speedCurve": [{"temp": 0, "speed": 0}]
            }
        }
    }"#;
    let path = create_temp_config(content);
    let config = Config::load(&path).unwrap();
    let strategy = config.get_discharging_strategy();
    assert_eq!(strategy.fan_speed_update_frequency, 2);
}
