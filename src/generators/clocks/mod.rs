use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClockSchematic {
  oscillators: HashMap<String, Oscillator>,
  demuxers: HashMap<String, Demux>,
  dividers: HashMap<String, Divider>,
  multipliers: HashMap<String, Multiplier>,
  taps: HashMap<String, Tap>,
}

#[derive(Deserialize)]
pub struct Oscillator {
  frequency: u64,
}

#[derive(Deserialize)]
pub struct Demux {
  inputs: Vec<String>,
  default: String,
}

#[derive(Deserialize)]
pub struct Divider {
  input: String,
  default: u64,
  values: Vec<u64>,
}

#[derive(Deserialize)]
pub struct Multiplier {
  input: String,
  default: u64,
  values: Vec<u64>,
}

#[derive(Deserialize)]
pub struct Tap {
  input: String,
  max: u64,
}

#[cfg(test)]
mod tests {
  use super::*;
  use ron;

  static spec_str: &'static str = r#"
        ClockSchematic(
            oscillators: {
                "Hse": (
                    frequency: 8000000
                )
            },
            demuxers: {
                "PllSourceMux": (
                    inputs: [ "Hse" ]
                )
            },
            dividers: {
                "HseToPllSourceMux": (
                    input: "PllSourceMux",
                    default: 1,
                    values: [1]
                )
            },
            multipliers: {
                "PllMul": (
                    input: "Hse", 
                    default: 3,
                    values: [2,3,4]
                )
            }
        )
    "#;

  #[test]
  fn deserializes_spec_string() {
    let spec: ClockSchematic = ron::from_str(spec_str).unwrap();

    assert_eq!(1, spec.oscillators.len());
    assert_eq!(8_000_000, spec.oscillators["Hse"].frequency);

    assert_eq!(1, spec.demuxers.len());
    assert_eq!(1, spec.demuxers["PllSourceMux"].inputs.len());
    assert_eq!("Hse", spec.demuxers["PllSourceMux"].inputs[0]);

    assert_eq!(1, spec.dividers.len());
    assert_eq!("PllSourceMux", spec.dividers["HseToPllSourceMux"].input);
    assert_eq!(1, spec.dividers["HseToPllSourceMux"].default);
    assert_eq!(1, spec.dividers["HseToPllSourceMux"].values.len());
    assert_eq!(1, spec.dividers["HseToPllSourceMux"].values[0]);

    assert_eq!(1, spec.multipliers.len());
    assert_eq!("Hse", spec.multipliers["PllMul"].input);
    assert_eq!(3, spec.multipliers["PllMul"].default);
    assert_eq!(3, spec.multipliers["PllMul"].values.len());
    assert_eq!(2, spec.multipliers["PllMul"].values[0]);
    assert_eq!(3, spec.multipliers["PllMul"].values[1]);
    assert_eq!(4, spec.multipliers["PllMul"].values[2]);
  }
}
