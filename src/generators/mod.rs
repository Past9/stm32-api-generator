use crate::{file::OutputDirectory, system_info::SystemInfo};
use anyhow::Result;
use askama::Template;
use heck::{CamelCase, KebabCase};
use svd_expander::DeviceSpec;

pub mod clocks;
pub mod gpio;
//pub mod timers;

pub fn generate(dry_run: bool, device_spec: &DeviceSpec, out_dir: &OutputDirectory) -> Result<()> {
  let sys_info = SystemInfo::new(device_spec)?;

  let mut submodules = Vec::<SubmoduleModel>::new();

  clocks::generate(dry_run, device_spec, out_dir)?;

  let gpio_metadata = gpio::generate(dry_run, device_spec, &sys_info, out_dir)?;
  submodules.extend(
    gpio_metadata
      .submodules
      .iter()
      .map(|n| SubmoduleModel::new("gpio::", n)),
  );

  /*
  submodules.extend(
    timers::generate(dry_run, device_spec, out_dir, &gpio_metadata.timer_channels)?
      .iter()
      .map(|n| SubmoduleModel::new("timers::", n)),
  );
  */

  let lib_template = LibTemplate {
    device: &device_spec,
    sys: &sys_info, //submodules,
  };

  out_dir.publish(
    dry_run,
    "includes/memory.x",
    &IncludeMemoryXTemplate {}.render()?,
  )?;
  out_dir.publish(
    dry_run,
    "includes/openocd.cfg",
    &IncludeOpenOcdCfgTemplate {}.render()?,
  )?;
  out_dir.publish(
    dry_run,
    "includes/openocd.gdb",
    &IncludeOpenOcdGdbTemplate {}.render()?,
  )?;
  out_dir.publish(
    dry_run,
    "includes/build.rs",
    &IncludeBuildRsTemplate {}.render()?,
  )?;
  out_dir.publish(
    dry_run,
    "includes/Cargo.toml",
    &IncludeCargoTomlTemplate {}.render()?,
  )?;
  out_dir.publish(dry_run, "src/lib.rs", &lib_template.render()?)?;
  out_dir.publish(dry_run, ".rustfmt.toml", &RustFmtTemplate {}.render()?)?;
  out_dir.publish(
    dry_run,
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
  pub sys: &'a SystemInfo<'a>, //pub submodules: Vec<SubmoduleModel>,
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
  pub parent_path: String,
  pub module_name: String,
  pub field_name: String,
  pub struct_name: String,
}
impl SubmoduleModel {
  pub fn new(parent_path: &str, module_name: &str) -> Self {
    Self {
      parent_path: parent_path.to_owned(),
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
  fn write_val(&self, path: &str, expr: &str, interrupt_free: bool) -> String;
  fn write_bit(&self, path: &str, expr: &str, interrupt_free: bool) -> String;
  fn reset(&self, path: &str, interrupt_free: bool) -> String;
  fn set_bit(&self, path: &str, interrupt_free: bool) -> String;
  fn clear_bit(&self, path: &str, interrupt_free: bool) -> String;
  fn read_val(&self, path: &str) -> String;
  fn is_set(&self, path: &str) -> String;
  fn is_clear(&self, path: &str) -> String;
  fn wait_for_val(&self, path: &str, expr: &str, max_loops: u32, interrupt_free: bool) -> String;
  fn wait_for_clear(&self, path: &str, max_loops: u32, interrupt_free: bool) -> String;
  fn wait_for_set(&self, path: &str, max_loops: u32, interrupt_free: bool) -> String;
}
impl ReadWrite for DeviceSpec {
  fn write_val(&self, path: &str, expr: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();
    let offset = field.offset;
    let itf = itf(interrupt_free);

    f!("write_val{itf}({address:#010x}, {mask:#034b}, {offset}, {expr}) /* Set {path} = {expr} */")
  }

  fn write_bit(&self, path: &str, expr: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();
    let offset = field.offset;
    let itf = itf(interrupt_free);

    f!("write_bit{itf}({address:#010x}, {mask:#034b}, {offset}, {expr}) /* Set {path} = {expr} */")
  }

  fn reset(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();
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

    f!("write_val{itf}({address:#010x}, {mask:#034b}, {offset}, {reset_value}) /* Reset {path} */")
  }

  fn set_bit(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();
    if field.width != 1 {
      panic!("Cannot set single bit for a multi-bit field");
    }

    let address = field.address();
    let mask = field.mask();
    let itf = itf(interrupt_free);

    f!("set_bit{itf}({address:#010x}, {mask:#034b}) /* Set {path} */")
  }

  fn clear_bit(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();
    if field.width != 1 {
      panic!("Cannot clear single bit for a multi-bit field");
    }

    let itf = itf(interrupt_free);
    let address = field.address();
    let mask = field.mask();

    f!("clear_bit{itf}({address:#010x}, {mask:#034b}) /* Clear {path} */")
  }

  fn read_val(&self, path: &str) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();
    let offset = field.offset;

    f!("read_val({address:#010x}, {mask:#034b}, {offset}) /* Read {path} */")
  }

  fn is_set(&self, path: &str) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();

    f!("is_set({address:#010x}, {mask:#034b}) /* Check if {path} is 1 */")
  }

  fn is_clear(&self, path: &str) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let mask = field.mask();

    f!("is_clear({address:#010x}, {mask:#034b}) /* Check if {path} is 0 */")
  }

  fn wait_for_val(&self, path: &str, expr: &str, max_loops: u32, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let itf = itf(interrupt_free);
    let address = field.address();
    let mask = field.mask();
    let offset = field.offset;

    f!("wait_for_val{itf}({address:#010x}, {mask:#034b}, {offset}, {expr}, {max_loops}) /* Block until {path} == {expr} */")
  }

  fn wait_for_clear(&self, path: &str, max_loops: u32, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let itf = itf(interrupt_free);
    let address = field.address();
    let mask = field.mask();

    f!("wait_for_clear{itf}({address:#010x}, {mask:#034b}, {max_loops}) /* Block until {path} is cleared */")
  }

  fn wait_for_set(&self, path: &str, max_loops: u32, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let itf = itf(interrupt_free);
    let address = field.address();
    let mask = field.mask();

    f!("wait_for_set{itf}({address:#010x}, {mask:#034b}, {max_loops}) /* Block until {path} is set */")
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TimerChannelInfo {
  pub timer_field_name: String,
  pub timer_struct_name: String,
  pub channel_field_name: String,
  pub channel_struct_name: String,
}
impl TimerChannelInfo {
  pub fn field_name(&self) -> String {
    format!("{}_{}", self.timer_field_name, self.channel_field_name)
  }

  pub fn struct_name(&self) -> String {
    format!("{}{}", self.timer_struct_name, self.channel_struct_name)
  }
}
impl PartialOrd for TimerChannelInfo {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.field_name().cmp(&other.field_name()))
  }
}
impl Ord for TimerChannelInfo {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.field_name().cmp(&other.field_name())
  }
}

#[macro_export]
macro_rules! write_val {
  ($device:ident, $path:expr, $val:expr) => {
    $device.write_val(&$path, &$val.to_string(), true);
  };
  ($device:ident, $path:expr, $val:expr, $interrupt_free:expr) => {
    $device.write_val(&$path, &$val.to_string(), $interrupt_free);
  };
}

#[macro_export]
macro_rules! write_bit {
  ($device:ident, $path:expr, $val:expr) => {
    $device.write_bit(&$path, &$val.to_string(), true);
  };
  ($device:ident, $path:expr, $val:expr, $interrupt_free:expr) => {
    $device.write_bit(&$path, &$val.to_string(), $interrupt_free);
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

#[macro_export]
macro_rules! read_val {
  ($device:ident, $path:expr) => {
    $device.read_val(&$path);
  };
}

#[macro_export]
macro_rules! is_set {
  ($device:ident, $path:expr) => {
    $device.is_set(&$path);
  };
}

#[macro_export]
macro_rules! is_clear {
  ($device:ident, $path:expr) => {
    $device.is_clear(&$path);
  };
}

#[macro_export]
macro_rules! wait_for_val {
  ($device:ident, $path:expr, $val:expr) => {
    $device.wait_for_val(&$path, &$val.to_string(), 1000, true);
  };
  ($device:ident, $path:expr, $val:expr, $interrupt_free:expr) => {
    $device.wait_for_val(&$path, &$val.to_string(), 1000, $interrupt_free);
  };
  ($device:ident, $path:expr, $val:expr, $max_loops:expr) => {
    $device.wait_for_val(&$path, &$val.to_string(), $max_loops, true);
  };
  ($device:ident, $path:expr, $val:expr, $max_loops:expr, $interrupt_free:expr) => {
    $device.wait_for_val(&$path, &$val.to_string(), $max_loops, $interrupt_free);
  };
}

#[macro_export]
macro_rules! wait_for_clear {
  ($device:ident, $path:expr) => {
    $device.wait_for_clear(&$path, 1000, true);
  };
  ($device:ident, $path:expr, $interrupt_free:expr) => {
    $device.wait_for_clear(&$path, 1000, $interrupt_free);
  };
  ($device:ident, $path:expr, $max_loops:expr) => {
    $device.wait_for_clear(&$path, $max_loops, true);
  };
  ($device:ident, $path:expr, &max_loops:expr, $interrupt_free:expr) => {
    $device.wait_for_clear(&$path, $max_loops, $interrupt_free);
  };
}

#[macro_export]
macro_rules! wait_for_set {
  ($device:ident, $path:expr) => {
    $device.wait_for_set(&$path, 1000, true);
  };
  ($device:ident, $path:expr, $interrupt_free:expr) => {
    $device.wait_for_set(&$path, 1000, $interrupt_free);
  };
  ($device:ident, $path:expr, $max_loops:expr) => {
    $device.wait_for_set(&$path, $max_loops, true);
  };
  ($device:ident, $path:expr, $max_loops:expr, $interrupt_free:expr) => {
    $device.wait_for_set(&$path, $max_loops, $interrupt_free);
  };
}
