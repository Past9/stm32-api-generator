use anyhow::{anyhow, Result};
use svd_expander::{DeviceSpec, PeripheralSpec};

use super::{Name, Submodule};

pub struct Spi {
  pub name: Name,
  pub peripheral_enable_field: String,
}
impl Spi {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Self> {
    let name = Name::from(&peripheral.name);
    let enable_field_name = format!("{}en", name.snake());

    Ok(Self {
      name,
      peripheral_enable_field: match device
        .iter_fields()
        .find(|f| f.name.to_lowercase() == enable_field_name)
      {
        Some(field) => Ok(field.path()),
        None => Err(anyhow!(
          "Could not find timer enable field {}",
          enable_field_name
        )),
      }?,
    })
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "spi".to_owned(),
      name: self.name.clone(),
      needs_clocks: true,
    }
  }
}
