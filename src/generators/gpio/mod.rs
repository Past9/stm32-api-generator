use crate::file::OutputDirectory;
use crate::generators::ReadWrite;
use anyhow::{anyhow, Result};
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
    let model = PeripheralModel::new(d, peripheral)?;
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
  pub enable_writer: String,
  pub disable_writer: String,
}
impl PeripheralModel {
  pub fn new(d: &DeviceSpec, p: &PeripheralSpec) -> Result<Self> {
    let letter = match p.name.chars().nth(4) {
      Some(l) => l,
      None => return Err(anyhow!("")),
    };

    Ok(Self {
      struct_name: p.name.to_camel_case(),
      module_name: p.name.to_snake_case(),
      field_name: p.name.to_snake_case(),
      enable_writer: d.set_bit(&f!("RCC.AHBENR.IOP{letter}EN"))?,
      disable_writer: d.clear_bit(&f!("RCC.AHBENR.IOP{letter}EN"))?,
    })
  }
}
