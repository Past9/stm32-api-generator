use askama::Result;
use std::fmt;

pub fn all_caps(s: &dyn fmt::Display, str1: &str, num2: &usize) -> Result<String> {
  let s = s.to_string();
  Ok(format!(
    "{} {} {}",
    s.to_uppercase(),
    str1.to_string(),
    num2.to_string()
  ))
}

pub fn multi_write<S>(input: &[&str], separator: S) -> Result<String>
where
  S: AsRef<str>,
{
  let separator: &str = separator.as_ref();

  let mut rv = String::new();

  for (num, item) in input.iter().enumerate() {
    if num > 0 {
      rv.push_str(separator);
    }

    rv.push_str(&format!("{:?}", item));
  }

  Ok(rv)
}
