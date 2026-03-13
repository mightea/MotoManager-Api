use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub username: String,
    pub name: String,
    #[serde(rename = "passwordHash", skip_serializing)]
    pub password_hash: String,
    pub role: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "lastLoginAt")]
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicUser {
    pub id: i64,
    pub email: String,
    pub username: String,
    pub name: String,
    pub role: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "lastLoginAt")]
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
pub struct Session {
    pub id: i64,
    pub token: String,
    #[serde(rename = "userId")]
    pub user_id: i64,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Motorcycle model — mirrors the actual DB schema.
/// DB columns of note:
///   model_year        → model_year (exposed as "fabricationDate")
///   vehicle_nr        → vehicle_nr (exposed as "vehicleNr")
///   firstRegistration → first_registration (camelCase in DB, quoted in SQL)
///   initialOdo        → initial_odo (camelCase in DB, quoted in SQL)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Motorcycle {
    pub id: i64,
    pub make: String,
    pub model: String,
    /// Stored as model_year in DB, exposed as fabricationDate
    #[serde(rename = "fabricationDate")]
    pub model_year: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: i64,
    pub vin: Option<String>,
    #[serde(rename = "engineNumber")]
    pub engine_number: Option<String>,
    #[serde(rename = "vehicleNr")]
    pub vehicle_nr: Option<String>,
    #[serde(rename = "numberPlate")]
    pub number_plate: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "isVeteran")]
    pub is_veteran: bool,
    #[serde(rename = "isArchived")]
    pub is_archived: bool,
    #[serde(rename = "firstRegistration")]
    pub first_registration: Option<String>,
    #[serde(rename = "initialOdo")]
    pub initial_odo: i64,
    #[serde(rename = "manualOdo")]
    pub manual_odo: Option<i64>,
    #[serde(rename = "purchaseDate")]
    pub purchase_date: Option<String>,
    #[serde(rename = "purchasePrice")]
    pub purchase_price: Option<f64>,
    #[serde(rename = "normalizedPurchasePrice")]
    pub normalized_purchase_price: Option<f64>,
    #[serde(rename = "currencyCode")]
    pub currency_code: Option<String>,
    #[serde(rename = "fuelTankSize")]
    pub fuel_tank_size: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MaintenanceRecord {
    pub id: i64,
    pub date: String,
    pub odo: i64,
    #[serde(rename = "motorcycleId")]
    pub motorcycle_id: i64,
    pub cost: Option<f64>,
    #[serde(rename = "normalizedCost")]
    pub normalized_cost: Option<f64>,
    pub currency: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub record_type: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "tirePosition")]
    pub tire_position: Option<String>,
    #[serde(rename = "tireSize")]
    pub tire_size: Option<String>,
    #[serde(rename = "dotCode")]
    pub dot_code: Option<String>,
    #[serde(rename = "batteryType")]
    pub battery_type: Option<String>,
    #[serde(rename = "fluidType")]
    pub fluid_type: Option<String>,
    pub viscosity: Option<String>,
    #[serde(rename = "oilType")]
    pub oil_type: Option<String>,
    #[serde(rename = "inspectionLocation")]
    pub inspection_location: Option<String>,
    #[serde(rename = "locationId")]
    pub location_id: Option<i64>,
    #[serde(rename = "fuelType")]
    pub fuel_type: Option<String>,
    #[serde(rename = "fuelAmount")]
    pub fuel_amount: Option<f64>,
    #[serde(rename = "pricePerUnit")]
    pub price_per_unit: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    #[serde(rename = "locationName")]
    pub location_name: Option<String>,
    #[serde(rename = "fuelConsumption")]
    pub fuel_consumption: Option<f64>,
    #[serde(rename = "tripDistance")]
    pub trip_distance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Issue {
    pub id: i64,
    #[serde(rename = "motorcycleId")]
    pub motorcycle_id: i64,
    pub odo: i64,
    pub description: Option<String>,
    pub priority: String,
    pub status: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Location {
    pub id: i64,
    pub name: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    #[serde(rename = "userId")]
    pub user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LocationRecord {
    pub id: i64,
    #[serde(rename = "motorcycleId")]
    pub motorcycle_id: i64,
    #[serde(rename = "locationId")]
    pub location_id: i64,
    pub odometer: Option<i64>,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TorqueSpec {
    pub id: i64,
    #[serde(rename = "motorcycleId")]
    pub motorcycle_id: i64,
    pub category: String,
    pub name: String,
    pub torque: f64,
    #[serde(rename = "torqueEnd")]
    pub torque_end: Option<f64>,
    pub variation: Option<f64>,
    #[serde(rename = "toolSize")]
    pub tool_size: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserSettings {
    pub id: i64,
    #[serde(rename = "userId")]
    pub user_id: i64,
    #[serde(rename = "tireInterval")]
    pub tire_interval: i64,
    #[serde(rename = "batteryLithiumInterval")]
    pub battery_lithium_interval: i64,
    #[serde(rename = "batteryDefaultInterval")]
    pub battery_default_interval: i64,
    #[serde(rename = "engineOilInterval")]
    pub engine_oil_interval: i64,
    #[serde(rename = "gearboxOilInterval")]
    pub gearbox_oil_interval: i64,
    #[serde(rename = "finalDriveOilInterval")]
    pub final_drive_oil_interval: i64,
    #[serde(rename = "forkOilInterval")]
    pub fork_oil_interval: i64,
    #[serde(rename = "brakeFluidInterval")]
    pub brake_fluid_interval: i64,
    #[serde(rename = "coolantInterval")]
    pub coolant_interval: i64,
    #[serde(rename = "chainInterval")]
    pub chain_interval: i64,
    #[serde(rename = "tireKmInterval")]
    pub tire_km_interval: Option<i64>,
    #[serde(rename = "engineOilKmInterval")]
    pub engine_oil_km_interval: Option<i64>,
    #[serde(rename = "gearboxOilKmInterval")]
    pub gearbox_oil_km_interval: Option<i64>,
    #[serde(rename = "finalDriveOilKmInterval")]
    pub final_drive_oil_km_interval: Option<i64>,
    #[serde(rename = "forkOilKmInterval")]
    pub fork_oil_km_interval: Option<i64>,
    #[serde(rename = "brakeFluidKmInterval")]
    pub brake_fluid_km_interval: Option<i64>,
    #[serde(rename = "coolantKmInterval")]
    pub coolant_km_interval: Option<i64>,
    #[serde(rename = "chainKmInterval")]
    pub chain_km_interval: Option<i64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CurrencySetting {
    pub id: i64,
    pub code: String,
    pub symbol: String,
    pub label: Option<String>,
    #[serde(rename = "conversionFactor")]
    pub conversion_factor: f64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Document {
    pub id: i64,
    pub title: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "previewPath")]
    pub preview_path: Option<String>,
    #[serde(rename = "uploadedBy")]
    pub uploaded_by: Option<String>,
    #[serde(rename = "ownerId")]
    pub owner_id: Option<i64>,
    #[serde(rename = "isPrivate")]
    pub is_private: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PreviousOwner {
    pub id: i64,
    #[serde(rename = "motorcycleId")]
    pub motorcycle_id: i64,
    pub name: String,
    pub surname: String,
    #[serde(rename = "purchaseDate")]
    pub purchase_date: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub postcode: Option<String>,
    pub country: Option<String>,
    #[serde(rename = "phoneNumber")]
    pub phone_number: Option<String>,
    pub email: Option<String>,
    pub comments: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

