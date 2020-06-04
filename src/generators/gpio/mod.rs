use crate::file::OutputDirectory;
use crate::generators::ReadWrite;
use crate::{clear_bit, reset, set_bit, write_val};
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
    let model = PeripheralModel::new(peripheral)?;
    out_dir.publish(
      &f!("src/gpio/{model.module_name}.rs"),
      &PeripheralTemplate {
        device: &d,
        peripheral: &model,
      }
      .render()?,
    )?;

    submodules.push(model.module_name);
  }

  out_dir.publish(
    &f!("src/gpio/mod.rs"),
    &ModTemplate {
      submodules: &submodules,
    }
    .render()?,
  )?;

  Ok(submodules)
}

#[derive(Template)]
#[template(path = "gpio/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  submodules: &'a Vec<String>,
}

#[derive(Template)]
#[template(path = "gpio/peripheral.rs.askama", escape = "none")]
struct PeripheralTemplate<'a> {
  device: &'a DeviceSpec,
  peripheral: &'a PeripheralModel,
}

struct PeripheralModel {
  pub struct_name: String,
  pub module_name: String,
  pub field_name: String,
  pub enable_field: String,
  pub pins: Vec<PinModel>,
}
impl PeripheralModel {
  pub fn new(p: &PeripheralSpec) -> Result<Self> {
    let letter = match p.name.chars().nth(4) {
      Some(l) => l,
      None => return Err(anyhow!("")),
    };

    Ok(Self {
      struct_name: p.name.to_camel_case(),
      module_name: p.name.to_snake_case(),
      field_name: p.name.to_snake_case(),
      enable_field: f!("RCC.AHBENR.IOP{letter}EN"),
      pins: (0..16)
        .map(|n| PinModel::new(&letter, n))
        .collect::<Result<Vec<PinModel>>>()?,
    })
  }
}

struct PinModel {
  pub struct_name: String,
  pub field_name: String,
  pub moder_field: String,
  pub pupdr_field: String,
  pub otyper_field: String,
  pub ospeedr_field: String,
  pub odr_field: String,
}
impl PinModel {
  pub fn new(letter: &char, pin_number: i32) -> Result<Self> {
    let pin_name = f!("P{letter}{pin_number}");

    Ok(Self {
      struct_name: pin_name.to_camel_case(),
      field_name: pin_name.to_snake_case(),
      moder_field: f!("GPIO{letter}.MODER.MODER{pin_number}"),
      pupdr_field: f!("GPIO{letter}.PUPDR.PUPDR{pin_number}"),
      otyper_field: f!("GPIO{letter}.OTYPER.OT{pin_number}"),
      ospeedr_field: f!("GPIO{letter}.OSPEEDR.OSPEEDR{pin_number}"),
      odr_field: f!("GPIO{letter}.ODR.ODR{pin_number}"),
    })
  }
}
