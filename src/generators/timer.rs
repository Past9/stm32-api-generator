use crate::{clear_bit, is_set, read_val, reset, set_bit, write_val};
use crate::{
  generators::ReadWrite,
  system::{timer::Timer, SystemInfo},
};
use anyhow::Result;
use askama::Template;
use svd_expander::DeviceSpec;

use crate::file::OutputDirectory;

pub fn generate(
  dry_run: bool,
  sys_info: &SystemInfo,
  src_dir: &OutputDirectory,
  api_path: String,
) -> Result<()> {
  for timer in sys_info.timers.iter() {
    src_dir.publish(
      dry_run,
      &format!("timer/{}.rs", timer.name.snake()),
      &PeripheralTemplate {
        api_path: api_path.clone(),
        t: &timer,
        d: &sys_info.device,
      }
      .render()?,
    )?;
  }

  src_dir.publish(
    dry_run,
    &f!("timer/mod.rs"),
    &ModTemplate {
      api_path: api_path.clone(),
      s: sys_info,
    }
    .render()?,
  )?;

  Ok(())
}

#[derive(Template)]
#[template(path = "timer/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  api_path: String,
  s: &'a SystemInfo<'a>,
}

#[derive(Template)]
#[template(path = "timer/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  api_path: String,
  t: &'a Timer,
  d: &'a DeviceSpec,
}
