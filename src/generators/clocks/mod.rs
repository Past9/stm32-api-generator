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

  #[cfg(test)]
  pub fn from_ron<S: Into<String>>(ron: S, spec: &'a DeviceSpec) -> Result<ClockGenerator<'a>> {
    let generator = ClockGenerator {
      spec,
      schematic: ClockSchematic::from_ron(ron)?,
    };
    generator.validate()?;
    Ok(generator)
  }

  pub fn generate(&self, out_dir: &OutputDirectory) -> Result<()> {
    let clocks_file = ClocksTemplate::new(&self.schematic, &self.spec)?.render()?;

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
      .filter_map(|c| match c {
        ClockComponent::Multiplexer(m) => Some(m.path.clone()),
        ClockComponent::Divider(d) => match d.is_fixed_value() {
          true => None,
          false => Some(d.path.clone()),
        },
        ClockComponent::Multiplier(m) => match m.is_fixed_value() {
          true => None,
          false => Some(m.path.clone()),
        },
        _ => None,
      })
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
        ClockComponent::Multiplexer(m) => m
          .inputs
          .values()
          .map(|i| (m.path.clone(), i.bit_value, i.name.clone()))
          .collect::<Vec<(String, u32, String)>>(),
        ClockComponent::Divider(d) => d
          .values
          .values()
          .filter_map(|v| match d.is_fixed_value() {
            true => None,
            false => Some((d.path.clone(), v.bit_value, v.name.clone())),
          })
          .collect::<Vec<(String, u32, String)>>(),
        ClockComponent::Multiplier(m) => m
          .values
          .values()
          .filter_map(|v| match m.is_fixed_value() {
            true => None,
            false => Some((m.path.clone(), v.bit_value, v.name.clone())),
          })
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
  use crate::generators::ReadWrite;
  use crate::{clear_bit, is_set, read_val, set_bit, wait_for_clear, wait_for_set, write_val};
  use anyhow::Result;
  use askama::Template;
  use fstrings::f;
  use heck::{CamelCase, SnakeCase};
  use svd_expander::DeviceSpec;

  #[derive(Template)]
  #[template(path = "clocks/mod.rs.askama", escape = "none")]
  pub struct ClocksTemplate<'a> {
    device: &'a DeviceSpec,
    sys_clk_mux: Mux,
    flash_latency: FlashLat,
    oscillators: Vec<Osc>,
    multiplexers: Vec<Mux>,
    variable_dividers: Vec<VarDiv>,
    variable_multipliers: Vec<VarMul>,
    fixed_dividers: Vec<FixedDiv>,
    fixed_multipliers: Vec<FixedMul>,
    taps: Vec<Tap>,
    has_pll: bool,
    pll_power: String,
    pll_ready: String,
  }
  impl<'a> ClocksTemplate<'a> {
    pub fn new(schematic: &ClockSchematic, spec: &'a DeviceSpec) -> Result<ClocksTemplate<'a>> {
      let mut clocks = ClocksTemplate {
        device: spec,
        sys_clk_mux: Mux::new(schematic.get_sys_clk_mux()?)?,
        flash_latency: FlashLat::new(schematic.flash_latency()),
        oscillators: schematic.oscillators().map(|o| Osc::new(o)).collect(),
        multiplexers: schematic
          .multiplexers()
          .map(|m| Mux::new(m))
          .collect::<Result<Vec<Mux>>>()?,
        variable_dividers: schematic
          .dividers()
          .filter(|v| !v.is_fixed_value())
          .map(|v| VarDiv::new(v))
          .collect::<Result<Vec<VarDiv>>>()?,
        variable_multipliers: schematic
          .multipliers()
          .filter(|v| !v.is_fixed_value())
          .map(|v| VarMul::new(v))
          .collect::<Result<Vec<VarMul>>>()?,
        fixed_dividers: schematic
          .dividers()
          .filter(|v| v.is_fixed_value())
          .map(|v| FixedDiv::new(v))
          .collect::<Result<Vec<FixedDiv>>>()?,
        fixed_multipliers: schematic
          .multipliers()
          .filter(|v| v.is_fixed_value())
          .map(|v| FixedMul::new(v))
          .collect::<Result<Vec<FixedMul>>>()?,
        taps: schematic
          .taps()
          .map(|v| Tap::new(v))
          .collect::<Result<Vec<Tap>>>()?,
        has_pll: schematic.pll().is_some(),
        pll_power: match schematic.pll() {
          Some(p) => &p.power,
          None => "",
        }
        .to_owned(),
        pll_ready: match schematic.pll() {
          Some(p) => &p.ready,
          None => "",
        }
        .to_owned(),
      };

      clocks.flash_latency.ranges.sort_by_key(|r| r.bit_value);
      clocks.oscillators.sort_by_key(|o| o.name.clone());
      clocks.multiplexers.sort_by_key(|o| o.field_name.clone());
      clocks
        .variable_dividers
        .sort_by_key(|o| o.field_name.clone());
      clocks
        .variable_multipliers
        .sort_by_key(|o| o.field_name.clone());
      clocks.fixed_dividers.sort_by_key(|o| o.field_name.clone());
      clocks
        .fixed_multipliers
        .sort_by_key(|o| o.field_name.clone());
      clocks.taps.sort_by_key(|o| o.field_name.clone());

      Ok(clocks)
    }
  }

  pub struct FlashLat {
    path: String,
    ranges: Vec<LatencyRange>,
  }
  impl FlashLat {
    pub fn new(flash_latency: &schematic::FlashLatency) -> FlashLat {
      FlashLat {
        path: flash_latency.path.clone(),
        ranges: flash_latency
          .ranges
          .values()
          .map(|r| LatencyRange::new(r))
          .collect(),
      }
    }
  }

  pub struct LatencyRange {
    has_min: bool,
    min_code: String,
    has_max: bool,
    max_code: String,
    bit_value: u32,
  }
  impl LatencyRange {
    pub fn new(range: &schematic::FlashLatencyRange) -> LatencyRange {
      LatencyRange {
        has_min: range.min.is_some(),
        min_code: match range.min {
          Some(min) => f!("freq >= {min}f32"),
          None => "".to_owned(),
        },
        has_max: range.max.is_some(),
        max_code: match range.max {
          Some(max) => f!("freq <= {max}f32"),
          None => "".to_owned(),
        },
        bit_value: range.bit_value,
      }
    }
  }

  pub struct Osc {
    name: String,
    default_freq: u64,
    is_external: bool,
    ext_power: String,
    ext_ready: String,
    ext_bypass: String,
  }
  impl Osc {
    pub fn new(oscillator: &schematic::Oscillator) -> Osc {
      let ext_vals = match oscillator.external {
        Some(ref ext) => (
          true,
          ext.power.clone(),
          ext.ready.clone(),
          ext.bypass.clone(),
        ),
        None => (false, "".to_owned(), "".to_owned(), "".to_owned()),
      };

      Osc {
        name: oscillator.name.to_snake_case(),
        default_freq: oscillator.frequency,
        is_external: ext_vals.0,
        ext_power: ext_vals.1,
        ext_ready: ext_vals.2,
        ext_bypass: ext_vals.3,
      }
    }
  }

  pub struct Mux {
    struct_name: String,
    field_name: String,
    inputs: Vec<MuxIn>,
    default: MuxIn,
    path: String,
    is_sys_clk_mux: bool,
  }
  impl Mux {
    pub fn new(multiplexer: &schematic::Multiplexer) -> Result<Mux> {
      let default_input = multiplexer.default_input()?;

      let mut mux = Mux {
        struct_name: multiplexer.name.to_camel_case(),
        field_name: multiplexer.name.to_snake_case(),
        inputs: multiplexer
          .inputs
          .values()
          .map(|v| MuxIn::new(&v))
          .collect::<Vec<MuxIn>>(),
        default: MuxIn::new(&default_input),
        path: multiplexer.path.clone(),
        is_sys_clk_mux: multiplexer.is_sys_clk_mux,
      };

      mux.inputs.sort_by_key(|m| m.bit_value);

      Ok(mux)
    }
  }

  pub struct MuxIn {
    struct_name: String,
    real_field_name: String,
    bit_value: u32,
    is_off: bool,
  }
  impl MuxIn {
    pub fn new(input: &schematic::MultiplexerInput) -> MuxIn {
      MuxIn {
        struct_name: input.public_name().to_camel_case(),
        real_field_name: input.name.to_snake_case(),
        bit_value: input.bit_value,
        is_off: input.public_name() == "off",
      }
    }
  }

  pub struct FixedMul {
    field_name: String,
    factor: f32,
    input_field_name: String,
  }
  impl FixedMul {
    pub fn new(multiplier: &schematic::Multiplier) -> Result<FixedMul> {
      Ok(FixedMul {
        field_name: multiplier.name.to_snake_case(),
        factor: multiplier.default,
        input_field_name: multiplier.input.clone(),
      })
    }
  }

  pub struct VarDiv {
    struct_name: String,
    field_name: String,
    options: Vec<DivOpt>,
    default: DivOpt,
    input_field_name: String,
    path: String,
  }
  impl VarDiv {
    pub fn new(divider: &schematic::Divider) -> Result<VarDiv> {
      let default_input = divider.default_input()?;

      let mut div = VarDiv {
        struct_name: divider.name.to_camel_case(),
        field_name: divider.name.to_snake_case(),
        options: divider
          .values
          .values()
          .map(|v| DivOpt::new(&v))
          .collect::<Result<Vec<DivOpt>>>()?,
        default: DivOpt::new(&default_input)?,
        input_field_name: divider.input.clone(),
        path: divider.path.clone(),
      };

      div.options.sort_by_key(|d| d.bit_value);

      Ok(div)
    }
  }

  pub struct DivOpt {
    struct_name: String,
    bit_value: u32,
    divisor: f32,
  }
  impl DivOpt {
    pub fn new(option: &schematic::DividerOption) -> Result<DivOpt> {
      Ok(DivOpt {
        struct_name: option.name.to_camel_case(),
        bit_value: option.bit_value,
        divisor: option.divisor,
      })
    }
  }

  pub struct VarMul {
    struct_name: String,
    field_name: String,
    options: Vec<MulOpt>,
    default: MulOpt,
    input_field_name: String,
    path: String,
  }
  impl VarMul {
    pub fn new(multiplier: &schematic::Multiplier) -> Result<VarMul> {
      let default_input = multiplier.default_input()?;

      let mut mul = VarMul {
        struct_name: multiplier.name.to_camel_case(),
        field_name: multiplier.name.to_snake_case(),
        options: multiplier
          .values
          .values()
          .map(|v| MulOpt::new(v))
          .collect::<Result<Vec<MulOpt>>>()?,
        default: MulOpt::new(&default_input)?,
        input_field_name: multiplier.input.clone(),
        path: multiplier.path.clone(),
      };

      mul.options.sort_by_key(|m| m.bit_value);

      Ok(mul)
    }
  }

  pub struct MulOpt {
    struct_name: String,
    bit_value: u32,
    factor: f32,
  }
  impl MulOpt {
    pub fn new(option: &schematic::MultiplierOption) -> Result<MulOpt> {
      Ok(MulOpt {
        struct_name: option.name.to_camel_case(),
        bit_value: option.bit_value,
        factor: option.factor,
      })
    }
  }

  pub struct FixedDiv {
    field_name: String,
    divisor: f32,
    input_field_name: String,
  }
  impl FixedDiv {
    pub fn new(divider: &schematic::Divider) -> Result<FixedDiv> {
      Ok(FixedDiv {
        field_name: divider.name.to_snake_case(),
        divisor: divider.default,
        input_field_name: divider.input.clone(),
      })
    }
  }

  pub struct Tap {
    field_name: String,
    input_field_name: String,
  }
  impl Tap {
    pub fn new(tap: &schematic::Tap) -> Result<Tap> {
      Ok(Tap {
        field_name: tap.name.to_snake_case(),
        input_field_name: tap.input.clone(),
      })
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
            path: "timer0.cr.en",
            values: {
              "no_div": (
                divisor: 1, 
                bit_value: 0
              )
            },
            default: 1,
          )
        },
        multipliers: {
          "pll_mul": (
            input: "pll_div", 
            path: "bogus.field",
            values: {
              "no_mul": (
                factor: 2, 
                bit_value: 1
              )
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

    let device = DeviceSpec::from_file("specs/svd/arm_device.svd").unwrap();
    let res = ClockGenerator::from_ron(clock_ron, &device);

    assert!(res.is_err());
    assert_eq!(
      "No field named 'bogus.field' in SVD spec",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn allows_blank_paths_on_fixed_muls_and_divs() {
    let clock_ron = r#"
      ClockSchematic(
        oscillators: {
          "hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {
          "fixed_div": (
            input: "hse",
            default: 1,
          )
        },
        multipliers: {
          "fixed_mul": (
            input: "fixed_div", 
            default: 2,
          )
        },
        taps: {
          "tap1": (
            input: "fixed_mul", 
            max: 1000000, 
            terminal: true
          ),
        }
      )
    "#;

    let device = DeviceSpec::from_file("specs/svd/arm_device.svd").unwrap();
    let res = ClockGenerator::from_ron(clock_ron, &device);

    assert!(res.is_ok());
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
            path: "timer0.cr.mode",
            values: {
              "pll_div": (
                divisor: 1, 
                bit_value: 15
              )
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

    let device = DeviceSpec::from_file("specs/svd/arm_device.svd").unwrap();
    let res = ClockGenerator::from_ron(clock_ron, &device);

    assert!(res.is_err());
    assert_eq!(
      "Bit value '15' does not fit in 3-bit field 'timer0.cr.mode' (pll_div)",
      res.unwrap_err().to_string()
    );
  }
}
