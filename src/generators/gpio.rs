use crate::{clear_bit, is_set, reset, set_bit, write_val};
use crate::{file::OutputDirectory, system::SystemInfo};
use crate::{generators::ReadWrite, system::gpio::Gpio};
use anyhow::Result;
use askama::Template;
use svd_expander::DeviceSpec;

pub fn generate(dry_run: bool, sys_info: &SystemInfo, out_dir: &OutputDirectory) -> Result<()> {
  for gpio in sys_info.gpios.iter() {
    out_dir.publish(
      dry_run,
      &format!("src/gpio/{}.rs", gpio.name.snake()),
      &PeripheralTemplate {
        g: &gpio,
        d: sys_info.device,
      }
      .render()?,
    )?;
  }

  out_dir.publish(
    dry_run,
    &f!("src/gpio/mod.rs"),
    &ModTemplate { s: sys_info }.render()?,
  )?;

  Ok(())
}

#[derive(Template)]
#[template(path = "gpio/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  s: &'a SystemInfo<'a>,
}

#[derive(Template)]
#[template(path = "gpio/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  g: &'a Gpio,
  d: &'a DeviceSpec,
}
