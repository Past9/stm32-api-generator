use crate::{clear_bit, is_set, read_val, reset, set_bit, wait_for_clear, wait_for_set, write_val};
use crate::{
  file::OutputDirectory,
  generators::ReadWrite,
  system::{spi::Spi, SystemInfo},
};
use anyhow::Result;
use askama::Template;
use svd_expander::DeviceSpec;

pub fn generate(
  dry_run: bool,
  sys_info: &SystemInfo,
  src_dir: &OutputDirectory,
  api_path: String,
) -> Result<()> {
  for spi in sys_info.spis.iter() {
    src_dir.publish(
      dry_run,
      &format!("spi/{}.rs", spi.struct_name.snake()),
      &PeripheralTemplate {
        api_path: api_path.clone(),
        spi: &spi,
        d: &sys_info.device,
      }
      .render()?,
    )?;
  }

  src_dir.publish(
    dry_run,
    &f!("spi/mod.rs"),
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
  api_path: String,
  spi: &'a Spi,
  d: &'a DeviceSpec,
}
