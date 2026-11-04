//! BANDOHT — the present-state map inside a boundary, from open data, as
//! CadMini: roads (centre lines, kerbs rounded with the radius of the road's
//! class at junctions, names along them), places, buildings, water, rivers
//! and their names, greenery, farmland, railways and power lines, and the
//! terrain (contours, spot heights) from Terrarium elevation tiles.
//!
//! Everything here works in local coordinates (the drawing's minus an
//! origin near the boundary) so the polygon operations keep their
//! precision; the command adds the origin back. `build` is pure: it takes
//! the downloaded data and returns the items to draw. `fetch_overpass` and
//! `terrarium` do the downloads (blocking, cached on disk).

use std::collections::{BTreeSet, HashMap};

use geo::{Area, BooleanOps, Buffer, Contains, InteriorPoint};
use geo::{Coord, LineString, MultiLineString, MultiPolygon, Point, Polygon};
use serde::{Deserialize, Serialize};

pub type Xy = [f64; 2];
pub type Tags = HashMap<String, String>;

// highway → (group, kerb-to-kerb width m, kerb radius R m, name height m)
pub fn road_spec(hw: &str) -> Option<(&'static str, f64, f64, f64)> {
    Some(match hw {
        "motorway" => ("chinh", 24.0, 12.0, 4.0),
        "trunk" => ("chinh", 20.0, 12.0, 4.0),
        "primary" => ("chinh", 15.0, 12.0, 4.0),
        "secondary" => ("chinh", 12.0, 10.0, 3.5),
        "tertiary" => ("khu_vuc", 9.0, 8.0, 3.0),
        "unclassified" | "residential" | "road" => ("noi_bo", 6.0, 5.0, 2.5),
        "living_street" | "service" | "pedestrian" => ("ngo", 4.0, 3.0, 2.0),
        "track" => ("ngo", 3.5, 3.0, 2.0),
        "footway" | "path" | "cycleway" | "steps" => ("loi_di", 2.0, 2.0, 1.5),
        _ => return None,
    })
}

pub const L_TIM: &str = "Tim_duong";
pub const L_MEP: &str = "Mep_duong";
pub const L_TEN: &str = "Ten_duong";
pub const L_TEN_NGO: &str = "Ten_ngo";
pub const L_NHA: &str = "HT_Nha";
pub const L_NUOC: &str = "HT_Mat_nuoc";
pub const L_SONG: &str = "HT_Song_suoi";
pub const L_TEN_SONG: &str = "Ten_song_suoi";
pub const L_CX: &str = "HT_Cay_xanh";
pub const L_NN: &str = "HT_Nong_nghiep";
pub const L_SAT: &str = "HT_Duong_sat";
pub const L_DIEN: &str = "HT_Duong_dien";
pub const L_DM: &str = "HT_Duong_dong_muc";
pub const L_DM_CAI: &str = "HT_Duong_dong_muc_cai";
pub const L_CAODO: &str = "HT_Cao_do";
/// Place groups: (key, layer, label when nameless, colour, dialog text).
pub const POI: &[(&str, &str, &str, i16, &str)] = &[
    ("dia_danh", "DD_Dia_danh", "", 7, "Địa danh (thôn, xóm, phường...)"),
    ("truong_hoc", "DD_Truong_hoc", "Trường học", 3, "Trường học"),
    ("quan_an", "DD_Quan_an", "Quán ăn", 30, "Quán ăn, cà phê"),
    ("nha_nghi", "DD_Nha_nghi", "Nhà nghỉ", 4, "Khách sạn, nhà nghỉ"),
    ("khac", "DD_Khac", "", 2, "Địa điểm khác"),
];
pub const LAYERS: &[(&str, i16)] = &[
    (L_TIM, 1),
    (L_MEP, 6),
    (L_TEN, 7),
    (L_TEN_NGO, 8),
    (L_NHA, 7),
    (L_NUOC, 5),
    (L_SONG, 5),
    (L_TEN_SONG, 5),
    (L_CX, 3),
    (L_NN, 52),
    (L_SAT, 8),
    (L_DIEN, 30),
    (L_DM, 33),
    (L_DM_CAI, 32),
    (L_CAODO, 7),
    ("DD_Dia_danh", 7),
    ("DD_Truong_hoc", 3),
    ("DD_Quan_an", 30),
    ("DD_Nha_nghi", 4),
    ("DD_Khac", 2),
];

/// What to fetch and draw (CadMini's options, without the AI ones).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub roads: bool,
    pub road_names: bool,
    pub paths: bool,
    pub places: BTreeSet<String>,
    pub buildings: bool,
    pub water: bool,
    pub green: bool,
    pub farm: bool,
    pub rail_power: bool,
    pub elevation: bool,
    /// 0: chosen from the relief.
    pub contour_step: f64,
    /// 0: chosen from the boundary's size.
    pub spot_spacing: f64,
    pub clear: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            roads: true,
            road_names: true,
            paths: false,
            places: POI.iter().map(|p| p.0.to_string()).collect(),
            buildings: true,
            water: true,
            green: true,
            farm: true,
            rail_power: false,
            elevation: false,
            contour_step: 0.0,
            spot_spacing: 0.0,
            clear: false,
        }
    }
}

impl Options {
    /// Roads only (OSMDUONG).
    pub fn roads_only() -> Self {
        Self { buildings: false, water: false, green: false, farm: false, ..Self::default() }
    }

    pub fn osm_parts(&self) -> BTreeSet<&'static str> {
        let mut p = BTreeSet::new();
        if self.roads || self.road_names {
            p.insert("roads");
        }
        if !self.places.is_empty() {
            p.insert("places");
        }
        for (on, name) in
            [(self.buildings, "buildings"), (self.water, "water"), (self.green, "green"), (self.farm, "farm")]
        {
            if on {
                p.insert(name);
            }
        }
        if self.rail_power {
            p.insert("rail");
            p.insert("power");
        }
        p
    }
}

/// One thing to draw, in local coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Vertices with bulges.
    Pline { layer: &'static str, pts: Vec<[f64; 3]>, closed: bool },
    /// Middle-centre text, or middle-left when `left`.
    Text { layer: &'static str, at: Xy, s: String, h: f64, rot_deg: f64, left: bool },
    Circle { layer: &'static str, c: Xy, r: f64 },
    /// Solid fill, outer ring first then holes.
    Hatch { layer: &'static str, rings: Vec<Vec<Xy>>, color: i16 },
}

impl Item {
    pub fn layer(&self) -> &'static str {
        match self {
            Item::Pline { layer, .. } | Item::Text { layer, .. } | Item::Circle { layer, .. } | Item::Hatch { layer, .. } => {
                layer
            }
        }
    }
}

// ── Overpass ────────────────────────────────────────────────────────────────

/// The Overpass query for `bbox` (lon0, lat0, lon1, lat1).
pub fn overpass_query(bbox: [f64; 4], parts: &BTreeSet<&str>) -> String {
    let b = format!("({:.7},{:.7},{:.7},{:.7})", bbox[1], bbox[0], bbox[3], bbox[2]);
    let mut q: Vec<String> = Vec::new();
    let mut add = |s: &str| q.push(s.replace("{b}", &b));
    if parts.contains("roads") {
        add(r#"way["highway"]{b};"#);
    }
    if parts.contains("water") {
        for s in [
            r#"way["natural"="water"]{b};"#,
            r#"relation["natural"="water"]{b};"#,
            r#"way["waterway"]{b};"#,
            r#"relation["waterway"="riverbank"]{b};"#,
            r#"way["landuse"~"^(reservoir|basin|aquaculture)$"]{b};"#,
        ] {
            add(s);
        }
    }
    if parts.contains("green") {
        for s in [
            r#"way["natural"~"^(wood|scrub|grassland|wetland)$"]{b};"#,
            r#"relation["natural"~"^(wood|scrub)$"]{b};"#,
            r#"way["landuse"~"^(forest|grass|meadow|orchard|cemetery|village_green|recreation_ground)$"]{b};"#,
            r#"relation["landuse"="forest"]{b};"#,
            r#"way["leisure"~"^(park|garden|pitch|stadium)$"]{b};"#,
        ] {
            add(s);
        }
    }
    if parts.contains("farm") {
        add(r#"way["landuse"~"^(farmland|farmyard|paddy|plant_nursery)$"]{b};"#);
    }
    if parts.contains("buildings") {
        add(r#"way["building"]{b};"#);
        add(r#"relation["building"]{b};"#);
    }
    if parts.contains("places") {
        for s in [
            r#"node["place"]{b};"#,
            r#"node["name"]["amenity"]{b};"#,
            r#"node["amenity"~"^(school|kindergarten)$"]{b};"#,
            r#"way["amenity"]["name"]{b};"#,
            r#"node["name"]["tourism"]{b};"#,
            r#"way["name"]["tourism"]{b};"#,
            r#"node["name"]["shop"]{b};"#,
            r#"node["name"]["historic"]{b};"#,
            r#"node["name"]["office"="government"]{b};"#,
        ] {
            add(s);
        }
    }
    if parts.contains("rail") {
        add(r#"way["railway"~"^(rail|narrow_gauge|light_rail)$"]{b};"#);
    }
    if parts.contains("power") {
        add(r#"way["power"~"^(line|minor_line)$"]{b};"#);
    }
    format!("[out:json][timeout:180];({});out body geom;", q.concat())
}

const OVERPASS: &[&str] = &[
    "https://overpass-api.de/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
    "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
    "http://overpass-api.de/api/interpreter",
];

fn hash(s: &str) -> String {
    use sha2::Digest;
    let d = sha2::Sha256::digest(s.as_bytes());
    d.iter().take(10).map(|b| format!("{b:02x}")).collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn cache_dir(sub: &str) -> Option<std::path::PathBuf> {
    let mut p = crate::config::config_dir()?;
    p.push(sub);
    Some(p)
}

/// The answer to an Overpass query: from the disk cache (30 days), else
/// downloaded, trying the mirrors in turn. Blocking.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_overpass(query: &str, say: &dyn Fn(&str)) -> Result<serde_json::Value, String> {
    let path = cache_dir("osm").map(|d| d.join(format!("op_{}.json", hash(query))));
    if let Some(p) = &path {
        let fresh = std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() < 30 * 86_400);
        if fresh {
            if let Some(v) = std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
                return Ok(v);
            }
        }
    }
    let agent = crate::network::agent(std::time::Duration::from_secs(200));
    let mut errors = Vec::new();
    for url in OVERPASS {
        say(&format!("Đang tải OpenStreetMap ({url}) — máy chủ có thể mất 30–90 giây…"));
        let r = agent
            .post(*url)
            .header("User-Agent", concat!("CadMiniStudio/", env!("OCS_APP_VERSION"), " (desktop CAD)"))
            .send_form([("data", query)]);
        let mut resp = match r {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("{url}: {e}"));
                continue;
            }
        };
        let bytes = match resp.body_mut().with_config().limit(400 << 20).read_to_vec() {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("{url}: {e}"));
                continue;
            }
        };
        let v: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{url}: {e}"));
                continue;
            }
        };
        if v.get("remark").and_then(|r| r.as_str()).is_some_and(|r| r.starts_with("runtime error")) {
            errors.push(format!("{url}: {}", v["remark"]));
            continue;
        }
        if let Some(p) = &path {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(p, &bytes);
        }
        return Ok(v);
    }
    Err(format!("Không tải được OpenStreetMap: {}", errors.join("; ")))
}

// ── Terrarium elevation ─────────────────────────────────────────────────────

pub const TERRARIUM: &str = "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png";
/// ~9 m a pixel in Vietnam: finer than the 30 m data.
pub const TERRAIN_ZOOM: u8 = 14;

/// Terrarium encoding: R·256 + G + B/256 − 32768 metres.
pub fn terrarium_height(r: u8, g: u8, b: u8) -> f64 {
    f64::from(r) * 256.0 + f64::from(g) + f64::from(b) / 256.0 - 32768.0
}

fn tile_xy(lon: f64, lat: f64, z: u8) -> Xy {
    let n = f64::from(1u32 << z);
    let lr = lat.to_radians();
    [(lon + 180.0) / 360.0 * n, (1.0 - (lr.tan() + 1.0 / lr.cos()).ln() / std::f64::consts::PI) / 2.0 * n]
}

/// Where a Terrarium tile is kept.
#[cfg(not(target_arch = "wasm32"))]
pub fn terrain_tile_path(z: u8, x: u32, y: u32) -> Option<std::path::PathBuf> {
    cache_dir("terrain").map(|d| d.join("terrarium").join(z.to_string()).join(x.to_string()).join(format!("{y}.png")))
}

#[cfg(not(target_arch = "wasm32"))]
fn terrain_tile(z: u8, x: u32, y: u32) -> Result<Vec<f64>, String> {
    let path = terrain_tile_path(z, x, y);
    let bytes = match path.as_ref().and_then(|p| std::fs::read(p).ok()) {
        Some(b) => b,
        None => {
            let url = TERRARIUM.replace("{z}", &z.to_string()).replace("{x}", &x.to_string()).replace("{y}", &y.to_string());
            let agent = crate::network::agent(std::time::Duration::from_secs(60));
            let mut r = agent.get(&url).call().map_err(|e| format!("{url}: {e}"))?;
            let b = r.body_mut().with_config().limit(8 << 20).read_to_vec().map_err(|e| e.to_string())?;
            if let Some(p) = &path {
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::write(p, &b);
            }
            b
        }
    };
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.to_rgb8();
    if img.dimensions() != (256, 256) {
        return Err("ô cao độ sai kích thước".into());
    }
    Ok(img.pixels().map(|p| terrarium_height(p[0], p[1], p[2])).collect())
}

/// Heights (m) at (lon, lat) points, bilinear on the Terrarium tiles. Blocking.
#[cfg(not(target_arch = "wasm32"))]
pub fn terrain_sample(lonlat: &[Xy], say: &dyn Fn(&str)) -> Result<Vec<f64>, String> {
    let z = TERRAIN_ZOOM;
    let px: Vec<Xy> = lonlat
        .iter()
        .map(|q| {
            let t = tile_xy(q[0], q[1], z);
            [t[0] * 256.0 - 0.5, t[1] * 256.0 - 0.5]
        })
        .collect();
    let minx = px.iter().map(|p| p[0]).fold(f64::MAX, f64::min).floor() as i64;
    let miny = px.iter().map(|p| p[1]).fold(f64::MAX, f64::min).floor() as i64;
    let maxx = px.iter().map(|p| p[0]).fold(f64::MIN, f64::max).floor() as i64 + 1;
    let maxy = px.iter().map(|p| p[1]).fold(f64::MIN, f64::max).floor() as i64 + 1;
    let (tx0, ty0, tx1, ty1) = (minx.div_euclid(256), miny.div_euclid(256), maxx.div_euclid(256), maxy.div_euclid(256));
    let (nx, ny) = ((tx1 - tx0 + 1) as usize, (ty1 - ty0 + 1) as usize);
    if nx * ny > 400 {
        return Err("Vùng lấy cao độ quá lớn (hơn 400 ô ảnh) — chọn phạm vi nhỏ hơn.".into());
    }
    let w = nx * 256;
    let mut mosaic = vec![0.0f64; w * ny * 256];
    let mut k = 0;
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            k += 1;
            say(&format!("Đang tải cao độ địa hình {k}/{}…", nx * ny));
            let t = terrain_tile(z, tx as u32, ty as u32)?;
            let (ox, oy) = (((tx - tx0) * 256) as usize, ((ty - ty0) * 256) as usize);
            for r in 0..256 {
                mosaic[(oy + r) * w + ox..(oy + r) * w + ox + 256].copy_from_slice(&t[r * 256..r * 256 + 256]);
            }
        }
    }
    let h = ny * 256;
    Ok(px
        .iter()
        .map(|p| {
            let gx = p[0] - (tx0 * 256) as f64;
            let gy = p[1] - (ty0 * 256) as f64;
            let ix = (gx.floor().max(0.0) as usize).min(w - 2);
            let iy = (gy.floor().max(0.0) as usize).min(h - 2);
            let (fx, fy) = ((gx - ix as f64).clamp(0.0, 1.0), (gy - iy as f64).clamp(0.0, 1.0));
            let at = |x: usize, y: usize| mosaic[y * w + x];
            let top = at(ix, iy) * (1.0 - fx) + at(ix + 1, iy) * fx;
            let bot = at(ix, iy + 1) * (1.0 - fx) + at(ix + 1, iy + 1) * fx;
            top * (1.0 - fy) + bot * fy
        })
        .collect())
}

// ── Geometry helpers ────────────────────────────────────────────────────────

fn ls(pts: &[Xy]) -> LineString<f64> {
    LineString::new(pts.iter().map(|p| Coord { x: p[0], y: p[1] }).collect())
}

fn pts_of(l: &LineString<f64>) -> Vec<Xy> {
    l.0.iter().map(|c| [c.x, c.y]).collect()
}

fn dist(a: Xy, b: Xy) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

pub fn length(p: &[Xy]) -> f64 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// The point `s` along a polyline.
pub fn point_at(p: &[Xy], s: f64) -> Xy {
    let mut left = s.max(0.0);
    for w in p.windows(2) {
        let d = dist(w[0], w[1]);
        if left <= d && d > 0.0 {
            let t = left / d;
            return [w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])];
        }
        left -= d;
    }
    *p.last().unwrap_or(&[0.0, 0.0])
}

fn seg_dist(p: Xy, a: Xy, b: Xy) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    dist(p, [a[0] + t * dx, a[1] + t * dy])
}

/// Douglas–Peucker.
pub fn simplify(p: &[Xy], eps: f64) -> Vec<Xy> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let (mut k, mut best) = (0, 0.0);
    for (i, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
        let d = seg_dist(*q, p[0], p[p.len() - 1]);
        if d > best {
            best = d;
            k = i;
        }
    }
    if best <= eps {
        return vec![p[0], p[p.len() - 1]];
    }
    let mut a = simplify(&p[..=k], eps);
    a.pop();
    a.extend(simplify(&p[k..], eps));
    a
}

/// Join polylines sharing end points into longer ones.
pub fn merge_lines(lines: Vec<Vec<Xy>>, tol: f64) -> Vec<Vec<Xy>> {
    let mut pool: Vec<Vec<Xy>> = lines.into_iter().filter(|l| l.len() >= 2).collect();
    let mut out = Vec::new();
    let key = |p: Xy| ((p[0] / tol).round() as i64, (p[1] / tol).round() as i64);
    while let Some(mut cur) = pool.pop() {
        loop {
            let (s, e) = (key(cur[0]), key(*cur.last().unwrap()));
            if s == e && cur.len() > 2 {
                break;
            }
            let found = pool.iter().position(|l| {
                let (a, b) = (key(l[0]), key(*l.last().unwrap()));
                a == e || b == e || a == s || b == s
            });
            let Some(k) = found else { break };
            let mut l = pool.swap_remove(k);
            let (a, b) = (key(l[0]), key(*l.last().unwrap()));
            if a == e {
                cur.extend(l.drain(1..));
            } else if b == e {
                l.reverse();
                cur.extend(l.drain(1..));
            } else if b == s {
                l.extend(cur.drain(1..));
                cur = l;
            } else {
                l.reverse();
                l.extend(cur.drain(1..));
                cur = l;
            }
        }
        out.push(cur);
    }
    out
}

fn clip_lines(boundary: &MultiPolygon<f64>, lines: &[Vec<Xy>]) -> Vec<Vec<Xy>> {
    let mls = MultiLineString::new(lines.iter().filter(|l| l.len() >= 2).map(|l| ls(l)).collect());
    boundary.clip(&mls, false).0.iter().map(pts_of).filter(|p| p.len() >= 2).collect()
}

fn readable(deg: f64) -> f64 {
    let mut d = (deg + 180.0).rem_euclid(360.0) - 180.0;
    if d > 90.0 {
        d -= 180.0;
    } else if d <= -90.0 {
        d += 180.0;
    }
    d
}

fn polys(m: &MultiPolygon<f64>) -> impl Iterator<Item = &Polygon<f64>> {
    m.0.iter()
}

fn ring_pts(r: &LineString<f64>) -> Vec<Xy> {
    let mut p = pts_of(r);
    if p.len() > 1 && p.first() == p.last() {
        p.pop();
    }
    p
}

/// Runs of vertices lying on a circle become one arc (a bulge); points in
/// line go. CadMini's `fit_bulges`.
pub fn fit_bulges(coords: &[Xy], closed: bool, tol: f64, max_r: f64) -> Vec<[f64; 3]> {
    let mut p: Vec<Xy> = coords.to_vec();
    if closed && p.len() > 1 && dist(p[0], p[p.len() - 1]) < 1e-9 {
        p.pop();
    }
    let n = p.len();
    if n < 3 {
        return p.iter().map(|q| [q[0], q[1], 0.0]).collect();
    }
    if closed {
        // Start on the longest (straight) edge so no arc is cut in two.
        let k = (0..n).max_by(|&a, &b| dist(p[a], p[(a + 1) % n]).total_cmp(&dist(p[b], p[(b + 1) % n]))).unwrap_or(0);
        p.rotate_left(k);
    }
    let mut seq = p.clone();
    if closed {
        seq.push(p[0]);
    }
    let m = seq.len();
    let circle = |a: Xy, b: Xy, c: Xy| -> Option<(f64, f64, f64)> {
        let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
        if d.abs() < 1e-12 {
            return None;
        }
        let (a2, b2, c2) = (a[0] * a[0] + a[1] * a[1], b[0] * b[0] + b[1] * b[1], c[0] * c[0] + c[1] * c[1]);
        let ux = (a2 * (b[1] - c[1]) + b2 * (c[1] - a[1]) + c2 * (a[1] - b[1])) / d;
        let uy = (a2 * (c[0] - b[0]) + b2 * (a[0] - c[0]) + c2 * (b[0] - a[0])) / d;
        Some((ux, uy, dist([ux, uy], a)))
    };
    let turn = |a: Xy, b: Xy, c: Xy| (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
    let mut out: Vec<[f64; 3]> = Vec::new();
    let mut i = 0;
    while i + 1 < m {
        let mut best = None;
        let mut j = i + 3;
        while j < m {
            let Some((ux, uy, r)) = circle(seq[i], seq[(i + j) / 2], seq[j]) else { break };
            if r > max_r {
                break;
            }
            let pts = &seq[i..=j];
            if pts.iter().any(|q| (dist([ux, uy], *q) - r).abs() > tol) {
                break;
            }
            let sg: Vec<f64> = (0..pts.len() - 2).map(|k| turn(pts[k], pts[k + 1], pts[k + 2])).collect();
            if !(sg.iter().all(|s| *s > 0.0) || sg.iter().all(|s| *s < 0.0)) {
                break;
            }
            if pts.windows(2).any(|w| dist(w[0], w[1]) > r) {
                break;
            }
            best = Some((j, ux, uy, sg[0] > 0.0));
            j += 1;
        }
        if let Some((j, ux, uy, ccw)) = best {
            let a0 = (seq[i][1] - uy).atan2(seq[i][0] - ux);
            let a1 = (seq[j][1] - uy).atan2(seq[j][0] - ux);
            let tau = std::f64::consts::TAU;
            let sweep = if ccw { (a1 - a0).rem_euclid(tau) } else { -(a0 - a1).rem_euclid(tau) };
            out.push([seq[i][0], seq[i][1], (sweep / 4.0).tan()]);
            i = j;
        } else {
            out.push([seq[i][0], seq[i][1], 0.0]);
            i += 1;
        }
    }
    if !closed {
        out.push([seq[m - 1][0], seq[m - 1][1], 0.0]);
    }
    // Drop vertices in line between two straight segments.
    let k = out.len();
    let mut res: Vec<[f64; 3]> = Vec::with_capacity(k);
    for idx in 0..k {
        let v = out[idx];
        if let Some(prev) = res.last().copied() {
            let inner = closed || idx + 1 < k;
            if v[2] == 0.0 && prev[2] == 0.0 && inner {
                let nx = out[(idx + 1) % k];
                let span = dist([prev[0], prev[1]], [nx[0], nx[1]]).max(1.0);
                if turn([prev[0], prev[1]], [v[0], v[1]], [nx[0], nx[1]]).abs() < 1e-6 * span * span {
                    continue;
                }
            }
        }
        res.push(v);
    }
    res
}

fn pline(items: &mut Vec<Item>, layer: &'static str, pts: &[Xy], closed: bool, arcs: bool) {
    let mut p = pts.to_vec();
    if closed && p.len() > 1 && p.first() == p.last() {
        p.pop();
    }
    if p.len() < 2 {
        return;
    }
    let v = if arcs { fit_bulges(&p, closed, 0.02, 2000.0) } else { p.iter().map(|q| [q[0], q[1], 0.0]).collect() };
    if v.len() >= 2 {
        items.push(Item::Pline { layer, pts: v, closed });
    }
}

fn poly_items(items: &mut Vec<Item>, layer: &'static str, m: &MultiPolygon<f64>, hatch: Option<i16>) {
    for p in polys(m) {
        if p.unsigned_area() < 1.0 {
            continue;
        }
        let rings: Vec<Vec<Xy>> = std::iter::once(p.exterior()).chain(p.interiors()).map(ring_pts).collect();
        if let Some(color) = hatch {
            items.push(Item::Hatch { layer, rings: rings.clone(), color });
        }
        for r in &rings {
            pline(items, layer, r, true, false);
        }
    }
}

// ── OSM ─────────────────────────────────────────────────────────────────────

/// OSM elements in local coordinates.
#[derive(Default)]
pub struct Osm {
    pub nodes: Vec<(Tags, Xy)>,
    /// Tags, points, node ids.
    pub ways: Vec<(Tags, Vec<Xy>, Vec<i64>)>,
    /// Tags, members (role, points).
    pub rels: Vec<(Tags, Vec<(String, Vec<Xy>)>)>,
}

fn tags_of(v: &serde_json::Value) -> Tags {
    v.get("tags")
        .and_then(|t| t.as_object())
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default()
}

fn geom(v: &serde_json::Value, to_xy: &dyn Fn(f64, f64) -> Xy) -> Vec<Xy> {
    v.get("geometry")
        .and_then(|g| g.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|q| Some(to_xy(q.get("lon")?.as_f64()?, q.get("lat")?.as_f64()?)))
                .collect()
        })
        .unwrap_or_default()
}

impl Osm {
    /// From Overpass JSON; `to_xy(lon, lat)` gives local coordinates.
    pub fn parse(js: &serde_json::Value, to_xy: &dyn Fn(f64, f64) -> Xy) -> Self {
        let mut o = Osm::default();
        for el in js.get("elements").and_then(|e| e.as_array()).into_iter().flatten() {
            let tags = tags_of(el);
            match el.get("type").and_then(|t| t.as_str()) {
                Some("node") => {
                    if let (Some(lon), Some(lat)) = (el.get("lon").and_then(|v| v.as_f64()), el.get("lat").and_then(|v| v.as_f64())) {
                        o.nodes.push((tags, to_xy(lon, lat)));
                    }
                }
                Some("way") => {
                    let g = geom(el, to_xy);
                    if g.len() >= 2 {
                        let ids = el
                            .get("nodes")
                            .and_then(|n| n.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
                            .unwrap_or_default();
                        o.ways.push((tags, g, ids));
                    }
                }
                Some("relation") => {
                    let mem: Vec<(String, Vec<Xy>)> = el
                        .get("members")
                        .and_then(|m| m.as_array())
                        .into_iter()
                        .flatten()
                        .filter(|m| m.get("type").and_then(|t| t.as_str()) == Some("way"))
                        .map(|m| (m.get("role").and_then(|r| r.as_str()).unwrap_or("outer").to_string(), geom(m, to_xy)))
                        .filter(|(_, g)| g.len() >= 2)
                        .collect();
                    if !mem.is_empty() {
                        o.rels.push((tags, mem));
                    }
                }
                _ => {}
            }
        }
        o
    }

    /// Closed ways and multipolygon relations whose tags `hit`.
    pub fn areas(&self, hit: &dyn Fn(&Tags) -> bool) -> Vec<(Tags, MultiPolygon<f64>)> {
        let mut out = Vec::new();
        for (tags, xy, _) in &self.ways {
            if hit(tags) && xy.len() >= 4 && dist(xy[0], xy[xy.len() - 1]) < 1e-6 {
                let p = Polygon::new(ls(xy), Vec::new());
                // Self-touching rings come out valid through the union.
                out.push((tags.clone(), MultiPolygon::new(vec![p.clone()]).union(&MultiPolygon::new(vec![]))));
            }
        }
        for (tags, mem) in &self.rels {
            if !hit(tags) {
                continue;
            }
            let rings = |inner: bool| -> Vec<Polygon<f64>> {
                let lines: Vec<Vec<Xy>> =
                    mem.iter().filter(|(r, _)| (r == "inner") == inner).map(|(_, g)| g.clone()).collect();
                merge_lines(lines, 1e-4)
                    .into_iter()
                    .filter(|l| l.len() >= 4 && dist(l[0], l[l.len() - 1]) < 1e-3)
                    .map(|l| Polygon::new(ls(&l), Vec::new()))
                    .collect()
            };
            let outer = rings(false);
            if outer.is_empty() {
                continue;
            }
            let mut g = geo::unary_union(outer.iter());
            let inner = rings(true);
            if !inner.is_empty() {
                g = g.difference(&geo::unary_union(inner.iter()));
            }
            out.push((tags.clone(), g));
        }
        out
    }
}

// ── Layers ──────────────────────────────────────────────────────────────────

fn width_of(tags: &Tags, default: f64) -> f64 {
    let w: f64 = tags
        .get("width")
        .map(|s| s.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    if (1.0..=80.0).contains(&w) { w } else { default }
}

/// Names along their lines, mid-way; long ones every ~400 m.
fn names(items: &mut Vec<Item>, lines: Vec<(Vec<Xy>, f64, &'static str, String)>, boundary: &MultiPolygon<f64>) {
    let mut by: HashMap<String, (Vec<Vec<Xy>>, f64, &'static str)> = HashMap::new();
    for (l, h, layer, name) in lines {
        if name.is_empty() {
            continue;
        }
        let e = by.entry(name).or_insert((Vec::new(), h, layer));
        e.0.push(l);
        if h > e.1 {
            e.1 = h;
            e.2 = layer;
        }
    }
    let mut keys: Vec<&String> = by.keys().collect();
    keys.sort();
    for name in keys {
        let (lines, h, layer) = &by[name];
        let tl = name.chars().count().max(1) as f64 * h * 0.62;
        for line in merge_lines(clip_lines(boundary, lines), 1e-3) {
            let l = length(&line);
            if l < tl * 1.15 {
                continue;
            }
            let spots: Vec<f64> = if l < 600.0 {
                vec![l / 2.0]
            } else {
                let mut v = Vec::new();
                let mut s = 200.0;
                while s < l - 100.0 {
                    v.push(s);
                    s += 400.0;
                }
                v
            };
            for s in spots {
                let a = point_at(&line, s - tl / 2.0);
                let b = point_at(&line, (s + tl / 2.0).min(l));
                let m = point_at(&line, s);
                let ang = readable((b[1] - a[1]).atan2(b[0] - a[0]).to_degrees());
                items.push(Item::Text { layer: *layer, at: m, s: name.clone(), h: *h, rot_deg: ang, left: false });
            }
        }
    }
}

/// Centre lines, kerbs rounded at junctions, names. Returns the road count.
pub fn roads(osm: &Osm, boundary: &MultiPolygon<f64>, opts: &Options, items: &mut Vec<Item>) -> usize {
    let work = boundary.buffer(60.0);
    let wbox = bbox_of(&work);
    struct Seg<'a> {
        pts: &'a [Xy],
        spec: (&'static str, f64, f64, f64),
        w: f64,
        tags: &'a Tags,
        ids: &'a [i64],
    }
    let mut segs: Vec<Seg> = Vec::new();
    for (tags, xy, ids) in &osm.ways {
        let hw = tags.get("highway").map(|s| s.replace("_link", "")).unwrap_or_default();
        let Some(spec) = road_spec(&hw) else { continue };
        if tags.get("area").map(String::as_str) == Some("yes") || (spec.0 == "loi_di" && !opts.paths) {
            continue;
        }
        // A long road with few vertices can cross the boundary between two of them.
        let mut lb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in xy {
            lb = [lb[0].min(p[0]), lb[1].min(p[1]), lb[2].max(p[0]), lb[3].max(p[1])];
        }
        if !boxes_meet(lb, wbox) {
            continue;
        }
        segs.push(Seg { pts: xy, spec, w: width_of(tags, spec.1), tags, ids });
    }
    if segs.is_empty() {
        return 0;
    }
    if opts.roads {
        for part in clip_lines(boundary, &segs.iter().map(|s| s.pts.to_vec()).collect::<Vec<_>>()) {
            if length(&part) > 0.5 {
                pline(items, L_TIM, &part, false, false);
            }
        }
        let bufs: Vec<MultiPolygon<f64>> = segs.iter().map(|s| ls(s.pts).buffer(s.w / 2.0)).collect();
        let mut area = geo::unary_union(bufs.iter());
        // Junctions: OSM nodes shared by two roads or more (or used twice).
        let mut used: HashMap<i64, (Xy, BTreeSet<usize>, usize)> = HashMap::new();
        for (k, s) in segs.iter().enumerate() {
            if s.ids.len() != s.pts.len() {
                continue;
            }
            for (id, p) in s.ids.iter().zip(s.pts) {
                let e = used.entry(*id).or_insert((*p, BTreeSet::new(), 0));
                e.1.insert(k);
                e.2 += 1;
            }
        }
        let mut fills: Vec<MultiPolygon<f64>> = Vec::new();
        for (c, ks, cnt) in used.values() {
            if ks.len() < 2 && *cnt < 3 {
                continue;
            }
            let r = ks.iter().map(|k| segs[*k].spec.2).fold(f64::MAX, f64::min);
            let wmax = ks.iter().map(|k| segs[*k].w).fold(0.0, f64::max);
            let p = Point::new(c[0], c[1]);
            if !p.buffer(wmax + 3.0 * r).intersection(boundary).0.iter().any(|q| q.unsigned_area() > 0.0) {
                continue;
            }
            let big = p.buffer(wmax / 2.0 + 3.0 * r);
            let bb = bbox_of(&big);
            let near: Vec<&MultiPolygon<f64>> =
                bufs.iter().filter(|b| boxes_meet(bbox_of(b), bb)).collect();
            if near.is_empty() {
                continue;
            }
            let local = geo::unary_union(near.into_iter()).intersection(&big);
            let closed = local.buffer(r).buffer(-r);
            let f = closed.difference(&local).intersection(&p.buffer(wmax / 2.0 + 2.0 * r));
            if f.unsigned_area() > 0.01 {
                fills.push(f);
            }
        }
        if !fills.is_empty() {
            fills.push(area);
            area = geo::unary_union(fills.iter());
        }
        let blocks = boundary.difference(&area);
        let edge: Vec<Vec<Xy>> = polys(boundary)
            .flat_map(|p| std::iter::once(p.exterior()).chain(p.interiors()).map(ring_pts))
            .collect();
        let on_edge = |q: Xy| {
            edge.iter().any(|r| (0..r.len()).any(|k| seg_dist(q, r[k], r[(k + 1) % r.len()]) < 0.02))
        };
        for blk in polys(&blocks) {
            if blk.unsigned_area() < 2.0 {
                continue;
            }
            for ring in std::iter::once(blk.exterior()).chain(blk.interiors()) {
                let pts = ring_pts(ring);
                let n = pts.len();
                // A segment lies along the boundary when its middle does.
                let off: Vec<bool> = (0..n)
                    .map(|k| {
                        let (a, b) = (pts[k], pts[(k + 1) % n]);
                        !on_edge([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])
                    })
                    .collect();
                if off.iter().all(|f| *f) {
                    // A block closed by roads: one closed polyline.
                    pline(items, L_MEP, &pts, true, true);
                    continue;
                }
                // The kerb: each run of segments off the boundary.
                let Some(start) = (0..n).find(|&k| off[k] && !off[(k + n - 1) % n]) else { continue };
                let mut run: Vec<Xy> = Vec::new();
                for step in 0..n {
                    let k = (start + step) % n;
                    if off[k] {
                        if run.is_empty() {
                            run.push(pts[k]);
                        }
                        run.push(pts[(k + 1) % n]);
                    } else if !run.is_empty() {
                        if length(&run) > 0.3 {
                            pline(items, L_MEP, &run, false, true);
                        }
                        run.clear();
                    }
                }
                if length(&run) > 0.3 {
                    pline(items, L_MEP, &run, false, true);
                }
            }
        }
    }
    if opts.road_names {
        let lines = segs
            .iter()
            .map(|s| {
                let layer = if matches!(s.spec.0, "ngo" | "loi_di") { L_TEN_NGO } else { L_TEN };
                (s.pts.to_vec(), s.spec.3, layer, s.tags.get("name").cloned().unwrap_or_default())
            })
            .collect();
        names(items, lines, boundary);
    }
    segs.len()
}

fn bbox_of(m: &MultiPolygon<f64>) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in polys(m) {
        for c in &p.exterior().0 {
            b = [b[0].min(c.x), b[1].min(c.y), b[2].max(c.x), b[3].max(c.y)];
        }
    }
    b
}

fn boxes_meet(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

fn place_height(kind: &str) -> Option<f64> {
    Some(match kind {
        "city" => 10.0,
        "town" => 8.0,
        "suburb" => 6.0,
        "village" => 5.0,
        "quarter" => 4.5,
        "hamlet" | "neighbourhood" => 4.0,
        "isolated_dwelling" | "locality" => 3.0,
        _ => return None,
    })
}

/// Places and points of interest. Returns how many.
pub fn places(osm: &Osm, boundary: &MultiPolygon<f64>, groups: &BTreeSet<String>, items: &mut Vec<Item>) -> usize {
    let group = |t: &Tags| -> Option<&'static str> {
        if t.get("place").is_some_and(|p| place_height(p).is_some()) {
            return Some("dia_danh");
        }
        let am = t.get("amenity").map(String::as_str).unwrap_or("");
        if matches!(am, "school" | "kindergarten" | "college" | "university") {
            return Some("truong_hoc");
        }
        if matches!(am, "restaurant" | "cafe" | "fast_food" | "food_court" | "bar" | "pub" | "biergarten") {
            return Some("quan_an");
        }
        if t.get("tourism").is_some_and(|v| matches!(v.as_str(), "hotel" | "motel" | "guest_house" | "hostel" | "apartment" | "chalet")) {
            return Some("nha_nghi");
        }
        if t.contains_key("name") && ["amenity", "tourism", "shop", "historic", "office"].iter().any(|k| t.contains_key(*k)) {
            return Some("khac");
        }
        None
    };
    let mut pts: Vec<(&Tags, Xy)> = osm.nodes.iter().map(|(t, p)| (t, *p)).collect();
    for (t, xy, _) in &osm.ways {
        if xy.len() >= 3 && (t.contains_key("amenity") || t.contains_key("tourism")) {
            if let Some(c) = Polygon::new(ls(xy), Vec::new()).interior_point() {
                pts.push((t, [c.x(), c.y()]));
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut n = 0;
    for (t, [x, y]) in pts {
        let Some(g) = group(t) else { continue };
        if !groups.contains(g) || !boundary.contains(&Point::new(x, y)) {
            continue;
        }
        let Some(&(_, layer, default, _, _)) = POI.iter().find(|p| p.0 == g) else { continue };
        let name = t.get("name").cloned().unwrap_or_else(|| default.to_string());
        if name.is_empty() || !seen.insert((name.clone(), x.round() as i64, y.round() as i64)) {
            continue;
        }
        if g == "dia_danh" {
            let h = t.get("place").and_then(|p| place_height(p)).unwrap_or(4.0);
            items.push(Item::Text { layer, at: [x, y], s: name, h, rot_deg: 0.0, left: false });
        } else {
            items.push(Item::Circle { layer, c: [x, y], r: 1.2 });
            items.push(Item::Text { layer, at: [x + 2.0, y], s: name, h: 2.0, rot_deg: 0.0, left: true });
        }
        n += 1;
    }
    n
}

fn tag_in(t: &Tags, key: &str, values: &[&str]) -> bool {
    t.get(key).is_some_and(|v| values.contains(&v.as_str()))
}

/// Water, rivers, greenery, farmland, buildings, railways, power lines.
/// Fills go first (under everything). Returns counts by layer.
pub fn areas(osm: &Osm, boundary: &MultiPolygon<f64>, opts: &Options, items: &mut Vec<Item>) -> Vec<(&'static str, usize)> {
    let mut cnt: HashMap<&'static str, usize> = HashMap::new();
    let (mut fills, mut lines) = (Vec::new(), Vec::new());
    let mut put = |layer: &'static str, g: &MultiPolygon<f64>, hatch: Option<i16>, fills: &mut Vec<Item>, lines: &mut Vec<Item>| {
        let g = g.intersection(boundary);
        if g.unsigned_area() < 1.0 {
            return;
        }
        *cnt.entry(layer).or_default() += 1;
        if hatch.is_some() {
            poly_items(fills, layer, &g, hatch);
        } else {
            poly_items(lines, layer, &g, None);
        }
    };
    if opts.farm {
        for (_, g) in osm.areas(&|t| tag_in(t, "landuse", &["farmland", "farmyard", "paddy", "plant_nursery"])) {
            put(L_NN, &g, None, &mut fills, &mut lines);
        }
    }
    if opts.green {
        let green = |t: &Tags| {
            tag_in(t, "natural", &["wood", "scrub", "grassland", "wetland"])
                || tag_in(t, "landuse", &["forest", "grass", "meadow", "orchard", "cemetery", "village_green", "recreation_ground"])
                || tag_in(t, "leisure", &["park", "garden", "pitch", "stadium"])
        };
        for (_, g) in osm.areas(&green) {
            put(L_CX, &g, Some(82), &mut fills, &mut lines);
        }
    }
    let mut extra: Vec<(&'static str, usize)> = Vec::new();
    if opts.water {
        let water = |t: &Tags| {
            tag_in(t, "natural", &["water"])
                || tag_in(t, "waterway", &["riverbank"])
                || tag_in(t, "landuse", &["reservoir", "basin", "aquaculture"])
        };
        for (_, g) in osm.areas(&water) {
            put(L_NUOC, &g, Some(151), &mut fills, &mut lines);
        }
        let mut rivers = Vec::new();
        let mut n = 0;
        for (t, xy, _) in &osm.ways {
            let ww = t.get("waterway").map(String::as_str).unwrap_or("");
            if matches!(ww, "river" | "stream" | "canal" | "ditch" | "drain" | "brook") {
                for part in clip_lines(boundary, std::slice::from_ref(xy)) {
                    pline(&mut lines, L_SONG, &part, false, false);
                    n += 1;
                }
                let h = if matches!(ww, "river" | "canal") { 4.0 } else { 2.5 };
                rivers.push((xy.clone(), h, L_TEN_SONG, t.get("name").cloned().unwrap_or_default()));
            }
        }
        if n > 0 {
            extra.push((L_SONG, n));
        }
        names(&mut lines, rivers, boundary);
    }
    if opts.rail_power {
        let (mut ns, mut nd) = (0, 0);
        for (t, xy, _) in &osm.ways {
            let layer = if t.contains_key("railway") {
                L_SAT
            } else if t.contains_key("power") {
                L_DIEN
            } else {
                continue;
            };
            for part in clip_lines(boundary, std::slice::from_ref(xy)) {
                pline(&mut lines, layer, &part, false, false);
                if layer == L_SAT { ns += 1 } else { nd += 1 }
            }
        }
        extra.extend([(L_SAT, ns), (L_DIEN, nd)].into_iter().filter(|(_, n)| *n > 0));
    }
    if opts.buildings {
        for (_, g) in osm.areas(&|t| t.contains_key("building")) {
            put(L_NHA, &g, None, &mut fills, &mut lines);
        }
    }
    let mut out: Vec<(&'static str, usize)> = cnt.into_iter().chain(extra).collect();
    out.sort();
    fills.extend(std::mem::take(items));
    fills.extend(lines);
    *items = fills;
    out
}

/// A good contour interval: about 10–25 lines over the whole relief.
pub fn nice_interval(zmin: f64, zmax: f64) -> f64 {
    let rng = (zmax - zmin).max(1.0);
    [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0].into_iter().find(|s| rng / s <= 25.0).unwrap_or(200.0)
}

/// Contour lines of a regular grid (marching squares), per level, joined.
pub fn contours(xs: &[f64], ys: &[f64], z: &[Vec<f64>], level: f64) -> Vec<Vec<Xy>> {
    let mut segs: Vec<Vec<Xy>> = Vec::new();
    let (nx, ny) = (xs.len(), ys.len());
    let v = |i: usize, j: usize| {
        let d = z[i][j] - level;
        if d == 0.0 { 1e-9 } else { d }
    };
    for i in 0..ny.saturating_sub(1) {
        for j in 0..nx.saturating_sub(1) {
            let (z00, z10, z01, z11) = (v(i, j), v(i, j + 1), v(i + 1, j), v(i + 1, j + 1));
            let (x0, x1, y0, y1) = (xs[j], xs[j + 1], ys[i], ys[i + 1]);
            let cut = |za: f64, zb: f64, a: Xy, b: Xy| -> Option<Xy> {
                (za * zb < 0.0).then(|| {
                    let t = za / (za - zb);
                    [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
                })
            };
            // Bottom, right, top, left.
            let e = [
                cut(z00, z10, [x0, y0], [x1, y0]),
                cut(z10, z11, [x1, y0], [x1, y1]),
                cut(z01, z11, [x0, y1], [x1, y1]),
                cut(z00, z01, [x0, y0], [x0, y1]),
            ];
            let hits: Vec<Xy> = e.iter().flatten().copied().collect();
            match hits.len() {
                2 => segs.push(hits),
                4 => {
                    // A saddle: join by the cell's centre value.
                    let centre = (z00 + z10 + z01 + z11) / 4.0;
                    let p = [e[0].unwrap(), e[1].unwrap(), e[2].unwrap(), e[3].unwrap()];
                    if (centre > 0.0) == (z00 > 0.0) {
                        segs.push(vec![p[0], p[1]]);
                        segs.push(vec![p[2], p[3]]);
                    } else {
                        segs.push(vec![p[0], p[3]]);
                        segs.push(vec![p[1], p[2]]);
                    }
                }
                _ => {}
            }
        }
    }
    let tol = (xs.get(1).copied().unwrap_or(1.0) - xs[0]).abs() * 1e-6;
    merge_lines(segs, tol.max(1e-9))
}

/// Contours and spot heights inside the boundary. `heights(points)` gives
/// the terrain at local points. Returns (z min, z max, interval).
pub fn elevation(
    boundary: &MultiPolygon<f64>,
    heights: &mut dyn FnMut(&[Xy]) -> Result<Vec<f64>, String>,
    opts: &Options,
    items: &mut Vec<Item>,
) -> Result<(f64, f64, f64), String> {
    let [x0, y0, x1, y1] = bbox_of(boundary);
    let side = boundary.unsigned_area().max(1.0).sqrt();
    // About 200 cells a side.
    let g = (side / 200.0).round().max(5.0);
    let axis = |a: f64, b: f64| {
        let mut v = Vec::new();
        let mut t = a - g;
        while t < b + 2.0 * g {
            v.push(t);
            t += g;
        }
        v
    };
    let (xs, ys) = (axis(x0, x1), axis(y0, y1));
    let pts: Vec<Xy> = ys.iter().flat_map(|y| xs.iter().map(move |x| [*x, *y])).collect();
    let flat = heights(&pts)?;
    let (nx, ny) = (xs.len(), ys.len());
    let raw = |i: usize, j: usize| flat[i * nx + j];
    // A light 3×3 smoothing: no saw teeth from the source pixels.
    let z: Vec<Vec<f64>> = (0..ny)
        .map(|i| {
            (0..nx)
                .map(|j| {
                    let mut s = 0.0;
                    for di in -1i64..=1 {
                        for dj in -1i64..=1 {
                            let ii = (i as i64 + di).clamp(0, ny as i64 - 1) as usize;
                            let jj = (j as i64 + dj).clamp(0, nx as i64 - 1) as usize;
                            s += raw(ii, jj);
                        }
                    }
                    s / 9.0
                })
                .collect()
        })
        .collect();
    let mut inside = Vec::new();
    for i in (0..ny).step_by(3) {
        for j in (0..nx).step_by(3) {
            if boundary.contains(&Point::new(xs[j], ys[i])) {
                inside.push(z[i][j]);
            }
        }
    }
    if inside.is_empty() {
        inside = z.iter().flatten().copied().collect();
    }
    let zmin = inside.iter().copied().fold(f64::MAX, f64::min);
    let zmax = inside.iter().copied().fold(f64::MIN, f64::max);
    let step = if opts.contour_step > 0.0 { opts.contour_step } else { nice_interval(zmin, zmax) };
    let mut lv = (zmin / step).ceil() * step;
    while lv <= zmax + 1e-9 {
        let major = ((lv / (step * 5.0)) - (lv / (step * 5.0)).round()).abs() < 1e-6;
        let layer = if major { L_DM_CAI } else { L_DM };
        let lines: Vec<Vec<Xy>> = contours(&xs, &ys, &z, lv).into_iter().map(|l| simplify(&l, g * 0.1)).collect();
        for part in clip_lines(boundary, &lines) {
            let l = length(&part);
            if l < g * 1.5 {
                continue;
            }
            pline(items, layer, &part, false, false);
            if major && l > 80.0 {
                let m = point_at(&part, l / 2.0);
                let a = point_at(&part, l / 2.0 - 3.0);
                let b = point_at(&part, l / 2.0 + 3.0);
                let ang = readable((b[1] - a[1]).atan2(b[0] - a[0]).to_degrees());
                items.push(Item::Text { layer, at: m, s: format!("{}", (lv * 100.0).round() / 100.0), h: 2.0, rot_deg: ang, left: false });
            }
        }
        lv += step;
    }
    let sp = if opts.spot_spacing > 0.0 { opts.spot_spacing } else { (side / 15.0 / 10.0).round().max(2.0) * 10.0 };
    let mut x = x0 + sp / 2.0;
    while x < x1 {
        let mut y = y0 + sp / 2.0;
        while y < y1 {
            if boundary.contains(&Point::new(x, y)) {
                let i = (((y - ys[0]) / g).round() as usize).min(ny - 1);
                let j = (((x - xs[0]) / g).round() as usize).min(nx - 1);
                items.push(Item::Circle { layer: L_CAODO, c: [x, y], r: 0.4 });
                items.push(Item::Text { layer: L_CAODO, at: [x + 0.8, y], s: format!("{:.1}", z[i][j]), h: 1.5, rot_deg: 0.0, left: true });
            }
            y += sp;
        }
        x += sp;
    }
    Ok((zmin, zmax, step))
}

/// The drawing from the OSM answer: areas, roads, places. Returns notes.
pub fn build(osm: &Osm, boundary: &MultiPolygon<f64>, opts: &Options, items: &mut Vec<Item>) -> Vec<String> {
    let mut notes = Vec::new();
    let cnt = areas(osm, boundary, opts, items);
    if opts.roads || opts.road_names {
        notes.push(format!("{} đoạn đường OSM", roads(osm, boundary, opts, items)));
    }
    if !opts.places.is_empty() {
        notes.push(format!("{} địa danh / điểm", places(osm, boundary, &opts.places, items)));
    }
    for (layer, n) in cnt {
        notes.push(format!("{layer}: {n}"));
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> MultiPolygon<f64> {
        MultiPolygon::new(vec![Polygon::new(ls(&[[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]), vec![])])
    }

    fn way(id: i64, tags: &[(&str, &str)], pts: &[(i64, Xy)]) -> serde_json::Value {
        // Local coordinates pass as lon/lat with an identity to_xy.
        serde_json::json!({
            "type": "way", "id": id,
            "tags": tags.iter().map(|(k, v)| (k.to_string(), serde_json::Value::from(*v))).collect::<serde_json::Map<_, _>>(),
            "nodes": pts.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            "geometry": pts.iter().map(|(_, p)| serde_json::json!({"lon": p[0], "lat": p[1]})).collect::<Vec<_>>(),
        })
    }

    fn sample() -> Osm {
        let js = serde_json::json!({"elements": [
            way(1, &[("highway", "primary"), ("name", "Đường Trần Phú")], &[(10, [-200.0, 0.0]), (11, [0.0, 0.0]), (12, [200.0, 0.0])]),
            way(2, &[("highway", "residential"), ("name", "Ngõ 5")], &[(11, [0.0, 0.0]), (13, [0.0, 200.0])]),
            way(3, &[("building", "yes")], &[(20, [20.0, 20.0]), (21, [40.0, 20.0]), (22, [40.0, 35.0]), (23, [20.0, 35.0]), (20, [20.0, 20.0])]),
            way(4, &[("natural", "water")], &[(30, [-80.0, 20.0]), (31, [-40.0, 20.0]), (32, [-40.0, 60.0]), (33, [-80.0, 60.0]), (30, [-80.0, 20.0])]),
            way(5, &[("waterway", "stream"), ("name", "Suối Cạn")], &[(40, [-100.0, -80.0]), (41, [100.0, -60.0])]),
            {"type": "node", "id": 50, "lon": 60.0, "lat": 60.0, "tags": {"place": "hamlet", "name": "Thôn 3"}},
            {"type": "node", "id": 51, "lon": -60.0, "lat": -30.0, "tags": {"amenity": "school", "name": "Trường TH Kỳ Anh"}},
            {"type": "node", "id": 52, "lon": 500.0, "lat": 500.0, "tags": {"place": "village", "name": "Xa"}},
        ]});
        Osm::parse(&js, &|lon, lat| [lon, lat])
    }

    #[test]
    fn the_query_asks_for_what_is_ticked() {
        let q = overpass_query([105.0, 18.0, 105.1, 18.1], &Options::default().osm_parts());
        assert!(q.starts_with("[out:json]") && q.ends_with("out body geom;"));
        assert!(q.contains(r#"way["highway"](18.0000000,105.0000000,18.1000000,105.1000000);"#));
        assert!(q.contains(r#"way["building"]"#) && !q.contains("railway"));
        assert!(!overpass_query([0.0; 4], &Options::roads_only().osm_parts()).contains("building"));
    }

    #[test]
    fn a_small_map_is_drawn() {
        let osm = sample();
        let boundary = square(-150.0, -100.0, 150.0, 150.0);
        let mut items = Vec::new();
        let notes = build(&osm, &boundary, &Options::default(), &mut items);
        let on = |l: &str| items.iter().filter(|i| i.layer() == l).count();
        assert!(on(L_TIM) >= 2, "{notes:?}");
        // Kerbs: the blocks between the roads, rounded (some bulge) at the junction.
        assert!(on(L_MEP) >= 2);
        let rounded = items.iter().any(|i| matches!(i, Item::Pline { layer: L_MEP, pts, .. } if pts.iter().any(|p| p[2] != 0.0)));
        assert!(rounded, "no arc at the junction");
        // The kerb stays off the road: no kerb vertex within the half width (7.5 m) of the main road.
        for i in &items {
            if let Item::Pline { layer: L_MEP, pts, .. } = i {
                for p in pts {
                    if p[0].abs() < 140.0 && p[0].abs() > 30.0 {
                        assert!(p[1].abs() > 7.4, "{p:?}");
                    }
                }
            }
        }
        let texts: Vec<&str> = items.iter().filter_map(|i| match i { Item::Text { s, .. } => Some(s.as_str()), _ => None }).collect();
        for t in ["Đường Trần Phú", "Ngõ 5", "Thôn 3", "Trường TH Kỳ Anh", "Suối Cạn"] {
            assert!(texts.contains(&t), "{t} missing: {texts:?}");
        }
        assert!(!texts.contains(&"Xa"));
        assert_eq!(on(L_NHA), 1);
        assert!(items.iter().any(|i| matches!(i, Item::Hatch { layer: L_NUOC, .. })));
        // Fills come first, under the lines.
        let first_line = items.iter().position(|i| !matches!(i, Item::Hatch { .. })).unwrap();
        assert!(items[first_line..].iter().all(|i| !matches!(i, Item::Hatch { .. })));
        assert!(on(L_SONG) >= 1);
    }

    #[test]
    fn contours_of_a_cone() {
        let boundary = square(0.0, 0.0, 1000.0, 1000.0);
        let mut items = Vec::new();
        let mut h = |p: &[Xy]| -> Result<Vec<f64>, String> {
            Ok(p.iter().map(|q| 100.0 - dist(*q, [500.0, 500.0]) / 10.0).collect())
        };
        let (zmin, zmax, step) = elevation(&boundary, &mut h, &Options::default(), &mut items).unwrap();
        assert!(zmax > 98.0 && zmin < 35.0, "{zmin} {zmax}");
        assert_eq!(step, 5.0);
        // The 80 m contour is a circle of radius ~200 m round the top.
        let ring = items.iter().find_map(|i| match i {
            Item::Pline { pts, .. } if pts.iter().all(|p| (dist([p[0], p[1]], [500.0, 500.0]) - 200.0).abs() < 8.0) => Some(pts.len()),
            _ => None,
        });
        assert!(ring.is_some_and(|n| n > 8));
        assert!(items.iter().any(|i| matches!(i, Item::Text { layer: L_DM_CAI, .. })));
        assert!(items.iter().any(|i| matches!(i, Item::Circle { layer: L_CAODO, .. })));
    }

    #[test]
    fn arcs_and_terrarium() {
        // A quarter circle of radius 10 sampled every 5°, then a straight run.
        let mut p: Vec<Xy> = (0..=18).map(|k| {
            let a = (k as f64 * 5.0).to_radians();
            [10.0 * a.cos(), 10.0 * a.sin()]
        }).collect();
        p.push([-20.0, 10.0]);
        let v = fit_bulges(&p, false, 0.02, 2000.0);
        assert_eq!(v.len(), 3, "{v:?}");
        assert!((v[0][2] - (std::f64::consts::FRAC_PI_2 / 4.0).tan()).abs() < 1e-6);
        assert_eq!(terrarium_height(128, 0, 0), 0.0);
        assert_eq!(terrarium_height(128, 100, 128), 100.5);
        assert_eq!(nice_interval(10.0, 300.0), 20.0);
        let merged = merge_lines(vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[2.0, 0.0], [1.0, 0.0]], vec![[2.0, 0.0], [3.0, 1.0]]], 1e-6);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].len(), 4);
    }
}
