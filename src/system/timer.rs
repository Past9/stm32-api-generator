use anyhow::{anyhow, Result};
use svd_expander::{DeviceSpec, FieldSpec, PeripheralSpec};

use super::{EnumField, Name, Submodule};
use super::{RangedField, SystemInfo};

#[derive(Clone)]
pub struct Timer {
  pub name: Name,
  pub auto_reload_field: RangedField,
  pub prescaler_field: RangedField,
  pub counter_field: RangedField,
  pub enable_field: String,
  pub channels: Vec<TimerChannel>,
}
impl Timer {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Self> {
    let name = Name::from(&peripheral.name);
    let enable_field_name = format!("{}en", name.snake());

    let mut channels: Vec<TimerChannel> = Vec::new();
    for channel_number in 1..=10 {
      if let Some(tc) = TimerChannel::new(device, peripheral, channel_number)? {
        channels.push(tc);
      }
    }

    Ok(Self {
      name: name.clone(),
      auto_reload_field: Self::find_single_field(peripheral, "arr")?,
      prescaler_field: Self::find_single_field(peripheral, "psc")?,
      counter_field: Self::find_single_field(peripheral, "cnt")?,
      enable_field: match device
        .iter_fields()
        .find(|f| f.name.to_lowercase() == enable_field_name)
      {
        Some(field) => Ok(field.path()),
        None => Err(anyhow!(
          "Could not find timer enable field {}",
          enable_field_name
        )),
      }?,
      channels,
    })
  }

  fn find_single_field(p: &PeripheralSpec, name: &str) -> Result<RangedField> {
    match p.iter_fields().find(|f| f.name.to_lowercase() == name) {
      Some(f) => Ok(RangedField {
        path: f.path().to_lowercase(),
        min: 0,
        max: (2u64.pow(f.width) - 1) as u32,
      }),
      None => Err(anyhow!(
        "Could not find field named '{}' on {}",
        name,
        p.name
      )),
    }
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "timer".to_owned(),
      name: self.name.clone(),
    }
  }
}

#[derive(Clone)]
pub struct TimerChannel {
  pub name: Name,
  pub output: Option<OutputChannel>,
  pub input: Option<InputChannel>,
}
impl TimerChannel {
  pub fn new(
    device: &DeviceSpec,
    peripheral: &PeripheralSpec,
    channel_number: u32,
  ) -> Result<Option<Self>> {
    let name = Name::from(format!("Ch{}", channel_number,));

    match (
      OutputChannel::new(device, peripheral, channel_number)?,
      InputChannel::new(device, peripheral, channel_number)?,
    ) {
      (None, None) => Ok(None),
      (output, input) => Ok(Some(Self {
        name,
        output,
        input,
      })),
    }
  }

  pub fn is_output(&self) -> bool {
    self.output.is_some()
  }

  pub fn as_output(&self) -> OutputChannel {
    match self.output {
      Some(ref output) => output.clone(),
      None => panic!("{} is not an output channel", self.name.camel()),
    }
  }

  pub fn is_input(&self) -> bool {
    self.input.is_some()
  }

  pub fn as_input(&self) -> InputChannel {
    match self.input {
      Some(ref input) => input.clone(),
      None => panic!("{} is not an input channel", self.name.camel()),
    }
  }
}

#[derive(Clone)]
pub struct OutputChannel {
  io_select: Option<EnumField>,
  compare_mode_field: EnumField,
}
impl OutputChannel {
  pub fn new(
    device: &DeviceSpec,
    peripheral: &PeripheralSpec,
    channel_number: u32,
  ) -> Result<Option<Self>> {
    let (ccmr_path, compare_mode_field) = match peripheral
      .iter_fields()
      .find(|f| f.name.to_lowercase() == f!("oc{channel_number}m"))
      .map(|f| (f, EnumField::new(f)))
    {
      Some((raw_f, f)) => (raw_f.parent_path(), f),
      None => {
        return Ok(None);
      }
    };

    let io_select = device
      .get_register(&ccmr_path)?
      .fields
      .iter()
      .find(|f| f.name.to_lowercase() == format!("cc{}s", channel_number))
      .map(|f| EnumField::new(f));

    Ok(Some(Self {
      io_select,
      compare_mode_field,
    }))
  }

  pub fn has_io_select(&self) -> bool {
    self.io_select.is_some()
  }

  pub fn io_select(&self) -> EnumField {
    match self.io_select {
      Some(ref f) => f.clone(),
      None => panic!("Channel output mode does not have an I/O mode select field"),
    }
  }
}

#[derive(Clone)]
pub struct InputChannel {
  io_select: Option<EnumField>,
  compare_mode_field: EnumField,
}
impl InputChannel {
  pub fn new(
    device: &DeviceSpec,
    peripheral: &PeripheralSpec,
    channel_number: u32,
  ) -> Result<Option<Self>> {
    let (ccmr_path, compare_mode_field) = match peripheral
      .iter_fields()
      .find(|f| f.name.to_lowercase() == f!("ic{channel_number}f"))
      .map(|f| (f, EnumField::new(f)))
    {
      Some((raw_f, f)) => (raw_f.parent_path(), f),
      None => {
        return Ok(None);
      }
    };

    let io_select = device
      .get_register(&ccmr_path)?
      .fields
      .iter()
      .find(|f| f.name.to_lowercase() == format!("cc{}s", channel_number))
      .map(|f| EnumField::new(f));

    Ok(Some(Self {
      io_select,
      compare_mode_field,
    }))
  }

  pub fn has_io_select(&self) -> bool {
    self.io_select.is_some()
  }

  pub fn io_select(&self) -> EnumField {
    match self.io_select {
      Some(ref f) => f.clone(),
      None => panic!("Channel input mode does not have an I/O mode select field"),
    }
  }
}
