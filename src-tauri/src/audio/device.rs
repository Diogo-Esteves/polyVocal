use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// List all available audio input devices on the current host.
pub fn list_input_devices() -> Result<Vec<InputDevice>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let devices = host
        .input_devices()?
        .filter_map(|d| {
            d.name().ok().map(|name| InputDevice {
                id: name.clone(),
                name: name.clone(),
                is_default: name == default_name,
            })
        })
        .collect();

    Ok(devices)
}
