use std::collections::HashMap;

use crate::generators::ReadWrite;
use crate::{read_val, write_val};
use anyhow::{anyhow, Result};
use askama::Template;
use heck::{CamelCase, SnakeCase};
use regex::Regex;
use svd_expander::{DeviceSpec, FieldSpec, PeripheralSpec, RegisterSpec};

use crate::file::OutputDirectory;

pub fn generate(
  dry_run: bool,
  d: &DeviceSpec,
  out_dir: &OutputDirectory,
  timer_channels: &HashMap<String, Vec<String>>,
) -> Result<Vec<String>> {
  let p_name_test = Regex::new(r"TIM[0-9]+")?;
  let mut submodules: Vec<String> = Vec::new();

  for peripheral in d
    .peripherals
    .iter()
    .filter(|p| p_name_test.is_match(&p.name))
  {
    let model = PeripheralModel::new(peripheral, timer_channels.get(&peripheral.name))?;
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
  pub auto_reload_field: AutoReloadField, //pub fields: Vec<FieldModel>,
  pub counter_field: CounterField,
  pub prescaler_field: PrescalerField,
}
impl PeripheralModel {
  pub fn new(p: &PeripheralSpec, channels: Option<&Vec<String>>) -> Result<Self> {
    Ok(Self {
      struct_name: p.name.to_camel_case(),
      module_name: p.name.to_snake_case(),
      field_name: p.name.to_snake_case(),
      auto_reload_field: Self::get_auto_reload_field(p)?,
      counter_field: Self::get_counter_field(p)?,
      prescaler_field: Self::get_prescaler_field(p)?,
    })
  }

  fn get_auto_reload_field(p: &PeripheralSpec) -> Result<AutoReloadField> {
    let field = Self::find_single_field(p, "arr")?;

    Ok(AutoReloadField {
      path: field.path(),
      min: 0,
      max: (2u64.pow(field.width) - 1) as u32,
    })
  }

  fn get_prescaler_field(p: &PeripheralSpec) -> Result<PrescalerField> {
    let field = Self::find_single_field(p, "psc")?;

    Ok(PrescalerField {
      path: field.path(),
      min: 0,
      max: (2u64.pow(field.width) - 1) as u32,
    })
  }

  fn get_counter_field(p: &PeripheralSpec) -> Result<CounterField> {
    let field = Self::find_single_field(p, "cnt")?;

    Ok(CounterField { path: field.path() })
  }

  fn find_single_field<'a>(p: &'a PeripheralSpec, name: &str) -> Result<&'a FieldSpec> {
    let fields = p
      .iter_fields()
      .filter(|f| f.name.to_lowercase() == name)
      .collect::<Vec<&FieldSpec>>();

    if fields.len() == 0 {
      return Err(anyhow!(
        "Could not find field named '{}' on {}",
        name,
        p.name
      ));
    } else if fields.len() > 1 {
      return Err(anyhow!(
        "Multiple fields found named '{}' in {}",
        name,
        p.name
      ));
    }

    Ok(fields[0])
  }
}

struct AutoReloadField {
  path: String,
  min: u32,
  max: u32,
}

struct CounterField {
  path: String,
}

struct PrescalerField {
  path: String,
  min: u32,
  max: u32,
}

/*
struct FieldModel {
  pub description: String,
  pub field_name: String,
  pub register_field_name: String,
}
impl FieldModel {
  pub fn new_from_register(r: &RegisterSpec) -> Vec<Self> {
    r.fields.iter().map(|f| FieldModel::new(f, r)).collect()
  }

  pub fn new(f: &FieldSpec, r: &RegisterSpec) -> Self {
    Self {
      description: match &f.description {
        Some(d) => d.clone(),
        None => "".to_owned(),
      },
      register_field_name: r.name.to_snake_case(),
      field_name: f.name.to_snake_case(),
    }
  }
}
*/
