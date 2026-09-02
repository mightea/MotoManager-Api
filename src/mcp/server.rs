//! The MCP tool set. Every tool runs as the token's user through the same
//! handler functions the web and iOS clients use, so ownership checks and
//! business rules are shared rather than reimplemented. See `mod.rs` for the
//! security model.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use http::request::Parts;
use rmcp::{
    handler::server::{
        router::tool::ToolRouter,
        tool::{Extension, ToolCallContext},
        wrapper::Parameters,
    },
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        ErrorData, Implementation, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ResultType, ServerCapabilities, ServerInfo,
    },
    schemars::{self, JsonSchema},
    service::RequestContext,
    tool, tool_router, RoleServer, ServerHandler,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::validate::{self as v, ToolFailure};
use crate::{
    auth::{api_token::McpPrincipal, AuthUser},
    config::Config,
    error::AppError,
    handlers::{
        self,
        expenses::ExpenseRequest,
        issues::{CreateIssueRequest, UpdateIssueRequest},
        maintenance::{MaintenanceFilter, MaintenanceRequest},
        motorcycles::verify_motorcycle_ownership,
        parts::{
            CreatePartConsumptionRequest, CreatePartRequest, CreatePartStockRequest, PartsFilter,
        },
        torque_specs::TorqueFilter,
    },
};

const SERVER_INSTRUCTIONS: &str = "MotoManager is a motorcycle fleet manager (maintenance log, \
fuel stops, issues, parts inventory, recurring expenses). All tools act as the owner of the API \
token and only ever see that user's own data. Dates are YYYY-MM-DD, distances are kilometres, \
fuel is litres. Call list_motorcycles first to learn the motorcycle IDs, and pass an \
idempotency_key when you might retry a write. Read-only tokens cannot call the write tools.";

const AUDIT_RETENTION_DAYS: i64 = 90;
const AUDIT_ARGS_MAX_CHARS: usize = 4000;
const DEFAULT_LIST_LIMIT: usize = 50;
const MAX_LIST_LIMIT: usize = 200;
/// How long a client may cache the tool list (5 minutes).
const TOOL_LIST_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Clone)]
pub struct McpServer {
    pool: SqlitePool,
    config: Config,
    tool_router: ToolRouter<Self>,
}

type ToolResult = Result<CallToolResult, ToolFailure>;

fn json_result(value: Value) -> ToolResult {
    Ok(CallToolResult::success(vec![ContentBlock::text(
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
    )]))
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT)
}

/// Keep at most `limit` entries of the array under `key` (the handlers
/// return complete lists; MCP responses should stay bounded).
fn truncate_list(mut value: Value, key: &str, limit: usize) -> Value {
    if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
        items.truncate(limit);
    }
    value
}

// MARK: - Parameter types
//
// `deny_unknown_fields` turns a hallucinated argument into an error instead of
// silently ignoring it — the model then sees exactly which field was wrong.

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MotorcycleIdParams {
    /// ID of the motorcycle (from list_motorcycles).
    pub motorcycle_id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListMaintenanceParams {
    /// ID of the motorcycle (from list_motorcycles).
    pub motorcycle_id: i64,
    /// Optional record types to include, e.g. ["service", "fluid"]. Allowed:
    /// tire, battery, brakepad, brakerotor, chain, fluid, general, repair,
    /// service, inspection, location, fuel.
    pub types: Option<Vec<String>>,
    /// Maximum number of records, newest first (default 50, max 200).
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListIssuesParams {
    /// Restrict to one motorcycle; omit for all motorcycles.
    pub motorcycle_id: Option<i64>,
    /// Include issues with status "done" (default false).
    pub include_done: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListPartsParams {
    /// Only parts compatible with this motorcycle's model series.
    pub motorcycle_id: Option<i64>,
    /// Case-insensitive substring match on part number, name or manufacturer.
    pub search: Option<String>,
    /// Only parts with nothing on hand (default false).
    pub out_of_stock_only: Option<bool>,
    /// Maximum number of parts (default 50, max 200).
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogMaintenanceParams {
    /// ID of the motorcycle (from list_motorcycles).
    pub motorcycle_id: i64,
    /// Date of the work, YYYY-MM-DD.
    pub date: String,
    /// Odometer reading in kilometres at the time of the work.
    pub odo: i64,
    /// Kind of work: tire, battery, brakepad, brakerotor, chain, fluid,
    /// general, repair, service, inspection. Use log_fuel for fuel stops.
    #[serde(rename = "type")]
    pub record_type: String,
    /// What was done (free text, up to 4000 characters).
    pub description: Option<String>,
    /// Total cost in `currency`.
    pub cost: Option<f64>,
    /// ISO currency code, e.g. CHF or EUR. Required when cost is given.
    pub currency: Option<String>,
    /// Brand of the fitted part/fluid (e.g. Michelin, Castrol).
    pub brand: Option<String>,
    /// Product/model of the fitted part/fluid (e.g. Road 6, Power 1 10W-40).
    pub model: Option<String>,
    /// For type "fluid": engineoil, gearboxoil, finaldriveoil,
    /// finaldrivegearboxoil, forkoil, brakefluid, coolant.
    pub fluid_type: Option<String>,
    /// Oil viscosity, e.g. "10W-40".
    pub viscosity: Option<String>,
    /// synthetic, semi-synthetic or mineral.
    pub oil_type: Option<String>,
    /// For tires and brakes: front, rear or sidecar.
    pub tire_position: Option<String>,
    /// Tire size, e.g. "120/70 ZR17".
    pub tire_size: Option<String>,
    /// Tire DOT date code.
    pub dot_code: Option<String>,
    /// For type "battery": lead-acid, gel, agm, lithium-ion, other.
    pub battery_type: Option<String>,
    /// Client-chosen key (8-64 chars) that makes a retried call return the
    /// existing record instead of creating a second one.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogFuelParams {
    /// ID of the motorcycle (from list_motorcycles).
    pub motorcycle_id: i64,
    /// Date of the fuel stop, YYYY-MM-DD.
    pub date: String,
    /// Odometer reading in kilometres at the fuel stop.
    pub odo: i64,
    /// Litres filled.
    pub liters: f64,
    /// Total amount paid in `currency`.
    pub total_cost: Option<f64>,
    /// Price per litre in `currency` (derived from total_cost when omitted).
    pub price_per_liter: Option<f64>,
    /// ISO currency code, e.g. CHF. Required when a cost is given.
    pub currency: Option<String>,
    /// 95E10 (default), 98E5 or Diesel.
    pub fuel_type: Option<String>,
    /// Trip distance since the previous stop in km; computed from the
    /// previous fuel record when omitted.
    pub trip_distance_km: Option<f64>,
    /// Free-text note.
    pub description: Option<String>,
    /// Client-chosen key (8-64 chars) that makes a retried call return the
    /// existing record instead of creating a second one.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateIssueParams {
    /// ID of the motorcycle (from list_motorcycles).
    pub motorcycle_id: i64,
    /// Short title (up to 200 characters).
    pub title: String,
    /// Odometer reading in kilometres when the issue was noticed.
    pub odo: i64,
    /// Longer description (up to 4000 characters).
    pub description: Option<String>,
    /// low, medium (default) or high.
    pub priority: Option<String>,
    /// Date noticed, YYYY-MM-DD (default today).
    pub date: Option<String>,
    /// Client-chosen key (8-64 chars) that makes a retried call return the
    /// existing issue instead of creating a second one.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateIssueStatusParams {
    /// ID of the motorcycle the issue belongs to.
    pub motorcycle_id: i64,
    /// ID of the issue (from list_issues).
    pub issue_id: i64,
    /// new, in_progress or done.
    pub status: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddExpenseParams {
    /// Date of the expense, YYYY-MM-DD.
    pub date: String,
    /// Amount in `currency`.
    pub amount: f64,
    /// ISO currency code, e.g. CHF.
    pub currency: String,
    /// Versicherung, Steuern, Vignette, Parkplatz, Ausrüstung or Sonstiges.
    pub category: String,
    /// Free-text description (up to 200 characters).
    pub description: Option<String>,
    /// For recurring expenses: repeat interval in months (1-120).
    pub interval_months: Option<i64>,
    /// Motorcycles this expense is attributed to (from list_motorcycles).
    pub motorcycle_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePartParams {
    /// Manufacturer part number.
    pub part_number: String,
    /// Human-readable name.
    pub name: String,
    /// Manufacturer/brand (default "Unbekannt").
    pub manufacturer: Option<String>,
    /// Free-text description (up to 4000 characters).
    pub description: Option<String>,
    /// Client-chosen key (8-64 chars) that makes a retried call return the
    /// existing part instead of creating a second one.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddPartStockParams {
    /// ID of the part (from list_parts).
    pub part_id: i64,
    /// Number of pieces received (default 1).
    pub quantity: Option<i64>,
    /// Purchase price per piece in `currency`.
    pub price: Option<f64>,
    /// ISO currency code, e.g. CHF. Required when price is given.
    pub currency: Option<String>,
    /// Purchase date, YYYY-MM-DD.
    pub purchase_date: Option<String>,
    /// Free-text note (up to 200 characters).
    pub notes: Option<String>,
    /// Used/salvaged piece rather than new (default false).
    pub is_used: Option<bool>,
    /// Client-chosen key (8-64 chars) that makes a retried call return the
    /// existing stock entry instead of creating a second one.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumePartParams {
    /// ID of the part (from list_parts).
    pub part_id: i64,
    /// Number of pieces taken from stock.
    pub quantity: i64,
    /// Date of use, YYYY-MM-DD (default today).
    pub date: Option<String>,
    /// Maintenance record ID to book the parts against (from
    /// list_maintenance or a log_maintenance result).
    pub maintenance_record_id: Option<i64>,
    /// Free-text note (up to 200 characters).
    pub notes: Option<String>,
    /// Client-chosen key (8-64 chars) that makes a retried call return the
    /// existing consumption instead of creating a second one.
    pub idempotency_key: Option<String>,
}

// MARK: - Tools

#[tool_router]
impl McpServer {
    pub fn new(pool: SqlitePool, config: Config) -> Self {
        Self {
            pool,
            config,
            tool_router: Self::tool_router(),
        }
    }

    fn state(&self) -> State<SqlitePool> {
        State(self.pool.clone())
    }

    // ---- read tools -------------------------------------------------------

    #[tool(
        name = "list_motorcycles",
        description = "List the user's motorcycles with their IDs, status (active/sold), latest odometer reading and open-issue count. Call this first to resolve motorcycle IDs.",
        annotations(title = "List motorcycles", read_only_hint = true)
    )]
    async fn list_motorcycles(&self, Extension(p): Extension<McpPrincipal>) -> ToolResult {
        let Json(value) =
            handlers::motorcycles::list_motorcycles(self.state(), AuthUser(p.user)).await?;
        json_result(value)
    }

    #[tool(
        name = "get_fleet_overview",
        description = "Fleet dashboard: per-motorcycle current odometer, kilometres this year, open issues, next inspection, current location and which service items are overdue, plus yearly totals.",
        annotations(title = "Fleet overview", read_only_hint = true)
    )]
    async fn get_fleet_overview(&self, Extension(p): Extension<McpPrincipal>) -> ToolResult {
        let Json(value) = handlers::home::get_home_data(
            self.state(),
            State(self.config.clone()),
            AuthUser(p.user),
        )
        .await?;
        json_result(value)
    }

    #[tool(
        name = "list_maintenance",
        description = "Maintenance history of one motorcycle, newest first: services, fluid changes, tires, brakes, repairs, inspections and fuel stops with odometer, cost and details.",
        annotations(title = "List maintenance records", read_only_hint = true)
    )]
    async fn list_maintenance(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<ListMaintenanceParams>,
    ) -> ToolResult {
        let all_types: Vec<&str> = v::MAINTENANCE_TYPES
            .iter()
            .copied()
            .chain(["location", "fuel"])
            .collect();
        let types = match args.types {
            None => None,
            Some(list) => {
                let mut validated = Vec::with_capacity(list.len());
                for t in list {
                    validated.push(v::one_of("types", &t, &all_types)?);
                }
                Some(validated.join(","))
            }
        };
        let Json(value) = handlers::maintenance::list_maintenance(
            self.state(),
            AuthUser(p.user),
            Path(args.motorcycle_id),
            Query(MaintenanceFilter { types, since: None }),
        )
        .await?;
        json_result(truncate_list(
            value,
            "maintenanceRecords",
            clamp_limit(args.limit),
        ))
    }

    #[tool(
        name = "list_issues",
        description = "Open issues (known defects, to-dos) across all motorcycles or for one motorcycle, with priority, status and the odometer reading when noticed.",
        annotations(title = "List issues", read_only_hint = true)
    )]
    async fn list_issues(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<ListIssuesParams>,
    ) -> ToolResult {
        if let Some(mid) = args.motorcycle_id {
            verify_motorcycle_ownership(&self.pool, mid, p.user.id).await?;
        }
        let include_done = args.include_done.unwrap_or(false);
        // Ownership is enforced by the join on the user's motorcycles.
        let issues = sqlx::query_as::<_, crate::models::Issue>(
            "SELECT i.* FROM issues i \
             JOIN motorcycles m ON m.id = i.motorcycleId \
             WHERE m.userId = ? AND i.deletedAt IS NULL \
               AND (? IS NULL OR i.motorcycleId = ?) \
               AND (? OR i.status != 'done') \
             ORDER BY CASE i.priority WHEN 'high' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END, \
                      i.date DESC, i.id DESC \
             LIMIT ?",
        )
        .bind(p.user.id)
        .bind(args.motorcycle_id)
        .bind(args.motorcycle_id)
        .bind(include_done)
        .bind(MAX_LIST_LIMIT as i64)
        .fetch_all(&self.pool)
        .await?;
        json_result(json!({ "issues": issues }))
    }

    #[tool(
        name = "list_parts",
        description = "Parts inventory: part numbers, names, manufacturers and the quantity on hand (stock minus consumption). Optionally filtered by motorcycle compatibility, a search term, or out-of-stock only.",
        annotations(title = "List parts", read_only_hint = true)
    )]
    async fn list_parts(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<ListPartsParams>,
    ) -> ToolResult {
        let search =
            v::text("search", args.search.as_deref(), v::SHORT_TEXT)?.map(|s| s.to_lowercase());
        let Json(mut value) = handlers::parts::list_parts(
            self.state(),
            AuthUser(p.user),
            Query(PartsFilter {
                since: None,
                motorcycle_id: args.motorcycle_id,
            }),
        )
        .await?;
        if let Some(parts) = value.get_mut("parts").and_then(Value::as_array_mut) {
            let out_of_stock_only = args.out_of_stock_only.unwrap_or(false);
            parts.retain(|part| {
                let matches_search = search.as_deref().is_none_or(|needle| {
                    ["partNumber", "name", "manufacturer"].iter().any(|key| {
                        part.get(key)
                            .and_then(Value::as_str)
                            .is_some_and(|s| s.to_lowercase().contains(needle))
                    })
                });
                let on_hand = part.get("onHand").and_then(Value::as_i64).unwrap_or(0);
                matches_search && (!out_of_stock_only || on_hand <= 0)
            });
            parts.truncate(clamp_limit(args.limit));
        }
        json_result(value)
    }

    #[tool(
        name = "list_expenses",
        description = "Recurring and one-off fleet expenses (insurance, tax, vignette, parking, gear) with amount, currency, interval and the motorcycles they are attributed to.",
        annotations(title = "List expenses", read_only_hint = true)
    )]
    async fn list_expenses(&self, Extension(p): Extension<McpPrincipal>) -> ToolResult {
        let Json(value) = handlers::expenses::list_expenses(self.state(), AuthUser(p.user)).await?;
        json_result(value)
    }

    #[tool(
        name = "list_torque_specs",
        description = "Torque specifications recorded for one motorcycle (component, value in Nm, notes).",
        annotations(title = "List torque specs", read_only_hint = true)
    )]
    async fn list_torque_specs(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<MotorcycleIdParams>,
    ) -> ToolResult {
        let Json(value) = handlers::torque_specs::list_torque_specs(
            self.state(),
            AuthUser(p.user),
            Path(args.motorcycle_id),
            Query(TorqueFilter { since: None }),
        )
        .await?;
        json_result(value)
    }

    // ---- write tools (additive only) -------------------------------------

    #[tool(
        name = "log_maintenance",
        description = "Record completed maintenance work on a motorcycle (service, fluid change, tires, brakes, chain, battery, repair, inspection). Additive: creates one record and never edits or deletes existing ones. Use log_fuel for fuel stops.",
        annotations(
            title = "Log maintenance",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn log_maintenance(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<LogMaintenanceParams>,
    ) -> ToolResult {
        let record_type = v::one_of("type", &args.record_type, v::MAINTENANCE_TYPES)?;
        let fluid_type =
            v::optional_one_of("fluid_type", args.fluid_type.as_deref(), v::FLUID_TYPES)?;
        if record_type == "fluid" && fluid_type.is_none() {
            return Err(v::Invalid("fluid_type is required for type \"fluid\"".to_string()).into());
        }
        let (cost, normalized_cost, currency) = self
            .priced("cost", args.cost, args.currency.as_deref())
            .await?;

        let body = MaintenanceRequest {
            date: Some(v::date("date", &args.date)?),
            odo: Some(v::odo(args.odo)?),
            record_type: Some(record_type),
            cost,
            normalized_cost,
            currency,
            description: v::text("description", args.description.as_deref(), v::LONG_TEXT)?,
            brand: v::text("brand", args.brand.as_deref(), v::SHORT_TEXT)?,
            model: v::text("model", args.model.as_deref(), v::SHORT_TEXT)?,
            tire_position: v::optional_one_of(
                "tire_position",
                args.tire_position.as_deref(),
                v::TIRE_POSITIONS,
            )?,
            tire_size: v::text("tire_size", args.tire_size.as_deref(), v::SHORT_TEXT)?,
            dot_code: v::text("dot_code", args.dot_code.as_deref(), v::SHORT_TEXT)?,
            battery_type: v::optional_one_of(
                "battery_type",
                args.battery_type.as_deref(),
                v::BATTERY_TYPES,
            )?,
            fluid_type,
            viscosity: v::text("viscosity", args.viscosity.as_deref(), v::SHORT_TEXT)?,
            oil_type: v::optional_one_of("oil_type", args.oil_type.as_deref(), v::OIL_TYPES)?,
            location_id: None,
            fuel_type: None,
            fuel_amount: None,
            price_per_unit: None,
            fuel_consumption: None,
            trip_distance: None,
            fuel_additive_added: None,
            lead_substitute_added: None,
            parent_id: None,
            bundled_items: None,
            client_id: v::idempotency_key(args.idempotency_key.as_deref())?,
        };
        let (_, Json(value)) = handlers::maintenance::create_maintenance(
            self.state(),
            AuthUser(p.user),
            Path(args.motorcycle_id),
            Json(body),
        )
        .await?;
        json_result(value)
    }

    #[tool(
        name = "log_fuel",
        description = "Record a fuel stop (litres, cost, odometer). Consumption per 100 km is derived from the previous fuel stop automatically. Additive: never edits or deletes.",
        annotations(
            title = "Log fuel stop",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn log_fuel(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<LogFuelParams>,
    ) -> ToolResult {
        let liters = v::liters(args.liters)?;
        let total = match args.total_cost {
            Some(c) => Some(v::amount("total_cost", c)?),
            None => None,
        };
        let per_liter = match args.price_per_liter {
            Some(ppl) => Some(v::amount("price_per_liter", ppl)?),
            None => total.map(|t| ((t / liters) * 1000.0).round() / 1000.0),
        };
        let total = total.or(per_liter.map(|ppl| ((ppl * liters) * 100.0).round() / 100.0));
        let (cost, normalized_cost, currency) = self
            .priced("total_cost", total, args.currency.as_deref())
            .await?;
        let trip_distance = match args.trip_distance_km {
            Some(d) if d.is_finite() && d > 0.0 && d <= 5_000.0 => Some(d),
            Some(_) => {
                return Err(
                    v::Invalid("trip_distance_km must be between 0 and 5000".to_string()).into(),
                )
            }
            None => None,
        };

        let body = MaintenanceRequest {
            date: Some(v::date("date", &args.date)?),
            odo: Some(v::odo(args.odo)?),
            record_type: Some("fuel".to_string()),
            cost,
            normalized_cost,
            currency,
            description: v::text("description", args.description.as_deref(), v::LONG_TEXT)?,
            brand: None,
            model: None,
            tire_position: None,
            tire_size: None,
            dot_code: None,
            battery_type: None,
            fluid_type: None,
            viscosity: None,
            oil_type: None,
            location_id: None,
            fuel_type: Some(
                v::optional_one_of("fuel_type", args.fuel_type.as_deref(), v::FUEL_TYPES)?
                    .unwrap_or_else(|| v::FUEL_TYPES[0].to_string()),
            ),
            fuel_amount: Some(liters),
            price_per_unit: per_liter,
            fuel_consumption: None,
            trip_distance,
            fuel_additive_added: None,
            lead_substitute_added: None,
            parent_id: None,
            bundled_items: None,
            client_id: v::idempotency_key(args.idempotency_key.as_deref())?,
        };
        let (_, Json(value)) = handlers::maintenance::create_maintenance(
            self.state(),
            AuthUser(p.user),
            Path(args.motorcycle_id),
            Json(body),
        )
        .await?;
        json_result(value)
    }

    #[tool(
        name = "create_issue",
        description = "Open a new issue (defect, to-do) on a motorcycle. Additive: never edits or deletes.",
        annotations(
            title = "Create issue",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn create_issue(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<CreateIssueParams>,
    ) -> ToolResult {
        let body = CreateIssueRequest {
            odo: v::odo(args.odo)?,
            title: v::required_text("title", &args.title, v::SHORT_TEXT)?,
            description: v::text("description", args.description.as_deref(), v::LONG_TEXT)?,
            priority: v::optional_one_of(
                "priority",
                args.priority.as_deref(),
                v::ISSUE_PRIORITIES,
            )?,
            status: None,
            date: match args.date.as_deref() {
                Some(d) => Some(v::date("date", d)?),
                None => None,
            },
            client_id: v::idempotency_key(args.idempotency_key.as_deref())?,
        };
        let (_, Json(value)) = handlers::issues::create_issue(
            self.state(),
            AuthUser(p.user),
            Path(args.motorcycle_id),
            Json(body),
        )
        .await?;
        json_result(value)
    }

    #[tool(
        name = "update_issue_status",
        description = "Move an issue to new, in_progress or done. This is the only field an MCP client can change on an issue.",
        annotations(
            title = "Update issue status",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn update_issue_status(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<UpdateIssueStatusParams>,
    ) -> ToolResult {
        let body = UpdateIssueRequest {
            odo: None,
            title: None,
            description: None,
            priority: None,
            status: Some(v::one_of("status", &args.status, v::ISSUE_STATUSES)?),
            date: None,
        };
        let Json(value) = handlers::issues::update_issue(
            self.state(),
            AuthUser(p.user),
            Path((args.motorcycle_id, args.issue_id)),
            Json(body),
        )
        .await?;
        json_result(value)
    }

    #[tool(
        name = "add_expense",
        description = "Record a fleet expense such as insurance, tax, vignette, parking or gear, optionally recurring and attributed to specific motorcycles. Additive: never edits or deletes.",
        annotations(
            title = "Add expense",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn add_expense(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<AddExpenseParams>,
    ) -> ToolResult {
        let currency = v::currency(&self.pool, &args.currency).await?;
        let interval_months = match args.interval_months {
            None => None,
            Some(m) if (1..=120).contains(&m) => Some(m),
            Some(_) => {
                return Err(
                    v::Invalid("interval_months must be between 1 and 120".to_string()).into(),
                )
            }
        };
        let motorcycle_ids = args.motorcycle_ids.unwrap_or_default();
        if motorcycle_ids.len() > 50 {
            return Err(v::Invalid("motorcycle_ids: too many entries".to_string()).into());
        }
        for mid in &motorcycle_ids {
            verify_motorcycle_ownership(&self.pool, *mid, p.user.id).await?;
        }
        let body = ExpenseRequest {
            date: Some(v::date("date", &args.date)?),
            amount: Some(v::amount("amount", args.amount)?),
            currency: Some(currency.code),
            category: Some(v::one_of(
                "category",
                &args.category,
                v::EXPENSE_CATEGORIES,
            )?),
            description: v::text("description", args.description.as_deref(), v::SHORT_TEXT)?,
            interval_months,
            motorcycle_ids,
        };
        let (_, Json(value)) =
            handlers::expenses::create_expense(self.state(), AuthUser(p.user), Json(body)).await?;
        json_result(value)
    }

    #[tool(
        name = "create_part",
        description = "Add a part to the user's private parts catalogue (no stock yet — use add_part_stock afterwards). Additive: never edits or deletes.",
        annotations(
            title = "Create part",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn create_part(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<CreatePartParams>,
    ) -> ToolResult {
        let body = CreatePartRequest {
            part_number: v::required_text("part_number", &args.part_number, v::SHORT_TEXT)?,
            name: v::required_text("name", &args.name, v::SHORT_TEXT)?,
            manufacturer: v::text("manufacturer", args.manufacturer.as_deref(), v::SHORT_TEXT)?,
            description: v::text("description", args.description.as_deref(), v::LONG_TEXT)?,
            // Never publish to the shared catalogue from an AI client.
            is_public: Some(false),
            series_ids: None,
            client_id: v::idempotency_key(args.idempotency_key.as_deref())?,
        };
        let (_, Json(value)) =
            handlers::parts::create_part(self.state(), AuthUser(p.user), Json(body)).await?;
        json_result(value)
    }

    #[tool(
        name = "add_part_stock",
        description = "Book received pieces of a part into stock (purchase). Additive: never edits or deletes.",
        annotations(
            title = "Add part stock",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn add_part_stock(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<AddPartStockParams>,
    ) -> ToolResult {
        let (price, normalized_price, currency) = self
            .priced("price", args.price, args.currency.as_deref())
            .await?;
        let body = CreatePartStockRequest {
            part_id: args.part_id,
            quantity: Some(v::quantity("quantity", args.quantity.unwrap_or(1))?),
            price,
            currency,
            normalized_price,
            purchase_date: match args.purchase_date.as_deref() {
                Some(d) => Some(v::date("purchase_date", d)?),
                None => None,
            },
            storage_location_id: None,
            notes: v::text("notes", args.notes.as_deref(), v::SHORT_TEXT)?,
            is_used: args.is_used,
            client_id: v::idempotency_key(args.idempotency_key.as_deref())?,
        };
        let (_, Json(value)) =
            handlers::parts::create_part_stock(self.state(), AuthUser(p.user), Json(body)).await?;
        json_result(value)
    }

    #[tool(
        name = "consume_part",
        description = "Take pieces of a part out of stock, optionally booking them against a maintenance record so the record's parts cost is updated. Additive: never edits or deletes.",
        annotations(
            title = "Consume part",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn consume_part(
        &self,
        Extension(p): Extension<McpPrincipal>,
        Parameters(args): Parameters<ConsumePartParams>,
    ) -> ToolResult {
        let body = CreatePartConsumptionRequest {
            part_id: args.part_id,
            quantity: v::quantity("quantity", args.quantity)?,
            maintenance_record_id: args.maintenance_record_id,
            date: match args.date.as_deref() {
                Some(d) => Some(v::date("date", d)?),
                None => None,
            },
            notes: v::text("notes", args.notes.as_deref(), v::SHORT_TEXT)?,
            client_id: v::idempotency_key(args.idempotency_key.as_deref())?,
        };
        let (_, Json(value)) =
            handlers::parts::create_part_consumption(self.state(), AuthUser(p.user), Json(body))
                .await?;
        json_result(value)
    }
}

impl McpServer {
    /// Validates an optional amount + currency pair and derives the
    /// normalized (CHF) value the way the webapp does. A cost without a
    /// currency is rejected rather than guessed.
    async fn priced(
        &self,
        field: &str,
        amount: Option<f64>,
        currency: Option<&str>,
    ) -> Result<(Option<f64>, Option<f64>, Option<String>), ToolFailure> {
        match (amount, currency) {
            (None, _) => Ok((None, None, None)),
            (Some(_), None) => {
                Err(v::Invalid(format!("currency is required when {field} is given")).into())
            }
            (Some(a), Some(c)) => {
                let a = v::amount(field, a)?;
                let cur = v::currency(&self.pool, c).await?;
                let normalized = ((a * cur.conversion_factor) * 100.0).round() / 100.0;
                Ok((Some(a), Some(normalized), Some(cur.code)))
            }
        }
    }

    fn is_read_only(&self, name: &str) -> bool {
        self.tool_router
            .get(name)
            .and_then(|t| t.annotations.as_ref())
            .and_then(|a| a.read_only_hint)
            .unwrap_or(false)
    }

    async fn audit(
        &self,
        p: &McpPrincipal,
        tool: &str,
        args: &Option<rmcp::model::JsonObject>,
        outcome: &str,
        detail: Option<&str>,
    ) {
        let arguments = args.as_ref().map(|a| {
            let s = Value::Object(a.clone()).to_string();
            if s.chars().count() > AUDIT_ARGS_MAX_CHARS {
                let truncated: String = s.chars().take(AUDIT_ARGS_MAX_CHARS).collect();
                format!("{truncated}…")
            } else {
                s
            }
        });
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO mcpAuditLog (tokenId, userId, tool, arguments, outcome, detail, createdAt) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(p.token_id)
        .bind(p.user.id)
        .bind(tool)
        .bind(&arguments)
        .bind(outcome)
        .bind(detail)
        .bind(&now)
        .execute(&self.pool)
        .await;
        if let Err(e) = result {
            tracing::error!(
                "Failed to write MCP audit entry for user {}: {}",
                p.user.id,
                e
            );
            return;
        }
        let cutoff = (Utc::now() - chrono::Duration::days(AUDIT_RETENTION_DAYS)).to_rfc3339();
        let _ = sqlx::query("DELETE FROM mcpAuditLog WHERE userId = ? AND createdAt < ?")
            .bind(p.user.id)
            .bind(&cutoff)
            .execute(&self.pool)
            .await;
    }
}

/// Tool-level failure the model can read and act on. Application errors that
/// are the user's concern (not found, bad request, conflict) pass through;
/// everything else is logged and reported generically.
fn failure_to_result(failure: ToolFailure) -> (CallToolResult, String) {
    let message = match failure {
        ToolFailure::Invalid(msg) => format!("Invalid input: {msg}"),
        ToolFailure::App(AppError::NotFound(msg)) => format!("Not found: {msg}"),
        ToolFailure::App(AppError::BadRequest(msg)) => format!("Invalid input: {msg}"),
        ToolFailure::App(AppError::Conflict(msg)) => format!("Conflict: {msg}"),
        ToolFailure::App(AppError::Forbidden) => "Forbidden".to_string(),
        ToolFailure::App(AppError::Unauthorized) => "Unauthorized".to_string(),
        ToolFailure::App(other) => {
            tracing::error!("MCP tool failed: {}", other);
            "Internal error while executing the tool".to_string()
        }
    };
    (
        CallToolResult::error(vec![ContentBlock::text(message.clone())]),
        message,
    )
}

impl rmcp::handler::server::tool::IntoCallToolResult for ToolFailure {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        Ok(failure_to_result(self).0.into())
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let mut implementation = Implementation::from_build_env();
        implementation.name = "motomanager".to_string();
        implementation.title = Some("MotoManager".to_string());
        implementation.version = env!("CARGO_PKG_VERSION").to_string();
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::default())
            .with_server_info(implementation)
            .with_instructions(SERVER_INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // MCP 2026-07-28 (SEP-2549) makes `ttlMs`/`cacheScope` mandatory on
        // list results; rmcp advertises that revision via `server/discover`
        // and Claude Code rejects a tools/list reply without them. Older
        // peers ignore the extra fields. The tool set is fixed per build,
        // so a private, short-lived cache entry is safe.
        Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools: self.tool_router.list_all(),
            ttl_ms: Some(TOOL_LIST_TTL_MS),
            cache_scope: Some(CacheScope::Private),
            ..Default::default()
        })
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        self.tool_router.get(name).cloned()
    }

    /// Central gate for every tool call: resolve the principal the middleware
    /// attached, enforce the token scope, dispatch, and audit the outcome.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        mut context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let principal = context
            .extensions
            .get::<Parts>()
            .and_then(|parts| parts.extensions.get::<McpPrincipal>())
            .cloned()
            .ok_or_else(|| ErrorData::invalid_request("missing API token principal", None))?;

        let name = request.name.to_string();
        let arguments = request.arguments.clone();

        if self.tool_router.get(&name).is_none() {
            self.audit(&principal, &name, &arguments, "error", Some("unknown tool"))
                .await;
            return Err(ErrorData::invalid_params("tool not found", None));
        }
        if !self.is_read_only(&name) && !principal.scope.allows_write() {
            let detail = "token scope is read-only";
            self.audit(&principal, &name, &arguments, "denied", Some(detail))
                .await;
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Forbidden: this API token is read-only and cannot call {name}. Create a token with write scope in MotoManager settings to use write tools."
            ))])
            .into());
        }

        context.extensions.insert(principal.clone());
        let tcc = ToolCallContext::new(self, request, context);
        let response = self.tool_router.call(tcc).await;

        let (outcome, detail): (&str, Option<String>) = match &response {
            Ok(CallToolResponse::Complete(result)) if result.is_error == Some(true) => {
                let text = result
                    .content
                    .iter()
                    .find_map(|c| c.as_text().map(|t| t.text.clone()));
                ("error", text)
            }
            Ok(_) => ("ok", None),
            Err(e) => ("error", Some(e.message.to_string())),
        };
        self.audit(&principal, &name, &arguments, outcome, detail.as_deref())
            .await;
        response
    }
}
