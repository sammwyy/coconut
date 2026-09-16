use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct GeoEntry {
    pub code: String,
    pub name: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct RegionEntry {
    pub code: String,
    pub country: String,
    pub name: String,
}
#[derive(Deserialize)]
struct GeoData {
    countries: Vec<GeoEntry>,
    regions: Vec<RegionEntry>,
}

static CATALOG: OnceLock<GeoData> = OnceLock::new();
fn catalog() -> &'static GeoData {
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/geo.json"))
            .expect("valid generated geo catalog")
    })
}
pub fn countries(query: &str) -> Vec<&'static GeoEntry> {
    let query = query.to_ascii_lowercase();
    catalog()
        .countries
        .iter()
        .filter(|item| {
            query.is_empty()
                || item.name.to_ascii_lowercase().contains(&query)
                || item.code.to_ascii_lowercase().contains(&query)
        })
        .collect()
}
pub fn regions(country: &str, query: &str) -> Vec<&'static RegionEntry> {
    let query = query.to_ascii_lowercase();
    catalog()
        .regions
        .iter()
        .filter(|item| {
            item.country == country
                && (query.is_empty()
                    || item.name.to_ascii_lowercase().contains(&query)
                    || item.code.to_ascii_lowercase().contains(&query))
        })
        .collect()
}
pub fn is_country(code: &str) -> bool {
    catalog().countries.iter().any(|item| item.code == code)
}
pub fn is_region(code: &str) -> bool {
    catalog().regions.iter().any(|item| item.code == code)
}

pub fn country_name(code: &str) -> Option<&'static str> {
    catalog()
        .countries
        .iter()
        .find(|item| item.code == code)
        .map(|item| item.name.as_str())
}

pub fn region_name(code: &str) -> Option<&'static str> {
    catalog()
        .regions
        .iter()
        .find(|item| item.code == code)
        .map(|item| item.name.as_str())
}
