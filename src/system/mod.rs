use anyhow::Result;
use heck::{CamelCase, SnakeCase};
use svd_expander::DeviceSpec;

use self::{
  gpio::Gpio,
  gpio::{AltFuncKind, TimerChannel},
  timer::Timer,
};

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

  fn all_timer_channels(&self) -> Vec<TimerChannel> {
    let mut channels = self
      .gpios
      .iter()
      .flat_map(|g| g.pins.iter())
      .flat_map(|p| p.alt_funcs.iter())
      .filter_map(|f| match &f.kind {
        AltFuncKind::Other => None,
        AltFuncKind::TimerChannel(tc) => Some(tc.clone()),
      })
      .collect::<Vec<TimerChannel>>();

    channels.sort();
    channels.dedup();

    channels
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
      self
        .timers
        .push(Timer::new(peripheral, self.all_timer_channels())?);
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
