use anyhow::{bail, Result};
use svd_expander::{DeviceSpec, PeripheralSpec};

use super::*;

pub struct Spi {
  pub name: Name,
  pub struct_name: Name,
  pub number: String,
  pub peripheral_enable_field: String,
  pub i2smod_field: String,
  pub spe_field: String,
  pub br_field: EnumField,
  pub cpol_field: String,
  pub cpha_field: String,
  pub rxonly_field: String,
  pub bidimode_field: String,
  pub bidioe_field: String,
  pub nssp_field: String,
  pub lsbfirst_field: String,
  pub crcl_field: String,
  pub crcen_field: String,
  pub ssm_field: String,
  pub ssi_field: String,
  pub mstr_field: String,

  pub ds_field: String,
  pub ssoe_field: String,
  pub frf_field: String,
  pub frxth_field: String,
  pub ldma_tx_field: String,
  pub ldma_rx_field: String,

  pub dr_field: String,

  pub bsy_field: String,
}
impl Spi {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Self> {
    let name = Name::from(&peripheral.name);

    let number = match &peripheral.name.chars().last() {
      Some(n) => n.to_string(),
      None => bail!("Could not determine SPI number for peripheral"),
    };

    let struct_name = Name::from(format!("spi_i2s_{}", number)); //Name::from(&peripheral.name);

    let enable_field_name = format!("{}en", name.original.to_lowercase());

    let rcc = match device
      .peripherals
      .iter()
      .find(|p| p.name.to_lowercase() == "rcc")
    {
      Some(p) => p,
      None => bail!("Could not find RCC peripheral"),
    };

    let cr1 = match peripheral
      .iter_registers()
      .find(|r| r.name.to_lowercase() == "cr1")
    {
      Some(p) => p,
      None => bail!("Could not find CR1 register"),
    };

    let cr2 = match peripheral
      .iter_registers()
      .find(|r| r.name.to_lowercase() == "cr2")
    {
      Some(p) => p,
      None => bail!("Could not find CR2 register"),
    };

    let sr = match peripheral
      .iter_registers()
      .find(|r| r.name.to_lowercase() == "sr")
    {
      Some(p) => p,
      None => bail!("Could not find SR register"),
    };

    let i2scfgr = match peripheral
      .iter_registers()
      .find(|r| r.name.to_lowercase() == "i2scfgr")
    {
      Some(p) => p,
      None => bail!("Could not find I2SCFGR peripheral"),
    };

    let i2spr = match peripheral
      .iter_registers()
      .find(|r| r.name.to_lowercase() == "i2spr")
    {
      Some(p) => p,
      None => bail!("Could not find I2SPR peripheral"),
    };

    Ok(Self {
      name,
      struct_name,
      number,
      peripheral_enable_field: try_find_field_in_peripheral(rcc, &enable_field_name)?.path(),
      i2smod_field: try_find_field_in_peripheral(peripheral, "i2smod")?.path(),
      spe_field: try_find_field_in_register(cr1, "spe")?.path(),
      br_field: try_find_enum_field_in_register(cr1, "br")?,
      cpol_field: try_find_field_in_register(cr1, "cpol")?.path(),

      cpha_field: try_find_field_in_register(cr1, "cpha")?.path(),
      rxonly_field: try_find_field_in_register(cr1, "rxonly")?.path(),
      bidimode_field: try_find_field_in_register(cr1, "bidimode")?.path(),
      bidioe_field: try_find_field_in_register(cr1, "bidioe")?.path(),
      nssp_field: try_find_field_in_register(cr2, "nssp")?.path(),
      lsbfirst_field: try_find_field_in_register(cr1, "lsbfirst")?.path(),
      crcl_field: try_find_field_in_register(cr1, "crcl")?.path(),
      crcen_field: try_find_field_in_register(cr1, "crcen")?.path(),
      ssm_field: try_find_field_in_register(cr1, "ssm")?.path(),
      ssi_field: try_find_field_in_register(cr1, "ssi")?.path(),
      mstr_field: try_find_field_in_register(cr1, "mstr")?.path(),

      ds_field: try_find_field_in_register(cr2, "ds")?.path(),
      ssoe_field: try_find_field_in_register(cr2, "ssoe")?.path(),
      frf_field: try_find_field_in_register(cr2, "frf")?.path(),

      frxth_field: try_find_field_in_register(cr2, "frxth")?.path(),
      ldma_tx_field: try_find_field_in_register(cr2, "ldma_tx")?.path(),
      ldma_rx_field: try_find_field_in_register(cr2, "ldma_rx")?.path(),

      dr_field: try_find_field_in_peripheral(peripheral, "dr")?.path(),

      bsy_field: try_find_field_in_register(sr, "bsy")?.path(),
    })
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "spi".to_owned(),
      name: self.struct_name.clone(),
      needs_clocks: true,
    }
  }
}
