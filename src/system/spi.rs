use anyhow::{bail, Result};
use svd_expander::{DeviceSpec, PeripheralSpec};

use super::*;

pub struct Spi {
  pub name: Name,
  pub peripheral_enable_field: String,
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

  pub ds_field: RangedField,
  pub ssoe_field: String,
  pub frf_field: String,
  pub frxth_field: String,
  pub ldma_tx_field: String,
  pub ldma_rx_field: String,
}
impl Spi {
  pub fn new(device: &DeviceSpec, peripheral: &PeripheralSpec) -> Result<Self> {
    let name = Name::from(&peripheral.name);
    let enable_field_name = format!("{}en", name.original.to_lowercase());

    let rcc = match device
      .peripherals
      .iter()
      .find(|p| p.name.to_lowercase() == "rcc")
    {
      Some(p) => p,
      None => bail!("Could not find RCC peripheral"),
    };

    Ok(Self {
      name,
      peripheral_enable_field: try_find_field_in_peripheral(rcc, &enable_field_name)?.path(),
      br_field: try_find_enum_field_in_peripheral(peripheral, "br")?,
      cpol_field: try_find_field_in_peripheral(peripheral, "cpol")?.path(),

      cpha_field: try_find_field_in_peripheral(peripheral, "cpha")?.path(),
      rxonly_field: try_find_field_in_peripheral(peripheral, "rxonly")?.path(),
      bidimode_field: try_find_field_in_peripheral(peripheral, "bidimode")?.path(),
      bidioe_field: try_find_field_in_peripheral(peripheral, "bidioe")?.path(),
      nssp_field: try_find_field_in_peripheral(peripheral, "nssp")?.path(),
      lsbfirst_field: try_find_field_in_peripheral(peripheral, "lsbfirst")?.path(),
      crcl_field: try_find_field_in_peripheral(peripheral, "crcl")?.path(),
      crcen_field: try_find_field_in_peripheral(peripheral, "crcen")?.path(),
      ssm_field: try_find_field_in_peripheral(peripheral, "ssm")?.path(),
      ssi_field: try_find_field_in_peripheral(peripheral, "ssi")?.path(),
      mstr_field: try_find_field_in_peripheral(peripheral, "mstr")?.path(),

      ds_field: try_find_ranged_field_in_peripheral(peripheral, "ds")?,
      ssoe_field: try_find_field_in_peripheral(peripheral, "ssoe")?.path(),
      frf_field: try_find_field_in_peripheral(peripheral, "frf")?.path(),

      frxth_field: try_find_field_in_peripheral(peripheral, "frxth")?.path(),
      ldma_tx_field: try_find_field_in_peripheral(peripheral, "ldma_tx")?.path(),
      ldma_rx_field: try_find_field_in_peripheral(peripheral, "ldma_rx")?.path(),
    })
  }

  pub fn submodule(&self) -> Submodule {
    Submodule {
      parent_path: "spi".to_owned(),
      name: self.name.clone(),
      needs_clocks: true,
    }
  }
}
