use chrono::Local;
use colored::{ColoredString, Colorize};

fn timestamp() -> String {
    Local::now().format("%m/%d/%Y, %I:%M:%S %p").to_string()
}

fn log(tag: ColoredString, message: &str) {
    println!("[{}] - {} | {}", tag, message, timestamp().dimmed());
}

pub fn info(message: impl AsRef<str>) {
    log("info".blue(), message.as_ref());
}

pub fn success(message: impl AsRef<str>) {
    log("success".bright_green(), message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    log("warn".yellow(), message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    log("error".red(), message.as_ref());
}

pub fn chat(message: impl AsRef<str>) {
    log("chat".red(), message.as_ref());
}

pub fn advancement(message: impl AsRef<str>) {
    log("advancement".yellow(), message.as_ref());
}

pub fn death(message: impl AsRef<str>) {
    log("death".cyan(), message.as_ref());
}

pub fn join(message: impl AsRef<str>) {
    log("join".magenta(), message.as_ref());
}

pub fn leave(message: impl AsRef<str>) {
    log("leave".magenta(), message.as_ref());
}

pub fn kick(message: impl AsRef<str>) {
    log("kick".red(), message.as_ref());
}

pub fn login(message: impl AsRef<str>) {
    log("login".green(), message.as_ref());
}

pub fn logout(message: impl AsRef<str>) {
    log("logout".red(), message.as_ref());
}

pub fn spawn(message: impl AsRef<str>) {
    log("spawn".green(), message.as_ref());
}

pub fn world(message: impl AsRef<str>) {
    log("world".yellow(), message.as_ref());
}

pub fn command(message: impl AsRef<str>) {
    log("command".cyan(), message.as_ref());
}

pub fn websocket(message: impl AsRef<str>) {
    log("websocket".yellow(), message.as_ref());
}

pub fn debug(message: impl AsRef<str>) {
    if std::env::var("DEBUG").is_ok() {
        log("debug".bright_black(), message.as_ref());
    }
}
