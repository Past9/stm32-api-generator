use crate::file::OutputDirectory;
use anyhow::Result;
use askama::Template;
use svd_expander::DeviceSpec;

pub fn generate(d: &DeviceSpec, out_dir: &OutputDirectory) -> Result<()> {
  let clocks_file = match &d.name[..] {
    "STM32F303" => F303Template {}.render()?,
    _ => DefaultTemplate {}.render()?,
  };

  out_dir.publish(&f!("src/clocks/clock_config.rs"), &clocks_file)?;
  out_dir.publish(&f!("src/clocks/mod.rs"), &ModTemplate {}.render()?)?;

  Ok(())
}

#[derive(Template)]
#[template(path = "clocks/mod.rs.askama", escape = "none")]
struct ModTemplate {}

#[derive(Template)]
#[template(path = "clocks/default.rs.askama", escape = "none")]
struct DefaultTemplate {}

#[derive(Template)]
#[template(path = "clocks/f303.rs.askama", escape = "none")]
struct F303Template {}
