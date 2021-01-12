use crate::{file::OutputDirectory, system::SystemInfo};
use anyhow::Result;
use askama::Template;
use heck::KebabCase;
use svd_expander::DeviceSpec;

pub mod clocks;
pub mod gpio;
pub mod spi;
pub mod timer;

pub fn generate(
  dry_run: bool,
  device_spec: &DeviceSpec,
  out_dir: &OutputDirectory,
  as_source: bool,
) -> Result<OutputDirectory> {
  let sys_info = SystemInfo::new(device_spec)?;

  let (base_dir, src_dir, includes_dir, api_path) = match as_source {
    true => {
      let api_name = format!("{}_api", device_spec.name.to_kebab_case());
      let base_dir = out_dir.to_owned();
      let src_dir = out_dir.new_in_subdir(&format!("src/{}", api_name))?;
      let includes_dir = out_dir.clone();
      let api_path = format!("crate::{}", api_name);
      (base_dir, src_dir, includes_dir, api_path)
    }
    false => {
      let base_dir = out_dir.new_in_subdir(&format!("{}-api", device_spec.name.to_kebab_case()))?;
      let src_dir = base_dir.new_in_subdir("src")?;
      let includes_dir = base_dir.new_in_subdir("includes")?;
      let api_path = "crate".to_owned();
      (base_dir, src_dir, includes_dir, api_path)
    }
  };

  clocks::generate(dry_run, device_spec, &src_dir, api_path.clone())?;
  gpio::generate(dry_run, &sys_info, &src_dir, api_path.clone())?;
  timer::generate(dry_run, &sys_info, &src_dir, api_path.clone())?;
  spi::generate(dry_run, &sys_info, &src_dir, api_path.clone())?;

  let lib_template = LibTemplate {
    as_source,
    device: &device_spec,
    sys: &sys_info,
  };

  includes_dir.publish(dry_run, "memory.x", &IncludeMemoryXTemplate {}.render()?)?;
  includes_dir.publish(
    dry_run,
    "openocd.cfg",
    &IncludeOpenOcdCfgTemplate {}.render()?,
  )?;
  includes_dir.publish(
    dry_run,
    "openocd.gdb",
    &IncludeOpenOcdGdbTemplate {}.render()?,
  )?;
  includes_dir.publish(dry_run, "build.rs", &IncludeBuildRsTemplate {}.render()?)?;

  if !as_source {
    includes_dir.publish(
      dry_run,
      "Cargo.toml",
      &IncludeCargoTomlTemplate {}.render()?,
    )?;
  }

  if as_source {
    src_dir.publish(dry_run, "mod.rs", &lib_template.render()?)?;
  } else {
    src_dir.publish(dry_run, "lib.rs", &lib_template.render()?)?;
  }

  if !as_source {
    base_dir.publish(dry_run, ".rustfmt.toml", &RustFmtTemplate {}.render()?)?;
    base_dir.publish(
      dry_run,
      "Cargo.toml",
      &CargoTemplate {
        crate_name: format!("{}-api", &device_spec.name.to_kebab_case()),
      }
      .render()?,
    )?;
  }

  Ok(base_dir)
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
  pub as_source: bool,
  pub device: &'a DeviceSpec,
  pub sys: &'a SystemInfo<'a>,
}

#[derive(Template)]
#[template(path = ".rustfmt.toml.askama", escape = "none")]
struct RustFmtTemplate {}

#[derive(Template)]
#[template(path = "Cargo.toml.askama", escape = "none")]
struct CargoTemplate {
  pub crate_name: String,
}

fn itf(interrupt_free: bool) -> &'static str {
  match interrupt_free {
    true => "_itf",
    false => "",
  }
}

pub trait ReadWrite {
  fn write_val(&self, path: &str, expr: &str, interrupt_free: bool) -> String;
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

  fn reset(&self, path: &str, interrupt_free: bool) -> String {
    let field = self.get_field(path).unwrap();

    let address = field.address();
    let offset = field.offset;

    let reset_mask = match field.reset_mask {
      Some(rm) => rm,
      None => {
        warn!(
          "No reset mask configured for field {}, defaulting to field mask.",
          path
        );
        field.mask()
      }
    };

    let reset_value = match field.reset_value {
      Some(rv) => rv,
      None => {
        warn!(
          "No reset value configured for field {}, defaulting to 0.",
          path
        );
        0
      }
    };

    let itf = itf(interrupt_free);

    f!("write_val{itf}({address:#010x}, {reset_mask:#034b}, {offset}, {reset_value}) /* Reset {path} */")
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
