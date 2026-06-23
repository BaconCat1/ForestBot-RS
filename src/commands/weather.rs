use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["weather", "w"],
    description: "Current weather for a location. Usage: {prefix}weather <city>",
    whitelisted: false,
    execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}weather <city>", ctx.runtime.prefix));
            return Ok(());
        }
        let location = ctx.args.join(" ");
        match fetch_weather(&location).await {
            Some(msg) => ctx.chat(msg),
            None => ctx.chat(format!("No weather data found for: {location}")),
        }
        Ok(())
    })
}

async fn fetch_weather(location: &str) -> Option<String> {
    let client = reqwest::Client::new();

    // Step 1: geocode
    let geo_url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en",
        percent_encode(location)
    );
    let geo: serde_json::Value = client
        .get(&geo_url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let result = geo["results"].get(0)?;
    let lat = result["latitude"].as_f64()?;
    let lon = result["longitude"].as_f64()?;
    let city = result["name"].as_str().unwrap_or(location);
    let country = result["country_code"].as_str().unwrap_or("");
    let population = result["population"].as_u64().map(format_pop);

    // Step 2: current weather
    let wx_url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &current=temperature_2m,apparent_temperature,weather_code,\
        wind_speed_10m,wind_direction_10m,relative_humidity_2m,precipitation,is_day\
        &wind_speed_unit=kmh&temperature_unit=celsius&timezone=auto"
    );
    let wx: serde_json::Value = client
        .get(&wx_url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let cur = &wx["current"];
    let temp = cur["temperature_2m"].as_f64()?;
    let feels = cur["apparent_temperature"].as_f64()?;
    let code = cur["weather_code"].as_u64().unwrap_or(0);
    let wind_spd = cur["wind_speed_10m"].as_f64().unwrap_or(0.0);
    let wind_deg = cur["wind_direction_10m"].as_f64().unwrap_or(0.0);
    let humidity = cur["relative_humidity_2m"].as_f64().unwrap_or(0.0);
    let precip = cur["precipitation"].as_f64().unwrap_or(0.0);
    let is_day = cur["is_day"].as_u64().unwrap_or(1) == 1;

    let desc = wmo_desc(code);
    let dir = wind_dir(wind_deg);
    let day_emoji = if is_day { "☀" } else { "🌙" };
    let temp_str = format!("{temp:.0}°C");
    let feels_str = format!("{feels:.0}°C");

    let local_time = wx["utc_offset_seconds"].as_i64().map(|offset| {
        let utc = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let local = utc + offset;
        let h = (local % 86400) / 3600;
        let m = (local % 3600) / 60;
        format!(" [{h:02}:{m:02} local]")
    }).unwrap_or_default();

    let pop_str = population.map(|p| format!(", pop. {p}")).unwrap_or_default();
    let mut msg = format!(
        "{day_emoji} {city}, {country}{pop_str}{local_time}: {temp_str} (feels {feels_str}), {desc} | Wind {wind_spd:.0} km/h {dir} | {humidity:.0}% humidity",
    );
    if precip > 0.0 {
        msg.push_str(&format!(" | {precip:.1} mm precip"));
    }

    if msg.chars().count() > 255 {
        msg = format!("{}...", msg.chars().take(252).collect::<String>());
    }

    Some(msg)
}

fn wmo_desc(code: u64) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Fog",
        48 => "Icy fog",
        51 => "Light drizzle",
        53 => "Drizzle",
        55 => "Heavy drizzle",
        56 | 57 => "Freezing drizzle",
        61 => "Light rain",
        63 => "Rain",
        65 => "Heavy rain",
        66 | 67 => "Freezing rain",
        71 => "Light snow",
        73 => "Snow",
        75 => "Heavy snow",
        77 => "Snow grains",
        80 => "Light showers",
        81 => "Showers",
        82 => "Heavy showers",
        85 => "Snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm + hail",
        99 => "Thunderstorm + heavy hail",
        _ => "Unknown",
    }
}

fn wind_dir(deg: f64) -> &'static str {
    let idx = ((deg + 22.5) / 45.0) as usize % 8;
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][idx]
}

fn format_pop(p: u64) -> String {
    if p >= 1_000_000 {
        format!("{:.1}M", p as f64 / 1_000_000.0)
    } else if p >= 1_000 {
        format!("{:.0}K", p as f64 / 1_000.0)
    } else {
        p.to_string()
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
