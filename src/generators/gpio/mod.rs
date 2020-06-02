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

macro_rules! write_val {
  ($device:expr, $path:expr, $val:expr) => {
    $device.write_val(&$path, &$val);
  };
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
  pub fn new(d: &DeviceSpec, p: &PeripheralSpec) -> Result<Self> {
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
        .map(|n| PinModel::new(d, &letter, n))
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

  pub as_alt_func_writer: String,
  pub as_analog_writer: String,

  pub output_value_writer: String,
  pub reset_mode_writer: String,
  pub reset_pull_dir_writer: String,
  pub reset_output_type_writer: String,
  pub reset_output_speed_writer: String,
  pub reset_output_value_writer: String,
}
impl PinModel {
  pub fn new(d: &DeviceSpec, letter: &char, pin_number: i32) -> Result<Self> {
    let pin_name = f!("P{letter}{pin_number}");

    Ok(Self {
      struct_name: pin_name.to_camel_case(),
      field_name: pin_name.to_snake_case(),
      moder_field: f!("GPIO{letter}.MODER.MODER{pin_number}"),
      pupdr_field: f!("GPIO{letter}.PUPDR.PUPDR{pin_number}"),
      otyper_field: f!("GPIO{letter}.OTYPER.OT{pin_number}"),
      ospeedr_field: f!("GPIO{letter}.OSPEEDR.OSPEEDR{pin_number}"),

      as_alt_func_writer: d.write_val(&f!("GPIO{letter}.MODER.MODER{pin_number}"), "0b10"),
      as_analog_writer: d.write_val(&f!("GPIO{letter}.MODER.MODER{pin_number}"), "0b11"),

      output_value_writer: d.write_val(&f!("GPIO{letter}.ODR.ODR{pin_number}"), "value.val()"),
      reset_mode_writer: d.reset(&f!("GPIO{letter}.MODER.MODER{pin_number}"))?,
      reset_pull_dir_writer: d.reset(&f!("GPIO{letter}.PUPDR.PUPDR{pin_number}"))?,
      reset_output_type_writer: d.reset(&f!("GPIO{letter}.OTYPER.OT{pin_number}"))?,
      reset_output_speed_writer: d.reset(&f!("GPIO{letter}.OSPEEDR.OSPEEDR{pin_number}"))?,
      reset_output_value_writer: d.reset(&f!("GPIO{letter}.ODR.ODR{pin_number}"))?,
    })
  }
}
