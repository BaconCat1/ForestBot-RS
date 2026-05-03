use colored::Colorize;

pub fn info(message: impl AsRef<str>) {
    println!("{}", message.as_ref().blue());
}

pub fn success(message: impl AsRef<str>) {
    println!("{}", message.as_ref().green());
}

pub fn warn(message: impl AsRef<str>) {
    println!("{}", message.as_ref().yellow());
}

pub fn error(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref().red());
}
