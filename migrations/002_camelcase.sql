-- Rename tables from snake_case to camelCase
ALTER TABLE maintenance_records RENAME TO maintenanceRecords;
ALTER TABLE location_records    RENAME TO locationRecords;
ALTER TABLE torque_specs        RENAME TO torqueSpecs;
ALTER TABLE user_settings       RENAME TO userSettings;
ALTER TABLE document_motorcycles RENAME TO documentMotorcycles;
ALTER TABLE previous_owners     RENAME TO previousOwners;

-- users
ALTER TABLE users RENAME COLUMN password_hash  TO passwordHash;
ALTER TABLE users RENAME COLUMN created_at     TO createdAt;
ALTER TABLE users RENAME COLUMN updated_at     TO updatedAt;
ALTER TABLE users RENAME COLUMN last_login_at  TO lastLoginAt;

-- sessions
ALTER TABLE sessions RENAME COLUMN user_id     TO userId;
ALTER TABLE sessions RENAME COLUMN expires_at  TO expiresAt;
ALTER TABLE sessions RENAME COLUMN created_at  TO createdAt;

-- authenticators
ALTER TABLE authenticators RENAME COLUMN user_id     TO userId;
ALTER TABLE authenticators RENAME COLUMN public_key  TO publicKey;
ALTER TABLE authenticators RENAME COLUMN device_type TO deviceType;
ALTER TABLE authenticators RENAME COLUMN backed_up   TO backedUp;
ALTER TABLE authenticators RENAME COLUMN created_at  TO createdAt;

-- challenges
ALTER TABLE challenges RENAME COLUMN user_id    TO userId;
ALTER TABLE challenges RENAME COLUMN expires_at TO expiresAt;
ALTER TABLE challenges RENAME COLUMN created_at TO createdAt;

-- motorcycles (firstRegistration and initialOdo are already camelCase)
ALTER TABLE motorcycles RENAME COLUMN model_year                 TO modelYear;
ALTER TABLE motorcycles RENAME COLUMN user_id                    TO userId;
ALTER TABLE motorcycles RENAME COLUMN engine_number              TO engineNumber;
ALTER TABLE motorcycles RENAME COLUMN vehicle_nr                 TO vehicleNr;
ALTER TABLE motorcycles RENAME COLUMN number_plate               TO numberPlate;
ALTER TABLE motorcycles RENAME COLUMN is_veteran                 TO isVeteran;
ALTER TABLE motorcycles RENAME COLUMN is_archived                TO isArchived;
ALTER TABLE motorcycles RENAME COLUMN manual_odo                 TO manualOdo;
ALTER TABLE motorcycles RENAME COLUMN purchase_date              TO purchaseDate;
ALTER TABLE motorcycles RENAME COLUMN purchase_price             TO purchasePrice;
ALTER TABLE motorcycles RENAME COLUMN normalized_purchase_price  TO normalizedPurchasePrice;
ALTER TABLE motorcycles RENAME COLUMN currency_code              TO currencyCode;
ALTER TABLE motorcycles RENAME COLUMN fuel_tank_size             TO fuelTankSize;

-- maintenanceRecords
ALTER TABLE maintenanceRecords RENAME COLUMN motorcycle_id        TO motorcycleId;
ALTER TABLE maintenanceRecords RENAME COLUMN normalized_cost      TO normalizedCost;
ALTER TABLE maintenanceRecords RENAME COLUMN tire_position        TO tirePosition;
ALTER TABLE maintenanceRecords RENAME COLUMN tire_size            TO tireSize;
ALTER TABLE maintenanceRecords RENAME COLUMN dot_code             TO dotCode;
ALTER TABLE maintenanceRecords RENAME COLUMN battery_type         TO batteryType;
ALTER TABLE maintenanceRecords RENAME COLUMN fluid_type           TO fluidType;
ALTER TABLE maintenanceRecords RENAME COLUMN oil_type             TO oilType;
ALTER TABLE maintenanceRecords RENAME COLUMN inspection_location  TO inspectionLocation;
ALTER TABLE maintenanceRecords RENAME COLUMN location_id          TO locationId;
ALTER TABLE maintenanceRecords RENAME COLUMN fuel_type            TO fuelType;
ALTER TABLE maintenanceRecords RENAME COLUMN fuel_amount          TO fuelAmount;
ALTER TABLE maintenanceRecords RENAME COLUMN price_per_unit       TO pricePerUnit;
ALTER TABLE maintenanceRecords RENAME COLUMN location_name        TO locationName;
ALTER TABLE maintenanceRecords RENAME COLUMN fuel_consumption     TO fuelConsumption;
ALTER TABLE maintenanceRecords RENAME COLUMN trip_distance        TO tripDistance;

-- issues
ALTER TABLE issues RENAME COLUMN motorcycle_id TO motorcycleId;

-- locations
ALTER TABLE locations RENAME COLUMN user_id      TO userId;
ALTER TABLE locations RENAME COLUMN country_code TO countryCode;

-- locationRecords
ALTER TABLE locationRecords RENAME COLUMN motorcycle_id TO motorcycleId;
ALTER TABLE locationRecords RENAME COLUMN location_id   TO locationId;

-- torqueSpecs
ALTER TABLE torqueSpecs RENAME COLUMN motorcycle_id TO motorcycleId;
ALTER TABLE torqueSpecs RENAME COLUMN torque_end    TO torqueEnd;
ALTER TABLE torqueSpecs RENAME COLUMN tool_size     TO toolSize;
ALTER TABLE torqueSpecs RENAME COLUMN created_at    TO createdAt;

-- userSettings
ALTER TABLE userSettings RENAME COLUMN user_id                     TO userId;
ALTER TABLE userSettings RENAME COLUMN tire_interval               TO tireInterval;
ALTER TABLE userSettings RENAME COLUMN battery_lithium_interval    TO batteryLithiumInterval;
ALTER TABLE userSettings RENAME COLUMN battery_default_interval    TO batteryDefaultInterval;
ALTER TABLE userSettings RENAME COLUMN engine_oil_interval         TO engineOilInterval;
ALTER TABLE userSettings RENAME COLUMN gearbox_oil_interval        TO gearboxOilInterval;
ALTER TABLE userSettings RENAME COLUMN final_drive_oil_interval    TO finalDriveOilInterval;
ALTER TABLE userSettings RENAME COLUMN fork_oil_interval           TO forkOilInterval;
ALTER TABLE userSettings RENAME COLUMN brake_fluid_interval        TO brakeFluidInterval;
ALTER TABLE userSettings RENAME COLUMN coolant_interval            TO coolantInterval;
ALTER TABLE userSettings RENAME COLUMN chain_interval              TO chainInterval;
ALTER TABLE userSettings RENAME COLUMN tire_km_interval            TO tireKmInterval;
ALTER TABLE userSettings RENAME COLUMN engine_oil_km_interval      TO engineOilKmInterval;
ALTER TABLE userSettings RENAME COLUMN gearbox_oil_km_interval     TO gearboxOilKmInterval;
ALTER TABLE userSettings RENAME COLUMN final_drive_oil_km_interval TO finalDriveOilKmInterval;
ALTER TABLE userSettings RENAME COLUMN fork_oil_km_interval        TO forkOilKmInterval;
ALTER TABLE userSettings RENAME COLUMN brake_fluid_km_interval     TO brakeFluidKmInterval;
ALTER TABLE userSettings RENAME COLUMN coolant_km_interval         TO coolantKmInterval;
ALTER TABLE userSettings RENAME COLUMN chain_km_interval           TO chainKmInterval;
ALTER TABLE userSettings RENAME COLUMN updated_at                  TO updatedAt;

-- documents
ALTER TABLE documents RENAME COLUMN file_path    TO filePath;
ALTER TABLE documents RENAME COLUMN preview_path TO previewPath;
ALTER TABLE documents RENAME COLUMN uploaded_by  TO uploadedBy;
ALTER TABLE documents RENAME COLUMN owner_id     TO ownerId;
ALTER TABLE documents RENAME COLUMN is_private   TO isPrivate;
ALTER TABLE documents RENAME COLUMN created_at   TO createdAt;
ALTER TABLE documents RENAME COLUMN updated_at   TO updatedAt;

-- documentMotorcycles
ALTER TABLE documentMotorcycles RENAME COLUMN document_id   TO documentId;
ALTER TABLE documentMotorcycles RENAME COLUMN motorcycle_id TO motorcycleId;

-- previousOwners
ALTER TABLE previousOwners RENAME COLUMN motorcycle_id TO motorcycleId;
ALTER TABLE previousOwners RENAME COLUMN purchase_date TO purchaseDate;
ALTER TABLE previousOwners RENAME COLUMN phone_number  TO phoneNumber;
ALTER TABLE previousOwners RENAME COLUMN created_at    TO createdAt;
ALTER TABLE previousOwners RENAME COLUMN updated_at    TO updatedAt;

-- currencies
ALTER TABLE currencies RENAME COLUMN conversion_factor TO conversionFactor;
ALTER TABLE currencies RENAME COLUMN created_at        TO createdAt;
