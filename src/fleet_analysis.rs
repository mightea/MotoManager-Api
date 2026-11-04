//! Fleet statistics for the webapp's stats page (`GET /api/stats/fleet`):
//! distance per year and month, costs split by category, fuel consumption and
//! price trends, and total cost of ownership per motorcycle.
//!
//! Pure functions over plain inputs (no database), so the rules are unit
//! testable. All money is in the normalized currency (CHF), distances in km,
//! dates are `YYYY-MM-DD` strings (longer ISO timestamps are truncated).
//!
//! Distances come from odometer readings. Between two readings the driven km
//! are spread evenly over the days in between, so a ride across New Year or
//! across a month boundary lands in both periods. Yearly and monthly numbers
//! therefore always add up to the same total.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Datelike, NaiveDate};
use serde::Serialize;

/// Consumption values further than this factor from a bike's median are
/// treated as artifacts (missed fill-up, odometer typo) and dropped.
const CONSUMPTION_OUTLIER_FACTOR: f64 = 2.0;

/// Fuel prices per litre further than this factor from the median fill are
/// left out of the price trend (they are still counted as costs).
const PRICE_OUTLIER_FACTOR: f64 = 2.5;

// MARK: - Inputs

#[derive(Debug, Clone)]
pub struct BikeInput {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub status: String,
    pub purchase_date: Option<String>,
    pub sold_date: Option<String>,
    pub initial_odo: i64,
    /// Normalized purchase price; None when unknown.
    pub purchase_price: Option<f64>,
    /// Normalized sale price; None when not sold or unknown.
    pub sale_price: Option<f64>,
}

/// One `maintenanceRecords` row (any type, including fuel).
#[derive(Debug, Clone)]
pub struct RecordInput {
    pub motorcycle_id: i64,
    pub date: String,
    pub odo: i64,
    pub record_type: String,
    /// Normalized amount entered on the record (0 when none).
    pub cost: f64,
    /// Normalized value of the parts booked against the record (0 when none).
    pub parts_cost: f64,
    pub fuel_amount: Option<f64>,
    pub fuel_consumption: Option<f64>,
}

/// One user-level expense (insurance, tax, …). Linked bikes share it equally;
/// an expense without bikes counts for the fleet only.
#[derive(Debug, Clone)]
pub struct ExpenseInput {
    pub date: String,
    pub amount: f64,
    pub motorcycle_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct FleetInput {
    pub today: NaiveDate,
    pub bikes: Vec<BikeInput>,
    pub records: Vec<RecordInput>,
    pub expenses: Vec<ExpenseInput>,
}

// MARK: - Output

/// Costs split by where the money went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub fuel: f64,
    /// Service, fluids, chain, battery, inspection and other upkeep.
    pub service: f64,
    pub repair: f64,
    pub tires: f64,
    /// Parts from the inventory booked against a record.
    pub parts: f64,
    /// Expenses: insurance, tax, fees.
    pub fixed: f64,
}

impl CostBreakdown {
    pub fn total(&self) -> f64 {
        self.fuel + self.service + self.repair + self.tires + self.parts + self.fixed
    }

    fn add(&mut self, other: &CostBreakdown) {
        self.fuel += other.fuel;
        self.service += other.service;
        self.repair += other.repair;
        self.tires += other.tires;
        self.parts += other.parts;
        self.fixed += other.fixed;
    }

    fn rounded(&self) -> CostBreakdown {
        CostBreakdown {
            fuel: round2(self.fuel),
            service: round2(self.service),
            repair: round2(self.repair),
            tires: round2(self.tires),
            parts: round2(self.parts),
            fixed: round2(self.fixed),
        }
    }

    /// Book a record's cost into its category; parts always go to `parts`.
    fn add_record(&mut self, record: &RecordInput) {
        match record.record_type.to_lowercase().as_str() {
            "fuel" => self.fuel += record.cost,
            "repair" => self.repair += record.cost,
            "tire" => self.tires += record.cost,
            _ => self.service += record.cost,
        }
        self.parts += record.parts_cost;
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BikeYear {
    pub motorcycle_id: i64,
    pub distance: i64,
    pub liters: f64,
    pub costs: CostBreakdown,
    pub total_cost: f64,
    /// Mean of the plausible per-fill consumption values (L/100 km).
    pub avg_consumption: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearStats {
    pub year: i32,
    pub distance: i64,
    /// Distance per calendar month, January first.
    pub months: [i64; 12],
    pub liters: f64,
    /// Bikes owned at any point during the year.
    pub motorcycle_count: i64,
    pub costs: CostBreakdown,
    pub total_cost: f64,
    pub bikes: Vec<BikeYear>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BikeStats {
    pub id: i64,
    pub make: String,
    pub model: String,
    pub status: String,
    /// Purchase date, or the first own record when no purchase date is set.
    pub owned_from: Option<String>,
    /// Sale date; None while the bike is still owned.
    pub owned_until: Option<String>,
    pub years_owned: f64,
    pub distance: i64,
    pub liters: f64,
    pub avg_consumption: Option<f64>,
    pub costs: CostBreakdown,
    /// Everything spent while owning the bike (fuel, upkeep, parts, expenses).
    pub running_cost: f64,
    pub purchase_price: Option<f64>,
    pub sale_price: Option<f64>,
    /// Purchase minus sale price; None unless both are known.
    pub depreciation: Option<f64>,
    /// Total cost of ownership: purchase price − sale price + running cost.
    /// For a bike still owned this is the money put in so far, without any
    /// residual value. None when the purchase price is unknown, or the bike was
    /// sold without a sale price — the figure would be meaningless.
    pub net_cost: Option<f64>,
    pub running_cost_per_km: Option<f64>,
    pub net_cost_per_km: Option<f64>,
    pub net_cost_per_year: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuelPriceMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub avg_price_per_liter: f64,
    pub liters: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumptionPoint {
    pub motorcycle_id: i64,
    pub date: String,
    pub value: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodTotals {
    pub distance: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetStats {
    pub today: String,
    pub current_year: i32,
    /// Ascending, from the first year any bike was owned to the current year.
    pub years: Vec<YearStats>,
    pub motorcycles: Vec<BikeStats>,
    pub fuel_prices: Vec<FuelPriceMonth>,
    /// Plausible per-fill consumption values, date ascending.
    pub consumption: Vec<ConsumptionPoint>,
    /// Last year from 1 January up to today's day of the year, for an honest
    /// "vs. last year" comparison while the current year is still running.
    pub previous_year_to_date: PeriodTotals,
    pub totals: PeriodTotals,
}

// MARK: - Helpers

pub fn parse_date(date_str: &str) -> Option<NaiveDate> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.date_naive());
    }
    date_str
        .get(0..10)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(sorted[sorted.len() / 2])
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

/// Is `day` on or before the same day-of-year as `today` (for year-to-date)?
fn within_year_to_date(day: NaiveDate, today: NaiveDate) -> bool {
    (day.month(), day.day()) <= (today.month(), today.day())
}

/// Km per day for one bike, from its odometer readings.
///
/// Readings that go backwards (typos, late entries) are ignored. When the
/// first reading has no earlier baseline date, its km land on that day.
fn daily_distance(
    start: Option<(NaiveDate, i64)>,
    mut readings: Vec<(NaiveDate, i64)>,
) -> BTreeMap<NaiveDate, f64> {
    readings.sort();
    let mut days: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    let mut base = start;
    for (day, odo) in readings {
        let Some((base_day, base_odo)) = base else {
            base = Some((day, odo));
            continue;
        };
        if odo < base_odo {
            continue;
        }
        let delta = (odo - base_odo) as f64;
        if delta > 0.0 {
            let span = (day - base_day).num_days();
            if span <= 0 {
                *days.entry(day).or_default() += delta;
            } else {
                let per_day = delta / span as f64;
                let mut d = base_day;
                while d < day {
                    d = d.succ_opt().unwrap_or(d);
                    *days.entry(d).or_default() += per_day;
                }
            }
        }
        base = Some((day.max(base_day), odo));
    }
    days
}

// MARK: - Analysis

#[derive(Default)]
struct BikeYearAcc {
    distance: f64,
    liters: f64,
    costs: CostBreakdown,
    consumption: Vec<f64>,
}

pub fn analyze(input: FleetInput) -> FleetStats {
    let today = input.today;
    let current_year = today.year();

    // Expenses per bike (equal share) and unassigned fleet-level ones.
    let mut expense_share: HashMap<i64, Vec<(NaiveDate, f64)>> = HashMap::new();
    let mut fleet_expenses: Vec<(NaiveDate, f64)> = Vec::new();
    for e in &input.expenses {
        let Some(day) = parse_date(&e.date) else {
            continue;
        };
        if e.motorcycle_ids.is_empty() {
            fleet_expenses.push((day, e.amount));
        } else {
            let share = e.amount / e.motorcycle_ids.len() as f64;
            for id in &e.motorcycle_ids {
                expense_share.entry(*id).or_default().push((day, share));
            }
        }
    }

    let mut records_by_bike: HashMap<i64, Vec<(NaiveDate, &RecordInput)>> = HashMap::new();
    for r in &input.records {
        if let Some(day) = parse_date(&r.date) {
            records_by_bike
                .entry(r.motorcycle_id)
                .or_default()
                .push((day, r));
        }
    }

    // year -> bike -> accumulator
    let mut per_year: BTreeMap<i32, BTreeMap<i64, BikeYearAcc>> = BTreeMap::new();
    let mut fleet_months: BTreeMap<i32, [f64; 12]> = BTreeMap::new();
    let mut ownership_years: BTreeMap<i32, i64> = BTreeMap::new();
    let mut prev_ytd = (0.0f64, 0.0f64);
    let mut fills: Vec<(String, f64, f64)> = Vec::new(); // month, cost, liters
    let mut consumption_points: Vec<ConsumptionPoint> = Vec::new();
    let mut motorcycles = Vec::new();

    for bike in &input.bikes {
        let purchase_day = bike.purchase_date.as_deref().and_then(parse_date);
        let sold_day = bike.sold_date.as_deref().and_then(parse_date);

        // Records logged by previous owners predate the purchase: neither
        // their km nor their costs belong to this owner.
        let mut own: Vec<(NaiveDate, &RecordInput)> = records_by_bike
            .remove(&bike.id)
            .unwrap_or_default()
            .into_iter()
            .filter(|(day, _)| purchase_day.is_none_or(|p| *day >= p))
            .collect();
        own.sort_by_key(|(day, r)| (*day, r.odo));

        let owned_from = purchase_day.or_else(|| own.first().map(|(d, _)| *d));
        let owned_until = sold_day;

        // Distance
        let readings: Vec<(NaiveDate, i64)> = own
            .iter()
            .filter(|(_, r)| r.odo > 0)
            .map(|(d, r)| (*d, r.odo))
            .collect();
        let start = match purchase_day {
            Some(p) => Some((p, bike.initial_odo)),
            None => own.first().map(|(d, _)| (*d, bike.initial_odo)),
        };
        let days = daily_distance(start, readings);

        let mut bike_years: BTreeMap<i32, BikeYearAcc> = BTreeMap::new();
        for (day, km) in &days {
            let acc = bike_years.entry(day.year()).or_default();
            acc.distance += km;
            fleet_months.entry(day.year()).or_insert([0.0; 12])[day.month0() as usize] += km;
            if day.year() == current_year - 1 && within_year_to_date(*day, today) {
                prev_ytd.0 += km;
            }
        }

        // Costs, fuel and consumption
        let consumptions: Vec<f64> = own
            .iter()
            .filter(|(_, r)| r.record_type.eq_ignore_ascii_case("fuel"))
            .filter_map(|(_, r)| r.fuel_consumption)
            .filter(|c| *c > 0.0)
            .collect();
        let consumption_median = median(&consumptions);
        let plausible = |c: f64| match consumption_median {
            Some(m) => {
                c > 0.0
                    && c <= m * CONSUMPTION_OUTLIER_FACTOR
                    && c >= m / CONSUMPTION_OUTLIER_FACTOR
            }
            None => false,
        };

        for (day, r) in &own {
            let acc = bike_years.entry(day.year()).or_default();
            acc.costs.add_record(r);
            if day.year() == current_year - 1 && within_year_to_date(*day, today) {
                prev_ytd.1 += r.cost + r.parts_cost;
            }
            if r.record_type.eq_ignore_ascii_case("fuel") {
                let liters = r.fuel_amount.unwrap_or(0.0);
                acc.liters += liters;
                if liters > 0.0 && r.cost > 0.0 {
                    fills.push((day.format("%Y-%m").to_string(), r.cost, liters));
                }
                if let Some(c) = r.fuel_consumption.filter(|c| plausible(*c)) {
                    acc.consumption.push(c);
                    consumption_points.push(ConsumptionPoint {
                        motorcycle_id: bike.id,
                        date: day.format("%Y-%m-%d").to_string(),
                        value: round2(c),
                    });
                }
            }
        }
        for (day, amount) in expense_share.remove(&bike.id).unwrap_or_default() {
            bike_years.entry(day.year()).or_default().costs.fixed += amount;
            if day.year() == current_year - 1 && within_year_to_date(day, today) {
                prev_ytd.1 += amount;
            }
        }

        // Ownership years for the bike count.
        if let Some(from) = owned_from {
            let until = owned_until.map(|d| d.year()).unwrap_or(current_year);
            for y in from.year()..=until.max(from.year()) {
                *ownership_years.entry(y).or_default() += 1;
            }
        } else if bike.status == "active" {
            *ownership_years.entry(current_year).or_default() += 1;
        }

        // Lifetime totals for this bike.
        let mut costs = CostBreakdown::default();
        let mut distance = 0.0;
        let mut liters = 0.0;
        let mut all_consumption = Vec::new();
        for acc in bike_years.values() {
            costs.add(&acc.costs);
            distance += acc.distance;
            liters += acc.liters;
            all_consumption.extend_from_slice(&acc.consumption);
        }
        let running_cost = costs.total();
        let depreciation = match (bike.purchase_price, bike.sale_price) {
            (Some(p), Some(s)) => Some(p - s),
            _ => None,
        };
        let sold = owned_until.is_some() || bike.status == "sold";
        let net_cost = match (bike.purchase_price, bike.sale_price) {
            (Some(p), Some(s)) => Some(p - s + running_cost),
            (Some(p), None) if !sold => Some(p + running_cost),
            _ => None,
        };
        let years_owned = owned_from
            .map(|from| {
                let until = owned_until.unwrap_or(today);
                ((until - from).num_days().max(0) as f64) / 365.25
            })
            .unwrap_or(0.0);
        let distance_km = distance.round() as i64;
        let per_km = |v: f64| (distance_km > 0).then(|| round2(v / distance_km as f64));

        motorcycles.push(BikeStats {
            id: bike.id,
            make: bike.make.clone(),
            model: bike.model.clone(),
            status: bike.status.clone(),
            owned_from: owned_from.map(|d| d.format("%Y-%m-%d").to_string()),
            owned_until: owned_until.map(|d| d.format("%Y-%m-%d").to_string()),
            years_owned: round1(years_owned),
            distance: distance_km,
            liters: round1(liters),
            avg_consumption: mean(&all_consumption).map(round2),
            costs: costs.rounded(),
            running_cost: round2(running_cost),
            purchase_price: bike.purchase_price.map(round2),
            sale_price: bike.sale_price.map(round2),
            depreciation: depreciation.map(round2),
            net_cost: net_cost.map(round2),
            running_cost_per_km: per_km(running_cost),
            net_cost_per_km: net_cost.and_then(per_km),
            net_cost_per_year: net_cost
                .filter(|_| years_owned >= 0.1)
                .map(|n| round2(n / years_owned)),
        });

        for (year, acc) in bike_years {
            per_year.entry(year).or_default().insert(bike.id, acc);
        }
    }

    let mut fleet_fixed_by_year: BTreeMap<i32, f64> = BTreeMap::new();
    for (day, amount) in &fleet_expenses {
        *fleet_fixed_by_year.entry(day.year()).or_default() += amount;
        if day.year() == current_year - 1 && within_year_to_date(*day, today) {
            prev_ytd.1 += amount;
        }
    }

    // Year range: first year with ownership or data, up to the current year.
    let first_year = [
        ownership_years.keys().next().copied(),
        per_year.keys().next().copied(),
        fleet_fixed_by_year.keys().next().copied(),
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(current_year)
    .min(current_year);

    let mut years = Vec::new();
    let mut totals = PeriodTotals::default();
    for year in first_year..=current_year {
        let bikes_acc = per_year.remove(&year).unwrap_or_default();
        let mut costs = CostBreakdown::default();
        let mut distance = 0.0;
        let mut liters = 0.0;
        let mut bikes = Vec::new();
        for (id, acc) in bikes_acc {
            costs.add(&acc.costs);
            distance += acc.distance;
            liters += acc.liters;
            bikes.push(BikeYear {
                motorcycle_id: id,
                distance: acc.distance.round() as i64,
                liters: round1(acc.liters),
                costs: acc.costs.rounded(),
                total_cost: round2(acc.costs.total()),
                avg_consumption: mean(&acc.consumption).map(round2),
            });
        }
        costs.fixed += fleet_fixed_by_year.get(&year).copied().unwrap_or(0.0);
        let months_f = fleet_months.get(&year).copied().unwrap_or([0.0; 12]);
        let mut months = [0i64; 12];
        for (i, km) in months_f.iter().enumerate() {
            months[i] = km.round() as i64;
        }
        let distance_km = distance.round() as i64;
        totals.distance += distance_km;
        totals.cost += costs.total();
        years.push(YearStats {
            year,
            distance: distance_km,
            months,
            liters: round1(liters),
            motorcycle_count: ownership_years.get(&year).copied().unwrap_or(0),
            costs: costs.rounded(),
            total_cost: round2(costs.total()),
            bikes,
        });
    }
    totals.cost = round2(totals.cost);

    consumption_points.sort_by(|a, b| a.date.cmp(&b.date));

    // A price per litre far from the median is a data error (wrong currency,
    // total entered as unit price) and would flatten the whole price chart.
    let price_median = median(&fills.iter().map(|(_, c, l)| c / l).collect::<Vec<_>>());
    let mut fuel_by_month: BTreeMap<String, (f64, f64)> = BTreeMap::new(); // cost, liters
    for (month, cost, liters) in fills {
        let price = cost / liters;
        if let Some(m) = price_median {
            if price > m * PRICE_OUTLIER_FACTOR || price < m / PRICE_OUTLIER_FACTOR {
                continue;
            }
        }
        let entry = fuel_by_month.entry(month).or_default();
        entry.0 += cost;
        entry.1 += liters;
    }

    FleetStats {
        today: today.format("%Y-%m-%d").to_string(),
        current_year,
        years,
        motorcycles,
        fuel_prices: fuel_by_month
            .into_iter()
            .map(|(month, (cost, liters))| FuelPriceMonth {
                month,
                avg_price_per_liter: round2(cost / liters),
                liters: round1(liters),
            })
            .collect(),
        consumption: consumption_points,
        previous_year_to_date: PeriodTotals {
            distance: prev_ytd.0.round() as i64,
            cost: round2(prev_ytd.1),
        },
        totals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn bike(id: i64, purchase: Option<&str>, sold: Option<&str>, initial_odo: i64) -> BikeInput {
        BikeInput {
            id,
            make: "BMW".into(),
            model: format!("Bike {id}"),
            status: if sold.is_some() { "sold" } else { "active" }.into(),
            purchase_date: purchase.map(Into::into),
            sold_date: sold.map(Into::into),
            initial_odo,
            purchase_price: None,
            sale_price: None,
        }
    }

    fn record(bike: i64, date: &str, odo: i64, kind: &str, cost: f64) -> RecordInput {
        RecordInput {
            motorcycle_id: bike,
            date: date.into(),
            odo,
            record_type: kind.into(),
            cost,
            parts_cost: 0.0,
            fuel_amount: None,
            fuel_consumption: None,
        }
    }

    fn fuel(bike: i64, date: &str, odo: i64, liters: f64, cost: f64, cons: f64) -> RecordInput {
        RecordInput {
            fuel_amount: Some(liters),
            fuel_consumption: Some(cons),
            ..record(bike, date, odo, "fuel", cost)
        }
    }

    fn input(today: &str) -> FleetInput {
        FleetInput {
            today: day(today),
            bikes: vec![],
            records: vec![],
            expenses: vec![],
        }
    }

    #[test]
    fn distance_is_spread_across_year_boundary() {
        let mut i = input("2025-06-01");
        i.bikes = vec![bike(1, Some("2024-12-22"), None, 1000)];
        i.records = vec![record(1, "2025-01-11", 3000, "service", 0.0)];
        let s = analyze(i);
        let y2024 = s.years.iter().find(|y| y.year == 2024).unwrap();
        let y2025 = s.years.iter().find(|y| y.year == 2025).unwrap();
        // 20 days of 100 km: Dec 23..31 = 9 days, Jan 1..11 = 11 days.
        assert_eq!(y2024.distance, 900);
        assert_eq!(y2025.distance, 1100);
        assert_eq!(y2025.months[0], 1100);
        assert_eq!(y2024.months[11], 900);
        assert_eq!(s.totals.distance, 2000);
        assert_eq!(s.motorcycles[0].distance, 2000);
    }

    #[test]
    fn sold_bike_is_not_counted_after_sale() {
        let mut i = input("2026-03-01");
        i.bikes = vec![
            bike(1, Some("2022-05-01"), Some("2024-07-10"), 0),
            bike(2, Some("2023-01-01"), None, 0),
        ];
        let s = analyze(i);
        let count = |y: i32| {
            s.years
                .iter()
                .find(|x| x.year == y)
                .unwrap()
                .motorcycle_count
        };
        assert_eq!(s.years.first().unwrap().year, 2022);
        assert_eq!(count(2022), 1);
        assert_eq!(count(2024), 2);
        assert_eq!(count(2025), 1);
        assert_eq!(count(2026), 1);
    }

    #[test]
    fn previous_owner_records_are_ignored() {
        let mut i = input("2025-12-31");
        i.bikes = vec![bike(1, Some("2025-01-01"), None, 20000)];
        i.records = vec![
            record(1, "2020-01-01", 5000, "repair", 999.0),
            record(1, "2025-06-01", 21000, "service", 100.0),
        ];
        let s = analyze(i);
        assert_eq!(s.totals.distance, 1000);
        assert_eq!(s.motorcycles[0].running_cost, 100.0);
        assert_eq!(s.years.first().unwrap().year, 2025);
    }

    #[test]
    fn costs_are_split_by_category_and_expenses_shared() {
        let mut i = input("2025-12-31");
        i.bikes = vec![
            bike(1, Some("2025-01-01"), None, 0),
            bike(2, Some("2025-01-01"), None, 0),
        ];
        let mut with_parts = record(1, "2025-03-01", 100, "service", 200.0);
        with_parts.parts_cost = 50.0;
        i.records = vec![
            with_parts,
            record(1, "2025-04-01", 200, "tire", 300.0),
            record(1, "2025-05-01", 300, "repair", 80.0),
            fuel(1, "2025-06-01", 400, 10.0, 20.0, 5.0),
        ];
        i.expenses = vec![
            ExpenseInput {
                date: "2025-07-01".into(),
                amount: 400.0,
                motorcycle_ids: vec![1, 2],
            },
            ExpenseInput {
                date: "2025-08-01".into(),
                amount: 100.0,
                motorcycle_ids: vec![],
            },
        ];
        let s = analyze(i);
        let y = &s.years[0];
        assert_eq!(
            y.costs,
            CostBreakdown {
                fuel: 20.0,
                service: 200.0,
                repair: 80.0,
                tires: 300.0,
                parts: 50.0,
                fixed: 500.0,
            }
        );
        assert_eq!(y.total_cost, 1150.0);
        let b1 = s.motorcycles.iter().find(|m| m.id == 1).unwrap();
        let b2 = s.motorcycles.iter().find(|m| m.id == 2).unwrap();
        assert_eq!(b1.costs.fixed, 200.0);
        assert_eq!(b2.running_cost, 200.0);
        assert_eq!(b1.running_cost, 850.0);
    }

    #[test]
    fn total_cost_of_ownership() {
        let mut i = input("2026-01-01");
        let mut b = bike(1, Some("2022-01-01"), Some("2024-01-01"), 10000);
        b.purchase_price = Some(8000.0);
        b.sale_price = Some(5000.0);
        i.bikes = vec![b];
        i.records = vec![record(1, "2023-12-31", 20000, "service", 1000.0)];
        let s = analyze(i);
        let m = &s.motorcycles[0];
        assert_eq!(m.depreciation, Some(3000.0));
        assert_eq!(m.net_cost, Some(4000.0));
        assert_eq!(m.distance, 10000);
        assert_eq!(m.net_cost_per_km, Some(0.4));
        assert_eq!(m.running_cost_per_km, Some(0.1));
        assert_eq!(m.years_owned, 2.0);
        // 730 days = 1.9986 years.
        assert_eq!(m.net_cost_per_year, Some(2001.37));
    }

    #[test]
    fn total_cost_of_ownership_needs_known_prices() {
        let mut i = input("2026-01-01");
        let mut no_purchase = bike(1, None, Some("2025-01-01"), 0);
        no_purchase.sale_price = Some(5000.0);
        let mut sold_unknown_price = bike(2, Some("2020-01-01"), Some("2025-01-01"), 0);
        sold_unknown_price.purchase_price = Some(5000.0);
        let mut owned = bike(3, Some("2020-01-01"), None, 0);
        owned.purchase_price = Some(5000.0);
        i.bikes = vec![no_purchase, sold_unknown_price, owned];
        let s = analyze(i);
        assert_eq!(s.motorcycles[0].net_cost, None);
        assert_eq!(s.motorcycles[1].net_cost, None);
        assert_eq!(s.motorcycles[2].net_cost, Some(5000.0));
    }

    #[test]
    fn consumption_outliers_and_fuel_prices() {
        let mut i = input("2025-12-31");
        i.bikes = vec![bike(1, Some("2025-01-01"), None, 0)];
        i.records = vec![
            fuel(1, "2025-03-01", 200, 10.0, 20.0, 5.0),
            fuel(1, "2025-03-15", 400, 10.0, 22.0, 5.5),
            fuel(1, "2025-04-01", 600, 10.0, 21.0, 25.0), // artifact
            fuel(1, "2025-05-01", 800, 10.0, 160.0, 5.0), // wrong currency
        ];
        let s = analyze(i);
        assert_eq!(s.consumption.len(), 3);
        assert_eq!(s.motorcycles[0].avg_consumption, Some(5.17));
        // The 16/l fill is dropped from the price trend, but still a cost.
        assert_eq!(s.fuel_prices.len(), 2);
        assert_eq!(s.years[0].costs.fuel, 223.0);
        assert_eq!(s.fuel_prices[0].month, "2025-03");
        assert_eq!(s.fuel_prices[0].avg_price_per_liter, 2.1);
        assert_eq!(s.years[0].liters, 40.0);
    }

    #[test]
    fn previous_year_to_date() {
        let mut i = input("2025-03-31");
        i.bikes = vec![bike(1, Some("2024-01-01"), None, 0)];
        i.records = vec![
            record(1, "2024-03-01", 600, "service", 50.0),
            record(1, "2024-09-01", 1000, "service", 70.0),
        ];
        let s = analyze(i);
        // Jan 1 – Mar 1 2024 covers 60 days at 10 km/day; Mar 2 – 31 more at
        // 400 km / 184 days.
        let expected: f64 = 600.0 + 30.0 * 400.0 / 184.0;
        assert_eq!(s.previous_year_to_date.distance, expected.round() as i64);
        assert_eq!(s.previous_year_to_date.cost, 50.0);
    }

    #[test]
    fn backwards_reading_is_ignored() {
        let days = daily_distance(
            Some((day("2025-01-01"), 1000)),
            vec![
                (day("2025-01-11"), 2000),
                (day("2025-01-12"), 200), // typo
                (day("2025-01-21"), 2500),
            ],
        );
        let total: f64 = days.values().sum();
        assert!((total - 1500.0).abs() < 1e-6);
    }
}
