use framework_lib::chromium_ec::{CrosEc, CrosEcDriver};
use framework_lib::power;
use framework_lib::smbios::Platform;

use crate::error::{Error, Result};

const EC_MEMMAP_TEMP_SENSOR: u16 = 0x00;

fn get_battery_sensor_index(platform: Option<Platform>) -> Option<usize> {
    match platform {
        // Based on framework_lib/src/power.rs sensor mappings
        // Index 3 = Battery for Intel Gen 11/12/13
        Some(Platform::IntelGen11 | Platform::IntelGen12 | Platform::IntelGen13) => Some(3),
        // Index 2 = Battery for Intel Core Ultra 1
        Some(Platform::IntelCoreUltra1) => Some(2),
        // Index 3 = Battery for Framework 12 Gen 13
        Some(Platform::Framework12IntelGen13) => Some(3),
        // AMD platforms (7040/AI300): No separate battery sensor in EC memory map
        // Framework 16: Different sensor layout with dGPU
        // These platforms should use max temp from all sensors
        _ => None,
    }
}

pub struct HardwareController {
    ec: CrosEc,
    battery_sensor_index: Option<usize>,
    platform_name: String,
}

impl HardwareController {
    pub fn new(no_battery_sensors: bool) -> Result<Self> {
        let ec = CrosEc::new();

        let platform = framework_lib::smbios::get_platform();
        let platform_name = format!("{:?}", platform);

        // Only determine battery sensor index if flag is set
        let battery_index = if no_battery_sensors {
            get_battery_sensor_index(platform)
        } else {
            None
        };

        tracing::info!(
            "Platform: {}, Battery sensor index to exclude: {:?}",
            platform_name,
            battery_index
        );

        Ok(Self {
            ec,
            battery_sensor_index: battery_index,
            platform_name,
        })
    }

    pub fn get_temperature(&self) -> Result<f64> {
        let temps = self
            .ec
            .read_memory(EC_MEMMAP_TEMP_SENSOR, 0x0F)
            .ok_or_else(|| Error::Ec("Failed to read temperature from EC".into()))?;

        // Filter invalid values (0xFF=NotPresent, 0xFE=Error, 0xFD=NotPowered, 0xFC=NotCalibrated)
        // and convert from EC raw value to Celsius (subtract 73)
        let valid_temps: Vec<(usize, u8)> = temps
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, t)| *t < 0xFC)
            .map(|(i, t)| (i, t.saturating_sub(73)))
            .filter(|(_, t)| *t > 0)
            .collect();

        tracing::debug!("Raw temperature sensors: {:02x?}", temps);
        tracing::debug!(
            "Valid temperature sensors (index, Celsius): {:?}",
            valid_temps
        );

        if valid_temps.is_empty() {
            return Ok(50.0);
        }

        let max_temp = if let Some(battery_idx) = self.battery_sensor_index {
            // Exclude the battery sensor at the known index
            let non_battery: Vec<u8> = valid_temps
                .iter()
                .filter(|(i, _)| *i != battery_idx)
                .map(|(_, t)| *t)
                .collect();

            if non_battery.is_empty() {
                // If all sensors were filtered out, fall back to max of all
                *valid_temps.iter().map(|(_, t)| t).max().unwrap()
            } else {
                *non_battery.iter().max().unwrap()
            }
        } else {
            // No battery exclusion (unknown platform or flag not set) - use max of all
            *valid_temps.iter().map(|(_, t)| t).max().unwrap()
        };

        tracing::debug!(
            "Selected max temperature: {}°C (platform: {})",
            max_temp,
            self.platform_name
        );

        Ok(max_temp as f64)
    }

    pub fn set_fan_speed(&self, speed: u32) -> Result<()> {
        self.ec
            .fan_set_duty(None, speed)
            .map_err(|e| Error::Ec(format!("{:?}", e)))
    }

    pub fn get_fan_speed(&self) -> Result<u32> {
        let fans = self
            .ec
            .read_memory(0x10, 8)
            .ok_or_else(|| Error::Ec("Failed to read fan info from EC".into()))?;

        let duty = fans[4];
        if duty > 100 {
            return Ok(0);
        }
        Ok(duty as u32)
    }

    pub fn is_on_ac(&self) -> Result<bool> {
        let info = power::power_info(&self.ec)
            .ok_or_else(|| Error::Ec("Failed to read power info from EC".into()))?;
        Ok(info.ac_present)
    }

    pub fn enable_auto_fan(&self) -> Result<()> {
        self.ec
            .autofanctrl(None)
            .map_err(|e| Error::Ec(format!("{:?}", e)))
    }

    #[allow(dead_code)]
    pub fn get_fan_rpm(&self) -> Result<u16> {
        let fans = self
            .ec
            .read_memory(0x10, 8)
            .ok_or_else(|| Error::Ec("Failed to read fan RPM from EC".into()))?;

        let rpm = u16::from_le_bytes([fans[0], fans[1]]);
        Ok(rpm)
    }

    pub fn check_temperature(&self) -> Result<f64> {
        let temp = self.get_temperature()?;
        if !(0.0..=100.0).contains(&temp) {
            return Err(Error::Ec(format!(
                "Temperature {}°C is out of valid range (0-100)",
                temp
            )));
        }
        Ok(temp)
    }

    pub fn test_fan_control(&self, steps: u32) -> Result<Vec<(u32, u16)>> {
        let original_speed = self.get_fan_speed().unwrap_or(0);
        let mut results = Vec::new();

        let speed_step = 100 / steps.max(1);

        for i in 1..=steps {
            let speed = (speed_step * i).min(100);
            if let Err(e) = self.set_fan_speed(speed) {
                let _ = self.set_fan_speed(original_speed.min(100));
                return Err(e);
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            let rpm = self.get_fan_rpm().unwrap_or(0);
            results.push((speed, rpm));
        }

        let _ = self.set_fan_speed(original_speed.min(100));

        Ok(results)
    }

    #[allow(dead_code)]
    pub fn restore_fan(&self, speed: u32) -> Result<()> {
        self.set_fan_speed(speed)
    }
}
