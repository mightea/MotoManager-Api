//! Trip analytics derived from fuel stops and typed locations: per-location
//! visit aggregates, legs between consecutive fuel stops, multi-day tour
//! detection, per-year range statistics and a riding calendar.
//!
//! Pure functions over plain inputs so the `GET /api/map` handler and the MCP
//! `get_riding_summary` tool share one implementation and the rules are unit
//! testable without a database. All distances are kilometres, dates are
//! `YYYY-MM-DD` strings (longer ISO timestamps are truncated to the date).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use serde::Serialize;

use crate::handlers::locations::haversine_meters;
use crate::models::{Location, LocationType};

/// A fuel stop closer than this to home counts as "home side"; anything
/// farther away is part of a trip.
pub const HOME_RADIUS_KM: f64 = 80.0;
/// A gap of more than this many days between two stops ends a tour.
pub const TOUR_MAX_GAP_DAYS: i64 = 4;
/// Key used in `kmByCountry` for legs whose destination has no country code.
pub const UNKNOWN_COUNTRY: &str = "??";

// MARK: - Inputs

#[derive(Debug, Clone)]
pub struct MotorcycleRef {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub status: String,
}

/// One `maintenanceRecords` row of type `fuel`.
#[derive(Debug, Clone)]
pub struct FuelRecord {
    pub id: i64,
    pub motorcycle_id: i64,
    pub date: String,
    pub odo: i64,
    pub location_id: Option<i64>,
    pub liters: Option<f64>,
    /// `normalizedCost ?? cost`.
    pub cost: Option<f64>,
    pub consumption: Option<f64>,
}

/// A non-fuel maintenance record that happened at a location.
#[derive(Debug, Clone)]
pub struct MaintenanceVisit {
    pub motorcycle_id: i64,
    pub date: String,
    pub location_id: i64,
    /// `(normalizedCost ?? cost ?? 0) + (partsCost ?? 0)`.
    pub cost: f64,
}

/// An explicit "the bike is here" marker (`locationRecords`, or a
/// maintenance record of type `location`).
#[derive(Debug, Clone)]
pub struct ParkedVisit {
    pub id: i64,
    pub motorcycle_id: i64,
    pub date: String,
    pub location_id: i64,
}

#[derive(Debug, Clone, Default)]
pub struct MapInput {
    pub motorcycles: Vec<MotorcycleRef>,
    pub locations: Vec<Location>,
    pub fuel_records: Vec<FuelRecord>,
    pub maintenance_visits: Vec<MaintenanceVisit>,
    pub parked_visits: Vec<ParkedVisit>,
}

// MARK: - Outputs

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotorcycleOut {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    pub location_id: i64,
    pub latitude: f64,
    pub longitude: f64,
    /// How home was chosen: `locationRecord`, `storage` or `mostVisited`.
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerMotorcycleVisits {
    pub motorcycle_id: i64,
    pub fuel_stops: i64,
    pub liters: f64,
    pub maintenance_visits: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct YearVisits {
    pub fuel_stops: i64,
    pub liters: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationOut {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub location_type: LocationType,
    pub latitude: f64,
    pub longitude: f64,
    pub country_code: Option<String>,
    pub fuel_stops: i64,
    pub liters: f64,
    pub fuel_cost: f64,
    pub maintenance_visits: i64,
    pub maintenance_cost: f64,
    pub parked_visits: i64,
    pub first_visit: Option<String>,
    pub last_visit: Option<String>,
    pub distance_from_home_km: Option<f64>,
    pub per_motorcycle: Vec<PerMotorcycleVisits>,
    pub years: BTreeMap<String, YearVisits>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UngeocodedOut {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub location_type: LocationType,
    pub visits: i64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leg {
    pub id: String,
    pub motorcycle_id: i64,
    pub from_record_id: i64,
    pub to_record_id: i64,
    pub from_location_id: Option<i64>,
    pub to_location_id: Option<i64>,
    pub from: Option<Point>,
    pub to: Option<Point>,
    pub start_date: String,
    pub end_date: String,
    pub days: i64,
    pub distance_km: i64,
    pub straight_km: Option<f64>,
    pub liters: Option<f64>,
    pub consumption: Option<f64>,
    pub cost: Option<f64>,
    pub year: i32,
    pub tour_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tour {
    pub id: String,
    pub motorcycle_id: i64,
    pub start_date: String,
    pub end_date: String,
    pub days: i64,
    pub distance_km: i64,
    pub liters: f64,
    pub cost: f64,
    pub stops: i64,
    pub max_distance_from_home_km: Option<f64>,
    pub farthest_location_id: Option<i64>,
    pub countries: Vec<String>,
    pub location_ids: Vec<i64>,
    pub leg_ids: Vec<String>,
    pub year: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Farthest {
    pub location_id: i64,
    pub distance_km: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PerMotorcycleYear {
    pub motorcycle_id: i64,
    pub distance_km: i64,
    pub liters: f64,
    pub fuel_stops: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearStats {
    pub year: i32,
    pub distance_km: i64,
    pub liters: f64,
    pub fuel_cost: f64,
    pub fuel_stops: i64,
    pub riding_days: i64,
    pub tours: i64,
    pub day_trips: i64,
    pub countries: Vec<String>,
    pub km_by_country: BTreeMap<String, i64>,
    pub farthest: Option<Farthest>,
    pub season_start: Option<String>,
    pub season_end: Option<String>,
    pub per_motorcycle: Vec<PerMotorcycleYear>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDay {
    pub date: String,
    pub km: i64,
    pub liters: f64,
    pub stops: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPayload {
    pub motorcycles: Vec<MotorcycleOut>,
    pub home: Option<Home>,
    pub locations: Vec<LocationOut>,
    pub ungeocoded: Vec<UngeocodedOut>,
    pub unlocated_fuel_count: i64,
    pub legs: Vec<Leg>,
    pub tours: Vec<Tour>,
    pub years: Vec<YearStats>,
    pub calendar: Vec<CalendarDay>,
}

// MARK: - Helpers

/// `YYYY-MM-DD` prefix of a date or ISO timestamp.
pub fn date_only(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.len() > 10 {
        &trimmed[..10]
    } else {
        trimmed
    }
}

fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_only(raw), "%Y-%m-%d").ok()
}

fn year_of(raw: &str) -> i32 {
    date_only(raw)
        .get(..4)
        .and_then(|y| y.parse().ok())
        .unwrap_or(0)
}

/// Calendar-day difference `b - a`; 0 when either side is unparsable.
fn day_delta(a: &str, b: &str) -> i64 {
    match (parse_date(a), parse_date(b)) {
        (Some(a), Some(b)) => (b - a).num_days(),
        _ => 0,
    }
}

fn coords(loc: &Location) -> Option<Point> {
    match (loc.latitude, loc.longitude) {
        (Some(latitude), Some(longitude)) => Some(Point {
            latitude,
            longitude,
        }),
        _ => None,
    }
}

fn km_between(a: Point, b: Point) -> f64 {
    haversine_meters(a.latitude, a.longitude, b.latitude, b.longitude) / 1000.0
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn min_date(current: &mut Option<String>, candidate: &str) {
    let candidate = date_only(candidate);
    if current.as_deref().is_none_or(|c| candidate < c) {
        *current = Some(candidate.to_string());
    }
}

fn max_date(current: &mut Option<String>, candidate: &str) {
    let candidate = date_only(candidate);
    if current.as_deref().is_none_or(|c| candidate > c) {
        *current = Some(candidate.to_string());
    }
}

fn per_motorcycle_entry(list: &mut Vec<PerMotorcycleVisits>, id: i64) -> &mut PerMotorcycleVisits {
    if let Some(index) = list.iter().position(|p| p.motorcycle_id == id) {
        &mut list[index]
    } else {
        list.push(PerMotorcycleVisits {
            motorcycle_id: id,
            ..Default::default()
        });
        list.last_mut().expect("just pushed")
    }
}

fn per_motorcycle_year(list: &mut Vec<PerMotorcycleYear>, id: i64) -> &mut PerMotorcycleYear {
    if let Some(index) = list.iter().position(|p| p.motorcycle_id == id) {
        &mut list[index]
    } else {
        list.push(PerMotorcycleYear {
            motorcycle_id: id,
            ..Default::default()
        });
        list.last_mut().expect("just pushed")
    }
}

// MARK: - Analysis

/// Pick the user's home: the latest explicit "bike is here" marker with
/// coordinates, else a `storage` location with coordinates (lowest id), else
/// the fuel station with the most stops.
fn select_home(
    locations: &HashMap<i64, &Location>,
    fuel_records: &[FuelRecord],
    parked_visits: &[ParkedVisit],
) -> Option<Home> {
    let mut parked: Vec<&ParkedVisit> = parked_visits.iter().collect();
    parked.sort_by(|a, b| {
        date_only(&b.date)
            .cmp(date_only(&a.date))
            .then(b.id.cmp(&a.id))
    });
    for visit in parked {
        if let Some(point) = locations.get(&visit.location_id).and_then(|l| coords(l)) {
            return Some(Home {
                location_id: visit.location_id,
                latitude: point.latitude,
                longitude: point.longitude,
                source: "locationRecord",
            });
        }
    }

    let mut storages: Vec<&&Location> = locations
        .values()
        .filter(|l| matches!(l.location_type, LocationType::Storage) && coords(l).is_some())
        .collect();
    storages.sort_by_key(|l| l.id);
    if let Some(storage) = storages.first() {
        let point = coords(storage).expect("filtered on coords");
        return Some(Home {
            location_id: storage.id,
            latitude: point.latitude,
            longitude: point.longitude,
            source: "storage",
        });
    }

    let mut stop_counts: HashMap<i64, i64> = HashMap::new();
    for record in fuel_records {
        if let Some(id) = record.location_id {
            if locations.get(&id).and_then(|l| coords(l)).is_some() {
                *stop_counts.entry(id).or_default() += 1;
            }
        }
    }
    stop_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(id, _)| {
            let point = coords(locations[&id]).expect("counted only with coords");
            Home {
                location_id: id,
                latitude: point.latitude,
                longitude: point.longitude,
                source: "mostVisited",
            }
        })
}

/// A tour candidate while walking one motorcycle's fuel stops.
struct OpenTour {
    /// Indices into the motorcycle's sorted stop list.
    stop_indices: Vec<usize>,
}

pub fn analyze(input: MapInput) -> MapPayload {
    let location_map: HashMap<i64, &Location> = input.locations.iter().map(|l| (l.id, l)).collect();
    let home = select_home(&location_map, &input.fuel_records, &input.parked_visits);
    let home_point = home.as_ref().map(|h| Point {
        latitude: h.latitude,
        longitude: h.longitude,
    });
    let distance_from_home = |id: i64| -> Option<f64> {
        let point = location_map.get(&id).and_then(|l| coords(l))?;
        home_point.map(|h| km_between(h, point))
    };

    // Per-location aggregates.
    #[derive(Default)]
    struct LocAgg {
        fuel_stops: i64,
        liters: f64,
        fuel_cost: f64,
        maintenance_visits: i64,
        maintenance_cost: f64,
        parked_visits: i64,
        first_visit: Option<String>,
        last_visit: Option<String>,
        per_motorcycle: Vec<PerMotorcycleVisits>,
        years: BTreeMap<String, YearVisits>,
    }
    let mut aggregates: HashMap<i64, LocAgg> = HashMap::new();
    let mut unlocated_fuel_count = 0i64;
    for record in &input.fuel_records {
        let Some(id) = record.location_id else {
            unlocated_fuel_count += 1;
            continue;
        };
        if !location_map.contains_key(&id) {
            unlocated_fuel_count += 1;
            continue;
        }
        let liters = record.liters.unwrap_or(0.0);
        let agg = aggregates.entry(id).or_default();
        agg.fuel_stops += 1;
        agg.liters += liters;
        agg.fuel_cost += record.cost.unwrap_or(0.0);
        min_date(&mut agg.first_visit, &record.date);
        max_date(&mut agg.last_visit, &record.date);
        let per = per_motorcycle_entry(&mut agg.per_motorcycle, record.motorcycle_id);
        per.fuel_stops += 1;
        per.liters += liters;
        let year = agg
            .years
            .entry(year_of(&record.date).to_string())
            .or_default();
        year.fuel_stops += 1;
        year.liters += liters;
    }
    for visit in &input.maintenance_visits {
        if !location_map.contains_key(&visit.location_id) {
            continue;
        }
        let agg = aggregates.entry(visit.location_id).or_default();
        agg.maintenance_visits += 1;
        agg.maintenance_cost += visit.cost;
        min_date(&mut agg.first_visit, &visit.date);
        max_date(&mut agg.last_visit, &visit.date);
        per_motorcycle_entry(&mut agg.per_motorcycle, visit.motorcycle_id).maintenance_visits += 1;
    }
    for visit in &input.parked_visits {
        if !location_map.contains_key(&visit.location_id) {
            continue;
        }
        let agg = aggregates.entry(visit.location_id).or_default();
        agg.parked_visits += 1;
        min_date(&mut agg.first_visit, &visit.date);
        max_date(&mut agg.last_visit, &visit.date);
    }

    let mut locations_out = Vec::new();
    let mut ungeocoded = Vec::new();
    let mut sorted_locations: Vec<&Location> = input.locations.iter().collect();
    sorted_locations.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    for loc in sorted_locations {
        let agg = aggregates.remove(&loc.id);
        match coords(loc) {
            Some(point) => {
                let mut agg = agg.unwrap_or_default();
                agg.per_motorcycle.sort_by_key(|p| p.motorcycle_id);
                for per in &mut agg.per_motorcycle {
                    per.liters = round2(per.liters);
                }
                for year in agg.years.values_mut() {
                    year.liters = round2(year.liters);
                }
                locations_out.push(LocationOut {
                    id: loc.id,
                    name: loc.name.clone(),
                    location_type: loc.location_type,
                    latitude: point.latitude,
                    longitude: point.longitude,
                    country_code: loc.country_code.clone(),
                    fuel_stops: agg.fuel_stops,
                    liters: round2(agg.liters),
                    fuel_cost: round2(agg.fuel_cost),
                    maintenance_visits: agg.maintenance_visits,
                    maintenance_cost: round2(agg.maintenance_cost),
                    parked_visits: agg.parked_visits,
                    first_visit: agg.first_visit,
                    last_visit: agg.last_visit,
                    distance_from_home_km: distance_from_home(loc.id).map(round1),
                    per_motorcycle: agg.per_motorcycle,
                    years: agg.years,
                });
            }
            None => {
                let visits = agg
                    .map(|a| a.fuel_stops + a.maintenance_visits + a.parked_visits)
                    .unwrap_or(0);
                ungeocoded.push(UngeocodedOut {
                    id: loc.id,
                    name: loc.name.clone(),
                    location_type: loc.location_type,
                    visits,
                });
            }
        }
    }
    ungeocoded.sort_by(|a, b| b.visits.cmp(&a.visits).then(a.name.cmp(&b.name)));

    // Legs and tours, per motorcycle.
    let mut legs: Vec<Leg> = Vec::new();
    let mut tours: Vec<Tour> = Vec::new();
    let mut day_trips_by_year: BTreeMap<i32, i64> = BTreeMap::new();

    let mut by_motorcycle: BTreeMap<i64, Vec<&FuelRecord>> = BTreeMap::new();
    for record in &input.fuel_records {
        by_motorcycle
            .entry(record.motorcycle_id)
            .or_default()
            .push(record);
    }

    for (motorcycle_id, mut stops) in by_motorcycle {
        stops.sort_by(|a, b| {
            date_only(&a.date)
                .cmp(date_only(&b.date))
                .then(a.odo.cmp(&b.odo))
                .then(a.id.cmp(&b.id))
        });

        // Legs: consecutive stops with a positive odometer delta.
        let mut leg_index: HashMap<(i64, i64), usize> = HashMap::new();
        for pair in stops.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let distance_km = to.odo - from.odo;
            if distance_km <= 0 {
                continue;
            }
            let from_point = from
                .location_id
                .and_then(|id| location_map.get(&id))
                .and_then(|l| coords(l));
            let to_point = to
                .location_id
                .and_then(|id| location_map.get(&id))
                .and_then(|l| coords(l));
            let straight_km = match (from_point, to_point) {
                (Some(a), Some(b)) => Some(round1(km_between(a, b))),
                _ => None,
            };
            leg_index.insert((from.id, to.id), legs.len());
            legs.push(Leg {
                id: format!("{motorcycle_id}-{}-{}", from.id, to.id),
                motorcycle_id,
                from_record_id: from.id,
                to_record_id: to.id,
                from_location_id: from.location_id.filter(|id| location_map.contains_key(id)),
                to_location_id: to.location_id.filter(|id| location_map.contains_key(id)),
                from: from_point,
                to: to_point,
                start_date: date_only(&from.date).to_string(),
                end_date: date_only(&to.date).to_string(),
                days: day_delta(&from.date, &to.date).max(0),
                distance_km,
                straight_km,
                liters: to.liters,
                consumption: to.consumption,
                cost: to.cost,
                year: year_of(&to.date),
                tour_id: None,
            });
        }

        // Tours.
        let stop_distance =
            |index: usize| -> Option<f64> { stops[index].location_id.and_then(distance_from_home) };
        let is_away = |index: usize| stop_distance(index).is_some_and(|d| d > HOME_RADIUS_KM);
        let is_home_side = |index: usize| stop_distance(index).is_some_and(|d| d <= HOME_RADIUS_KM);
        let gap = |a: usize, b: usize| day_delta(&stops[a].date, &stops[b].date);

        let mut finished: Vec<Vec<usize>> = Vec::new();
        let mut open: Option<OpenTour> = None;
        for index in 0..stops.len() {
            if let Some(tour) = open.as_mut() {
                let previous = *tour.stop_indices.last().expect("open tour has stops");
                if gap(previous, index) > TOUR_MAX_GAP_DAYS {
                    finished.push(open.take().expect("checked").stop_indices);
                } else {
                    tour.stop_indices.push(index);
                    if is_home_side(index) {
                        finished.push(open.take().expect("checked").stop_indices);
                    }
                    continue;
                }
            }
            if is_away(index) {
                let start = if index > 0 && gap(index - 1, index) <= TOUR_MAX_GAP_DAYS {
                    index - 1
                } else {
                    index
                };
                open = Some(OpenTour {
                    stop_indices: (start..=index).collect(),
                });
            }
        }
        if let Some(tour) = open.take() {
            finished.push(tour.stop_indices);
        }

        for indices in finished {
            let first = stops[indices[0]];
            let last = stops[*indices.last().expect("non-empty")];
            let start_date = date_only(&first.date).to_string();
            let end_date = date_only(&last.date).to_string();
            if end_date <= start_date {
                *day_trips_by_year.entry(year_of(&first.date)).or_default() += 1;
                continue;
            }
            let tour_id = format!("{motorcycle_id}-{start_date}");
            let mut distance_km = 0i64;
            let mut liters = 0.0;
            let mut cost = 0.0;
            let mut leg_ids = Vec::new();
            for pair in indices.windows(2) {
                let (from, to) = (stops[pair[0]], stops[pair[1]]);
                if let Some(&leg_position) = leg_index.get(&(from.id, to.id)) {
                    let leg = &mut legs[leg_position];
                    leg.tour_id = Some(tour_id.clone());
                    distance_km += leg.distance_km;
                    leg_ids.push(leg.id.clone());
                }
                liters += to.liters.unwrap_or(0.0);
                cost += to.cost.unwrap_or(0.0);
            }
            let mut location_ids: Vec<i64> = Vec::new();
            let mut countries: BTreeSet<String> = BTreeSet::new();
            let mut farthest: Option<(i64, f64)> = None;
            for &index in &indices {
                let Some(id) = stops[index].location_id else {
                    continue;
                };
                let Some(loc) = location_map.get(&id) else {
                    continue;
                };
                if !location_ids.contains(&id) {
                    location_ids.push(id);
                }
                if let Some(code) = &loc.country_code {
                    countries.insert(code.clone());
                }
                if let Some(distance) = distance_from_home(id) {
                    if farthest.is_none_or(|(_, best)| distance > best) {
                        farthest = Some((id, distance));
                    }
                }
            }
            tours.push(Tour {
                id: tour_id,
                motorcycle_id,
                days: day_delta(&start_date, &end_date) + 1,
                year: year_of(&start_date),
                start_date,
                end_date,
                distance_km,
                liters: round2(liters),
                cost: round2(cost),
                stops: indices.len() as i64,
                max_distance_from_home_km: farthest.map(|(_, d)| round1(d)),
                farthest_location_id: farthest.map(|(id, _)| id),
                countries: countries.into_iter().collect(),
                location_ids,
                leg_ids,
            });
        }
    }
    tours.sort_by(|a, b| {
        a.start_date
            .cmp(&b.start_date)
            .then(a.motorcycle_id.cmp(&b.motorcycle_id))
    });

    // Years.
    #[derive(Default)]
    struct YearAgg {
        distance_km: i64,
        liters: f64,
        fuel_cost: f64,
        fuel_stops: i64,
        riding_days: BTreeSet<String>,
        countries: BTreeSet<String>,
        km_by_country: BTreeMap<String, i64>,
        farthest: Option<(i64, f64)>,
        season_start: Option<String>,
        season_end: Option<String>,
        per_motorcycle: Vec<PerMotorcycleYear>,
    }
    let mut years: BTreeMap<i32, YearAgg> = BTreeMap::new();
    let mut calendar: BTreeMap<String, CalendarDay> = BTreeMap::new();
    for record in &input.fuel_records {
        let liters = record.liters.unwrap_or(0.0);
        let date = date_only(&record.date).to_string();
        let year = years.entry(year_of(&record.date)).or_default();
        year.liters += liters;
        year.fuel_cost += record.cost.unwrap_or(0.0);
        year.fuel_stops += 1;
        year.riding_days.insert(date.clone());
        min_date(&mut year.season_start, &date);
        max_date(&mut year.season_end, &date);
        let per = per_motorcycle_year(&mut year.per_motorcycle, record.motorcycle_id);
        per.liters += liters;
        per.fuel_stops += 1;
        if let Some(id) = record.location_id {
            if let Some(loc) = location_map.get(&id) {
                if let Some(code) = &loc.country_code {
                    year.countries.insert(code.clone());
                }
                if let Some(distance) = distance_from_home(id) {
                    if year.farthest.is_none_or(|(_, best)| distance > best) {
                        year.farthest = Some((id, distance));
                    }
                }
            }
        }
        let day = calendar.entry(date.clone()).or_insert_with(|| CalendarDay {
            date,
            ..Default::default()
        });
        day.liters += liters;
        day.stops += 1;
    }
    for leg in &legs {
        let year = years.entry(leg.year).or_default();
        year.distance_km += leg.distance_km;
        per_motorcycle_year(&mut year.per_motorcycle, leg.motorcycle_id).distance_km +=
            leg.distance_km;
        let country = leg
            .to_location_id
            .and_then(|id| location_map.get(&id))
            .and_then(|l| l.country_code.clone())
            .unwrap_or_else(|| UNKNOWN_COUNTRY.to_string());
        *year.km_by_country.entry(country).or_default() += leg.distance_km;
        if let Some(day) = calendar.get_mut(&leg.end_date) {
            day.km += leg.distance_km;
        }
    }
    for tour in &tours {
        years.entry(tour.year).or_default();
    }
    let tour_counts: BTreeMap<i32, i64> = tours.iter().fold(BTreeMap::new(), |mut acc, t| {
        *acc.entry(t.year).or_default() += 1;
        acc
    });
    let years_out: Vec<YearStats> = years
        .into_iter()
        .map(|(year, mut agg)| {
            agg.per_motorcycle.sort_by_key(|p| p.motorcycle_id);
            for per in &mut agg.per_motorcycle {
                per.liters = round2(per.liters);
            }
            YearStats {
                year,
                distance_km: agg.distance_km,
                liters: round2(agg.liters),
                fuel_cost: round2(agg.fuel_cost),
                fuel_stops: agg.fuel_stops,
                riding_days: agg.riding_days.len() as i64,
                tours: tour_counts.get(&year).copied().unwrap_or(0),
                day_trips: day_trips_by_year.get(&year).copied().unwrap_or(0),
                countries: agg.countries.into_iter().collect(),
                km_by_country: agg.km_by_country,
                farthest: agg.farthest.map(|(location_id, distance)| Farthest {
                    location_id,
                    distance_km: round1(distance),
                }),
                season_start: agg.season_start,
                season_end: agg.season_end,
                per_motorcycle: agg.per_motorcycle,
            }
        })
        .collect();

    let calendar: Vec<CalendarDay> = calendar
        .into_values()
        .map(|mut day| {
            day.liters = round2(day.liters);
            day
        })
        .collect();

    MapPayload {
        motorcycles: input
            .motorcycles
            .iter()
            .map(|m| MotorcycleOut {
                id: m.id,
                make: m.make.clone(),
                model: m.model.clone(),
                status: m.status.clone(),
            })
            .collect(),
        home,
        locations: locations_out,
        ungeocoded,
        unlocated_fuel_count,
        legs,
        tours,
        years: years_out,
        calendar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(
        id: i64,
        kind: LocationType,
        lat: Option<f64>,
        lon: Option<f64>,
        cc: Option<&str>,
    ) -> Location {
        Location {
            id,
            name: format!("Loc {id}"),
            location_type: kind,
            latitude: lat,
            longitude: lon,
            country_code: cc.map(str::to_string),
            user_id: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: None,
        }
    }

    fn fuel(id: i64, date: &str, odo: i64, location_id: Option<i64>) -> FuelRecord {
        FuelRecord {
            id,
            motorcycle_id: 1,
            date: date.to_string(),
            odo,
            location_id,
            liters: Some(10.0),
            cost: Some(20.0),
            consumption: Some(5.0),
        }
    }

    /// Zurich home, a station 30 km away, Milan (~215 km) and Florence (~450 km).
    fn geo_input() -> MapInput {
        MapInput {
            motorcycles: vec![MotorcycleRef {
                id: 1,
                make: "BMW".into(),
                model: "GS".into(),
                status: "active".into(),
            }],
            locations: vec![
                location(
                    1,
                    LocationType::Storage,
                    Some(47.3769),
                    Some(8.5417),
                    Some("CH"),
                ),
                location(
                    2,
                    LocationType::FuelStation,
                    Some(47.5),
                    Some(8.9),
                    Some("CH"),
                ),
                location(
                    3,
                    LocationType::FuelStation,
                    Some(45.4642),
                    Some(9.19),
                    Some("IT"),
                ),
                location(
                    4,
                    LocationType::FuelStation,
                    Some(43.7696),
                    Some(11.2558),
                    Some("IT"),
                ),
                location(5, LocationType::MaintenanceShop, None, None, None),
            ],
            fuel_records: Vec::new(),
            maintenance_visits: Vec::new(),
            parked_visits: Vec::new(),
        }
    }

    #[test]
    fn date_helpers_truncate_and_diff() {
        assert_eq!(date_only("2024-06-01T10:00:00Z"), "2024-06-01");
        assert_eq!(day_delta("2024-06-01", "2024-06-03T08:00:00Z"), 2);
        assert_eq!(year_of("2023-12-31"), 2023);
        assert_eq!(day_delta("garbage", "2024-01-01"), 0);
    }

    #[test]
    fn legs_follow_consecutive_stops_with_positive_odo_delta() {
        let mut input = geo_input();
        input.fuel_records = vec![
            fuel(10, "2024-06-01", 1000, Some(2)),
            fuel(11, "2024-06-03", 1300, Some(3)),
            fuel(12, "2024-06-03", 1300, None), // zero delta → no leg
            fuel(13, "2024-06-04", 1500, None), // no coords on either end
        ];
        let payload = analyze(input);
        assert_eq!(payload.legs.len(), 2);
        let first = &payload.legs[0];
        assert_eq!(first.id, "1-10-11");
        assert_eq!(first.distance_km, 300);
        assert_eq!(first.days, 2);
        assert_eq!(first.year, 2024);
        assert!(first.straight_km.unwrap() > 200.0);
        assert_eq!(first.liters, Some(10.0));
        let second = &payload.legs[1];
        assert_eq!(second.from_record_id, 12);
        assert!(second.from.is_none() && second.to.is_none());
        assert_eq!(second.straight_km, None);
        assert_eq!(payload.unlocated_fuel_count, 2);
    }

    #[test]
    fn home_prefers_parked_marker_then_storage_then_most_visited() {
        let mut input = geo_input();
        input.fuel_records = vec![
            fuel(1, "2024-01-01", 100, Some(2)),
            fuel(2, "2024-01-02", 200, Some(2)),
        ];
        input.parked_visits = vec![
            ParkedVisit {
                id: 1,
                motorcycle_id: 1,
                date: "2024-05-01".into(),
                location_id: 5,
            }, // no coords → skipped
            ParkedVisit {
                id: 2,
                motorcycle_id: 1,
                date: "2024-04-01".into(),
                location_id: 3,
            },
        ];
        let home = analyze(input.clone()).home.unwrap();
        assert_eq!((home.location_id, home.source), (3, "locationRecord"));

        input.parked_visits.clear();
        let home = analyze(input.clone()).home.unwrap();
        assert_eq!((home.location_id, home.source), (1, "storage"));

        input.locations.retain(|l| l.id != 1);
        let home = analyze(input.clone()).home.unwrap();
        assert_eq!((home.location_id, home.source), (2, "mostVisited"));

        input.fuel_records.clear();
        assert!(analyze(input).home.is_none());
    }

    #[test]
    fn multi_day_trip_becomes_a_tour_with_outbound_and_return_legs() {
        let mut input = geo_input();
        input.fuel_records = vec![
            fuel(1, "2024-05-20", 900, Some(2)), // earlier home-side stop, too old to join
            fuel(2, "2024-06-01", 1000, Some(2)), // home side, starts the tour
            fuel(3, "2024-06-02", 1250, Some(3)), // Milan
            fuel(4, "2024-06-03", 1600, Some(4)), // Florence
            fuel(5, "2024-06-05", 2100, Some(2)), // back home → closes
            fuel(6, "2024-06-20", 2300, Some(2)), // later, unrelated
        ];
        let payload = analyze(input);
        assert_eq!(payload.tours.len(), 1);
        let tour = &payload.tours[0];
        assert_eq!(tour.id, "1-2024-06-01");
        assert_eq!(
            (tour.start_date.as_str(), tour.end_date.as_str()),
            ("2024-06-01", "2024-06-05")
        );
        assert_eq!(tour.days, 5);
        assert_eq!(tour.stops, 4);
        assert_eq!(tour.distance_km, 1100);
        assert_eq!(tour.liters, 30.0);
        assert_eq!(tour.countries, vec!["CH", "IT"]);
        assert_eq!(tour.location_ids, vec![2, 3, 4]);
        assert_eq!(tour.farthest_location_id, Some(4));
        assert_eq!(tour.leg_ids, vec!["1-2-3", "1-3-4", "1-4-5"]);
        let tagged: Vec<&Leg> = payload
            .legs
            .iter()
            .filter(|l| l.tour_id.is_some())
            .collect();
        assert_eq!(tagged.len(), 3);
        assert_eq!(payload.years[0].tours, 1);
        assert_eq!(payload.years[0].day_trips, 0);
    }

    #[test]
    fn same_day_excursion_counts_as_day_trip_and_gap_splits_tours() {
        let mut input = geo_input();
        input.fuel_records = vec![
            fuel(1, "2024-06-01", 1000, Some(2)),
            fuel(2, "2024-06-01", 1250, Some(3)), // away, same day
            fuel(3, "2024-06-01", 1500, Some(2)), // home same day → day trip
            fuel(4, "2024-07-01", 2000, Some(3)), // away, opens tour (no recent predecessor)
            fuel(5, "2024-07-02", 2300, Some(4)), // still away
            fuel(6, "2024-07-20", 2600, Some(2)), // gap > 4 days: tour closed at stop 5
        ];
        let payload = analyze(input);
        assert_eq!(payload.tours.len(), 1);
        let tour = &payload.tours[0];
        assert_eq!(tour.start_date, "2024-07-01");
        assert_eq!(tour.end_date, "2024-07-02");
        assert_eq!(tour.stops, 2);
        assert_eq!(tour.distance_km, 300);
        assert_eq!(payload.years[0].day_trips, 1);
        assert_eq!(payload.years[0].tours, 1);
    }

    #[test]
    fn stops_without_coords_inside_a_tour_do_not_close_it() {
        let mut input = geo_input();
        input.fuel_records = vec![
            fuel(1, "2024-06-01", 1000, Some(2)),
            fuel(2, "2024-06-02", 1250, Some(3)),
            fuel(3, "2024-06-03", 1400, None), // unknown place
            fuel(4, "2024-06-04", 1800, Some(2)),
        ];
        let payload = analyze(input);
        assert_eq!(payload.tours.len(), 1);
        assert_eq!(payload.tours[0].stops, 4);
        assert_eq!(payload.tours[0].distance_km, 800);
    }

    #[test]
    fn years_calendar_and_country_split() {
        let mut input = geo_input();
        input.fuel_records = vec![
            fuel(1, "2023-10-01", 500, Some(2)),
            fuel(2, "2024-03-10", 1000, Some(2)),
            fuel(3, "2024-06-02", 1250, Some(3)),
            fuel(4, "2024-06-02", 1300, Some(3)),
            fuel(5, "2024-11-02", 1600, None),
        ];
        input.maintenance_visits = vec![MaintenanceVisit {
            motorcycle_id: 1,
            date: "2024-04-01".into(),
            location_id: 5,
            cost: 300.0,
        }];
        let payload = analyze(input);
        assert_eq!(payload.years.len(), 2);
        let y2024 = &payload.years[1];
        assert_eq!(y2024.year, 2024);
        assert_eq!(y2024.distance_km, 1100); // 500 + 250 + 50 + 300
        assert_eq!(y2024.fuel_stops, 4);
        assert_eq!(y2024.riding_days, 3);
        assert_eq!(y2024.liters, 40.0);
        assert_eq!(y2024.fuel_cost, 80.0);
        assert_eq!(y2024.countries, vec!["CH", "IT"]);
        assert_eq!(y2024.km_by_country["CH"], 500);
        assert_eq!(y2024.km_by_country["IT"], 300);
        assert_eq!(y2024.km_by_country[UNKNOWN_COUNTRY], 300);
        assert_eq!(y2024.season_start.as_deref(), Some("2024-03-10"));
        assert_eq!(y2024.season_end.as_deref(), Some("2024-11-02"));
        assert_eq!(y2024.farthest.as_ref().unwrap().location_id, 3);
        assert_eq!(y2024.per_motorcycle[0].distance_km, 1100);

        let june2 = payload
            .calendar
            .iter()
            .find(|d| d.date == "2024-06-02")
            .unwrap();
        assert_eq!((june2.km, june2.stops, june2.liters), (300, 2, 20.0));
        assert_eq!(payload.calendar.len(), 4);

        // Ungeocoded workshop is listed with its visit count.
        assert_eq!(payload.ungeocoded.len(), 1);
        assert_eq!(
            (payload.ungeocoded[0].id, payload.ungeocoded[0].visits),
            (5, 1)
        );
        // Geocoded station aggregates.
        let station = payload.locations.iter().find(|l| l.id == 2).unwrap();
        assert_eq!(station.fuel_stops, 2);
        assert_eq!(station.years["2024"].fuel_stops, 1);
        assert_eq!(station.first_visit.as_deref(), Some("2023-10-01"));
        assert!(station.distance_from_home_km.unwrap() > 20.0);
        assert_eq!(station.per_motorcycle[0].fuel_stops, 2);
    }
}
