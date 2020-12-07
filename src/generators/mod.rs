use crate::file::OutputDirectory;
use anyhow::Result;
use askama::Template;
use heck::{CamelCase, KebabCase};
use svd_expander::DeviceSpec;

pub mod clocks;
pub mod fields;
pub mod gpio;

pub fn generate(device_spec: &DeviceSpec, out_dir: &OutputDirectory) -> Result<()> {
  let mut submodule_names: Vec<String> = Vec::new();

  clocks::generate(device_spec, out_dir)?;

  submodule_names.extend(gpio::generate(device_spec, out_dir)?);

  let lib_template = LibTemplate {
    device: &device_spec,
    submodules: submodule_names
      .iter()
      .map(|n| SubmoduleModel::new(n))
      .collect(),
  };

  out_dir.publish("includes/memory.x", &IncludeMemoryXTemplate {}.render()?)?;
  out_dir.publish(
    "includes/openocd.cfg",
    &IncludeOpenOcdCfgTemplate {}.render()?,
  )?;
  out_dir.publish(
    "includes/openocd.gdb",
    &IncludeOpenOcdGdbTemplate {}.render()?,
  )?;
  out_dir.publish("includes/build.rs", &IncludeBuildRsTemplate {}.render()?)?;
  out_dir.publish(
    "includes/Cargo.toml",
    &IncludeCargoTomlTemplate {}.render()?,
  )?;
  out_dir.publish("src/lib.rs", &lib_template.render()?)?;
  out_dir.publish(".rustfmt.toml", &RustFmtTemplate {}.render()?)?;
  out_dir.publish(
    "Cargo.toml",
    &CargoTemplate {
      crate_name: format!("{}-api", &device_spec.name.to_kebab_case()),
    }
    .render()?,
  )?;

  Ok(())
}

#[derive(Template)]
#[template(path = "includes/memory.x.askama", escape = "none")]
struct IncludeMemoryXTemplate {}

#[derive(Template)]
#[template(path = "includes/openocd.cfg.askama", escape = "none")]
struct IncludeOpenOcdCfgTemplate {}

#[derive(Template)]
#[template(path = "includes/openocd.gdb.askama", escape = "none")]
struct IncludeOpenOcdGdbTemplate {}

#[derive(Template)]
#[template(path = "includes/build.rs.askama", escape = "none")]
struct IncludeBuildRsTemplate {}

#[derive(Template)]
#[template(path = "includes/Cargo.toml.askama", escape = "none")]
struct IncludeCargoTomlTemplate {}

#[derive(Template)]
#[template(path = "lib.rs.askama", escape = "none")]
struct LibTemplate<'a> {
  pub device: &'a DeviceSpec,
  pub submodules: Vec<SubmoduleModel>,
}

#[derive(Template)]
#[template(path = ".rustfmt.toml.askama", escape = "none")]
struct RustFmtTemplate {}

#[derive(Template)]
#[template(path = "Cargo.toml.askama", escape = "none")]
struct CargoTemplate {
  pub crate_name: String,
}

struct SubmoduleModel {
  pub module_name: String,
  pub field_name: String,
  pub struct_name: String,
}
impl SubmoduleModel {
  pub fn new(module_name: &str) -> Self {
    Self {
      module_name: module_name.to_owned(),
      field_name: module_name.to_owned(),
      struct_name: module_name.to_camel_case(),
    }
  }
}

fn itf(interrupt_free: bool) -> &'static str {
  match interrupt_free {
    true => "_itf",
    false => "",
  }
}

pub trait ReadWrite {
  //fn wv<S: Into<String>, Copy>(&self, path: S) -> String;
  fn write_val(&self, path: &str, expr: &str, interrupt_free: bool) -> String;
  fn reset(&self, path: &str, interrupt_free: bool) -> String;
  fn set_bit(&self, path: &str, interrupt_free: bool) -> String;
  fn clear_bit(&self, path: &str, interrupt_free: bool) -> String;
}
impl ReadWrite for DeviceSpec {
  /*
  fn wv<S: Into<String>>(&self, path: S) -> String {Vj
    path.into()
  }
  */

  fn write_val(&self, path: &str, expr: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();
    let inverse_mask = !field.mask();
    let offset = field.offset;
    let itf = itf(interrupt_free);

    f!(
      r##"// Set {path} = {expr}
      write_val{itf}({address:#010x}, {mask:#034b}, {inverse_mask:#034b}, {offset}, {expr});"##
    )
  }

  fn reset(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();
    let inverse_mask = !field.mask();
    let offset = field.offset;

    let register = self.get_register(&field.parent_path()).unwrap();

    let register_reset_val = match register.reset_value {
      Some(rv) => rv,
      None => 0,
    };

    let register_reset_mask = match register.reset_mask {
      Some(rm) => rm,
      None => u32::MAX,
    };

    let reset_value = register_reset_mask & register_reset_val & mask >> offset;
    let itf = itf(interrupt_free);

    f!(
      r##"// Reset {path}
      write_val{itf}({address:#010x}, {mask:#034b}, {inverse_mask:#034b}, {offset}, {reset_value});"##
    )
  }

  fn set_bit(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();
    if field.width != 1 {
      panic!("Cannot set single bit for a multi-bit field");
    }

    let address = field.address();
    let mask = field.mask();
    let itf = itf(interrupt_free);

    f!(
      r##"// Set {path}
      set_bit{itf}({address:#010x}, {mask:#034b});"##
    )
  }

  fn clear_bit(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();
    if field.width != 1 {
      panic!("Cannot clear single bit for a multi-bit field");
    }

    let address = field.address();
    let inverse_mask = !field.mask();
    let itf = itf(interrupt_free);

    f!(
      r##"// Clear {path}
      clear_bit{itf}({address:#010x}, {inverse_mask:#034b});"##
    )
  }
}

#[macro_export]
macro_rules! write_val {
  ($device:ident, $path:expr, $val:expr) => {
    $device.write_val(&$path, &$val, true);
  };
  ($device:ident, $path:expr, $val:expr, $interrupt_free:expr) => {
    $device.write_val(&$path, &$val, $interrupt_free);
  };
}

#[macro_export]
macro_rules! reset {
  ($device:ident, $path:expr) => {
    $device.reset(&$path, true);
  };
  ($device:ident, $path:expr, $interrupt_free:expr) => {
    $device.reset(&$path, $interrupt_free);
  };
}

#[macro_export]
macro_rules! set_bit {
  ($device:ident, $path:expr) => {
    $device.set_bit(&$path, true);
  };
  ($device:ident, $path:expr, $interrupt_free:expr) => {
    $device.set_bit(&$path, $interrupt_free);
  };
}

#[macro_export]
macro_rules! clear_bit {
  ($device:ident, $path:expr) => {
    $device.clear_bit(&$path, true);
  };
  ($device:ident, $path:expr, $interrupt_free:expr) => {
    $device.clear_bit(&$path, $interrupt_free);
  };
}
