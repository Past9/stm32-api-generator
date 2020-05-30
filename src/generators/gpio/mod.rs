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
      &f!("src/gpio/{model.module_name}.rs"),
      &PeripheralTemplate { peripheral: &model }.render()?,
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
  peripheral: &'a PeripheralModel,
}

struct PeripheralModel {
  pub struct_name: String,
  pub module_name: String,
  pub field_name: String,
  pub enable_writer: String,
  pub disable_writer: String,
  pub pins: Vec<PinModel>,
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
      pins: (0..16)
        .map(|n| PinModel::new(d, &letter, n))
        .collect::<Result<Vec<PinModel>>>()?,
    })
  }
}

struct PinModel {
  pub struct_name: String,
  pub field_name: String,
  pub as_input_writer: String,
  pub as_output_writer: String,
  pub as_alt_func_writer: String,
  pub as_analog_writer: String,
  pub pull_dir_writer: String,
  pub output_type_writer: String,
  pub output_speed_writer: String,
}
impl PinModel {
  pub fn new(d: &DeviceSpec, letter: &char, pin_number: i32) -> Result<Self> {
    let pin_name = f!("P{letter}{pin_number}");

    Ok(Self {
      struct_name: pin_name.to_camel_case(),
      field_name: pin_name.to_snake_case(),
      as_input_writer: d.write_val(&f!("GPIO{letter}.MODER.MODER{pin_number}"), "0b00")?,
      as_output_writer: d.write_val(&f!("GPIO{letter}.MODER.MODER{pin_number}"), "0b01")?,
      as_alt_func_writer: d.write_val(&f!("GPIO{letter}.MODER.MODER{pin_number}"), "0b10")?,
      as_analog_writer: d.write_val(&f!("GPIO{letter}.MODER.MODER{pin_number}"), "0b11")?,
      pull_dir_writer: d.write_val(
        &f!("GPIO{letter}.PUPDR.PUPDR{pin_number}"),
        "pull_dir.val()",
      )?,
      output_type_writer: d.write_val(
        &f!("GPIO{letter}.OTYPER.OT{pin_number}"),
        "output_type.val()",
      )?,
      output_speed_writer: d.write_val(
        &f!("GPIO{letter}.OSPEEDR.OSPEEDR{pin_number}"),
        "output_speed.val()",
      )?,
    })
  }
}
