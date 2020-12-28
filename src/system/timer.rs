use anyhow::{anyhow, Result};
use svd_expander::{DeviceSpec, PeripheralSpec};

use super::RangedField;
use super::{EnumField, Name, Submodule};

#[derive(Clone)]
pub struct Timer {
  pub name: Name,
  pub auto_reload_field: RangedField,
  pub prescaler_field: RangedField,
  pub counter_field: RangedField,
  pub arpe_field: String,
  pub ug_field: String,
  pub cen_field: String,
  pub moe_field: Option<String>,
  pub peripheral_enable_field: String,
  pub channels: Vec<TimerChannel>,
}
impl Timer {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Option<Self>> {
    let name = Name::from(&peripheral.name);
    let enable_field_name = format!("{}en", name.snake());

    let mut channels: Vec<TimerChannel> = Vec::new();
    for channel_number in 1..=10 {
      if let Some(tc) = TimerChannel::new(device, peripheral, channel_number)? {
        channels.push(tc);
      }
    }

    // Fill in empty compare mode enums in case the SVD doesn't have proper inheritance
    if let Some(good_enum) = channels
      .iter()
      .find(|c| c.is_output() && c.as_output().compare_mode.values.len() > 0)
      .map(|c| c.as_output().compare_mode.clone())
    {
      for channel in channels
        .iter_mut()
        .filter(|c| c.is_output() && c.as_output().compare_mode.values.len() == 0)
      {
        channel
          .as_output_mut()
          .compare_mode
          .clone_values_from(&good_enum);
      }
    } else {
      if channels.iter().filter(|c| c.is_output()).count() > 0 {
        warn!("Skipping timer {} because it has output channels but none of them have enumerated compare mode values.", name.camel());
        return Ok(None);
      }
    }

    // Fill in empty capture filter enums in case the SVD doesn't have proper inheritance
    if let Some(good_enum) = channels
      .iter()
      .find(|c| c.is_input() && c.as_input().capture_filter.values.len() > 0)
      .map(|c| c.as_input().capture_filter.clone())
    {
      for channel in channels
        .iter_mut()
        .filter(|c| c.is_input() && c.as_input().capture_filter.values.len() == 0)
      {
        channel
          .as_input_mut()
          .capture_filter
          .clone_values_from(&good_enum);
      }
    } else {
      if channels.iter().filter(|c| c.is_input()).count() > 0 {
        warn!("Skipping timer {} because it has input channels but none of them have enumerated capture filter values.", name.camel());
        return Ok(None);
      }
    }

    Ok(Some(Self {
      name: name.clone(),
      auto_reload_field: try_find_ranged_field(peripheral, "arr")?,
      prescaler_field: try_find_ranged_field(peripheral, "psc")?,
      counter_field: try_find_ranged_field(peripheral, "cnt")?,
      arpe_field: try_find_field_path(peripheral, "arpe")?,
      ug_field: try_find_field_path(peripheral, "ug")?,
      cen_field: try_find_field_path(peripheral, "cen")?,
      moe_field: find_field_path(peripheral, "moe"),
      peripheral_enable_field: match device
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
    }))
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "timer".to_owned(),
      name: self.name.clone(),
      needs_clocks: true,
    }
  }

  pub fn has_moe_field(&self) -> bool {
    self.moe_field.is_some()
  }

  pub fn moe_field(&self) -> String {
    match self.moe_field {
      Some(ref f) => f.clone(),
      None => panic!(
        "Timer {} has no MOE (Main Output Enable) field.",
        self.name.camel()
      ),
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

  pub fn as_output(&self) -> &OutputChannel {
    match self.output {
      Some(ref output) => output,
      None => panic!("{} is not an output channel", self.name.camel()),
    }
  }

  pub fn as_output_mut(&mut self) -> &mut OutputChannel {
    match self.output {
      Some(ref mut output) => output,
      None => panic!("{} is not an output channel", self.name.camel()),
    }
  }

  pub fn is_input(&self) -> bool {
    self.input.is_some()
  }

  pub fn as_input(&self) -> &InputChannel {
    match self.input {
      Some(ref input) => input,
      None => panic!("{} is not an input channel", self.name.camel()),
    }
  }

  pub fn as_input_mut(&mut self) -> &mut InputChannel {
    match self.input {
      Some(ref mut input) => input,
      None => panic!("{} is not an input channel", self.name.camel()),
    }
  }
}

#[derive(Clone)]
pub struct OutputChannel {
  pub io_select: Option<EnumField>,
  pub compare_mode: EnumField,
  pub preload_path: String,
  pub compare_field: RangedField,
  pub enable_path: String,
  pub polarity_path: String,
  pub complement: Option<OutputComplement>,
}
impl OutputChannel {
  pub fn new(
    device: &DeviceSpec,
    peripheral: &PeripheralSpec,
    channel_number: u32,
  ) -> Result<Option<Self>> {
    let (ccmr_path, compare_mode) = match peripheral
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
      .find(|f| f.name.to_lowercase() == f!("cc{channel_number}s"))
      .map(|f| EnumField::new(f));

    Ok(Some(Self {
      io_select,
      compare_mode,
      preload_path: format!("{}.oc{}pe", ccmr_path, channel_number),
      compare_field: match find_ranged_field_in_register(
        peripheral,
        &f!("ccr{channel_number}"),
        "ccr",
      ) {
        Some(f) => f,
        None => try_find_ranged_field(peripheral, &f!("ccr{channel_number}"))?,
      },
      enable_path: try_find_field_path(peripheral, &f!("cc{channel_number}e"))?,
      polarity_path: try_find_field_path(peripheral, &f!("cc{channel_number}p"))?,
      complement: OutputComplement::new(device, peripheral, channel_number)?,
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

  pub fn has_complement(&self) -> bool {
    self.complement.is_some()
  }

  pub fn complement(&self) -> &OutputComplement {
    match self.complement {
      Some(ref c) => &c,
      None => panic!("Output does not have a complementary channel"),
    }
  }
}

#[derive(Clone)]
pub struct OutputComplement {
  pub enable_path: String,
  pub polarity_path: String,
}
impl OutputComplement {
  pub fn new(
    device: &DeviceSpec,
    peripheral: &PeripheralSpec,
    channel_number: u32,
  ) -> Result<Option<Self>> {
    Ok(Some(Self {
      enable_path: match find_field_path(peripheral, &f!("cc{channel_number}ne")) {
        Some(path) => path,
        None => return Ok(None),
      },
      polarity_path: try_find_field_path(peripheral, &f!("cc{channel_number}np"))?,
    }))
  }
}

#[derive(Clone)]
pub struct InputChannel {
  pub io_select: Option<EnumField>,
  pub capture_filter: EnumField,
  pub capture_field: RangedField,
  pub enable_path: String,
}
impl InputChannel {
  pub fn new(
    device: &DeviceSpec,
    peripheral: &PeripheralSpec,
    channel_number: u32,
  ) -> Result<Option<Self>> {
    let (ccmr_path, capture_filter) = match peripheral
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
      capture_filter,
      capture_field: match find_ranged_field_in_register(
        peripheral,
        &f!("ccr{channel_number}"),
        "ccr",
      ) {
        Some(f) => f,
        None => try_find_ranged_field(peripheral, &f!("ccr{channel_number}"))?,
      },
      enable_path: try_find_field_path(peripheral, &f!("cc{channel_number}e"))?,
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

fn find_field_path(p: &PeripheralSpec, name: &str) -> Option<String> {
  match p.iter_fields().find(|f| f.name.to_lowercase() == name) {
    Some(f) => Some(f.path()),
    None => None,
  }
}

fn try_find_field_path(p: &PeripheralSpec, name: &str) -> Result<String> {
  match find_field_path(p, name) {
    Some(p) => Ok(p),
    None => Err(anyhow!(
      "Could not find field named '{}' on {}",
      name,
      p.name
    )),
  }
}

fn find_ranged_field_in_register(
  p: &PeripheralSpec,
  register_name: &str,
  field_name: &str,
) -> Option<RangedField> {
  match p
    .iter_registers()
    .find(|r| r.name.to_lowercase() == register_name)
  {
    Some(r) => match r
      .fields
      .iter()
      .find(|f| f.name.to_lowercase() == field_name)
    {
      Some(f) => Some(RangedField {
        path: f.path().to_lowercase(),
        min: 0,
        max: (2u64.pow(f.width) - 1) as u32,
      }),
      None => None,
    },
    None => None,
  }
}

fn try_find_ranged_field_in_register(
  p: &PeripheralSpec,
  register_name: &str,
  field_name: &str,
) -> Result<RangedField> {
  match find_ranged_field_in_register(p, register_name, field_name) {
    Some(f) => Ok(f),
    None => Err(anyhow!(
      "Could not find field named '{}' in register {} on peripheral {}",
      field_name,
      register_name,
      p.name
    )),
  }
}

fn find_ranged_field(p: &PeripheralSpec, name: &str) -> Option<RangedField> {
  match p.iter_fields().find(|f| f.name.to_lowercase() == name) {
    Some(f) => Some(RangedField {
      path: f.path().to_lowercase(),
      min: 0,
      max: (2u64.pow(f.width) - 1) as u32,
    }),
    None => None,
  }
}

fn try_find_ranged_field(p: &PeripheralSpec, name: &str) -> Result<RangedField> {
  match find_ranged_field(p, name) {
    Some(f) => Ok(f),
    None => Err(anyhow!(
      "Could not find field named '{}' on {}",
      name,
      p.name
    )),
  }
}
