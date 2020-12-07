mod generator;
mod schematic;

use std::path::Path;
use svd_expander::DeviceSpec;

use crate::file::OutputDirectory;

use self::templates::ClocksTemplate;
use askama::Template;

use anyhow::{anyhow, Result};
use schematic::{ClockComponent, ClockSchematic};

pub fn generate(d: &DeviceSpec, out_dir: &OutputDirectory) -> Result<()> {
  let clock_spec_filepath = format!("specs/clock/{}.ron", d.name.to_lowercase());

  ClockGenerator::from_ron_file(clock_spec_filepath, d)?.generate(out_dir)?;

  Ok(())
}

#[derive(Debug)]
pub struct ClockGenerator<'a> {
  spec: &'a DeviceSpec,
  schematic: ClockSchematic,
}
impl<'a> ClockGenerator<'a> {
  pub fn from_ron_file<P: AsRef<Path>>(
    path: P,
    spec: &'a DeviceSpec,
  ) -> Result<ClockGenerator<'a>> {
    let generator = ClockGenerator {
      spec,
      schematic: ClockSchematic::from_ron_file(path)?,
    };
    generator.validate()?;
    Ok(generator)
  }

  pub fn from_ron<S: Into<String>>(ron: S, spec: &'a DeviceSpec) -> Result<ClockGenerator<'a>> {
    let generator = ClockGenerator {
      spec,
      schematic: ClockSchematic::from_ron(ron)?,
    };
    generator.validate()?;
    Ok(generator)
  }

  pub fn generate(&self, out_dir: &OutputDirectory) -> Result<()> {
    let _tpl = ClocksTemplate::new(&self.schematic);
    let clocks_file = ClocksTemplate::new(&self.schematic).render()?;

    out_dir.publish(&f!("src/clocks.rs"), &clocks_file)?;

    Ok(())
  }

  fn validate(&self) -> Result<()> {
    self.check_valid_field_paths()?;
    self.check_valid_field_input_sizes()?;
    Ok(())
  }

  fn check_valid_field_paths(&self) -> Result<()> {
    let input_paths = self
      .schematic
      .get_all_components()
      .iter()
      .filter_map(|(_, c)| match c {
        ClockComponent::Multiplexer(m) => Some(
          m.inputs()
            .iter()
            .map(|(_, i)| i.path())
            .collect::<Vec<String>>(),
        ),
        ClockComponent::Divider(d) => Some(
          d.values()
            .iter()
            .map(|(_, i)| i.path())
            .collect::<Vec<String>>(),
        ),
        ClockComponent::Multiplier(m) => Some(
          m.values()
            .iter()
            .map(|(_, i)| i.path())
            .collect::<Vec<String>>(),
        ),
        _ => None,
      })
      .flat_map(|i| i)
      .collect::<Vec<String>>();

    for path in input_paths {
      match self.spec.try_get_field(&path) {
        None => {
          return Err(anyhow!("No field named '{}' in SVD spec", path));
        }
        _ => {}
      }
    }

    Ok(())
  }

  fn check_valid_field_input_sizes(&self) -> Result<()> {
    let field_vals = self
      .schematic
      .get_all_components()
      .iter()
      .flat_map(|c| match c {
        (name, ClockComponent::Multiplexer(m)) => m
          .inputs()
          .iter()
          .map(|(_, v)| (v.path(), v.bit_value(), name.clone()))
          .collect::<Vec<(String, u32, String)>>(),
        (name, ClockComponent::Divider(d)) => d
          .values()
          .iter()
          .map(|(_, v)| (v.path(), v.bit_value(), name.clone()))
          .collect::<Vec<(String, u32, String)>>(),
        _ => vec![],
        (name, ClockComponent::Multiplier(m)) => m
          .values()
          .iter()
          .map(|(_, v)| (v.path(), v.bit_value(), name.clone()))
          .collect::<Vec<(String, u32, String)>>(),
        _ => vec![],
      })
      .collect::<Vec<(String, u32, String)>>();

    for field_val in field_vals.iter() {
      self.check_valid_input_size(&field_val.0, field_val.1, &field_val.2)?;
    }

    Ok(())
  }

  fn check_valid_input_size(&self, path: &str, bit_value: u32, component_name: &str) -> Result<()> {
    let field_spec = self.spec.get_field(path)?;
    let shift = 32 - field_spec.width;
    let max_val = std::u32::MAX << shift >> shift;

    match bit_value > max_val {
      true => Err(anyhow!(
        "Bit value '{}' does not fit in {}-bit field '{}' ({})",
        bit_value,
        field_spec.width,
        path,
        component_name
      )),
      false => Ok(()),
    }
  }
}

mod templates {
  use super::ClockSchematic;
  use crate::generators::clocks::schematic;
  use askama::Template;

  #[derive(Template)]
  #[template(path = "clocks/mod.rs.askama", escape = "none")]
  pub struct ClocksTemplate {
    oscillators: Vec<Oscillator>,
  }
  impl ClocksTemplate {
    pub fn new(schematic: &ClockSchematic) -> ClocksTemplate {
      ClocksTemplate {
        oscillators: schematic
          .get_oscillators()
          .iter()
          .map(|t| Oscillator::new(t))
          .collect(),
      }
    }
  }

  pub struct Oscillator {
    name: String,
    default_freq: u64,
  }
  impl Oscillator {
    pub fn new(o: &(String, schematic::Oscillator)) -> Oscillator {
      Oscillator {
        name: o.0.clone(),
        default_freq: o.1.frequency(),
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use svd_expander::DeviceSpec;

  use super::*;

  #[test]
  fn rejects_invalid_field_paths() {
    let clock_ron = r#"
      ClockSchematic(
        oscillators: {
          "hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {
          "pll_div": (
            input: "hse",
            values: {
              "no_div": (1, "timer0.cr.en", 0)
            },
            default: 1,
          )
        },
        multipliers: {
          "pll_mul": (
            input: "pll_div", 
            values: {
              "no_mul": (2, "bogus.field", 1)
            },
            default: 2,
          )
        },
        taps: {
          "tap1": (
            input: "pll_mul", 
            max: 1000000, 
            terminal: true
          ),
        }
      )
    "#;

    let _device_xml = r#"
    "#;

    let device = DeviceSpec::from_file("specs/svd/arm_device.svd").unwrap();

    let res = ClockGenerator::from_ron(clock_ron, &device);

    assert!(res.is_err());
    assert_eq!(
      "No field named 'bogus.field' in SVD spec",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_too_big_bit_field_values() {
    let clock_ron = r#"
      ClockSchematic(
        oscillators: {
          "hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {
          "pll_div": (
            input: "hse",
            values: {
              "no_div": (1, "timer0.cr.mode", 15)
            },
            default: 1,
          )
        },
        multipliers: {},
        taps: {
          "tap1": (
            input: "pll_div", 
            max: 1000000, 
            terminal: true
          ),
        }
      )
    "#;

    let _device_xml = r#"
    "#;

    let device = DeviceSpec::from_file("specs/svd/arm_device.svd").unwrap();

    let res = ClockGenerator::from_ron(clock_ron, &device);

    assert!(res.is_err());
    assert_eq!(
      "Bit value '15' does not fit in 3-bit field 'timer0.cr.mode' (pll_div)",
      res.unwrap_err().to_string()
    );
  }
}
