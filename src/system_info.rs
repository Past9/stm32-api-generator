use anyhow::{anyhow, Result};
use heck::{CamelCase, SnakeCase};
use regex::{Captures, Regex};
use svd_expander::{DeviceSpec, PeripheralSpec, RegisterSpec};

pub struct SystemInfo<'a> {
  pub device: &'a DeviceSpec,
  pub gpios: Vec<Gpio>,
  pub timers: Vec<Timer>,
}
impl<'a> SystemInfo<'a> {
  pub fn new(device: &'a DeviceSpec) -> Result<Self> {
    let mut system_info = Self {
      device,
      gpios: Vec::new(),
      timers: Vec::new(),
    };
    system_info.load_gpios(device)?;
    system_info.load_timers(device)?;

    Ok(system_info)
  }

  fn load_gpios(&mut self, device: &DeviceSpec) -> Result<()> {
    for peripheral in device
      .peripherals
      .iter()
      .filter(|p| p.name.to_lowercase().starts_with("gpio"))
    {
      self.gpios.push(Gpio::new(peripheral)?);
    }
    Ok(())
  }

  fn load_timers(&self, device: &DeviceSpec) -> Result<()> {
    Ok(())
  }
}

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
}

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
    let pin_name = Name::from(f!("P{letter}{number}"));

    let af_register_name = match number {
      0..=7 => "AFRL",
      8..=15 => "AFRH",
      _ => {
        return Err(anyhow!(f!(
          "Pin number {number} out of bounds for alt functions."
        )))
      }
    };

    let mut alt_funcs = Vec::new();

    if let Some(ref afr) = peripheral
      .iter_registers()
      .find(|r| r.name == af_register_name)
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

pub struct AltFunc {
  pub name: Name,
  pub bit_value: u32,
  pub kind: AltFuncKind,
}
impl AltFunc {
  pub fn new_all(number: i32, afr: &RegisterSpec) -> Result<Vec<Self>> {
    let mut alt_funcs: Vec<AltFunc> = Vec::new();

    let generic_name_test = Regex::new(r"^af[0-9]+$/i")?;

    let opt_field = afr
      .fields
      .iter()
      .find(|f| f.name == f!("afrl{number}") || f.name == f!("afrh{number}"));

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

pub enum AltFuncKind {
  TimerChannel(TimerChannel),
  Other,
}

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

pub struct Timer {
  pub name: Name,
  pub channels: Vec<Name>,
}

pub struct Name {
  pub original: String,
}
impl Name {
  pub fn from<S: Into<String>>(s: S) -> Self {
    Self { original: s.into() }
  }

  pub fn camel(&self) -> String {
    self.original.to_camel_case()
  }

  pub fn snake(&self) -> String {
    self.original.to_snake_case()
  }
}
