use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const DEFAULT_CONFIG_PATH: &str = "/etc/fw-fanctrl/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "defaultStrategy")]
    pub default_strategy: String,
    #[serde(rename = "strategyOnDischarging")]
    pub strategy_on_discharging: String,
    pub strategies: HashMap<String, Strategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    #[serde(rename = "fanSpeedUpdateFrequency")]
    pub fan_speed_update_frequency: u32,
    #[serde(rename = "movingAverageInterval")]
    pub moving_average_interval: u32,
    #[serde(rename = "speedCurve")]
    pub speed_curve: Vec<CurvePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub temp: u32,
    pub speed: u32,
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;

        let config: Config = serde_json::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))?;

        config.validate()?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.strategies.contains_key(&self.default_strategy) {
            return Err(Error::Config(format!(
                "Default strategy '{}' is not a valid strategy",
                self.default_strategy
            )));
        }

        if !self.strategy_on_discharging.is_empty()
            && !self.strategies.contains_key(&self.strategy_on_discharging)
        {
            return Err(Error::Config(format!(
                "Discharging strategy '{}' is not a valid strategy",
                self.strategy_on_discharging
            )));
        }

        for (name, strategy) in &self.strategies {
            if strategy.speed_curve.is_empty() {
                return Err(Error::Config(format!(
                    "Strategy '{}' has an empty speed curve",
                    name
                )));
            }
        }

        Ok(())
    }

    pub fn get_strategy(&self, name: &str) -> Option<&Strategy> {
        self.strategies.get(name)
    }

    pub fn get_default_strategy(&self) -> &Strategy {
        self.strategies
            .get(&self.default_strategy)
            .expect("Default strategy must exist")
    }

    pub fn get_discharging_strategy(&self) -> &Strategy {
        let name = if self.strategy_on_discharging.is_empty() {
            &self.default_strategy
        } else {
            &self.strategy_on_discharging
        };
        self.strategies
            .get(name)
            .expect("Discharging strategy must exist")
    }

    pub fn strategy_names(&self) -> Vec<&String> {
        self.strategies.keys().collect()
    }
}
