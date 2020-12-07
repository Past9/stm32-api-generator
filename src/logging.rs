// RED   \x1b[0;31m
// GREEN  \x1b[0;32m
// CYAN  \x1b[0;36m
// NC    \x1b[0m

macro_rules! info {
  ($($arg:tt)*) => ({
    println!("\x1b[0;36m   [INFO]\x1b[0m {}", format!($($arg)*));
  })
}

macro_rules! error {
  ($($arg:tt)*) => ({
    println!("\x1b[0;31m  [ERROR]\x1b[0m {}", format!($($arg)*));
  })
}

macro_rules! success {
  ($($arg:tt)*) => ({
    println!("\x1b[0;32m[SUCCESS]\x1b[0m {}", format!($($arg)*));
  })
}
