use crate::file::OutputDirectory;
use anyhow::Result;
use askama::Template;
use heck::{CamelCase, KebabCase};
use svd_expander::DeviceSpec;

pub mod gpio;

pub fn generate(device_spec: &DeviceSpec, out_dir: &OutputDirectory) -> Result<()> {
  let mut submodule_names: Vec<String> = Vec::new();

  submodule_names.extend(gpio::generate(device_spec, out_dir)?);

  let mut lib_template = LibTemplate {
    submodules: submodule_names
      .iter()
      .map(|n| SubmoduleModel::new(n))
      .collect(),
  };

  out_dir.publish("src/lib.rs", &lib_template.render()?)?;
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
#[template(path = "lib.rs.askama", escape = "none")]
struct LibTemplate {
  pub submodules: Vec<SubmoduleModel>,
}

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
