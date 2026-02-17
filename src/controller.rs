use std::collections::VecDeque;

use crate::config::{Config, Strategy};
use crate::curve::interpolate;
use crate::error::{Error, Result};
use crate::hardware::HardwareController;

const TEMP_HISTORY_MAX_LEN: usize = 100;

pub struct FanController {
    hw: HardwareController,
    config: Config,
    overwritten_strategy: Option<String>,
    temp_history: VecDeque<f64>,
    current_speed: u32,
    active: bool,
    timecount: u32,
}

impl FanController {
    pub fn new(hw: HardwareController, config: Config, initial_strategy: Option<String>) -> Self {
        let overwritten_strategy = initial_strategy.filter(|s| !s.is_empty());
        Self {
            hw,
            config,
            overwritten_strategy,
            temp_history: VecDeque::with_capacity(TEMP_HISTORY_MAX_LEN),
            current_speed: 0,
            active: true,
            timecount: 0,
        }
    }

    pub fn get_current_strategy(&self) -> &Strategy {
        if let Some(ref name) = self.overwritten_strategy {
            self.config
                .get_strategy(name)
                .expect("Overwritten strategy must exist")
        } else if self.hw.is_on_ac().unwrap_or(false) {
            self.config.get_default_strategy()
        } else {
            self.config.get_discharging_strategy()
        }
    }

    pub fn get_current_strategy_name(&self) -> String {
        if let Some(ref name) = self.overwritten_strategy {
            return name.clone();
        }

        if self.hw.is_on_ac().unwrap_or(false) {
            return self.config.default_strategy.clone();
        }

        let discharging = &self.config.strategy_on_discharging;
        if discharging.is_empty() {
            return self.config.default_strategy.clone();
        }

        discharging.clone()
    }

    pub fn is_overwritten(&self) -> bool {
        self.overwritten_strategy.is_some()
    }

    pub fn overwrite_strategy(&mut self, name: &str) -> Result<()> {
        if self.config.get_strategy(name).is_none() {
            return Err(Error::Strategy(format!("Unknown strategy: {}", name)));
        }
        self.overwritten_strategy = Some(name.to_string());
        self.timecount = 0;
        Ok(())
    }

    pub fn clear_overwritten_strategy(&mut self) {
        self.overwritten_strategy = None;
        self.timecount = 0;
    }

    pub fn get_actual_temperature(&self) -> Result<f64> {
        self.hw.get_temperature()
    }

    pub fn get_moving_average_temperature(&self, interval: u32) -> f64 {
        let positive_temps: Vec<f64> = self
            .temp_history
            .iter()
            .filter(|&&t| t > 0.0)
            .copied()
            .collect();

        if positive_temps.is_empty() {
            return self.get_actual_temperature().unwrap_or(50.0);
        }

        let slice_start = positive_temps.len().saturating_sub(interval as usize);
        let slice = &positive_temps[slice_start..];

        if slice.is_empty() {
            return positive_temps.iter().sum::<f64>() / positive_temps.len() as f64;
        }

        slice.iter().sum::<f64>() / slice.len() as f64
    }

    pub fn get_effective_temperature(&self, current_temp: f64, interval: u32) -> f64 {
        let moving_avg = self.get_moving_average_temperature(interval);
        let effective = (moving_avg * 2.0 + current_temp) / 3.0;
        (effective * 100.0).round() / 100.0
    }

    pub fn adapt_speed(&mut self, current_temp: f64) -> Result<()> {
        let strategy = self.get_current_strategy();
        let effective_temp =
            self.get_effective_temperature(current_temp, strategy.moving_average_interval);

        let new_speed = interpolate(&strategy.speed_curve, effective_temp as u32);

        if self.active {
            self.hw.set_fan_speed(new_speed)?;
            self.current_speed = new_speed;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_speed(&mut self, speed: u32) -> Result<()> {
        self.hw.set_fan_speed(speed)?;
        self.current_speed = speed;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.active = false;
        self.hw.enable_auto_fan()
    }

    pub fn resume(&mut self) -> Result<()> {
        self.active = true;
        Ok(())
    }

    pub fn enable_auto_fan(&self) -> Result<()> {
        self.hw.enable_auto_fan()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn get_current_speed(&self) -> u32 {
        self.current_speed
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn step(&mut self) -> Result<f64> {
        let temp = self.get_actual_temperature()?;

        let strategy = self.get_current_strategy();
        if self.timecount % strategy.fan_speed_update_frequency == 0 {
            self.adapt_speed(temp)?;
            self.timecount = 0;
        }

        self.temp_history.push_back(temp);

        if self.temp_history.len() > TEMP_HISTORY_MAX_LEN {
            self.temp_history.pop_front();
        }

        self.timecount += 1;

        Ok(temp)
    }

    pub fn reload_config(&mut self, config: Config) {
        self.config = config;
        if let Some(ref name) = self.overwritten_strategy {
            if self.config.get_strategy(name).is_none() {
                self.overwritten_strategy = None;
            }
        }
    }
}
