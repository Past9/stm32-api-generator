use anyhow::Result;
use serde::Deserialize;
use svd_expander::DeviceSpec;

fn to_code(instructions: Vec<WriteInstruction>, spec: &DeviceSpec) -> Result<String> {
  Ok(
    instructions
      .iter()
      .map(|f| f.to_code(spec))
      .collect::<Result<Vec<String>>>()?
      .join("\n"),
  )
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum WriteInstruction {
  Set(String, u32),
  Block(Vec<(String, u32)>),
}
impl WriteInstruction {
  pub fn to_code(&self, spec: &DeviceSpec) -> Result<String> {
    match self {
      WriteInstruction::Set(path, value) => Self::field_to_code(path, value, spec),
      WriteInstruction::Block(fields) => Ok(format!(
        "cortex_m::interrupt::free(|_| {{\n  {}\n}});",
        fields
          .iter()
          .map(|(path, value)| Self::field_to_code(path, value, spec))
          .collect::<Result<Vec<String>>>()?
          .join("\n  ")
      )),
    }
  }

  fn field_to_code(path: &String, value: &u32, spec: &DeviceSpec) -> Result<String> {
    let field = spec.get_field(path)?;
    Ok(format!(
      "write_val({}, {}, {}, {}, {});",
      field.address(),
      field.mask(),
      !field.mask(),
      field.offset,
      value
    ))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  #[test]
  fn deserializes_from_ron() {
    let _spec =
      DeviceSpec::from_xml(&fs::read_to_string("./specs/svd/arm_device.svd").unwrap()).unwrap();

    let ron = r#"
        [
          Set("timer0.cr.en", 1),
          Block([
            ("timer1.cr.en", 1),
            ("timer1.cr.rst", 1)
          ]),
          Set("timer0.cr.rst", 1)
        ]
      "#;

    let settings: Vec<WriteInstruction> = ron::from_str(ron).unwrap();

    assert_eq!(3, settings.len());
    assert_eq!(
      WriteInstruction::Set("timer0.cr.en".to_string(), 1),
      settings[0]
    );
    assert_eq!(
      WriteInstruction::Block(vec![
        ("timer1.cr.en".to_string(), 1),
        ("timer1.cr.rst".to_string(), 1)
      ]),
      settings[1]
    );
    assert_eq!(
      WriteInstruction::Set("timer0.cr.rst".to_string(), 1),
      settings[2]
    );
  }

  #[test]
  fn generates_code() {
    let spec =
      DeviceSpec::from_xml(&fs::read_to_string("./specs/svd/arm_device.svd").unwrap()).unwrap();

    let ron = r#"
        [
          Set("timer0.cr.en", 1),
          Block([
            ("timer1.cr.en", 1),
            ("timer1.cr.rst", 1)
          ]),
          Set("timer0.cr.rst", 1)
        ]
      "#;

    let settings: Vec<WriteInstruction> = ron::from_str(ron).unwrap();

    assert_eq!(
      r#"write_val(1073807360, 1, 4294967294, 0, 1);
cortex_m::interrupt::free(|_| {
  write_val(1073807616, 1, 4294967294, 0, 1);
  write_val(1073807616, 2, 4294967293, 1, 1);
});
write_val(1073807360, 2, 4294967293, 1, 1);"#,
      to_code(settings, &spec).unwrap()
    );
  }
}
