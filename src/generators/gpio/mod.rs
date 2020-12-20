use std::collections::HashMap;

use crate::file::OutputDirectory;
use crate::generators::ReadWrite;
use crate::{clear_bit, is_set, reset, set_bit, write_val};
use anyhow::{anyhow, Result};
use askama::Template;
use heck::{CamelCase, SnakeCase};
use regex::{Captures, Regex};
use svd_expander::{DeviceSpec, PeripheralSpec, RegisterSpec};

use super::TimerChannelInfo;

pub struct GpioMetadata {
  pub submodules: Vec<String>,
  pub timer_channels: HashMap<String, Vec<TimerChannelInfo>>,
}

pub fn generate(dry_run: bool, d: &DeviceSpec, out_dir: &OutputDirectory) -> Result<GpioMetadata> {
  let mut submodules: Vec<String> = Vec::new();
  let mut timer_channels: HashMap<String, Vec<TimerChannelInfo>> = HashMap::new();

  for peripheral in d
    .peripherals
    .iter()
    .filter(|p| p.name.to_lowercase().starts_with("gpio"))
  {
    let model = PeripheralModel::new(peripheral)?;
    out_dir.publish(
      dry_run,
      &f!("src/gpio/{model.module_name}.rs"),
      &PeripheralTemplate {
        device: &d,
        peripheral: &model,
      }
      .render()?,
    )?;

    submodules.push(model.module_name);

    for pin in model.pins.iter() {
      for alt_func in pin.alt_funcs.iter() {
        if let Some(ref tc) = alt_func.timer_channel_info {
          timer_channels
            .entry(tc.timer_field_name.to_snake_case())
            .or_insert(Vec::new())
            .push(tc.clone())
        }
      }
    }
  }

  for channels in timer_channels.values_mut() {
    channels.sort();
    channels.dedup();
  }

  let mut mod_timer_channels: Vec<TimerChannelInfo> = Vec::new();
  for channels in timer_channels.values() {
    for channel in channels.iter() {
      mod_timer_channels.push(channel.clone());
    }
  }
  mod_timer_channels.sort();
  mod_timer_channels.dedup();

  out_dir.publish(
    dry_run,
    &f!("src/gpio/mod.rs"),
    &ModTemplate {
      submodules: &submodules,
      timer_channels: mod_timer_channels,
    }
    .render()?,
  )?;

  Ok(GpioMetadata {
    submodules,
    timer_channels,
  })
}

#[derive(Template)]
#[template(path = "gpio/mod.rs.askama", escape = "none")]
struct ModTemplate<'a> {
  submodules: &'a Vec<String>,
  timer_channels: Vec<TimerChannelInfo>,
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

  pub fn timer_channel_struct_names(&self) -> Vec<String> {
    let mut channel_names = Vec::new();
    for pin in self.pins.iter() {
      channel_names.extend(pin.timer_channel_struct_names());
    }
    channel_names.sort();
    channel_names.dedup();
    channel_names
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
  pub idr_field: String,
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
      idr_field: f!("GPIO{letter}.IDR.IDR{pin_number}"),
    })
  }

  pub fn timer_channel_struct_names(&self) -> Vec<String> {
    let mut channel_names = Vec::new();
    for alt_func in self.alt_funcs.iter() {
      if let Some(ref tci) = alt_func.timer_channel_info {
        channel_names.push(tci.struct_name());
      }
    }
    channel_names.sort();
    channel_names.dedup();
    channel_names
  }
}

#[derive(Debug)]
struct AltFuncModel {
  pub value: u32,
  pub struct_name: String,
  pub field_name: String,
  pub timer_channel_info: Option<TimerChannelInfo>,
}
impl AltFuncModel {
  pub fn create_for_pin(afr: &RegisterSpec, pin_number: i32) -> Result<Vec<Self>> {
    let generic_name_test = Regex::new(r"^AF[0-9]+$")?;
    let timer_channel_name_test = Regex::new(r"^(TIM[0-9]+)_(CH[0-9]N?$)")?;

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

              let u_name = name.to_uppercase();

              let timer_channel_info = match timer_channel_name_test.is_match(&u_name) {
                true => {
                  let captures = timer_channel_name_test
                    .captures_iter(&name)
                    .collect::<Vec<Captures>>();

                  if captures.len() == 0 {
                    return Err(anyhow!(
                      "Could not parse timer channel alt func '{}' for pin {}",
                      name,
                      pin_number
                    ));
                  } else if captures.len() > 1 {
                    return Err(anyhow!(
                      "Multiple timer channel names found in alt func name '{}' for pin {}",
                      name,
                      pin_number
                    ));
                  } else {
                    let timer_name = match captures[0].get(1) {
                      Some(c) => c.as_str().to_owned(),
                      None => {
                        return Err(anyhow!(
                          "Could not find timer name in '{}' for pin {}",
                          u_name,
                          pin_number
                        ));
                      }
                    };

                    let channel_name = match captures[0].get(2) {
                      Some(c) => c.as_str().to_owned(),
                      None => {
                        return Err(anyhow!(
                          "Could not find channel name in '{}' for pin {}",
                          u_name,
                          pin_number
                        ));
                      }
                    };

                    Some(TimerChannelInfo {
                      timer_field_name: timer_name.to_snake_case(),
                      timer_struct_name: timer_name.to_camel_case(),
                      channel_field_name: channel_name.to_snake_case(),
                      channel_struct_name: channel_name.to_camel_case(),
                    })
                  }
                }
                false => None,
              };

              if !generic_name_test.is_match(&name.to_uppercase()) {
                alt_funcs.push(Self {
                  value: *v,
                  struct_name: name.to_camel_case(),
                  field_name: name.to_snake_case(),
                  timer_channel_info,
                });
              }
            }
            None => {}
          }
        }
      }
    }

    Ok(alt_funcs)
  }
}
