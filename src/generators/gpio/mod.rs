use crate::file::OutputDirectory;
use anyhow::Result;
use askama::Template;
use heck::{CamelCase, SnakeCase};
use svd_expander::{DeviceSpec, PeripheralSpec};

pub fn generate(d: &DeviceSpec, out_dir: &OutputDirectory) -> Result<Vec<String>> {
  let mut submodules: Vec<String> = Vec::new();

  for peripheral in d
    .peripherals
    .iter()
    .filter(|p| p.name.to_lowercase().starts_with("gpio"))
  {
    let model = PeripheralModel::new(peripheral);
    out_dir.publish(
      &format!("src/{}.rs", model.module_name),
      &PeripheralTemplate { peripheral: &model }.render()?,
    )?;

    submodules.push(model.module_name);
  }

  Ok(submodules)
}

#[derive(Template)]
#[template(path = "gpio/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  peripheral: &'a PeripheralModel,
}

struct PeripheralModel {
  pub struct_name: String,
  pub module_name: String,
  pub field_name: String,
}
impl PeripheralModel {
  pub fn new(p: &PeripheralSpec) -> Self {
    Self {
      struct_name: p.name.to_camel_case(),
      module_name: p.name.to_snake_case(),
      field_name: p.name.to_snake_case(),
    }
  }
}
