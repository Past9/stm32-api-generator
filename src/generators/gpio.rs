use crate::{clear_bit, is_set, reset, set_bit, write_val};
use crate::{file::OutputDirectory, system_info::SystemInfo};
use crate::{generators::ReadWrite, system_info::Gpio};
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
