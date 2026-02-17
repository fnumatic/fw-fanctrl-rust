use framework_lib::chromium_ec::{CrosEc, CrosEcDriver};
use framework_lib::power;

use crate::error::{Error, Result};

const EC_MEMMAP_TEMP_SENSOR: u16 = 0x00;

pub struct HardwareController {
    ec: CrosEc,
    no_battery_sensors: bool,
}

impl HardwareController {
    pub fn new(no_battery_sensors: bool) -> Result<Self> {
        let ec = CrosEc::new();
        Ok(Self {
            ec,
            no_battery_sensors,
        })
    }

    pub fn get_temperature(&self) -> Result<f64> {
        let temps = self
            .ec
            .read_memory(EC_MEMMAP_TEMP_SENSOR, 0x0F)
            .ok_or_else(|| Error::Ec("Failed to read temperature from EC".into()))?;

        let mut valid_temps: Vec<u8> = temps.iter().copied().filter(|&t| t > 0).collect();

        if valid_temps.is_empty() {
            return Ok(50.0);
        }

        if self.no_battery_sensors {
            valid_temps.truncate(valid_temps.len().saturating_sub(1));
            if valid_temps.is_empty() {
                return Ok(50.0);
            }
        }

        let max_temp = *valid_temps.iter().max().unwrap();
        Ok(max_temp as f64)
    }

    pub fn set_fan_speed(&self, speed: u32) -> Result<()> {
        self.ec
            .fan_set_duty(None, speed)
            .map_err(|e| Error::Ec(format!("{:?}", e)))
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
}
