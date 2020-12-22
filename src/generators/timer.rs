use crate::{
  generators::ReadWrite,
  system::{timer::Timer, SystemInfo},
};
use crate::{read_val, set_bit, write_val};
use anyhow::Result;
use askama::Template;
use svd_expander::DeviceSpec;

use crate::file::OutputDirectory;

pub fn generate(dry_run: bool, sys_info: &SystemInfo, out_dir: &OutputDirectory) -> Result<()> {
  for timer in sys_info.timers.iter() {
    out_dir.publish(
      dry_run,
      &format!("src/timer/{}.rs", timer.name.snake()),
      &PeripheralTemplate {
        t: &timer,
        d: &sys_info.device,
      }
      .render()?,
    )?;
  }

  out_dir.publish(
    dry_run,
    &f!("src/timer/mod.rs"),
    &ModTemplate { s: sys_info }.render()?,
  )?;

  Ok(())
}

#[derive(Template)]
#[template(path = "timer/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  s: &'a SystemInfo<'a>,
}

#[derive(Template)]
#[template(path = "timer/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  t: &'a Timer,
  d: &'a DeviceSpec,
}
