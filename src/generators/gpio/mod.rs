use crate::file::OutputDirectory;
use crate::generators::ReadWrite;
use crate::{clear_bit, reset, set_bit, write_val};
use anyhow::{anyhow, Result};
use askama::Template;
use heck::{CamelCase, SnakeCase};
use regex::Regex;
use svd_expander::{DeviceSpec, PeripheralSpec, RegisterSpec};

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
        .map(|n| PinModel::new(&letter, n, &p))
        .collect::<Result<Vec<PinModel>>>()?,
    })
  }
}

struct PinModel {
  pub struct_name: String,
  pub field_name: String,
  pub afr_field: String,
  pub moder_field: String,
  pub pupdr_field: String,
  pub otyper_field: String,
  pub ospeedr_field: String,
  pub odr_field: String,
  pub alt_funcs: Vec<AltFuncModel>,
}
impl PinModel {
  pub fn new(letter: &char, pin_number: i32, p: &PeripheralSpec) -> Result<Self> {
    let pin_name = f!("P{letter}{pin_number}");

    let af_register_name = match pin_number {
      0..=7 => "AFRL",
      8..=15 => "AFRH",
      _ => {
        return Err(anyhow!(f!(
          "Pin number {pin_number} out of bounds for alt functions."
        )))
      }
    };

    let mut alt_funcs = Vec::new();

    if let Some(ref afr) = p.iter_registers().find(|r| r.name == af_register_name) {
      alt_funcs.extend(AltFuncModel::create_for_pin(&afr, pin_number)?);
    }

    Ok(Self {
      struct_name: pin_name.to_camel_case(),
      field_name: pin_name.to_snake_case(),
      alt_funcs,
      afr_field: f!("GPIO{letter}.{af_register_name}.{af_register_name}{pin_number}"),
      moder_field: f!("GPIO{letter}.MODER.MODER{pin_number}"),
      pupdr_field: f!("GPIO{letter}.PUPDR.PUPDR{pin_number}"),
      otyper_field: f!("GPIO{letter}.OTYPER.OT{pin_number}"),
      ospeedr_field: f!("GPIO{letter}.OSPEEDR.OSPEEDR{pin_number}"),
      odr_field: f!("GPIO{letter}.ODR.ODR{pin_number}"),
    })
  }
}

struct AltFuncModel {
  pub value: u32,
  pub struct_name: String,
  pub field_name: String,
}
impl AltFuncModel {
  pub fn create_for_pin(afr: &RegisterSpec, pin_number: i32) -> Result<Vec<Self>> {
    let generic_name_test = Regex::new(r"AF[0-9]+")?;

    let mut alt_funcs = Vec::new();

    let opt_field = afr
      .fields
      .iter()
      .find(|f| f.name == f!("AFRL{pin_number}") || f.name == f!("AFRH{pin_number}"));

    if let Some(field) = opt_field {
      for val_set in field.enumerated_value_sets.iter() {
        for enum_val in val_set.values.iter() {
          match enum_val.actual_value() {
            Some(ref v) => {
              let mut name = enum_val.name.clone();
              if let Some(ref description) = enum_val.description {
                name = description.clone()
              }

              if generic_name_test.is_match(&name) {
                break;
              }

              alt_funcs.push(Self {
                value: *v,
                struct_name: name.to_camel_case(),
                field_name: name.to_snake_case(),
              });
            }
            None => {}
          }
        }
      }
    }

    Ok(alt_funcs)
  }
}
