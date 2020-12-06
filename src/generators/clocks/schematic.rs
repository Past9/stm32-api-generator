use std::fs;
use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use serde::Deserialize;

enum ClockOutputNameSelection {
  TerminalTapsOnly,
  EverythingExceptTerminalTaps,
  Everything,
}

enum ClockComponent {
  Oscillator(Oscillator),
  Multiplexer(Multiplexer),
  Divider(Divider),
  Multiplier(Multiplier),
  Tap(Tap),
}

#[derive(Deserialize, Debug, Clone)]
pub struct ClockSchematic {
  oscillators: HashMap<String, Oscillator>,
  multiplexers: HashMap<String, Multiplexer>,
  dividers: HashMap<String, Divider>,
  multipliers: HashMap<String, Multiplier>,
  taps: HashMap<String, Tap>,
}
impl ClockSchematic {
  pub fn from_ron_file<P: AsRef<Path>>(path: P) -> Result<ClockSchematic> {
    let sch = Self::from_ron(&fs::read_to_string(path)?)?;
    sch.validate()?;
    Ok(sch)
  }

  pub fn from_ron<S: Into<String>>(ron: S) -> Result<ClockSchematic> {
    let sch: ClockSchematic = ron::from_str(&ron.into())?;
    sch.validate()?;
    Ok(sch)
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

  fn get_component<S: Into<String>>(&self, name: S) -> Option<ClockComponent> {
    let comp_name = name.into();

    if let Some((_, c)) = self.oscillators.iter().find(|(n, _)| **n == comp_name) {
      return Some(ClockComponent::Oscillator(c.clone()));
    }

    if let Some((_, c)) = self.multiplexers.iter().find(|(n, _)| **n == comp_name) {
      return Some(ClockComponent::Multiplexer(c.clone()));
    }

    if let Some((_, c)) = self.dividers.iter().find(|(n, _)| **n == comp_name) {
      return Some(ClockComponent::Divider(c.clone()));
    }

    if let Some((_, c)) = self.multipliers.iter().find(|(n, _)| **n == comp_name) {
      return Some(ClockComponent::Multiplier(c.clone()));
    }

    if let Some((_, c)) = self.taps.iter().find(|(n, _)| **n == comp_name) {
      return Some(ClockComponent::Tap(c.clone()));
    }

    None
  }

  fn get_next<S: Into<String>>(&self, name: S) -> Vec<String> {
    let comp_name = name.into();
    let mut next = Vec::new();

    next.extend(
      self
        .multiplexers
        .iter()
        .filter(|(_, c)| c.inputs.contains(&comp_name))
        .map(|(n, _)| n.clone()),
    );

    next.extend(
      self
        .dividers
        .iter()
        .filter(|(_, c)| c.input == comp_name)
        .map(|(n, _)| n.clone()),
    );

    next.extend(
      self
        .multipliers
        .iter()
        .filter(|(_, c)| c.input == comp_name)
        .map(|(n, _)| n.clone()),
    );

    next.extend(
      self
        .taps
        .iter()
        .filter(|(_, c)| c.input == comp_name)
        .map(|(n, _)| n.clone()),
    );

    next
  }

  fn list_outputs(&self, selection: ClockOutputNameSelection) -> Vec<String> {
    let terminal_taps_only = self
      .taps
      .iter()
      .filter(|(_, t)| t.terminal)
      .map(|(n, _)| n.clone());

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
          .iter()
          .filter(|(_, t)| !t.terminal)
          .map(|(n, _)| n.clone()),
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
      .flat_map(|d| d.inputs.iter().map(|i| i.clone()))
      .chain(self.dividers.values().map(|i| i.input.clone()))
      .chain(self.multipliers.values().map(|i| i.input.clone()))
      .chain(self.taps.values().map(|i| i.input.clone()))
      .collect::<Vec<String>>();

    inputs.sort();
    inputs.dedup();
    inputs
  }

  fn check_valid_names(&self) -> Result<()> {
    let allowed_chars: &'static str = "abcdefghijklmnopqrstuvwxyz0123456789";

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
        false => Some(i.clone()),
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
      .iter()
      .filter(|(_, d)| !d.inputs.contains(&d.default))
      .map(|(n, _)| n.clone())
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
      .iter()
      .filter(|(_, d)| !d.values.contains(&d.default))
      .map(|(n, _)| n.clone())
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
      .iter()
      .filter(|(_, m)| !m.values.contains(&m.default))
      .map(|(n, _)| n.clone())
      .collect::<Vec<String>>();

    if multipliers_with_bad_defaults.len() > 0 {
      return Err(anyhow!(
        "Multipliers have default values not in their value lists: {}",
        multipliers_with_bad_defaults.join(", ")
      ));
    }

    Ok(())
  }

  fn check_no_loops(&self) -> Result<()> {
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

    // Look for loops inside all the generated paths.
    let mut loops: Vec<Vec<String>> = Vec::new();
    for path in paths.iter() {
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

  fn make_paths(&self, path: &Vec<String>) -> Vec<Vec<String>> {
    match path.iter().last() {
      Some(l) => {
        // Get the possible next moves from the last path element.
        let next = self.get_next(l);

        // Multiply the original path into multiple copies, one for each
        // potential next move, then append those next moves to each. We
        // now have independent copies of each possible path.
        next
          .iter()
          .map(|n| {
            let mut new_path = path.clone();
            new_path.push(n.clone());
            new_path
          })
          .collect()
      }
      // If there was no last element, that means we were given an
      // empty path. There's nowhere to go, so just return an empty
      // one as well.
      None => Vec::new(),
    }
  }

  fn find_loop(path: &Vec<String>) -> Option<Vec<String>> {
    let mut path_loop = Vec::new();

    // Loop over every item except the last one in the path we were given. Each of these
    // is potentially the start of a loop.
    'outer: for (i, start_name) in path.iter().take(path.len() - 1).enumerate() {
      // Clear `path_loop` so we can store a new potential loop in it.
      path_loop = vec![start_name.clone()];

      // Loop over every item after our starting item and append it to `path_loop`.
      for next_name in path[i + 1..].iter() {
        // Append it to our potential path.
        path_loop.push(next_name.clone());
        // If an item after the starting item is the same as the starting item,
        // we've found a loop and can stop searching.
        if start_name == next_name {
          break 'outer;
        }
      }
    }

    // If we found a loop, return it.
    match path_loop.len() > 0 {
      true => Some(path_loop),
      false => None,
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Oscillator {
  frequency: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Multiplexer {
  inputs: Vec<String>,
  default: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Divider {
  input: String,
  default: u64,
  values: Vec<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Multiplier {
  input: String,
  default: u64,
  values: Vec<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tap {
  input: String,
  max: u64,
  terminal: bool,
}

#[cfg(test)]
mod tests {
  use super::*;
  use ron;

  const BASIC_RON: &'static str = r#"
      ClockSchematic(
        oscillators: {
          "Hse": (
            frequency: 8000000
          )
        },
        multiplexers: {
          "PllSourceMux": (
            inputs: [ "Hse" ],
            default: "Hse"
          )
        },
        dividers: {
          "PllDiv": (
            input: "PllSourceMux",
            default: 1,
            values: [1]
          )
        },
        multipliers: {
          "PllMul": (
            input: "PllDiv", 
            default: 3,
            values: [2,3,4]
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
          ),
          "Tap3": (
            input: "Tap1", 
            max: 0, 
            terminal: true
          )
        }
      )
    "#;

  #[test]
  fn deserializes_spec_string() {
    let spec: ClockSchematic = ron::from_str(BASIC_RON).unwrap();

    assert_eq!(1, spec.oscillators.len());
    assert_eq!(8_000_000, spec.oscillators["Hse"].frequency);

    assert_eq!(1, spec.multiplexers.len());
    assert_eq!(1, spec.multiplexers["PllSourceMux"].inputs.len());
    assert_eq!("Hse", spec.multiplexers["PllSourceMux"].default);
    assert_eq!("Hse", spec.multiplexers["PllSourceMux"].inputs[0]);

    assert_eq!(1, spec.dividers.len());
    assert_eq!("PllSourceMux", spec.dividers["PllDiv"].input);
    assert_eq!(1, spec.dividers["PllDiv"].default);
    assert_eq!(1, spec.dividers["PllDiv"].values.len());
    assert_eq!(1, spec.dividers["PllDiv"].values[0]);

    assert_eq!(1, spec.multipliers.len());
    assert_eq!("PllDiv", spec.multipliers["PllMul"].input);
    assert_eq!(3, spec.multipliers["PllMul"].default);
    assert_eq!(3, spec.multipliers["PllMul"].values.len());
    assert_eq!(2, spec.multipliers["PllMul"].values[0]);
    assert_eq!(3, spec.multipliers["PllMul"].values[1]);
    assert_eq!(4, spec.multipliers["PllMul"].values[2]);

    assert_eq!(3, spec.taps.len());

    assert_eq!("PllMul", spec.taps["Tap1"].input);
    assert_eq!(1000000, spec.taps["Tap1"].max);
    assert_eq!(false, spec.taps["Tap1"].terminal);

    assert_eq!("Tap1", spec.taps["Tap2"].input);
    assert_eq!(0, spec.taps["Tap2"].max);
    assert_eq!(true, spec.taps["Tap2"].terminal);

    assert_eq!("Tap1", spec.taps["Tap3"].input);
    assert_eq!(0, spec.taps["Tap3"].max);
    assert_eq!(true, spec.taps["Tap3"].terminal);
  }

  #[test]
  fn gets_components_by_name() {
    let spec: ClockSchematic = ron::from_str(BASIC_RON).unwrap();

    match spec.get_component("Hse") {
      Some(ClockComponent::Oscillator(_)) => {}
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("PllSourceMux") {
      Some(ClockComponent::Multiplexer(_)) => {}
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("PllDiv") {
      Some(ClockComponent::Divider(_)) => {}
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("PllMul") {
      Some(ClockComponent::Multiplier(_)) => {}
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("Tap1") {
      Some(ClockComponent::Tap(_)) => {}
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("Tap2") {
      Some(ClockComponent::Tap(_)) => {}
      _ => panic!("Returned wrong component"),
    };

    match spec.get_component("Bogus") {
      None => {}
      _ => panic!("Returned wrong component"),
    };
  }

  #[test]
  fn gets_next_components() {
    let spec: ClockSchematic = ron::from_str(BASIC_RON).unwrap();

    assert_eq!(vec!["PllSourceMux"], spec.get_next("Hse"));
    assert_eq!(vec!["PllDiv"], spec.get_next("PllSourceMux"));
    assert_eq!(vec!["PllMul"], spec.get_next("PllDiv"));
    assert_eq!(vec!["Tap1"], spec.get_next("PllMul"));

    let mut taps = spec.get_next("Tap1");
    taps.sort();
    assert_eq!(vec!["Tap2", "Tap3"], taps);

    assert_eq!(Vec::<String>::new(), spec.get_next("Bogus"));
  }

  #[test]
  fn lists_all_outputs() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut outputs = spec.list_outputs(ClockOutputNameSelection::Everything);

    outputs.sort();
    assert_eq!(
      outputs,
      vec![
        "Hse",
        "PllDiv",
        "PllMul",
        "PllSourceMux",
        "Tap1",
        "Tap2",
        "Tap3"
      ]
    );
  }

  #[test]
  fn lists_outputs_except_terminal_taps() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut outputs = spec.list_outputs(ClockOutputNameSelection::EverythingExceptTerminalTaps);

    outputs.sort();
    assert_eq!(
      vec!["Hse", "PllDiv", "PllMul", "PllSourceMux", "Tap1"],
      outputs
    );
  }

  #[test]
  fn lists_only_terminal_tap_outputs() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut outputs = spec.list_outputs(ClockOutputNameSelection::TerminalTapsOnly);
    outputs.sort();

    assert_eq!(vec!["Tap2", "Tap3"], outputs);
  }

  #[test]
  fn lists_inputs() {
    let spec = ClockSchematic::from_ron(BASIC_RON).unwrap();
    let mut inputs = spec.list_inputs();

    inputs.sort();
    assert_eq!(
      vec!["Hse", "PllDiv", "PllMul", "PllSourceMux", "Tap1"],
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
            inputs: [ "Hse" ],
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
            values: [1]
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
            values: [1]
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
            inputs: [ "Hse", "PllMul" ],
            default: "Hse"
          )
        },
        dividers: {
          "PllDiv": (
            input: "PllSourceMux",
            default: 1,
            values: [1]
          )
        },
        multipliers: {
          "PllMul": (
            input: "PllDiv", 
            default: 3,
            values: [2,3,4]
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
