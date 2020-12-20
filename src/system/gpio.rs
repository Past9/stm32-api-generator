use anyhow::{anyhow, Result};
use regex::{Captures, Regex};
use svd_expander::{PeripheralSpec, RegisterSpec};

use super::{Name, Submodule};

#[derive(Clone)]
pub struct Gpio {
  pub name: Name,
  pub pins: Vec<Pin>,
  pub enable_field: String,
}
impl Gpio {
  pub fn new(peripheral: &PeripheralSpec) -> Result<Self> {
    let letter = match peripheral.name.to_lowercase().chars().nth(4) {
      Some(l) => l,
      None => {
        return Err(anyhow!(
          "Peripheral '{}' is not named as expected for a GPIO peripheral (i.e. 'GPIOA')",
          peripheral.name
        ))
      }
    };

    Ok(Self {
      name: Name::from(f!("gpio_{letter}")),
      pins: Pin::new_all(&letter, peripheral)?,
      enable_field: f!("rcc.ahbenr.iop{letter}en").to_owned(),
    })
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "gpio".to_owned(),
      name: self.name.clone(),
    }
  }
}

#[derive(Clone)]
pub struct Pin {
  pub name: Name,
  pub alt_funcs: Vec<AltFunc>,
  pub afr_field: String,
  pub moder_field: String,
  pub pupdr_field: String,
  pub otyper_field: String,
  pub ospeedr_field: String,
  pub odr_field: String,
  pub idr_field: String,
}
impl Pin {
  pub fn new_all(letter: &char, peripheral: &PeripheralSpec) -> Result<Vec<Self>> {
    Ok(
      (0..16)
        .map(|n| Pin::new(letter, n, peripheral))
        .collect::<Result<Vec<Self>>>()?,
    )
  }

  pub fn new(letter: &char, number: i32, peripheral: &PeripheralSpec) -> Result<Self> {
    let pin_name = Name::from(f!("p{letter}{number}"));

    let af_register_name = match number {
      0..=7 => "afrl",
      8..=15 => "afrh",
      _ => {
        return Err(anyhow!(f!(
          "Pin number {number} out of bounds for alt functions."
        )))
      }
    };

    let mut alt_funcs = Vec::new();

    if let Some(ref afr) = peripheral
      .iter_registers()
      .find(|r| r.name.to_lowercase() == af_register_name)
    {
      alt_funcs.extend(AltFunc::new_all(number, &afr)?);
    }

    Ok(Self {
      name: pin_name,
      alt_funcs,
      afr_field: f!("gpio{letter}.{af_register_name}.{af_register_name}{number}"),
      moder_field: f!("gpio{letter}.moder.moder{number}"),
      pupdr_field: f!("gpio{letter}.pupdr.pupdr{number}"),
      otyper_field: f!("gpio{letter}.otyper.ot{number}"),
      ospeedr_field: f!("gpio{letter}.ospeedr.ospeedr{number}"),
      odr_field: f!("gpio{letter}.odr.odr{number}"),
      idr_field: f!("gpio{letter}.idr.idr{number}"),
    })
  }
}

#[derive(Clone)]
pub struct AltFunc {
  pub name: Name,
  pub bit_value: u32,
  pub kind: AltFuncKind,
}
impl AltFunc {
  pub fn new_all(number: i32, afr: &RegisterSpec) -> Result<Vec<Self>> {
    let mut alt_funcs: Vec<AltFunc> = Vec::new();

    let generic_name_test = Regex::new(r"^af[0-9]+$/i")?;

    let opt_field = afr.fields.iter().find(|f| {
      f.name.to_lowercase() == f!("afrl{number}") || f.name.to_lowercase() == f!("afrh{number}")
    });

    if let Some(field) = opt_field {
      for enum_val in field
        .enumerated_value_sets
        .iter()
        .flat_map(|vs| vs.values.iter())
      {
        if let Some(ref v) = enum_val.actual_value() {
          let mut name = enum_val.name.clone();
          if let Some(ref description) = enum_val.description {
            name = description.clone()
          }

          let alt_func = match TimerChannel::try_new(number, &name)? {
            Some(tc) => Some(Self {
              name: Name::from(name),
              bit_value: *v,
              kind: AltFuncKind::TimerChannel(tc),
            }),
            None => match generic_name_test.is_match(&name) {
              true => None,
              false => Some(Self {
                name: Name::from(name),
                bit_value: *v,
                kind: AltFuncKind::Other,
              }),
            },
          };

          if let Some(af) = alt_func {
            alt_funcs.push(af);
          }
        }
      }
    }

    Ok(alt_funcs)
  }
}

#[derive(Clone)]
pub enum AltFuncKind {
  TimerChannel(TimerChannel),
  Other,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TimerChannel {
  pub timer: Name,
  pub channel: Name,
}
impl TimerChannel {
  pub fn try_new(pin_number: i32, af_name: &str) -> Result<Option<Self>> {
    let timer_channel_name_test = Regex::new(r"^(tim[0-9]+)_(ch[0-9]n?$)/i")?;

    let timer_channel = match timer_channel_name_test.is_match(&af_name) {
      true => {
        let captures = timer_channel_name_test
          .captures_iter(&af_name)
          .collect::<Vec<Captures>>();

        if captures.len() == 0 {
          return Err(anyhow!(
            "Could not parse timer channel alt func '{}' for pin {}",
            af_name,
            pin_number
          ));
        } else if captures.len() > 1 {
          return Err(anyhow!(
            "Multiple timer channel names found in alt func name '{}' for pin {}",
            af_name,
            pin_number
          ));
        } else {
          let timer_name = match captures[0].get(1) {
            Some(c) => c.as_str().to_owned(),
            None => {
              return Err(anyhow!(
                "Could not find timer name in '{}' for pin {}",
                af_name,
                pin_number
              ));
            }
          };

          let channel_name = match captures[0].get(2) {
            Some(c) => c.as_str().to_owned(),
            None => {
              return Err(anyhow!(
                "Could not find channel name in '{}' for pin {}",
                af_name,
                pin_number
              ));
            }
          };

          Some(Self {
            timer: Name::from(timer_name),
            channel: Name::from(channel_name),
          })
        }
      }
      false => None,
    };

    Ok(timer_channel)
  }
}
impl PartialOrd for TimerChannel {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    match self.timer.partial_cmp(&other.timer) {
      None => None,
      Some(ord) => match ord {
        std::cmp::Ordering::Less => Some(std::cmp::Ordering::Less),
        std::cmp::Ordering::Equal => self.channel.partial_cmp(&other.channel),
        std::cmp::Ordering::Greater => Some(std::cmp::Ordering::Greater),
      },
    }
  }
}
impl Ord for TimerChannel {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    match self.timer.cmp(&other.timer) {
      std::cmp::Ordering::Less => std::cmp::Ordering::Less,
      std::cmp::Ordering::Equal => self.channel.cmp(&other.channel),
      std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    }
  }
}
