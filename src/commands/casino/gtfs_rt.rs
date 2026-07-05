// GTFS-RT TripUpdates support for !train phase 2.
// Covers open (auth_type=0) rail-focused feeds discovered via Mobility Database catalog.
// prost types mirror a minimal subset of the GTFS-RT proto2 spec.

use prost::Message;

// ── Proto2 type definitions ───────────────────────────────────────────────────

#[derive(Clone, PartialEq, Message)]
pub struct FeedMessage {
    #[prost(message, optional, tag = "1")]
    pub header: Option<FeedHeader>,
    #[prost(message, repeated, tag = "2")]
    pub entity: Vec<FeedEntity>,
}

#[derive(Clone, PartialEq, Message)]
pub struct FeedHeader {
    #[prost(string, tag = "1")]
    pub gtfs_realtime_version: String,
    #[prost(uint64, optional, tag = "3")]
    pub timestamp: Option<u64>,
}

#[derive(Clone, PartialEq, Message)]
pub struct FeedEntity {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(message, optional, tag = "3")]
    pub trip_update: Option<TripUpdate>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TripUpdate {
    #[prost(message, optional, tag = "1")]
    pub trip: Option<TripDescriptor>,
    #[prost(message, repeated, tag = "2")]
    pub stop_time_update: Vec<StopTimeUpdate>,
}

#[derive(Clone, PartialEq, Message)]
pub struct TripDescriptor {
    #[prost(string, optional, tag = "1")]
    pub trip_id: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub route_id: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StopTimeUpdate {
    #[prost(message, optional, tag = "2")]
    pub arrival: Option<StopTimeEvent>,
    #[prost(message, optional, tag = "3")]
    pub departure: Option<StopTimeEvent>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StopTimeEvent {
    #[prost(int32, optional, tag = "1")]
    pub delay: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    pub time: Option<i64>,
}

// ── Agency config ─────────────────────────────────────────────────────────────

pub struct AgencyConfig {
    pub slug: &'static str,
    pub display: &'static str,
    pub tu_url: &'static str,
}

pub const AGENCIES: &[AgencyConfig] = &[
    AgencyConfig {
        slug: "mbta",
        display: "MBTA",
        tu_url: "https://cdn.mbta.com/realtime/TripUpdates.pb",
    },
    AgencyConfig {
        slug: "mta",
        display: "MTA NYC (1/2/3/4/5/6/7)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs",
    },
    AgencyConfig {
        slug: "mta-ace",
        display: "MTA NYC (A/C/E)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-ace",
    },
    AgencyConfig {
        slug: "mta-bdfm",
        display: "MTA NYC (B/D/F/M)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-bdfm",
    },
    AgencyConfig {
        slug: "mta-nqrw",
        display: "MTA NYC (N/Q/R/W)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw",
    },
    AgencyConfig {
        slug: "mta-l",
        display: "MTA NYC (L)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-l",
    },
    AgencyConfig {
        slug: "mta-g",
        display: "MTA NYC (G)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-g",
    },
    AgencyConfig {
        slug: "mta-jz",
        display: "MTA NYC (J/Z)",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-jz",
    },
    AgencyConfig {
        slug: "lirr",
        display: "LIRR",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/lirr%2Fgtfs-lirr",
    },
    AgencyConfig {
        slug: "metro-north",
        display: "Metro-North",
        tu_url: "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/mnr%2Fgtfs-mnr",
    },
];

pub fn resolve_agency(slug: &str) -> Option<&'static AgencyConfig> {
    let lower = slug.to_lowercase();
    AGENCIES.iter().find(|a| a.slug == lower.as_str())
}

// Rail route filter — MBTA mixes bus/rail in one feed; all other agencies above
// are rail-only feeds so every route qualifies.
pub fn is_rail_route(agency_slug: &str, route_id: &str) -> bool {
    if agency_slug != "mbta" {
        return true;
    }
    matches!(
        route_id,
        "Red" | "Orange" | "Blue" | "Green-B" | "Green-C" | "Green-D" | "Green-E" | "Mattapan"
    ) || route_id.starts_with("CR-")
}

// ── Trip snapshot ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TripSnapshot {
    pub trip_id: String,
    pub route_id: String,
    pub first_stop_time: u64,   // unix ts of first remaining stop departure/arrival
    pub last_stop_time: u64,    // unix ts of last stop arrival/departure
    pub first_delay_secs: i32,
    pub last_delay_secs: i32,
}

// ── Fetch ─────────────────────────────────────────────────────────────────────

pub async fn fetch_trip_updates(
    client: &reqwest::Client,
    url: &str,
) -> Option<Vec<TripUpdate>> {
    let bytes = client
        .get(url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;
    let feed = FeedMessage::decode(bytes).ok()?;
    Some(
        feed.entity
            .into_iter()
            .filter_map(|e| e.trip_update)
            .collect(),
    )
}

// ── Parse helpers ─────────────────────────────────────────────────────────────

fn stu_time(stu: &StopTimeUpdate) -> Option<u64> {
    stu.departure
        .as_ref()
        .or(stu.arrival.as_ref())
        .and_then(|e| e.time)
        .map(|t| t as u64)
}

fn stu_delay(stu: &StopTimeUpdate) -> i32 {
    stu.departure
        .as_ref()
        .or(stu.arrival.as_ref())
        .and_then(|e| e.delay)
        .unwrap_or(0)
}

fn snapshot_of(tu: &TripUpdate) -> Option<TripSnapshot> {
    let first = tu.stop_time_update.first()?;
    let last = tu.stop_time_update.last()?;
    let first_time = stu_time(first)?;
    let last_time = stu_time(last).unwrap_or(first_time + 600);
    Some(TripSnapshot {
        trip_id: tu.trip.as_ref().and_then(|t| t.trip_id.clone()).unwrap_or_default(),
        route_id: tu.trip.as_ref().and_then(|t| t.route_id.clone()).unwrap_or_default(),
        first_stop_time: first_time,
        last_stop_time: last_time,
        first_delay_secs: stu_delay(first),
        last_delay_secs: stu_delay(last),
    })
}

// For !train list: returns one snapshot per route — soonest pre-departure trip.
pub fn rail_trips_by_route(
    trips: &[TripUpdate],
    agency_slug: &str,
    now: u64,
) -> std::collections::HashMap<String, TripSnapshot> {
    use std::collections::hash_map::Entry;
    let mut map: std::collections::HashMap<String, TripSnapshot> = Default::default();
    for tu in trips {
        let route_id = match tu.trip.as_ref().and_then(|t| t.route_id.as_deref()) {
            Some(r) => r,
            None => continue,
        };
        if !is_rail_route(agency_slug, route_id) {
            continue;
        }
        let Some(snap) = snapshot_of(tu) else { continue };
        if snap.first_stop_time <= now {
            continue; // already departed
        }
        match map.entry(route_id.to_owned()) {
            Entry::Vacant(e) => { e.insert(snap); }
            Entry::Occupied(mut e) => {
                if snap.first_stop_time < e.get().first_stop_time {
                    e.insert(snap);
                }
            }
        }
    }
    map
}

// For place_bet: soonest pre-departure trip for a specific route.
pub fn find_next_predeparture(
    trips: &[TripUpdate],
    agency_slug: &str,
    route_id: &str,
    now: u64,
) -> Option<TripSnapshot> {
    trips
        .iter()
        .filter(|tu| {
            tu.trip
                .as_ref()
                .and_then(|t| t.route_id.as_deref())
                .map(|r| r.eq_ignore_ascii_case(route_id))
                .unwrap_or(false)
                && is_rail_route(agency_slug, route_id)
        })
        .filter_map(snapshot_of)
        .filter(|s| s.first_stop_time > now)
        .min_by_key(|s| s.first_stop_time)
}

// For settlement: find a running trip by its trip_id.
pub fn find_trip_by_id(trips: &[TripUpdate], trip_id: &str) -> Option<TripSnapshot> {
    trips
        .iter()
        .find(|tu| {
            tu.trip
                .as_ref()
                .and_then(|t| t.trip_id.as_deref())
                == Some(trip_id)
        })
        .and_then(snapshot_of)
}
