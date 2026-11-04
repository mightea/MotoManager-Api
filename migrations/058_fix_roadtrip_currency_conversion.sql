-- Fix normalizedCost of fuel records imported from Road Trip in a foreign
-- currency. Road Trip's "Currency Rate" is foreign units per CHF (SEK 7.08 =
-- CHF 1), but the import multiplied by it instead of dividing, so e.g.
-- SEK 231.02 was stored as CHF 1'635.83 instead of CHF 32.63.
--
-- Values come from the original Road Trip exports (BMW R100, R1200 GS Rallye).
-- A row is only touched when date, odometer, amount and currency match the
-- export AND its normalizedCost still equals the wrong amount × rate — so the
-- migration is idempotent and leaves manually corrected rows alone. Changed
-- rows get a fresh updatedAt so offline clients pull the fix.
WITH roadtrip(date, odo, cost, currency, rate) AS (
    VALUES
        ('2010-07-08', 125337, 22.82, 'EUR', 0.748298),
        ('2010-07-09', 125540, 120.53, 'DKK', 5.575737),
        ('2010-07-10', 125832, 231.02, 'SEK', 7.080938),
        ('2010-07-14', 126129, 222.91, 'SEK', 7.020298),
        ('2010-07-15', 126383, 179.06, 'NOK', 5.962026),
        ('2010-07-16', 126692, 214.34, 'NOK', 5.962026),
        ('2010-07-17', 126999, 211.82, 'NOK', 5.962026),
        ('2010-07-20', 127271, 23.19, 'EUR', 0.73589),
        ('2017-07-18', 3982, 19.04, 'EUR', 0.832141),
        ('2017-07-19', 4208, 18.1, 'EUR', 0.832141),
        ('2020-07-04', 9869, 16.24, 'EUR', 0.941228),
        ('2020-07-10', 10602, 35.61, 'EUR', 0.941228),
        ('2020-07-10', 10862, 22.34, 'EUR', 0.941228),
        ('2020-07-11', 11093, 18.88, 'EUR', 0.941228),
        ('2020-07-12', 11402, 24.01, 'EUR', 0.940222),
        ('2022-08-12', 15964, 22.81, 'EUR', 1.029618),
        ('2022-08-13', 16272, 18.29, 'EUR', 1.029618),
        ('2022-08-13', 16356, 5.8, 'EUR', 1.029618),
        ('2022-08-15', 16657, 26.74, 'EUR', 1.029618),
        ('2022-09-17', 17558, 31.93, 'EUR', 1.034284),
        ('2022-09-21', 17865, 20.26, 'EUR', 1.040291),
        ('2022-09-28', 18171, 30, 'EUR', 1.053529),
        ('2022-09-29', 18433, 22.1, 'EUR', 1.051959),
        ('2022-10-01', 18635, 14.93, 'EUR', 1.03186),
        ('2022-10-01', 18870, 20.35, 'EUR', 1.03186),
        ('2022-10-03', 19063, 17.7, 'EUR', 1.034362),
        ('2022-10-03', 19280, 18.79, 'EUR', 1.034362),
        ('2022-10-04', 19599, 25.88, 'EUR', 1.034362),
        ('2023-09-22', 21192, 31.06, 'EUR', 1.022165),
        ('2024-06-19', 22838, 30.2, 'EUR', 1.053725),
        ('2024-06-22', 23093, 26.72, 'EUR', 1.0467),
        ('2024-06-23', 23339, 33.48, 'EUR', 1.0467),
        ('2025-06-06', 24191, 26.55, 'EUR', 1.044424),
        ('2025-06-07', 24467, 27.46, 'EUR', 1.067098),
        ('2025-06-08', 24733, 24.49, 'EUR', 1.066908),
        ('2025-06-09', 24950, 18.56, 'EUR', 1.06674),
        ('2025-06-21', 25246, 29.86, 'EUR', 1.061033),
        ('2025-06-22', 25459, 19.46, 'EUR', 1.060876),
        ('2025-06-24', 25732, 28.09, 'EUR', 1.061554)
)
UPDATE maintenanceRecords
SET normalizedCost = maintenanceRecords.cost / rt.rate,
    updatedAt = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM roadtrip rt
WHERE maintenanceRecords.type = 'fuel'
  AND substr(maintenanceRecords.date, 1, 10) = rt.date
  AND maintenanceRecords.odo = rt.odo
  AND maintenanceRecords.currency = rt.currency
  AND abs(maintenanceRecords.cost - rt.cost) < 0.005
  AND abs(maintenanceRecords.normalizedCost - maintenanceRecords.cost * rt.rate) < 0.01;
