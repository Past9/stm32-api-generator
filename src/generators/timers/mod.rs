use anyhow::{anyhow, Result};
use askama::Template;
use heck::{CamelCase, SnakeCase};
use regex::Regex;
use svd_expander::{DeviceSpec, PeripheralSpec};

use crate::file::OutputDirectory;

pub fn generate(dry_run: bool, d: &DeviceSpec, out_dir: &OutputDirectory) -> Result<Vec<String>> {
  let p_name_test = Regex::new(r"TIM[0-9]+")?;
  let mut submodules: Vec<String> = Vec::new();

  for peripheral in d
    .peripherals
    .iter()
    .filter(|p| p_name_test.is_match(&p.name))
  {
    let model = PeripheralModel::new(peripheral)?;
    out_dir.publish(
      dry_run,
      &f!("src/timers/{model.module_name}.rs"),
      &PeripheralTemplate {
        device: &d,
        peripheral: &model,
      }
      .render()?,
    )?;

    submodules.push(model.module_name);
  }

  out_dir.publish(
    dry_run,
    &f!("src/timers/mod.rs"),
    &ModTemplate {
      submodules: &submodules,
    }
    .render()?,
  )?;

  Ok(submodules)
}

#[derive(Template)]
#[template(path = "timers/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  submodules: &'a Vec<String>,
}

#[derive(Template)]
#[template(path = "timers/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  device: &'a DeviceSpec,
  peripheral: &'a PeripheralModel,
}

struct PeripheralModel {
  pub struct_name: String,
  pub module_name: String,
  pub field_name: String,
}
impl PeripheralModel {
  pub fn new(p: &PeripheralSpec) -> Result<Self> {
    Ok(Self {
      struct_name: p.name.to_camel_case(),
      module_name: p.name.to_snake_case(),
      field_name: p.name.to_snake_case(),
    })
  }
}
