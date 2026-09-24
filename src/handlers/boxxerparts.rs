//! Catalog enrichment for boxxerparts.de articles. The shop (modified
//! eCommerce) has no API and sends no CORS headers, so the webapp cannot query
//! it directly the way it queries BMWBike — this handler proxies and scrapes
//! the product page on the server: article-number search → candidate product
//! pages → the one whose `itemprop="sku"` equals the article number (the
//! shop's search is fuzzy and also matches descriptions, so hits are not
//! unique). Results are cached in-process for a few hours; a whole order is
//! six lookups, not sixty.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{extract::Path, Json};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    handlers::part_import::{extract_oem_part_numbers, BOXXERPARTS_PREFIX},
};

const SHOP_BASE_URL: &str = "https://www.boxxerparts.de";
const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Fuzzy search hits inspected before giving up on an article number.
const MAX_CANDIDATES: usize = 5;
const USER_AGENT: &str = "MotoManager/1.0 (+https://moto.herrmann.ltd; part-import)";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoxxerpartsProduct {
    pub article_no: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub product_url: String,
    /// "Hersteller" as listed by the shop (often "BMW" even for aftermarket
    /// items — the shop files parts by the bike they fit).
    pub brand: Option<String>,
    pub oem_part_numbers: Vec<String>,
    pub keywords: Vec<String>,
}

/// Article number → (fetched at, product or negative hit).
type ProductCache = HashMap<String, (Instant, Option<BoxxerpartsProduct>)>;

static CACHE: OnceLock<Mutex<ProductCache>> = OnceLock::new();

fn cache() -> &'static Mutex<ProductCache> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `GET /api/part-imports/boxxerparts/{article_no}` — article number with or
/// without the `BXP-` namespace prefix. Responds `{"product": null}` when the
/// shop does not carry the number; upstream failures are a 502.
pub async fn lookup_product(
    AuthUser(_user): AuthUser,
    Path(article_no): Path<String>,
) -> AppResult<Json<Value>> {
    let article_no = normalize_article_no(&article_no).ok_or_else(|| {
        AppError::BadRequest("Ungültige Boxxerparts-Artikelnummer (4-6 Ziffern)".to_string())
    })?;

    if let Some((stored, product)) = cache().lock().expect("cache lock").get(&article_no) {
        if stored.elapsed() < CACHE_TTL {
            return Ok(Json(json!({ "product": product })));
        }
    }

    let product = fetch_product(&article_no).await?;
    cache()
        .lock()
        .expect("cache lock")
        .insert(article_no, (Instant::now(), product.clone()));
    Ok(Json(json!({ "product": product })))
}

/// Strip the namespace prefix and validate the shop's numeric article format.
pub fn normalize_article_no(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let digits = trimmed
        .strip_prefix(BOXXERPARTS_PREFIX)
        .or_else(|| trimmed.strip_prefix(&BOXXERPARTS_PREFIX.to_lowercase()))
        .unwrap_or(trimmed);
    ((4..=6).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.to_string())
}

async fn fetch_product(article_no: &str) -> AppResult<Option<BoxxerpartsProduct>> {
    crate::install_crypto_provider();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| AppError::Internal(format!("HTTP client: {}", e)))?;

    let search_url = format!(
        "{}/advanced_search_result.php?keywords={}",
        SHOP_BASE_URL, article_no
    );
    let search_html = get_html(&client, &search_url).await?;
    for product_id in candidate_product_ids(&search_html) {
        let product_url = product_page_url(&product_id);
        let page = get_html(&client, &product_url).await?;
        if let Some(product) = parse_product_page(&page, &product_url) {
            if product.article_no == article_no {
                return Ok(Some(product));
            }
        }
    }
    Ok(None)
}

async fn get_html(client: &reqwest::Client, url: &str) -> AppResult<String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::BadGateway(format!("boxxerparts.de nicht erreichbar: {}", e)))?;
    if !response.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "boxxerparts.de antwortete mit HTTP {}",
            response.status()
        )));
    }
    response
        .text()
        .await
        .map_err(|e| AppError::BadGateway(format!("boxxerparts.de Antwort unlesbar: {}", e)))
}

fn product_page_url(product_id: &str) -> String {
    format!(
        "{}/product_info.php?products_id={}",
        SHOP_BASE_URL, product_id
    )
}

/// Product ids linked from a search-result page, in page order, deduplicated
/// and capped — the listing links every hit several times (image, title).
pub fn candidate_product_ids(search_html: &str) -> Vec<String> {
    let re = regex::Regex::new(r"product_info\.php\?products_id=(\d+)").expect("static regex");
    let mut ids: Vec<String> = Vec::new();
    for cap in re.captures_iter(search_html) {
        let id = cap[1].to_string();
        if !ids.contains(&id) {
            ids.push(id);
            if ids.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }
    ids
}

/// Scrape a product page. Returns None when the page has no article number
/// (e.g. a category page slipped into the candidates).
pub fn parse_product_page(html: &str, product_url: &str) -> Option<BoxxerpartsProduct> {
    let sku_re = regex::Regex::new(r#"itemprop="sku"[^>]*>\s*([^<]+?)\s*<"#).expect("static regex");
    let article_no = sku_re.captures(html).map(|c| c[1].to_string())?;

    let name_re =
        regex::Regex::new(r#"<h1[^>]*itemprop="name"[^>]*>([^<]*)</h1>"#).expect("static regex");
    let fallback_name_re = regex::Regex::new(r"<h1[^>]*>([^<]*)</h1>").expect("static regex");
    let name = name_re
        .captures(html)
        .or_else(|| fallback_name_re.captures(html))
        .map(|c| decode_entities(c[1].trim()))
        .filter(|n| !n.is_empty())?;

    let description = balanced_div_inner(html, r#"class="pd_description""#)
        .map(|inner| html_to_text(&inner))
        .filter(|t| !t.is_empty())
        .or_else(|| meta_content(html, r#"name="description""#));

    let image_url = meta_content(html, r#"property="og:image""#).or_else(|| {
        regex::Regex::new(r#"itemprop="image"[^>]*src="([^"]+)""#)
            .expect("static regex")
            .captures(html)
            .map(|c| decode_entities(&c[1]))
    });

    let brand_re =
        regex::Regex::new(r#"(?s)itemprop="brand"[^>]*>.{0,200}?itemprop="name"[^>]*>([^<]*)<"#)
            .expect("static regex");
    let brand = brand_re
        .captures(html)
        .map(|c| decode_entities(c[1].trim()))
        .filter(|b| !b.is_empty());

    let keywords: Vec<String> = meta_content(html, r#"name="keywords""#)
        .map(|raw| {
            raw.split(',')
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let mut oem_source = description.clone().unwrap_or_default();
    oem_source.push(' ');
    oem_source.push_str(&keywords.join(" "));
    let oem_part_numbers = extract_oem_part_numbers(&oem_source);

    Some(BoxxerpartsProduct {
        article_no,
        name,
        description,
        image_url,
        product_url: meta_content(html, r#"property="og:url""#)
            .unwrap_or_else(|| product_url.to_string()),
        brand,
        oem_part_numbers,
        keywords,
    })
}

fn meta_content(html: &str, attr: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(
        r#"<meta\s+{}\s+content="([^"]*)""#,
        regex::escape(attr)
    ))
    .expect("static regex");
    re.captures(html)
        .map(|c| decode_entities(c[1].trim()))
        .filter(|s| !s.is_empty())
}

/// Inner HTML of the first `<div …ATTR…>` whose div nesting is balanced —
/// the description contains nested `<div>`s, so a lazy "up to the first
/// `</div>`" match would cut it short.
fn balanced_div_inner(html: &str, attr: &str) -> Option<String> {
    let start_attr = html.find(attr)?;
    let open_tag_end = html[start_attr..].find('>')? + start_attr + 1;
    let rest = &html[open_tag_end..];
    let token_re = regex::Regex::new(r"(?i)<div\b|</div\s*>").expect("static regex");
    let mut depth = 1usize;
    for m in token_re.find_iter(rest) {
        if m.as_str().starts_with("</") || m.as_str().starts_with("</") {
            depth -= 1;
            if depth == 0 {
                return Some(rest[..m.start()].to_string());
            }
        } else {
            depth += 1;
        }
    }
    None
}

/// Flatten product-description HTML to plain text: block tags become line
/// breaks, other tags vanish, entities are decoded, whitespace collapses.
pub fn html_to_text(html: &str) -> String {
    let block_re =
        regex::Regex::new(r"(?i)<br\s*/?>|</(?:div|p|li|h\d|tr)\s*>").expect("static regex");
    let tag_re = regex::Regex::new(r"(?s)<[^>]+>").expect("static regex");
    let with_breaks = block_re.replace_all(html, "\n");
    let stripped = tag_re.replace_all(&with_breaks, " ");
    let decoded = decode_entities(&stripped);
    let lines: Vec<String> = decoded
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

/// Minimal HTML entity decoding for the handful the shop emits.
fn decode_entities(raw: &str) -> String {
    let named = [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
        ("&auml;", "ä"),
        ("&ouml;", "ö"),
        ("&uuml;", "ü"),
        ("&Auml;", "Ä"),
        ("&Ouml;", "Ö"),
        ("&Uuml;", "Ü"),
        ("&szlig;", "ß"),
        ("&euro;", "€"),
    ];
    let mut out = raw.to_string();
    for (entity, replacement) in named {
        out = out.replace(entity, replacement);
    }
    let numeric_re = regex::Regex::new(r"&#(x?)([0-9a-fA-F]+);").expect("static regex");
    numeric_re
        .replace_all(&out, |cap: &regex::Captures| {
            let value = if cap[1].is_empty() {
                cap[2].parse::<u32>().ok()
            } else {
                u32::from_str_radix(&cap[2], 16).ok()
            };
            value
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_default()
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed copies of the real markup (products_id 3052, article 44555).
    const PRODUCT_3052: &str = r#"<html><head><title>Regler Wehrle für alle R2V Boxer ab 69 - BMW</title>
<meta name="description" content="Regler Wehrle für alle R2V Boxer ab 69: Regler passend für alle 2 V Boxer ab Baujahr 1969   vom Erstausrüster   BMW 12321244409 Wehrle 55990002" />
<meta name="keywords" content="regler, wehrle, boxer, passend, baujahr, erstausrüster, bmw, 12321244409, 55990002" />
<meta property="og:url" content="https://www.boxxerparts.de/product_info.php?products_id=3052" />
<meta property="og:image" content="https://www.boxxerparts.de/images/product_images/popup_images/3052_0.jpg" />
</head><body>
<div itemscope itemtype="https://schema.org/Product">
  <form id="cart_quantity" action="https://www.boxxerparts.de/product_info.php?products_id=3052&amp;action=add_product" method="post">
  <div class="product_headline cf">
        <h1 itemprop="name">Regler Wehrle f&uuml;r alle R2V Boxer ab 69</h1>
  </div>
  <div id="product_details">
    <a class="cbimages"><img itemprop="image" src="https://www.boxxerparts.de/images/product_images/info_images/3052_0.jpg" alt="Regler" /></a>
    <div class="pd_inforow"><strong>Lieferzeit:</strong> 3-4 Tage</div>        <div class="pd_inforow"><strong>Art.Nr.:</strong> <span itemprop="sku">44555</span></div>
    <div class="pd_inforow" itemprop="brand" itemscope itemtype="https://schema.org/Brand"><strong>Hersteller:</strong> <span itemprop="name">BMW</span></div>
    <meta itemprop="price" content="58.5" />
  </div>
  </form>
  <h4 class="detailbox">Produktbeschreibung</h4><div class="pd_description" itemprop="description">Regler passend für alle 2 V Boxer ab Baujahr 1969<br />
&nbsp;
<div>vom Erstausrüster</div>

<div>&nbsp;</div>

<div>BMW 12321244409</div>

<div>Wehrle 55990002</div></div>            <h4 class="detailbox">Kunden, die diesen Artikel kauften, haben auch folgende Artikel bestellt:</h4>
  <div class="listingcontainer_details cf"><div class="listingrow"><a href="https://www.boxxerparts.de/product_info.php?products_id=330">Winkelventil</a></div></div>
</div></body></html>"#;

    const SEARCH_44545: &str = r#"<div class="listingbox"><a href="https://www.boxxerparts.de/product_info.php?products_id=18"><img /></a>
<div class="lb_title"><a href="https://www.boxxerparts.de/product_info.php?products_id=18">Auspuff Sternmutter</a></div></div>
<div class="listingbox"><a href="https://www.boxxerparts.de/product_info.php?products_id=1142"><img /></a>
<div class="lb_title"><a href="https://www.boxxerparts.de/product_info.php?products_id=1142">Sternmutterschlüssel</a></div></div>"#;

    #[test]
    fn parses_product_page_microdata() {
        let product = parse_product_page(
            PRODUCT_3052,
            "https://www.boxxerparts.de/product_info.php?products_id=3052",
        )
        .expect("product");
        assert_eq!(product.article_no, "44555");
        assert_eq!(product.name, "Regler Wehrle für alle R2V Boxer ab 69");
        assert_eq!(
            product.description.as_deref(),
            Some("Regler passend für alle 2 V Boxer ab Baujahr 1969\nvom Erstausrüster\nBMW 12321244409\nWehrle 55990002")
        );
        assert_eq!(
            product.image_url.as_deref(),
            Some("https://www.boxxerparts.de/images/product_images/popup_images/3052_0.jpg")
        );
        assert_eq!(
            product.product_url,
            "https://www.boxxerparts.de/product_info.php?products_id=3052"
        );
        assert_eq!(product.brand.as_deref(), Some("BMW"));
        assert_eq!(
            product.oem_part_numbers,
            vec!["12 32 1 244 409".to_string()]
        );
        assert_eq!(product.keywords[0], "regler");
        assert_eq!(product.keywords.len(), 9);
    }

    #[test]
    fn page_without_sku_is_rejected() {
        assert!(parse_product_page("<html><h1>Kategorie</h1></html>", "x").is_none());
    }

    #[test]
    fn description_falls_back_to_meta_when_block_is_missing() {
        let html = r#"<meta name="description" content="Nur Meta &amp; Text" /><h1>Teil</h1><span itemprop="sku">1</span>"#;
        let product = parse_product_page(html, "u").unwrap();
        assert_eq!(product.description.as_deref(), Some("Nur Meta & Text"));
        assert_eq!(product.product_url, "u");
        assert_eq!(product.image_url, None);
    }

    #[test]
    fn search_candidates_are_ordered_unique_and_capped() {
        assert_eq!(candidate_product_ids(SEARCH_44545), vec!["18", "1142"]);
        let many: String = (1..=10)
            .map(|i| format!("product_info.php?products_id={}", i))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(candidate_product_ids(&many).len(), MAX_CANDIDATES);
        assert!(candidate_product_ids("<p>Keine Treffer</p>").is_empty());
    }

    #[test]
    fn article_numbers_normalize_with_or_without_prefix() {
        assert_eq!(normalize_article_no("44555").as_deref(), Some("44555"));
        assert_eq!(normalize_article_no("BXP-44555").as_deref(), Some("44555"));
        assert_eq!(
            normalize_article_no(" bxp-96065 ").as_deref(),
            Some("96065")
        );
        assert_eq!(normalize_article_no("12 32 1 244 409"), None);
        assert_eq!(normalize_article_no("abc"), None);
        assert_eq!(normalize_article_no("123"), None);
    }

    /// Opt-in check against the live shop (the search for 44545 is fuzzy
    /// and returns two hits — the sku check must pick the Sternmutter):
    /// `cargo test --lib live_boxxerparts -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_boxxerparts_lookup_resolves_fuzzy_search() {
        let product = fetch_product("44545")
            .await
            .expect("shop reachable")
            .expect("article carried");
        assert_eq!(product.article_no, "44545");
        assert!(product.name.contains("Sternmutter"), "{}", product.name);
        assert!(product.image_url.is_some());
        assert!(product.description.is_some());
        let regulator = fetch_product("44555").await.unwrap().unwrap();
        assert_eq!(
            regulator.oem_part_numbers,
            vec!["12 32 1 244 409".to_string()]
        );
        assert!(fetch_product("00000").await.unwrap().is_none());
    }

    #[test]
    fn html_to_text_flattens_blocks_and_entities() {
        assert_eq!(
            html_to_text("A&nbsp;&amp;&nbsp;B<br/>\n<div>C &uuml; &#8364;</div><p>D</p>"),
            "A & B\nC ü €\nD"
        );
    }
}
