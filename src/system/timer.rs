use anyhow::{anyhow, Result};
use svd_expander::{DeviceSpec, PeripheralSpec};

use super::{gpio::TimerChannel, RangedField, SystemInfo};
use super::{Name, Submodule};

#[derive(Clone)]
pub struct Timer {
  pub name: Name,
  pub auto_reload_field: RangedField,
  pub prescaler_field: RangedField,
  pub counter_field: RangedField,
}
impl Timer {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Self> {
    let name = Name::from(&peripheral.name);
    Ok(Self {
      name,
      auto_reload_field: Self::find_single_field(peripheral, "arr")?,
      prescaler_field: Self::find_single_field(peripheral, "psc")?,
      counter_field: Self::find_single_field(peripheral, "cnt")?,
    })
  }

  fn find_single_field(p: &PeripheralSpec, name: &str) -> Result<RangedField> {
    match p.iter_fields().find(|f| f.name.to_lowercase() == name) {
      Some(f) => Ok(RangedField {
        path: f.path().to_lowercase(),
        min: 0,
        max: (2u64.pow(f.width) - 1) as u32,
      }),
      None => Err(anyhow!(
        "Could not find field named '{}' on {}",
        name,
        p.name
      )),
    }
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "timer".to_owned(),
      name: self.name.clone(),
    }
  }
}
