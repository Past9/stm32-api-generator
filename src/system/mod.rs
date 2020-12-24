use anyhow::Result;
use heck::{CamelCase, SnakeCase};
use svd_expander::{DeviceSpec, EnumeratedValueSpec, FieldSpec};

use self::{gpio::Gpio, timer::Timer};

pub mod gpio;
pub mod timer;

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

  pub fn submodules(&self) -> Vec<Submodule> {
    let mut submodules = self
      .gpios
      .iter()
      .map(|g| g.submodule())
      .chain(self.timers.iter().map(|t| t.submodule()))
      .collect::<Vec<Submodule>>();

    submodules.sort();

    submodules
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

  fn load_timers(&mut self, device: &DeviceSpec) -> Result<()> {
    for peripheral in device
      .peripherals
      .iter()
      .filter(|p| p.name.to_lowercase().starts_with("tim"))
    {
      if let Some(timer) = Timer::new(&self.device, peripheral)? {
        self.timers.push(timer);
      };
    }
    Ok(())
  }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Submodule {
  pub parent_path: String,
  pub name: Name,
}
impl PartialOrd for Submodule {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.name.snake().partial_cmp(&other.name.snake())
  }
}
impl Ord for Submodule {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.name.snake().cmp(&other.name.snake())
  }
}

#[derive(Clone, Eq, PartialEq)]
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
impl PartialOrd for Name {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.snake().partial_cmp(&other.snake())
  }
}
impl Ord for Name {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.snake().cmp(&other.snake())
  }
}

#[derive(Clone)]
pub struct RangedField {
  pub path: String,
  pub min: u32,
  pub max: u32,
}

#[derive(Clone)]
pub struct EnumField {
  pub description: String,
  pub path: String,
  pub name: Name,
  pub values: Vec<EnumValue>,
}
impl EnumField {
  pub fn new(field: &FieldSpec) -> Self {
    Self {
      description: match &field.description {
        Some(d) => d.clone(),
        None => "".to_owned(),
      },
      path: field.path(),
      name: Name::from(&field.name),
      values: field
        .enumerated_value_sets
        .iter()
        .flat_map(|vs| vs.values.iter())
        .filter_map(EnumValue::new)
        .collect::<Vec<EnumValue>>(),
    }
  }

  pub fn clone_values_from(&mut self, other: &EnumField) {
    for value in other.values.iter() {
      self.values.push(value.clone());
    }
  }
}

#[derive(Clone)]
pub struct EnumValue {
  pub description: String,
  pub name: Name,
  pub bit_value: u32,
}
impl EnumValue {
  pub fn new(spec: &EnumeratedValueSpec) -> Option<EnumValue> {
    match spec.actual_value() {
      Some(val) => Some(EnumValue {
        description: match &spec.description {
          Some(d) => d.clone(),
          None => "".to_owned(),
        },
        name: Name::from(&spec.name),
        bit_value: val,
      }),
      None => None,
    }
  }
}
