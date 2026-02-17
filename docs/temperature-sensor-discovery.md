# Temperature Sensor Discovery

## Goal

Improve `--no-battery-sensors` by dynamically identifying battery sensors from EC metadata,
instead of relying only on static platform mappings.

## Background

Platform-based assumptions (for example, "battery is index X") can be wrong when:

- EC firmware reports a different sensor order
- sensor availability changes at runtime
- platform mapping is incomplete

Wrong exclusion can make fan control react too late or to the wrong thermal source.

## New Approach

When `--no-battery-sensors` is enabled, `fw-fanctrl` can query sensor metadata directly
from EC, equivalent to what `ectool tempsinfo all` does.

For each sensor ID:

1. Check if the sensor is present
2. Query `EC_CMD_TEMP_SENSOR_GET_INFO` (`0x0070`)
3. Read:
   - `sensor_name` (string)
   - `sensor_type` (numeric type)
4. Classify battery sensors by metadata (name/type)
5. Exclude only battery sensors from max-temperature selection

This makes battery exclusion data-driven and firmware-aware.

## Behavior

### With `--no-battery-sensors`

- battery-classified sensors are excluded
- max temperature is computed from remaining valid sensors

### Without `--no-battery-sensors`

- all valid sensors are considered
- max temperature is computed across all sensors

## Fallback Strategy

If sensor metadata cannot be retrieved (unsupported command, partial failure, unknown labels):

- fall back to safe behavior: use max of all valid sensors
- do not guess battery index from position

This favors thermal safety over aggressive filtering.

## Logging

At debug level, implementation should log:

- detected platform
- discovered sensors: `(id, name, type, value)`
- excluded battery sensor IDs
- final selected temperature source and value

Example:

- `sensor discovery: id=3 name=Battery type=... temp=42`
- `excluding battery sensors: [3]`
- `selected max temperature: 58C`

## Compatibility Notes

- Dynamic sensor discovery is conceptually equivalent to Python/ectool behavior.
- Platform mappings remain useful as optional fallback hints, but not the primary
  source of truth.
