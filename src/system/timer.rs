use anyhow::{bail, Result};
use svd_expander::{DeviceSpec, PeripheralSpec};

use super::*;

#[derive(Clone)]
pub struct Timer {
  pub name: Name,
  pub peripheral_enable_field: String,
  pub auto_reload_field: RangedField,
  pub prescaler_field: RangedField,
  pub counter_field: RangedField,
  pub arpe_field: String,
  pub ug_field: String,
  pub cen_field: String,
  pub moe_field: Option<String>,
  pub channels: Vec<TimerChannel>,
}
impl Timer {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Option<Self>> {
    let name = Name::from(&peripheral.name);
    let enable_field_name = format!("{}en", name.snake());

    let rcc = match device
      .peripherals
      .iter()
      .find(|p| p.name.to_lowercase() == "rcc")
    {
      Some(p) => p,
      None => bail!("Could not find RCC peripheral"),
    };

    let mut channels: Vec<TimerChannel> = Vec::new();
    for channel_number in 1..=10 {
      if let Some(tc) = TimerChannel::new(peripheral, channel_number)? {
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
      peripheral_enable_field: try_find_field_in_peripheral(rcc, &enable_field_name)?.path(),
      auto_reload_field: try_find_ranged_field_in_peripheral(peripheral, "arr")?,
      prescaler_field: try_find_ranged_field_in_peripheral(peripheral, "psc")?,
      counter_field: try_find_ranged_field_in_peripheral(peripheral, "cnt")?,
      arpe_field: try_find_field_in_peripheral(peripheral, "arpe")?.path(),
      ug_field: try_find_field_in_peripheral(peripheral, "ug")?.path(),
      cen_field: try_find_field_in_peripheral(peripheral, "cen")?.path(),
      moe_field: find_field_in_peripheral(peripheral, "moe").map(|f| f.path()),
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
  pub fn new(peripheral: &PeripheralSpec, channel_number: u32) -> Result<Option<Self>> {
    let name = Name::from(format!("Ch{}", channel_number,));

    match (
      OutputChannel::new(peripheral, channel_number)?,
      InputChannel::new(peripheral, channel_number)?,
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
  pub enable_path: String,
  pub io_select: Option<EnumField>,
  pub compare_mode: EnumField,
  pub compare_field: RangedField,
  pub preload_path: String,
  pub polarity_path: String,
  pub complement: Option<OutputComplement>,
}
impl OutputChannel {
  pub fn new(peripheral: &PeripheralSpec, channel_number: u32) -> Result<Option<Self>> {
    Ok(Some(Self {
      enable_path: match find_field_in_peripheral(peripheral, &f!("cc{channel_number}e")) {
        Some(f) => f.path(),
        None => return Ok(None),
      },
      io_select: find_enum_field_in_peripheral(peripheral, &f!("cc{channel_number}s")),
      compare_mode: try_find_enum_field_in_peripheral(peripheral, &f!("oc{channel_number}m"))?,
      compare_field: match find_ranged_field_in_peripheral(peripheral, &f!("ccr{channel_number}")) {
        Some(f) => f,
        None => match peripheral
          .iter_registers()
          .find(|r| r.name.to_lowercase() == f!("ccr{channel_number}"))
        {
          Some(r) => try_find_ranged_field_in_register(r, &f!("ccr"))?,
          None => bail!(
            "Could not find Capture/Compare Mode field for {}",
            peripheral.name
          ),
        },
      },
      preload_path: try_find_field_in_peripheral(peripheral, &f!("oc{channel_number}pe"))?.path(),
      polarity_path: try_find_field_in_peripheral(peripheral, &f!("cc{channel_number}p"))?.path(),
      complement: OutputComplement::new(peripheral, channel_number)?,
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
  pub dtg_path: String,
}
impl OutputComplement {
  pub fn new(peripheral: &PeripheralSpec, channel_number: u32) -> Result<Option<Self>> {
    Ok(Some(Self {
      enable_path: match find_field_in_peripheral(peripheral, &f!("cc{channel_number}ne")) {
        Some(f) => f.path(),
        None => return Ok(None),
      },
      polarity_path: try_find_field_in_peripheral(peripheral, &f!("cc{channel_number}np"))?.path(),
      dtg_path: try_find_field_in_peripheral(peripheral, "dtg")?.path(),
    }))
  }
}

#[derive(Clone)]
pub struct InputChannel {
  pub capture_filter: EnumField,
  pub io_select: Option<EnumField>,
  pub capture_field: RangedField,
  pub enable_path: String,
}
impl InputChannel {
  pub fn new(peripheral: &PeripheralSpec, channel_number: u32) -> Result<Option<Self>> {
    Ok(Some(Self {
      capture_filter: match find_enum_field_in_peripheral(peripheral, &f!("ic{channel_number}f")) {
        Some(f) => f,
        None => return Ok(None),
      },
      io_select: find_enum_field_in_peripheral(peripheral, &f!("cc{channel_number}s")),
      capture_field: match find_ranged_field_in_peripheral(peripheral, &f!("ccr{channel_number}")) {
        Some(f) => f,
        None => match peripheral
          .iter_registers()
          .find(|r| r.name.to_lowercase() == f!("ccr{channel_number}"))
        {
          Some(r) => try_find_ranged_field_in_register(r, &f!("ccr"))?,
          None => bail!(
            "Could not find Capture/Compare Mode field for {}",
            peripheral.name
          ),
        },
      },
      enable_path: try_find_field_in_peripheral(peripheral, &f!("cc{channel_number}e"))?.path(),
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
