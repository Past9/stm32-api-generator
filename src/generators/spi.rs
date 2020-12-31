use crate::{clear_bit, set_bit};
use crate::{
  file::OutputDirectory,
  generators::ReadWrite,
  system::{spi::Spi, SystemInfo},
};
use anyhow::Result;
use askama::Template;
use svd_expander::DeviceSpec;

pub fn generate(dry_run: bool, sys_info: &SystemInfo, out_dir: &OutputDirectory) -> Result<()> {
  for spi in sys_info.spis.iter() {
    out_dir.publish(
      dry_run,
      &format!("src/spi/{}.rs", spi.name.snake()),
      &PeripheralTemplate {
        spi: &spi,
        d: &sys_info.device,
      }
      .render()?,
    )?;
  }

  out_dir.publish(
    dry_run,
    &f!("src/spi/mod.rs"),
    &ModTemplate { s: sys_info }.render()?,
  )?;

  Ok(())
}

#[derive(Template)]
#[template(path = "spi/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  s: &'a SystemInfo<'a>,
}

#[derive(Template)]
#[template(path = "spi/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  spi: &'a Spi,
  d: &'a DeviceSpec,
}
