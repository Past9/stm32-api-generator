#[macro_use]
extern crate fstrings;
#[macro_use]
mod logging;

use std::fs::File;
use std::io::Read;

use anyhow::{anyhow, Result};
use clap::{App, Arg};
use glob::glob;
use heck::KebabCase;

use file::OutputDirectory;
use svd_expander::DeviceSpec;

mod file;
mod generators;

fn main() {
  match run() {
    Ok(()) => {}
    Err(err) => error!("{}", err.to_string()),
  }
}

fn run() -> Result<()> {
  let matches = App::new("STM32 Register API Generator")
    .arg(
      Arg::with_name("files")
        .short("f")
        .long("files")
        .help("Glob pattern matching SVD files to generate APIs for.")
        .takes_value(true)
        .required(true),
    )
    .arg(
      Arg::with_name("out")
        .short("o")
        .long("out")
        .help("Output directory path.")
        .takes_value(true)
        .required(true),
    )
    /*
    .arg(
      Arg::with_name("features")
        .long("features")
        .help("List of features to generate. Defaults to all.")
        .takes_value(true)
        .min_values(0),
    )
    */
    .arg(
      Arg::with_name("no-fix")
        .long("no-fix")
        .help("Don't run `cargo fix` on the output crate(s).")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("no-fmt")
        .long("no-fmt")
        .help("Don't run `cargo fmt` on the output crate(s).")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("no-check")
        .long("no-check")
        .help("Don't run `cargo check` on the output crate(s).")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("build-release")
        .long("build-release")
        .help("Build the crate(s) in release mode.")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("build-debug")
        .long("build-debug")
        .help("Build the crate(s) in debug mode.")
        .takes_value(false),
    )
    .arg(
      Arg::with_name("build-docs")
        .long("build-docs")
        .help("Build documentation for the crate(s).")
        .takes_value(false),
    )
    .get_matches();

  let out_dir = OutputDirectory::new(match matches.value_of("out") {
    Some(od) => od,
    None => return Err(anyhow!("No output directory was provided.")),
  })?;

  let file_glob = matches.value_of("files").unwrap_or("./*");

  /*
  let only_features = match matches.values_of("features") {
    Some(ref v) => Some(
      v.clone()
        .map(|s| s.to_owned().to_lowercase())
        .collect::<Vec<_>>(),
    ),
    None => None,
  };
  */

  let run_fix = !matches.is_present("no-fix");
  let run_format = !matches.is_present("no-fmt");
  let run_check = !matches.is_present("no-check");
  let build_release = matches.is_present("build-release");
  let build_debug = matches.is_present("build-debug");
  let build_docs = matches.is_present("build-docs");

  let mut found_file = false;
  for entry in glob(file_glob)? {
    let entry = entry?;
    if !entry.is_dir() {
      found_file = true;

      let path_str = match entry.clone().into_os_string().into_string() {
        Ok(s) => s,
        Err(_) => return Err(anyhow!("Could not convert OS String to String")),
      };

      info!("Loading {}", &path_str);

      // Load and parse the SVD file
      let xml = &mut String::new();
      File::open(path_str).unwrap().read_to_string(xml)?;
      let spec = DeviceSpec::from_xml(xml)?;
      let crate_out_dir = out_dir.new_in_subdir(&format!("{}-api", spec.name.to_kebab_case()))?;

      generators::generate(&spec, &crate_out_dir)?;

      file::post_process(
        &crate_out_dir.get_path()?,
        run_fix,
        run_format,
        run_check,
        build_release,
        build_debug,
        build_docs,
      )?;
    }
  }

  if !found_file {
    error!("No files found");
  }

  Ok(())
}
