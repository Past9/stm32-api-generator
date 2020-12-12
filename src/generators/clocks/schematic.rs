use std::{collections::hash_map::Values, fs};
use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use serde::Deserialize;

enum ClockOutputNameSelection {
  TerminalTapsOnly,
  EverythingExceptTerminalTaps,
  Everything,
}

pub enum ClockComponent {
  Oscillator(Oscillator),
  Multiplexer(Multiplexer),
  Divider(Divider),
  Multiplier(Multiplier),
  Tap(Tap),
}

#[derive(Deserialize, Debug, Clone)]
pub struct ClockSchematic {
  sys_clk_mux: String,
  flash_latency: FlashLatency,
  pll: Option<Pll>,
  oscillators: HashMap<String, Oscillator>,
  multiplexers: HashMap<String, Multiplexer>,
  dividers: HashMap<String, Divider>,
  multipliers: HashMap<String, Multiplier>,
  taps: HashMap<String, Tap>,
}
impl ClockSchematic {
  pub fn from_ron_file<P: AsRef<Path>>(path: P) -> Result<ClockSchematic> {
    info!(
      "Parsing clock schematic from file '{}'",
      match path.as_ref().to_str() {
        Some(s) => s,
        None => "(could not create string from path)",
      }
    );
    let mut sch: ClockSchematic = ron::from_str(&fs::read_to_string(path)?)?;
    sch.postprocess()?;
    Ok(sch)
  }

  pub fn from_ron<S: Into<String>>(ron: S) -> Result<ClockSchematic> {
    info!("Parsing clock schematic from RON string");
    let mut sch: ClockSchematic = ron::from_str(&ron.into())?;
    sch.postprocess()?;
    Ok(sch)
  }

  fn postprocess(&mut self) -> Result<()> {
    self.set_names();
    self.flag_sys_clk_mux();
    self.validate()?;
    Ok(())
  }

  fn set_names(&mut self) {
    for (k, mut v) in self.flash_latency.ranges.iter_mut() {
      v.name = k.clone();
    }

    for (k, mut v) in self.oscillators.iter_mut() {
      v.name = k.clone();
    }

    for (k, mut v) in self.multiplexers.iter_mut() {
      v.name = k.clone();
      for (ik, iv) in v.inputs.iter_mut() {
        iv.name = ik.clone();
      }
    }

    for (k, mut v) in self.dividers.iter_mut() {
      v.name = k.clone();
      for (ik, iv) in v.values.iter_mut() {
        iv.name = ik.clone();
      }
    }

    for (k, mut v) in self.multipliers.iter_mut() {
      v.name = k.clone();
      for (ik, iv) in v.values.iter_mut() {
        iv.name = ik.clone();
      }
    }

    for (k, mut v) in self.taps.iter_mut() {
      v.name = k.clone();
    }
  }

  fn flag_sys_clk_mux(&mut self) {
    for mux in self.multiplexers.values_mut() {
      if mux.name == self.sys_clk_mux {
        mux.is_sys_clk_mux = true;
      }
    }
  }

  fn validate(&self) -> Result<()> {
    self.check_valid_names()?;
    self.check_no_duplicate_names()?;
    self.check_all_inputs_exist()?;
    self.check_all_outputs_are_used()?;
    self.check_multiplexer_defaults_exist()?;
    self.check_divider_defaults_exist()?;
    self.check_multiplier_defaults_exist()?;
    self.check_no_loops()?;

    Ok(())
  }

  pub fn pll(&self) -> Option<&Pll> {
    match self.pll {
      Some(ref p) => Some(p),
      None => None,
    }
  }

  pub fn get_sys_clk_mux(&self) -> Result<&Multiplexer> {
    match self.multiplexers().find(|o| o.name == self.sys_clk_mux) {
      Some(m) => Ok(m),
      None => Err(anyhow!(
        "System clock multiplexer '{}' does not exist",
        self.sys_clk_mux
      )),
    }
  }

  pub fn flash_latency(&self) -> &FlashLatency {
    &self.flash_latency
  }

  pub fn oscillators(&self) -> Values<String, Oscillator> {
    self.oscillators.values()
  }

  pub fn multiplexers(&self) -> Values<String, Multiplexer> {
    self.multiplexers.values()
  }

  pub fn dividers(&self) -> Values<String, Divider> {
    self.dividers.values()
  }

  pub fn multipliers(&self) -> Values<String, Multiplier> {
    self.multipliers.values()
  }

  pub fn taps(&self) -> Values<String, Tap> {
    self.taps.values()
  }

  pub fn get_all_components(&self) -> Vec<ClockComponent> {
    let oscillators = self
      .oscillators
      .values()
      .map(|v| ClockComponent::Oscillator(v.clone()));

    let multiplexers = self
      .multiplexers
      .values()
      .map(|v| ClockComponent::Multiplexer(v.clone()));

    let dividers = self
      .dividers
      .values()
      .map(|v| ClockComponent::Divider(v.clone()));

    let multipliers = self
      .multipliers
      .values()
      .map(|v| ClockComponent::Multiplier(v.clone()));

    let taps = self.taps.values().map(|v| ClockComponent::Tap(v.clone()));

    oscillators
      .chain(multiplexers)
      .chain(dividers)
      .chain(multipliers)
      .chain(taps)
      .collect()
  }

  pub fn get_component<S: Into<String>>(&self, name: S) -> Option<ClockComponent> {
    let comp_name = name.into();

    if let Some(c) = self.oscillators.values().find(|o| o.name == comp_name) {
      return Some(ClockComponent::Oscillator(c.clone()));
    }

    if let Some(c) = self.multiplexers.values().find(|m| m.name == comp_name) {
      return Some(ClockComponent::Multiplexer(c.clone()));
    }

    if let Some(c) = self.dividers.values().find(|d| d.name == comp_name) {
      return Some(ClockComponent::Divider(c.clone()));
    }

    if let Some(c) = self.multipliers.values().find(|m| m.name == comp_name) {
      return Some(ClockComponent::Multiplier(c.clone()));
    }

    if let Some(c) = self.taps.values().find(|t| t.name == comp_name) {
      return Some(ClockComponent::Tap(c.clone()));
    }

    None
  }

  fn get_next<S: Into<String>>(&self, name: S) -> Vec<String> {
    let comp_name: String = name.into();
    let mut next = Vec::new();

    next.extend(
      self
        .multiplexers
        .values()
        .filter(|c| c.inputs.values().any(|i| i.name == comp_name))
        .map(|c| c.name.clone()),
    );

    next.extend(
      self
        .dividers
        .values()
        .filter(|c| c.input == comp_name)
        .map(|c| c.name.clone()),
    );

    next.extend(
      self
        .multipliers
        .values()
        .filter(|c| c.input == comp_name)
        .map(|c| c.name.clone()),
    );

    next.extend(
      self
        .taps
        .values()
        .filter(|c| c.input == comp_name)
        .map(|c| c.name.clone()),
    );

    next
  }

  fn list_outputs(&self, selection: ClockOutputNameSelection) -> Vec<String> {
    let terminal_taps_only = self
      .taps
      .values()
      .filter(|t| t.terminal)
      .map(|t| t.name.clone());

    let everything_except_terminal_taps = self
      .oscillators
      .keys()
      .map(|k| k.clone())
      .chain(self.multiplexers.keys().map(|n| n.clone()))
      .chain(self.dividers.keys().map(|n| n.clone()))
      .chain(self.multipliers.keys().map(|n| n.clone()))
      .chain(
        self
          .taps
          .values()
          .filter(|t| !t.terminal)
          .map(|t| t.name.clone()),
      );

    match selection {
      ClockOutputNameSelection::TerminalTapsOnly => terminal_taps_only.collect(),
      ClockOutputNameSelection::EverythingExceptTerminalTaps => {
        everything_except_terminal_taps.collect()
      }
      ClockOutputNameSelection::Everything => terminal_taps_only
        .chain(everything_except_terminal_taps)
        .collect(),
    }
  }

  fn list_inputs(&self) -> Vec<String> {
    let mut inputs = self
      .multiplexers
      .values()
      .flat_map(|d| d.inputs.iter().map(|i| i.0.clone()))
      .chain(self.dividers.values().map(|i| i.input.clone()))
      .chain(self.multipliers.values().map(|i| i.input.clone()))
      .chain(self.taps.values().map(|i| i.input.clone()))
      .collect::<Vec<String>>();

    inputs.sort();
    inputs.dedup();
    inputs
  }

  fn check_valid_names(&self) -> Result<()> {
    let allowed_chars: &'static str = "abcdefghijklmnopqrstuvwxyz0123456789_";

    let mut names = self.list_inputs();
    names.append(&mut self.list_outputs(ClockOutputNameSelection::Everything));

    for name in names.iter() {
      for ch in name.to_lowercase().chars() {
        if !allowed_chars.contains(ch) {
          return Err(anyhow!(
            "Name '{}' contains invalid character: '{}'",
            name,
            ch
          ));
        }
      }
    }

    Ok(())
  }

  fn check_no_duplicate_names(&self) -> Result<()> {
    let mut names = self.list_outputs(ClockOutputNameSelection::Everything);
    names.sort();

    let mut last_name: Option<String> = None;
    for cur_name in names.iter() {
      match last_name {
        Some(ref ln) => {
          if ln == cur_name {
            return Err(anyhow!("Duplicate name: {}", cur_name));
          }
        }
        None => {}
      };
      last_name = Some(cur_name.clone());
    }

    Ok(())
  }

  fn check_all_inputs_exist(&self) -> Result<()> {
    let inputs = self.list_inputs();
    let outputs = self.list_outputs(ClockOutputNameSelection::EverythingExceptTerminalTaps);

    let nonexistent_inputs = inputs
      .iter()
      .filter_map(|i| match outputs.contains(i) {
        true => None,
        false => match i.as_str() {
          "off" => None,
          _ => Some(i.clone()),
        },
      })
      .collect::<Vec<String>>();

    if nonexistent_inputs.len() > 0 {
      return Err(anyhow!(
        "Nonexistent inputs: {} (maybe these are terminal taps?)",
        nonexistent_inputs.join(", ")
      ));
    }

    Ok(())
  }

  fn check_all_outputs_are_used(&self) -> Result<()> {
    let inputs = self.list_inputs();
    let outputs = self.list_outputs(ClockOutputNameSelection::EverythingExceptTerminalTaps);

    let unused_outputs = outputs
      .iter()
      .filter_map(|o| match inputs.contains(o) {
        true => None,
        false => Some(o.clone()),
      })
      .collect::<Vec<String>>();

    if unused_outputs.len() > 0 {
      return Err(anyhow!(
        "Unused outputs: {} (maybe these are non-terminal taps?)",
        unused_outputs.join(", ")
      ));
    }

    Ok(())
  }

  fn check_multiplexer_defaults_exist(&self) -> Result<()> {
    let multiplexers_with_bad_defaults = self
      .multiplexers
      .values()
      .filter(|m| !m.inputs.values().any(|i| i.name == m.default))
      .map(|m| m.name.clone())
      .collect::<Vec<String>>();

    if multiplexers_with_bad_defaults.len() > 0 {
      return Err(anyhow!(
        "Multiplexers have default inputs not in their input lists: {}",
        multiplexers_with_bad_defaults.join(", ")
      ));
    }

    Ok(())
  }

  fn check_divider_defaults_exist(&self) -> Result<()> {
    let dividers_with_bad_defaults = self
      .dividers
      .values()
      // Filter out any that have no values, the default will be used as the sole value
      .filter(|d| d.values.len() > 0)
      // Find any where the default isn't in the values list
      .filter(|d| !d.values.values().any(|v| v.divisor == d.default as f32))
      .map(|d| d.name.clone())
      .collect::<Vec<String>>();

    if dividers_with_bad_defaults.len() > 0 {
      return Err(anyhow!(
        "Dividers have default values not in their value lists: {}",
        dividers_with_bad_defaults.join(", ")
      ));
    }

    Ok(())
  }

  fn check_multiplier_defaults_exist(&self) -> Result<()> {
    let multipliers_with_bad_defaults = self
      .multipliers
      .values()
      // Filter out any that have no values, the default will be used as the sole value
      .filter(|m| m.values.len() > 0)
      // Find any where the default isn't in the values list
      .filter(|m| !m.values.values().any(|v| v.factor == m.default as f32))
      .map(|m| m.name.clone())
      .collect::<Vec<String>>();

    if multipliers_with_bad_defaults.len() > 0 {
      return Err(anyhow!(
        "Multipliers have default values not in their value lists: {}",
        multipliers_with_bad_defaults.join(", ")
      ));
    }

    Ok(())
  }

  pub fn get_paths(&self) -> Vec<Vec<String>> {
    const MAX_DEPTH: usize = 32;

    // Each oscillator is the start of a path.
    let mut paths: Vec<Vec<String>> = vec![self.oscillators.keys().map(|n| n.clone()).collect()];

    // Loop to a pre-determined depth so this doesn't run forever if there are loops.
    for _ in 0..MAX_DEPTH {
      // Add a step to each existing and replace `paths` with these new ones.
      // If the paths fork, the the number of items in `paths` will increase
      // because we make unique non-forking copies of each possible path.
      let mut new_paths: Vec<Vec<String>> = Vec::new();
      for path in paths.iter() {
        new_paths.extend(self.make_paths(path));
      }
      paths = new_paths;
    }

    paths
  }

  fn make_paths(&self, path: &Vec<String>) -> Vec<Vec<String>> {
    match path.iter().last() {
      Some(l) => {
        // Get the possible next moves from the last path element.
        let next = self.get_next(l);

        match next.len() {
          // If there's no next item, then we're at the end of the path so we
          // just return what we were given.
          0 => vec![path.clone()],
          // Multiply the original path into multiple copies, one for each
          // potential next move, then append those next moves to each. We
          // now have independent copies of each possible path.
          _ => next
            .iter()
            .map(|n| {
              let mut new_path = path.clone();
              new_path.push(n.clone());
              new_path
            })
            .collect(),
        }
      }
      // If there was no last element, that means we were given an
      // empty path. There's nowhere to go, so just return an empty
      // one as well.
      None => Vec::new(),
    }
  }

  fn check_no_loops(&self) -> Result<()> {
    // Look for loops inside all the paths.
    let mut loops: Vec<Vec<String>> = Vec::new();
    for path in self.get_paths().iter() {
      if let Some(lp) = Self::find_loop(path) {
        loops.push(lp);
      }
    }

    // Create text descriptions of any loops that we found.
    let mut loop_descriptions = loops
      .iter()
      .map(|l| l.join(" -> "))
      .collect::<Vec<String>>();

    // Loops are likely to appear more than once since we multiplied
    // the potential paths at each fork, so deduplicate those here.
    loop_descriptions.sort();
    loop_descriptions.dedup();

    // Throw an error if any paths were found.
    match loop_descriptions.len() > 0 {
      true => Err(anyhow!(
        "Loop(s) detected: {}",
        loop_descriptions.join(", ")
      )),
      false => Ok(()),
    }
  }

  fn find_loop(path: &Vec<String>) -> Option<Vec<String>> {
    // Loop over every item except the last one in the path we were given. Each of these
    // is potentially the start of a loop.
    for (i, start_name) in path.iter().take(path.len() - 1).enumerate() {
      let mut path_loop = vec![start_name.clone()];

      // Loop over every item after our starting item and append it to `path_loop`.
      for next_name in path[i + 1..].iter() {
        // Append it to our potential path.
        path_loop.push(next_name.clone());
        // If an item after the starting item is the same as the starting item,
        // we've found a loop and can stop searching.
        if start_name == next_name {
          match path_loop.len() > 0 {
            true => {
              return Some(path_loop);
            }
            false => {
              return None;
            }
          }
        }
      }
    }

    None
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct FlashLatency {
  pub path: String,
  pub ranges: HashMap<String, FlashLatencyRange>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FlashLatencyRange {
  #[serde(default)]
  pub name: String,
  pub min: Option<u32>,
  pub max: Option<u32>,
  pub bit_value: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Pll {
  pub power: String,
  pub ready: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Oscillator {
  #[serde(default)]
  pub name: String,
  pub frequency: u64,
  #[serde(default)]
  pub external: Option<ExternalOscillator>,
}
impl Oscillator {}

#[derive(Deserialize, Debug, Clone)]
pub struct Multiplexer {
  #[serde(default)]
  pub name: String,
  pub inputs: HashMap<String, MultiplexerInput>,
  pub default: String,
  pub path: String,
  #[serde(default)]
  pub is_sys_clk_mux: bool,
}
impl Multiplexer {
  pub fn default_input(&self) -> Result<MultiplexerInput> {
    match self.inputs.values().find(|v| v.name == self.default) {
      Some(v) => Ok(v.clone()),
      None => Err(anyhow!("Multiplexer default input not in map")),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExternalOscillator {
  pub power: String,
  pub ready: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MultiplexerInput {
  #[serde(default)]
  pub name: String,
  pub bit_value: u32,
  pub alias: Option<String>,
}
impl MultiplexerInput {
  pub fn public_name(&self) -> String {
    match self.alias {
      Some(ref a) => a.clone(),
      None => self.name.clone(),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Divider {
  #[serde(default)]
  pub name: String,
  pub input: String,
  pub default: f32,
  #[serde(default)]
  pub values: HashMap<String, DividerOption>,
  #[serde(default)]
  pub path: String,
}
impl Divider {
  pub fn is_fixed_value(&self) -> bool {
    self.values.len() == 0
  }

  pub fn default_input(&self) -> Result<&DividerOption> {
    match self.values.values().find(|v| v.divisor == self.default) {
      Some(v) => Ok(&v),
      None => Err(anyhow!("Divider default value not in map")),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DividerOption {
  #[serde(default)]
  pub name: String,
  pub divisor: f32,
  pub bit_value: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Multiplier {
  #[serde(default)]
  pub name: String,
  pub input: String,
  pub default: f32,
  #[serde(default)]
  pub values: HashMap<String, MultiplierOption>,
  #[serde(default)]
  pub path: String,
}
impl Multiplier {
  pub fn is_fixed_value(&self) -> bool {
    self.values.len() == 0
  }

  pub fn default_input(&self) -> Result<&MultiplierOption> {
    match self.values.values().find(|v| v.factor == self.default) {
      Some(v) => Ok(&v),
      None => Err(anyhow!("Multiplier default value not in map")),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct MultiplierOption {
  #[serde(default)]
  pub name: String,
  pub factor: f32,
  pub bit_value: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tap {
  #[serde(default)]
  pub name: String,
  pub input: String,
  pub max: u64,
  pub terminal: bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  const BASIC_RON: &'static str = r#"
      ClockSchematic(
        oscillators: {
          "hse": (
            frequency: 8000000
          )
        },
        multiplexers: {
          "pll_source_mux": (
            path: "path",
            inputs: { 
              "hse": (
                bit_value: 1
              )
            },
            default: "hse"
          )
        },
        dividers: {
          "pll_div": (
            input: "pll_source_mux",
            path: "path",
            values: {
              "no_div": (
                divisor: 1, 
                bit_value: 0
              )
            },
            default: 1,
          )
        },
        multipliers: {
          "pll_mul": (
            input: "pll_div", 
            path: "path",
            values: {
              "no_mul": (
                factor: 2, 
                bit_value: 1
              )
            },
            default: 2,
          )
        },
        taps: {
          "tap1": (
            input: "pll_mul", 
            max: 1000000, 
            terminal: false
          ),
          "tap2": (
            input: "tap1", 
            max: 0, 
            terminal: true
          ),
          "tap3": (
            input: "tap1", 
            max: 0, 
            terminal: true
          )
        }
      )
    "#;

  #[test]
  fn deserializes_spec_string() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();

    // Check oscillators
    assert_eq!(1, spec.oscillators.len());
    assert_eq!(8_000_000, spec.oscillators["hse"].frequency);

    // Check multiplexers
    assert_eq!(1, spec.multiplexers.len());
    assert_eq!(1, spec.multiplexers["pll_source_mux"].inputs.len());
    assert_eq!("hse", spec.multiplexers["pll_source_mux"].default);
    assert_eq!("path", spec.multiplexers["pll_source_mux"].path);
    assert_eq!(1, spec.multiplexers["pll_source_mux"].inputs.len());
    assert_eq!(
      1,
      spec.multiplexers["pll_source_mux"].inputs["hse"].bit_value
    );

    // Check dividers
    assert_eq!(1, spec.dividers.len());
    assert_eq!("pll_source_mux", spec.dividers["pll_div"].input);
    assert_eq!(1f32, spec.dividers["pll_div"].default);
    assert_eq!(1, spec.dividers["pll_div"].values.len());
    assert_eq!("path", spec.dividers["pll_div"].path);
    assert_eq!(1f32, spec.dividers["pll_div"].values["no_div"].divisor);
    assert_eq!(0, spec.dividers["pll_div"].values["no_div"].bit_value);

    // Check multipliers
    assert_eq!(1, spec.multipliers.len());
    assert_eq!("pll_div", spec.multipliers["pll_mul"].input);
    assert_eq!(2f32, spec.multipliers["pll_mul"].default);
    assert_eq!(1, spec.multipliers["pll_mul"].values.len());
    assert_eq!("path", spec.multipliers["pll_mul"].path);
    assert_eq!(2f32, spec.multipliers["pll_mul"].values["no_mul"].factor);
    assert_eq!(1, spec.multipliers["pll_mul"].values["no_mul"].bit_value);

    // Check taps
    assert_eq!(3, spec.taps.len());

    assert_eq!("pll_mul", spec.taps["tap1"].input);
    assert_eq!(1000000, spec.taps["tap1"].max);
    assert_eq!(false, spec.taps["tap1"].terminal);

    assert_eq!("tap1", spec.taps["tap2"].input);
    assert_eq!(0, spec.taps["tap2"].max);
    assert_eq!(true, spec.taps["tap2"].terminal);

    assert_eq!("tap1", spec.taps["tap3"].input);
    assert_eq!(0, spec.taps["tap3"].max);
    assert_eq!(true, spec.taps["tap3"].terminal);
  }

  #[test]
  fn gets_components_by_name() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();

    match spec.get_component("hse") {
      Some(ClockComponent::Oscillator(_)) => {}
      None => panic!("Returned None"),
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("pll_source_mux") {
      Some(ClockComponent::Multiplexer(_)) => {}
      None => panic!("Returned None"),
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("pll_div") {
      Some(ClockComponent::Divider(_)) => {}
      None => panic!("Returned None"),
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("pll_mul") {
      Some(ClockComponent::Multiplier(_)) => {}
      None => panic!("Returned None"),
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("tap1") {
      Some(ClockComponent::Tap(_)) => {}
      None => panic!("Returned None"),
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("tap2") {
      Some(ClockComponent::Tap(_)) => {}
      None => panic!("Returned None"),
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("bogus") {
      None => {}
      _ => panic!("Returned wrong component"),
    };
  }

  #[test]
  fn gets_next_components() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();

    assert_eq!(vec!["pll_source_mux"], spec.get_next("hse"));
    assert_eq!(vec!["pll_div"], spec.get_next("pll_source_mux"));
    assert_eq!(vec!["pll_mul"], spec.get_next("pll_div"));
    assert_eq!(vec!["tap1"], spec.get_next("pll_mul"));

    let mut taps = spec.get_next("tap1");
    taps.sort();
    assert_eq!(vec!["tap2", "tap3"], taps);

    assert_eq!(Vec::<String>::new(), spec.get_next("bogus"));
  }

  #[test]
  fn lists_all_outputs() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut outputs = spec.list_outputs(ClockOutputNameSelection::Everything);

    outputs.sort();
    assert_eq!(
      outputs,
      vec![
        "hse",
        "pll_div",
        "pll_mul",
        "pll_source_mux",
        "tap1",
        "tap2",
        "tap3"
      ]
    );
  }

  #[test]
  fn lists_outputs_except_terminal_taps() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut outputs = spec.list_outputs(ClockOutputNameSelection::EverythingExceptTerminalTaps);

    outputs.sort();
    assert_eq!(
      vec!["hse", "pll_div", "pll_mul", "pll_source_mux", "tap1"],
      outputs
    );
  }

  #[test]
  fn lists_only_terminal_tap_outputs() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut outputs = spec.list_outputs(ClockOutputNameSelection::TerminalTapsOnly);
    outputs.sort();

    assert_eq!(vec!["tap2", "tap3"], outputs);
  }

  #[test]
  fn lists_inputs() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut inputs = spec.list_inputs();

    inputs.sort();
    assert_eq!(
      vec!["hse", "pll_div", "pll_mul", "pll_source_mux", "tap1"],
      inputs
    );
  }

  #[test]
  fn rejects_invalid_names() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {},
        taps: {
          "Tap1": (
            input: "Hse ",
            max: 0,
            terminal: true
          )
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Name 'Hse ' contains invalid character: ' '",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_duplicate_names() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {},
        taps: {
          "Hse": (
            input: "Hse",
            max: 0,
            terminal: true
          )
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!("Duplicate name: Hse", res.unwrap_err().to_string());
  }

  #[test]
  fn rejects_nonexistent_inputs() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {},
        taps: {
          "Tap1": (
            input: "Bogus1",
            max: 0,
            terminal: true
          ),
          "Tap2": (
            input: "Bogus2",
            max: 0,
            terminal: true
          )
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Nonexistent inputs: Bogus1, Bogus2 (maybe these are terminal taps?)",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_terminal_taps_as_inputs() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {},
        taps: {
          "Tap1": (
            input: "Hse",
            max: 0,
            terminal: true
          ),
          "Tap2": (
            input: "Tap1",
            max: 0,
            terminal: true
          )
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Nonexistent inputs: Tap1 (maybe these are terminal taps?)",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_unused_outputs() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {},
        taps: {}
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Unused outputs: Hse (maybe these are non-terminal taps?)",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_nonterminal_tap_as_unused_output() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {},
        taps: {
          "Tap1": (
            input: "Hse",
            max: 0,
            terminal: false
          ),
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Unused outputs: Tap1 (maybe these are non-terminal taps?)",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_nonexistent_multiplexer_default() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {
          "Mux": (
            path: "path",
            inputs: { 
              "Hse": (
                bit_value: 0
              ) 
            },
            default: "Bogus"
          )
        },
        dividers: {},
        multipliers: {},
        taps: {
          "Tap1": (
            input: "Mux",
            max: 0,
            terminal: true
          ),
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Multiplexers have default inputs not in their input lists: Mux",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_nonexistent_divider_default() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {
          "Div": (
            input: "Hse",
            default: 2,
            path: "path",
            values: {
              "no_div": (
                divisor: 1, 
                bit_value: 0
              )
            }
          )
        },
        multipliers: {},
        taps: {
          "Tap1": (
            input: "Div",
            max: 0,
            terminal: true
          ),
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Dividers have default values not in their value lists: Div",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn rejects_nonexistent_multiplier_default() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {},
        dividers: {},
        multipliers: {
          "Mul": (
            input: "Hse",
            default: 2,
            path: "path",
            values: {
              "no_mul": (
                factor: 1, 
                bit_value: 0
              )
            }
          )
        },
        taps: {
          "Tap1": (
            input: "Mul",
            max: 0,
            terminal: true
          ),
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Multipliers have default values not in their value lists: Mul",
      res.unwrap_err().to_string()
    );
  }

  #[test]
  fn gets_all_paths() {
    let spec = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {
          "PllSourceMux": (
            path: "path",
            inputs: { 
              "Hse": (
                bit_value: 0
              ), 
            },
            default: "Hse"
          )
        },
        dividers: {
          "PllDiv": (
            input: "PllSourceMux",
            default: 1,
            path: "path",
            values: {
              "no_div": (
                divisor: 1, 
                bit_value: 0
              )
            }
          )
        },
        multipliers: {
          "PllMul": (
            input: "PllSourceMux", 
            default: 3,
            path: "path",
            values: {
              "no_div": (
                factor: 2, 
                bit_value: 0
              ),
              "mul1": (
                factor: 3, 
                bit_value: 1
              ),
              "mul2": (
                factor: 4, 
                bit_value: 2
              )
            }
          )
        },
        taps: {
          "Tap1": (
            input: "PllDiv", 
            max: 1000000, 
            terminal: true
          ),
          "Tap2": (
            input: "PllMul", 
            max: 0, 
            terminal: true
          )
        }
      )
    "#,
    )
    .unwrap();

    assert_eq!(
      vec![
        vec!["Hse", "PllSourceMux", "PllDiv", "Tap1"],
        vec!["Hse", "PllSourceMux", "PllMul", "Tap2"]
      ],
      spec.get_paths()
    );
  }

  #[test]
  fn rejects_loops() {
    let res = ClockSchematic::from_ron(
      r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {
          "PllSourceMux": (
            path: "path",
            inputs: { 
              "Hse": (
                bit_value: 0
              ), 
              "PllMul": (
                bit_value: 1
              )
            },
            default: "Hse"
          )
        },
        dividers: {
          "PllDiv": (
            input: "PllSourceMux",
            default: 1,
            path: "path",
            values: {
              "no_div": (
                divisor: 1, 
                bit_value: 0
              )
            }
          )
        },
        multipliers: {
          "PllMul": (
            input: "PllDiv", 
            default: 3,
            path: "path",
            values: {
              "no_div": (
                factor: 2, 
                bit_value: 0
              ),
              "mul1": (
                factor: 3, 
                bit_value: 1
              ),
              "mul2": (
                factor: 4, 
                bit_value: 2
              )
            }
          )
        },
        taps: {
          "Tap1": (
            input: "PllMul", 
            max: 1000000, 
            terminal: false
          ),
          "Tap2": (
            input: "Tap1", 
            max: 0, 
            terminal: true
          )
        }
      )
    "#,
    );

    assert!(res.is_err());
    assert_eq!(
      "Loop(s) detected: PllSourceMux -> PllDiv -> PllMul -> PllSourceMux",
      res.unwrap_err().to_string()
    );
  }
}
