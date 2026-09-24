//! BMWBike catalog lookup by BMW part number, server-side. The webapp can
//! query BMWBike directly (CORS `*`), but the iOS app has no copy of that
//! client — and both need the same fitment mapping onto the model catalog —
//! so this handler resolves a number to BMWBike's metadata and maps the
//! vehicle list onto `modelSeries` ids. Used to enrich aftermarket parts that
//! carry an OEM number (migration 053). Results are cached in-process.

use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::part_import::normalize_part_number,
};

const METADATA_API: &str = "https://admin.bmwbike.com/ss/api/v1/metadata/part";
const PART_PAGE: &str = "https://bmwbike.com/de/part";
// Public, search-only credentials embedded in bmwbike.com's own frontend
// (same as the webapp's `utils/bmwbike.ts`).
const ALGOLIA_APP_ID: &str = "J5Z2XFBN3D";
const ALGOLIA_SEARCH_KEY: &str = "0e975644d59dfe63927b2be969b3addb";
const ALGOLIA_INDEX: &str = "bmw_products_index_prod";
const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const USER_AGENT: &str = "MotoManager/1.0 (+https://moto.herrmann.ltd; part-lookup)";

/// Catalog data as BMWBike publishes it, before fitment mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct BmwbikeProduct {
    pub part_number: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub compat_names: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LookupResult {
    part_number: String,
    name: String,
    description: Option<String>,
    image_url: Option<String>,
    product_url: String,
    price: Option<f64>,
    currency: Option<String>,
    /// Catalog nodes the part fits, collapsed to the Serie where every Modell
    /// below it matched.
    series_ids: Vec<i64>,
    /// BMWBike vehicle names without a catalog counterpart (e.g. USA models).
    unmatched_compat: Vec<String>,
}

/// Normalized part number → (fetched at, product or negative hit).
type ProductCache = HashMap<String, (Instant, Option<BmwbikeProduct>)>;

static CACHE: OnceLock<Mutex<ProductCache>> = OnceLock::new();

fn cache() -> &'static Mutex<ProductCache> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `GET /api/part-imports/bmwbike/{part_number}` — any separators. Responds
/// `{"part": null}` when BMWBike does not carry the number; upstream failures
/// are a 502.
pub async fn lookup_part(
    State(pool): State<SqlitePool>,
    AuthUser(user): AuthUser,
    Path(part_number): Path<String>,
) -> AppResult<Json<Value>> {
    let normalized = normalize_part_number(&part_number);
    if normalized.len() < 5 {
        return Err(AppError::BadRequest(
            "Ungültige BMW-Teilenummer".to_string(),
        ));
    }

    let cached = cache()
        .lock()
        .expect("cache lock")
        .get(&normalized)
        .filter(|(stored, _)| stored.elapsed() < CACHE_TTL)
        .map(|(_, product)| product.clone());
    let product = match cached {
        Some(product) => product,
        None => {
            let product = fetch_product(&normalized).await?;
            cache()
                .lock()
                .expect("cache lock")
                .insert(normalized, (Instant::now(), product.clone()));
            product
        }
    };
    let Some(product) = product else {
        return Ok(Json(json!({ "part": null })));
    };

    let rows = sqlx::query(
        "SELECT id, name, parentId FROM modelSeries WHERE userId IS NULL OR userId = ?",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    let nodes: Vec<(i64, String, Option<i64>)> = rows
        .iter()
        .map(|r| (r.get("id"), r.get("name"), r.get("parentId")))
        .collect();
    let (series_ids, unmatched_compat) = map_compatibility(&product.compat_names, &nodes);

    Ok(Json(json!({
        "part": LookupResult {
            product_url: format!("{}/{}", PART_PAGE, product.slug),
            part_number: product.part_number,
            name: product.name,
            description: product.description,
            image_url: product.image_url,
            price: product.price,
            currency: product.currency,
            series_ids,
            unmatched_compat,
        }
    })))
}

async fn fetch_product(normalized: &str) -> AppResult<Option<BmwbikeProduct>> {
    crate::install_crypto_provider();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client: {}", e)))?;

    let search: Value = client
        .post(format!(
            "https://{}-dsn.algolia.net/1/indexes/{}/query",
            ALGOLIA_APP_ID.to_lowercase(),
            ALGOLIA_INDEX
        ))
        .header("x-algolia-application-id", ALGOLIA_APP_ID)
        .header("x-algolia-api-key", ALGOLIA_SEARCH_KEY)
        .json(&json!({
            "query": normalized,
            "hitsPerPage": 5,
            "restrictSearchableAttributes": ["article_no_search"],
            "attributesToRetrieve": ["article_no_search", "slug"],
        }))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| AppError::BadGateway(format!("BMWBike-Suche fehlgeschlagen: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::BadGateway(format!("BMWBike-Suche unlesbar: {}", e)))?;
    let Some(slug) = exact_hit_slug(&search, normalized) else {
        return Ok(None);
    };

    let response = client
        .get(format!("{}/{}", METADATA_API, slug))
        .send()
        .await
        .map_err(|e| AppError::BadGateway(format!("BMWBike nicht erreichbar: {}", e)))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let payload: Value = response
        .error_for_status()
        .map_err(|e| AppError::BadGateway(format!("BMWBike-Fehler: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::BadGateway(format!("BMWBike-Antwort unlesbar: {}", e)))?;
    Ok(parse_metadata(&payload, &slug))
}

/// Exact number match only — Algolia is typo-tolerant, and a fuzzy hit for a
/// different number must never enrich a part. BMWBike files some articles
/// under a variant suffix ("12321244409.1"); that counts as the same number,
/// but a plain exact hit wins over a variant.
fn exact_hit_slug(search: &Value, normalized: &str) -> Option<String> {
    let hits = search.get("hits")?.as_array()?;
    let with_number = |accept: &dyn Fn(&str) -> bool| {
        hits.iter().find_map(|hit| {
            let number = hit.get("article_no_search")?.as_str()?;
            accept(number)
                .then(|| hit.get("slug")?.as_str().map(str::to_string))
                .flatten()
        })
    };
    with_number(&|n| n == normalized).or_else(|| {
        with_number(&|n| {
            n.strip_prefix(normalized)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|variant| {
                    !variant.is_empty() && variant.chars().all(|c| c.is_ascii_digit())
                })
        })
    })
}

/// Mirror of the webapp's `fetchBmwbikePart` normalization.
fn parse_metadata(payload: &Value, slug: &str) -> Option<BmwbikeProduct> {
    let data = payload.get("data")?;
    let json_ld = payload.get("jsonLd");
    let text = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let part_number = text(data.get("article_no"))?;
    let name = text(data.get("description_de"))
        .or_else(|| text(data.get("name")))
        .unwrap_or_else(|| part_number.clone());
    let offers = json_ld.and_then(|ld| ld.get("offers"));
    let price = offers
        .and_then(|o| o.get("price"))
        .and_then(|p| p.as_f64().or_else(|| p.as_str()?.parse().ok()))
        .filter(|p| p.is_finite() && *p > 0.0);
    let compat_names = json_ld
        .and_then(|ld| ld.get("isAccessoryOrSparePartFor"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|v| text(v.get("name")))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(BmwbikeProduct {
        part_number,
        slug: slug.to_string(),
        name,
        description: text(
            json_ld
                .and_then(|ld| ld.get("category"))
                .and_then(|c| c.get("de")),
        ),
        image_url: text(data.get("image_url")),
        price,
        currency: text(offers.and_then(|o| o.get("priceCurrency"))),
        compat_names,
    })
}

/// Map BMWBike vehicle names onto catalog nodes `(id, name, parentId)` by
/// exact name (the catalog mirrors their structure, migration 023). When
/// every child of a node matched, the node itself replaces its children —
/// same coverage, shorter fitment list. Mirrors `mapCompatibility` in the
/// webapp.
pub fn map_compatibility(
    compat_names: &[String],
    nodes: &[(i64, String, Option<i64>)],
) -> (Vec<i64>, Vec<String>) {
    let by_name: HashMap<&str, i64> = nodes.iter().map(|(id, n, _)| (n.as_str(), *id)).collect();
    let mut matched: HashSet<i64> = HashSet::new();
    let mut unmatched: Vec<String> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for name in compat_names {
        if !seen.insert(name.as_str()) {
            continue;
        }
        match by_name.get(name.as_str()) {
            Some(id) => {
                matched.insert(*id);
            }
            None => unmatched.push(name.clone()),
        }
    }

    let mut children_of: HashMap<i64, Vec<i64>> = HashMap::new();
    for (id, _, parent) in nodes {
        if let Some(parent) = parent {
            children_of.entry(*parent).or_default().push(*id);
        }
    }
    for (parent, children) in &children_of {
        if !children.is_empty() && children.iter().all(|c| matched.contains(c)) {
            for c in children {
                matched.remove(c);
            }
            matched.insert(*parent);
        }
    }

    let mut ids: Vec<i64> = matched.into_iter().collect();
    ids.sort_unstable();
    (ids, unmatched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_payload() {
        let payload = json!({
            "data": {
                "article_no": "12 32 1 244 409",
                "description_de": "Spannungsregler",
                "image_url": "https://admin.bmwbike.com/media/regler.jpg"
            },
            "jsonLd": {
                "category": { "de": "Elektrik" },
                "offers": { "price": "89.50", "priceCurrency": "CHF" },
                "isAccessoryOrSparePartFor": [
                    { "name": "R 80 RT (82-84)" }, { "name": "" }, { "other": 1 }
                ]
            }
        });
        let product = parse_metadata(&payload, "regler-slug").unwrap();
        assert_eq!(product.part_number, "12 32 1 244 409");
        assert_eq!(product.name, "Spannungsregler");
        assert_eq!(product.description.as_deref(), Some("Elektrik"));
        assert_eq!(product.price, Some(89.5));
        assert_eq!(product.currency.as_deref(), Some("CHF"));
        assert_eq!(product.compat_names, vec!["R 80 RT (82-84)".to_string()]);
        assert!(parse_metadata(&json!({ "data": {} }), "x").is_none());
    }

    #[test]
    fn requires_exact_algolia_hit() {
        let search = json!({ "hits": [{ "article_no_search": "12321244409", "slug": "a" }] });
        assert_eq!(exact_hit_slug(&search, "12321244409").as_deref(), Some("a"));
        assert_eq!(exact_hit_slug(&search, "12321244408"), None);
        assert_eq!(exact_hit_slug(&json!({ "hits": [] }), "12321244409"), None);
        let variants = json!({ "hits": [
            { "article_no_search": "123212444091", "slug": "longer" },
            { "article_no_search": "12321244409.1", "slug": "variant" }
        ] });
        assert_eq!(
            exact_hit_slug(&variants, "12321244409").as_deref(),
            Some("variant")
        );
        let both = json!({ "hits": [
            { "article_no_search": "12321244409.1", "slug": "variant" },
            { "article_no_search": "12321244409", "slug": "plain" }
        ] });
        assert_eq!(
            exact_hit_slug(&both, "12321244409").as_deref(),
            Some("plain")
        );
    }

    #[test]
    fn maps_and_collapses_fitment() {
        let nodes = vec![
            (1, "R-Modelle 2V".to_string(), None),
            (10, "R 80 RT".to_string(), Some(1)),
            (11, "R 80 RT (82-84)".to_string(), Some(10)),
            (12, "R 80 RT (85-95)".to_string(), Some(10)),
            (20, "R 100 GS".to_string(), Some(1)),
            (21, "R 100 GS (87-90)".to_string(), Some(20)),
            (22, "R 100 GS PD (88-95)".to_string(), Some(20)),
        ];
        let names: Vec<String> = [
            "R 80 RT (82-84)",
            "R 80 RT (85-95)",
            "R 100 GS (87-90)",
            "R 80 RT (82-84)",
            "R 80 RT USA",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (ids, unmatched) = map_compatibility(&names, &nodes);
        assert_eq!(ids, vec![10, 21]);
        assert_eq!(unmatched, vec!["R 80 RT USA".to_string()]);
    }

    /// Live check against BMWBike: `cargo test --lib live_bmwbike -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_bmwbike_lookup() {
        let product = fetch_product("12321244409").await.unwrap().unwrap();
        assert_eq!(normalize_part_number(&product.part_number), "12321244409");
        assert!(!product.compat_names.is_empty());
    }
}
