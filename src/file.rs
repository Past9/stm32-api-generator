use std::fs;
use std::fs::create_dir_all;
use std::{io, ops::Deref, path::PathBuf, process::Command};

use anyhow::{anyhow, Result};
use io::Write;

pub struct OutputDirectory {
  dir_path: String,
}
impl OutputDirectory {
  pub fn new(dir_path: &str) -> Result<Self> {
    create_dir_all(dir_path.clone())?;
    Ok(Self {
      dir_path: dir_path.to_owned(),
    })
  }

  pub fn walk_into_subdir(&mut self, subdir: &str) -> Result<()> {
    let mut path_buf = PathBuf::from(&self.dir_path);
    path_buf.push(subdir);
    self.dir_path = match path_buf.into_os_string().into_string() {
      Ok(s) => s,
      Err(_) => return Err(anyhow!("Could not convert path to string")),
    };

    Ok(())
  }

  pub fn new_in_subdir(&self, subdir: &str) -> Result<Self> {
    let mut path_buf = PathBuf::from(&self.dir_path);
    path_buf.push(subdir);
    Self::new(match path_buf.into_os_string().into_string() {
      Ok(ref s) => s,
      Err(_) => return Err(anyhow!("Could not convert path to string")),
    })
  }

  pub fn get_path(&self) -> Result<String> {
    Ok(
      PathBuf::from(&self.dir_path)
        .canonicalize()?
        .to_string_lossy()
        .deref()
        .to_owned(),
    )
  }

  pub fn publish(&self, rel_file_path: &str, file_content: &str) -> Result<()> {
    // Add the relative file path to the output directory base path
    let mut file_path_buf = PathBuf::from(&self.dir_path);
    file_path_buf.push(rel_file_path);

    // Ensure the file's parent directory exists
    create_dir_all(match file_path_buf.parent() {
      Some(path) => path,
      None => {
        return Err(anyhow!(
          "File path {} has no parent directory",
          &file_path_buf.canonicalize()?.to_string_lossy()
        ))
      }
    })?;

    fs::write(file_path_buf, file_content)?;
    Ok(())
  }
}

pub fn run_command(path: &str, command: &str, args: Vec<&str>) -> Result<()> {
  let output = Command::new(command)
    .current_dir(path)
    .args(args)
    .output()?;

  if output.stdout.len() > 0 {
    io::stdout().write_all(&output.stdout)?;
  }

  if output.stderr.len() > 0 {
    io::stderr().write_all(&output.stderr)?;
  }

  if !output.status.success() {
    return Err(match output.status.code() {
      Some(code) => anyhow!("Command failed with exit code {}.", code),
      None => anyhow!("Command failed."),
    });
  }

  Ok(())
}

pub fn post_process(
  path: &str,
  run_fix: bool,
  run_format: bool,
  run_check: bool,
  build_release: bool,
  build_debug: bool,
  build_docs: bool,
) -> Result<()> {
  if run_fix {
    println!("Fixing...");
    run_command(
      path,
      "cargo",
      vec![
        "+nightly",
        "fix",
        "--allow-dirty",
        "--allow-no-vcs",
        "--all-features",
      ],
    )?;
  }

  if run_format {
    println!("Formatting...");
    run_command(path, "cargo", vec!["+nightly", "fmt"])?;
  }

  if run_check {
    println!("Checking...");
    run_command(path, "cargo", vec!["check", "--all-features"])?;
  }

  if build_release {
    println!("Building in release mode...");
    run_command(path, "cargo", vec!["build", "--release", "--all-features"])?;
  }

  if build_debug {
    println!("Building in debug mode...");
    run_command(path, "cargo", vec!["build", "--all-features"])?;
  }

  if build_docs {
    println!("Building documentation...");
    run_command(path, "cargo", vec!["doc", "--all-features"])?;
  }

  Ok(())
}
