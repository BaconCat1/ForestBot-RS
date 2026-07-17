pub const NAMES: &[&str] = &["hardware", "hw"];

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use sysinfo::{Disks, System};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Shows hardware information. Usage: {prefix}hardware",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let mut sys = System::new_all();
        sys.refresh_all();

        let os = System::name().unwrap_or_else(|| "Unknown".to_owned());
        let os_ver = System::os_version().unwrap_or_else(|| "Unknown".to_owned());
        let kernel = System::kernel_version().unwrap_or_else(|| "Unknown".to_owned());

        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_owned())
            .unwrap_or_else(|| "Unknown".to_owned());

        let total_ram = sys.total_memory() as f64 / 1_073_741_824.0;
        let used_ram = sys.used_memory() as f64 / 1_073_741_824.0;

        let disks = Disks::new_with_refreshed_list();
        let (disk_used, disk_total) = disks.iter().fold((0u64, 0u64), |(used, total), d| {
            (
                used + (d.total_space() - d.available_space()),
                total + d.total_space(),
            )
        });
        let disk_used_gb = disk_used as f64 / 1_073_741_824.0;
        let disk_total_gb = disk_total as f64 / 1_073_741_824.0;

        let uptime = format_uptime(System::uptime());

        ctx.chat_success(format!(
            "OS: {os} {os_ver} | Kernel: {kernel} | CPU: {cpu_brand} | RAM: {used_ram:.1}/{total_ram:.1} GB | Disk: {disk_used_gb:.1}/{disk_total_gb:.1} GB | Uptime: {uptime}"
        ));
        Ok(())
    })
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}
