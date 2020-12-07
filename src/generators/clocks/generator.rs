use std::{iter::Map, path::Path};

use svd_expander::DeviceSpec;

use super::schematic::{ClockComponent, ClockSchematic};
use anyhow::{anyhow, Result};

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
          return Err(anyhow!("No field named 'bogus.field' in SVD spec"));
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

#[cfg(test)]
mod tests {
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

    let device_xml = r#"
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

    let device_xml = r#"
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
