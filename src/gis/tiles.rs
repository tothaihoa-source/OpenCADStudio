//! Web map tiles (XYZ, Web Mercator) for the satellite / map background:
//! the sources CadMini offers, tile arithmetic, the disk cache and the
//! blocking download (run on a worker thread).

pub const MERC: f64 = 20_037_508.342_789_244;

#[derive(Clone, Debug, PartialEq)]
pub struct TileSource {
    pub key: &'static str,
    pub name: &'static str,
    /// `{z}`, `{x}`, `{y}` are replaced.
    pub url: String,
    pub max_zoom: u8,
    pub attribution: &'static str,
}

/// The background sources, as CadMini lists them.
pub fn sources() -> Vec<TileSource> {
    let s = |key, name, url: &str, max_zoom, attribution| TileSource {
        key,
        name,
        url: url.to_string(),
        max_zoom,
        attribution,
    };
    vec![
        s(
            "esri_imagery",
            "Esri World Imagery (satellite)",
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            19,
            "Imagery: Esri, Maxar, Earthstar Geographics",
        ),
        s(
            "esri_street",
            "Esri World Street Map",
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
            19,
            "Map: Esri",
        ),
        s(
            "osm",
            "OpenStreetMap",
            "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            19,
            "© OpenStreetMap contributors",
        ),
        s(
            "google_hybrid",
            "Google Hybrid (satellite + roads, names)",
            "https://mt1.google.com/vt/lyrs=y&x={x}&y={y}&z={z}",
            21,
            "Imagery: Google",
        ),
        s(
            "google_sat",
            "Google Satellite",
            "https://mt1.google.com/vt/lyrs=s&x={x}&y={y}&z={z}",
            21,
            "Imagery: Google",
        ),
        s(
            "google_terrain",
            "Google Terrain",
            "https://mt1.google.com/vt/lyrs=p&x={x}&y={y}&z={z}",
            20,
            "Map: Google",
        ),
        s(
            "opentopo",
            "OpenTopoMap (terrain, contours)",
            "https://tile.opentopomap.org/{z}/{x}/{y}.png",
            17,
            "© OpenTopoMap (CC-BY-SA), © OpenStreetMap contributors",
        ),
    ]
}

/// The source for `key`; `custom` uses `custom_url`.
pub fn source(key: &str, custom_url: &str) -> TileSource {
    if key == "custom" && custom_url.contains("{z}") {
        return TileSource {
            key: "custom",
            name: "Custom",
            url: custom_url.to_string(),
            max_zoom: 20,
            attribution: "",
        };
    }
    let all = sources();
    all.iter().find(|s| s.key == key).cloned().unwrap_or_else(|| all[0].clone())
}

pub type TileKey = (u8, u32, u32);

pub fn lonlat_to_merc(lon: f64, lat: f64) -> (f64, f64) {
    let lat = lat.clamp(-85.051_128_78, 85.051_128_78);
    let x = lon.to_radians() * 6_378_137.0;
    let y = (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * 6_378_137.0;
    (x, y)
}

pub fn merc_to_lonlat(x: f64, y: f64) -> (f64, f64) {
    let lon = (x / 6_378_137.0).to_degrees();
    let lat = (2.0 * (y / 6_378_137.0).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
    (lon, lat)
}

/// (mx0, my0, mx1, my1) of a tile in Web Mercator metres.
pub fn tile_bounds((z, x, y): TileKey) -> (f64, f64, f64, f64) {
    let n = f64::from(1u32 << z);
    let size = 2.0 * MERC / n;
    let mx0 = -MERC + f64::from(x) * size;
    let my1 = MERC - f64::from(y) * size;
    (mx0, my1 - size, mx0 + size, my1)
}

/// The tiles covering a Mercator box at zoom `z`.
pub fn tiles_for(mx0: f64, my0: f64, mx1: f64, my1: f64, z: u8) -> Vec<TileKey> {
    let n = 1u32 << z;
    let size = 2.0 * MERC / f64::from(n);
    let clamp = |v: f64| (v.floor().max(0.0) as u32).min(n - 1);
    let (x0, x1) = (clamp((mx0 + MERC) / size), clamp((mx1 + MERC) / size));
    let (y0, y1) = (clamp((MERC - my1) / size), clamp((MERC - my0) / size));
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            out.push((z, x, y));
        }
    }
    out
}

/// Zoom at which one image pixel is about one screen pixel.
pub fn zoom_for(metres_per_px: f64, lat: f64, max_zoom: u8) -> u8 {
    let ground = 156_543.033_92 * lat.to_radians().cos();
    let z = (ground / metres_per_px.max(1e-6)).log2().round();
    z.clamp(0.0, f64::from(max_zoom)) as u8
}

/// Where a tile is kept on disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn cache_path(source_key: &str, (z, x, y): TileKey) -> Option<std::path::PathBuf> {
    let mut p = crate::config::config_dir()?;
    p.push("tiles");
    p.push(source_key);
    p.push(z.to_string());
    p.push(x.to_string());
    p.push(format!("{y}.img"));
    Some(p)
}

/// A decoded tile.
#[derive(Clone, Debug)]
pub struct Tile {
    pub rgba: std::sync::Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

fn decode(bytes: &[u8]) -> Option<Tile> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    // Esri answers tiles it has no imagery for with a flat grey "Map data
    // not yet available" picture; treat a tile of one colour as missing.
    let px = img.as_raw();
    let first = &px[..4.min(px.len())];
    let flat = px.chunks_exact(4).step_by(97).all(|p| p == first);
    (!flat).then(|| Tile { rgba: std::sync::Arc::new(img.into_raw()), width, height })
}

/// The tile from the disk cache, else downloaded (and cached). `None` when
/// the server has no image for it. Blocking: call from a worker thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn load(source: &TileSource, key: TileKey) -> Option<Tile> {
    let path = cache_path(source.key, key);
    if let Some(bytes) = path.as_ref().and_then(|p| std::fs::read(p).ok()) {
        if let Some(tile) = decode(&bytes) {
            return Some(tile);
        }
    }
    let (z, x, y) = key;
    let url = source
        .url
        .replace("{z}", &z.to_string())
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string());
    let agent = crate::network::agent(std::time::Duration::from_secs(15));
    let mut response = agent
        .get(&url)
        .header("User-Agent", concat!("CadMiniStudio/", env!("OCS_APP_VERSION"), " (desktop CAD)"))
        .call()
        .ok()?;
    let bytes = response.body_mut().with_config().limit(8 << 20).read_to_vec().ok()?;
    let tile = decode(&bytes)?;
    if let Some(p) = path {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, &bytes);
    }
    Some(tile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mercator_round_trip_and_tiles() {
        let (x, y) = lonlat_to_merc(105.9, 18.35);
        let (lon, lat) = merc_to_lonlat(x, y);
        assert!((lon - 105.9).abs() < 1e-9 && (lat - 18.35).abs() < 1e-9);
        // The tile holding a point contains it.
        let z = 17;
        let key = tiles_for(x, y, x, y, z)[0];
        let (a, b, c, d) = tile_bounds(key);
        assert!(a <= x && x <= c && b <= y && y <= d);
        // A known tile: Hà Tĩnh area at z=10.
        assert_eq!(tiles_for(x, y, x, y, 10), vec![(10, 813, 458)]);
    }

    #[test]
    fn zoom_picks_about_one_pixel_per_pixel() {
        // ~0.3 m/px near 18°N is zoom 19.
        assert_eq!(zoom_for(0.28, 18.35, 21), 19);
        assert_eq!(zoom_for(0.28, 18.35, 17), 17);
        assert_eq!(zoom_for(1e6, 18.35, 19), 0);
    }

    #[test]
    fn sources_and_custom() {
        assert_eq!(sources().len(), 7);
        assert_eq!(source("osm", "").key, "osm");
        assert_eq!(source("nope", "").key, "esri_imagery");
        assert_eq!(source("custom", "https://x/{z}/{x}/{y}.png").key, "custom");
    }
}
