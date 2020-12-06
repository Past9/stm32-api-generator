use std::path::Path;

use svd_expander::DeviceSpec;

use super::schematic::ClockSchematic;
use anyhow::{anyhow, Result};

pub struct ClockGenerator<'a> {
  spec: &'a DeviceSpec,
  schematic: ClockSchematic,
}
impl<'a> ClockGenerator<'a> {
  pub fn from_ron_file<P: AsRef<Path>>(
    path: P,
    spec: &'a DeviceSpec,
  ) -> Result<ClockGenerator<'a>> {
    Ok(ClockGenerator {
      spec,
      schematic: ClockSchematic::from_ron_file(path)?,
    })
  }

  pub fn from_ron<S: Into<String>>(ron: S, spec: &'a DeviceSpec) -> Result<ClockGenerator<'a>> {
    Ok(ClockGenerator {
      spec,
      schematic: ClockSchematic::from_ron(ron)?,
    })
  }

  fn validate(&self) -> Result<()> {
    self.check_valid_paths()?;

    Ok(())
  }

  fn check_valid_paths(&self) -> Result<()> {
    unimplemented!();
  }
}
