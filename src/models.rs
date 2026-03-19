use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub email: String,
    pub username: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: i64,
    pub email: String,
    pub username: String,
    pub name: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

impl From<User> for PublicUser {
    fn from(u: User) -> Self {
        PublicUser {
            id: u.id,
            email: u.email,
            username: u.username,
            name: u.name,
            role: u.role,
            created_at: u.created_at,
            updated_at: u.updated_at,
            last_login_at: u.last_login_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Session {
    pub id: i64,
    pub token: String,
    pub user_id: i64,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Motorcycle {
    pub id: i64,
    pub make: String,
    pub model: String,
    #[serde(rename = "fabricationDate")]
    #[sqlx(rename = "modelYear")]
    pub model_year: Option<String>,
    pub user_id: i64,
    pub vin: Option<String>,
    pub engine_number: Option<String>,
    pub vehicle_nr: Option<String>,
    pub number_plate: Option<String>,
    pub image: Option<String>,
    pub is_veteran: bool,
    pub is_archived: bool,
    pub first_registration: Option<String>,
    pub initial_odo: i64,
    pub manual_odo: Option<i64>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub normalized_purchase_price: Option<f64>,
    pub currency_code: Option<String>,
    pub fuel_tank_size: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct MotorcycleWithStats {
    pub id: i64,
    pub make: String,
    pub model: String,
    #[serde(rename = "fabricationDate")]
    #[sqlx(rename = "modelYear")]
    pub model_year: Option<String>,
    pub user_id: i64,
    pub vin: Option<String>,
    pub engine_number: Option<String>,
    pub vehicle_nr: Option<String>,
    pub number_plate: Option<String>,
    pub image: Option<String>,
    pub is_veteran: bool,
    pub is_archived: bool,
    pub first_registration: Option<String>,
    pub initial_odo: i64,
    pub manual_odo: Option<i64>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub normalized_purchase_price: Option<f64>,
    pub currency_code: Option<String>,
    pub fuel_tank_size: Option<f64>,
    pub open_issues: i64,
    pub maintenance_count: i64,
    pub latest_odo: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct MaintenanceRecord {
    pub id: i64,
    pub date: String,
    pub odo: i64,
    pub motorcycle_id: i64,
    pub cost: Option<f64>,
    pub normalized_cost: Option<f64>,
    pub currency: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub record_type: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub tire_position: Option<String>,
    pub tire_size: Option<String>,
    pub dot_code: Option<String>,
    pub battery_type: Option<String>,
    pub fluid_type: Option<String>,
    pub viscosity: Option<String>,
    pub oil_type: Option<String>,
    pub inspection_location: Option<String>,
    pub location_id: Option<i64>,
    pub fuel_type: Option<String>,
    pub fuel_amount: Option<f64>,
    pub price_per_unit: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_name: Option<String>,
    pub fuel_consumption: Option<f64>,
    pub trip_distance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Issue {
    pub id: i64,
    pub motorcycle_id: i64,
    pub odo: i64,
    pub description: Option<String>,
    pub priority: String,
    pub status: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Location {
    pub id: i64,
    pub name: String,
    pub country_code: String,
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct LocationRecord {
    pub id: i64,
    pub motorcycle_id: i64,
    pub location_id: i64,
    pub odometer: Option<i64>,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct TorqueSpec {
    pub id: i64,
    pub motorcycle_id: i64,
    pub category: String,
    pub name: String,
    pub torque: f64,
    pub torque_end: Option<f64>,
    pub variation: Option<f64>,
    pub tool_size: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct UserSettings {
    pub id: i64,
    pub user_id: i64,
    pub tire_interval: i64,
    pub battery_lithium_interval: i64,
    pub battery_default_interval: i64,
    pub engine_oil_interval: i64,
    pub gearbox_oil_interval: i64,
    pub final_drive_oil_interval: i64,
    pub fork_oil_interval: i64,
    pub brake_fluid_interval: i64,
    pub coolant_interval: i64,
    pub chain_interval: i64,
    pub tire_km_interval: Option<i64>,
    pub engine_oil_km_interval: Option<i64>,
    pub gearbox_oil_km_interval: Option<i64>,
    pub final_drive_oil_km_interval: Option<i64>,
    pub fork_oil_km_interval: Option<i64>,
    pub brake_fluid_km_interval: Option<i64>,
    pub coolant_km_interval: Option<i64>,
    pub chain_km_interval: Option<i64>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct CurrencySetting {
    pub id: i64,
    pub code: String,
    pub symbol: String,
    pub label: Option<String>,
    pub conversion_factor: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Document {
    pub id: i64,
    pub title: String,
    pub file_path: String,
    pub preview_path: Option<String>,
    pub uploaded_by: Option<String>,
    pub owner_id: Option<i64>,
    pub is_private: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct PreviousOwner {
    pub id: i64,
    pub motorcycle_id: i64,
    pub name: String,
    pub surname: String,
    pub purchase_date: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub postcode: Option<String>,
    pub country: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub comments: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Authenticator {
    pub id: String,
    pub user_id: i64,
    pub public_key: Vec<u8>,
    pub counter: i64,
    pub device_type: String,
    pub backed_up: bool,
    pub transports: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "camelCase")]
pub struct Challenge {
    pub id: String,
    pub user_id: Option<i64>,
    pub challenge: String,
    pub expires_at: String,
    pub created_at: String,
}
