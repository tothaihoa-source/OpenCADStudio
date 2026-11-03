//! TRICHLUC — the parcel extract sheet (trích lục thửa đất), as CadMini.
//!
//! From one closed polyline: an A4/A3 portrait/landscape frame, the vertex
//! numbers, the edge lengths, the VN-2000 coordinate table (X = North,
//! Y = East), the area, the date, the parcel's data and the signatures.
//!
//! This module is the pure core: geometry, the sheet layout in paper mm and
//! the drawing items in model units. The command lives in
//! `app::commands::trichluc_cmd`. Parameters are kept as JSON in the
//! polyline's XDATA (`CADMINI_TL`, first string `TL:<id>`), each sheet entity
//! carries `<id>` — the same as CadMini, so both edit each other's sheets.

use serde::{Deserialize, Serialize};

pub type Xy = [f64; 2];
/// A polyline vertex: x, y and the bulge of the segment that starts here.
pub type Vtx = [f64; 3];

pub const APPID: &str = "CADMINI_TL";
pub const PAPERS_MM: &[(&str, f64, f64)] = &[("A4", 210.0, 297.0), ("A3", 297.0, 420.0)];
pub const SCALES: &[i64] = &[100, 200, 250, 500, 1000, 2000, 2500, 5000, 10000];
pub const L_FRAME: &str = "TL_KHUNG";
pub const L_TEXT: &str = "TL_CHU";
pub const L_VERTEX: &str = "TL_SODINH";
pub const L_EDGE: &str = "TL_CANH";
pub const L_PARCEL: &str = "TL_THUA";
pub const L_TAKE: &str = "TL_THUHOI";
pub const LAYERS: &[(&str, i16)] =
    &[(L_FRAME, 7), (L_TEXT, 7), (L_VERTEX, 1), (L_EDGE, 5), (L_PARCEL, 7), (L_TAKE, 1)];
pub const LAND_USES: &[&str] = &[
    "Đất ở tại nông thôn (ONT)",
    "Đất ở tại đô thị (ODT)",
    "Đất trồng lúa (LUC)",
    "Đất trồng cây hằng năm khác (HNK)",
    "Đất trồng cây lâu năm (CLN)",
    "Đất rừng sản xuất (RSX)",
    "Đất nuôi trồng thủy sản (NTS)",
    "Đất thương mại, dịch vụ (TMD)",
    "Đất cơ sở sản xuất phi nông nghiệp (SKC)",
    "Đất xây dựng trụ sở cơ quan (TSC)",
];
/// The fields remembered from one extract to the next (CadMini's `SHARED`).
pub const SHARED: &[&str] = &[
    "he_toa_do", "don_vi", "dia_danh", "nguoi_lap", "thu_truong", "paper", "landscape", "unit",
    "placement", "neighbors", "renumber", "edge_len", "comma", "tieu_de",
];

/// The sheet's parameters, with CadMini's JSON keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub id: String,
    pub tieu_de: String,
    pub so_thua: String,
    pub to_ban_do: String,
    pub chu: String,
    pub dia_chi: String,
    pub muc_dich: String,
    pub he_toa_do: String,
    pub ghi_chu: String,
    pub don_vi: String,
    pub dia_danh: String,
    pub ngay: String,
    pub nguoi_lap: String,
    pub thu_truong: String,
    pub paper: String,
    pub landscape: bool,
    /// 0: the smallest standard scale that fits.
    pub scale: i64,
    /// "m" or "mm".
    pub unit: String,
    /// "side": a copy beside the drawing; "here": the sheet around the parcel.
    pub placement: String,
    pub neighbors: bool,
    pub renumber: bool,
    pub edge_len: bool,
    pub comma: bool,
    /// The sheet's lower-left corner once placed beside the drawing.
    pub base: Option<Xy>,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            id: String::new(),
            tieu_de: "TRÍCH LỤC THỬA ĐẤT".into(),
            so_thua: String::new(),
            to_ban_do: String::new(),
            chu: String::new(),
            dia_chi: String::new(),
            muc_dich: String::new(),
            he_toa_do: "VN-2000".into(),
            ghi_chu: String::new(),
            don_vi: String::new(),
            dia_danh: "Hà Tĩnh".into(),
            ngay: today(),
            nguoi_lap: String::new(),
            thu_truong: String::new(),
            paper: "A4".into(),
            landscape: false,
            scale: 0,
            unit: "m".into(),
            placement: "side".into(),
            neighbors: true,
            renumber: true,
            edge_len: true,
            comma: true,
            base: None,
        }
    }
}

impl Params {
    /// The value of one field as text (for prompts).
    pub fn get(&self, key: &str) -> String {
        match serde_json::to_value(self).ok().and_then(|v| v.get(key).cloned()) {
            Some(serde_json::Value::String(s)) => s,
            Some(serde_json::Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
        }
    }

    /// Set one field from text; false when the key or value is not valid.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let Ok(serde_json::Value::Object(mut map)) = serde_json::to_value(&*self) else { return false };
        let v = value.trim();
        let new = match map.get(key) {
            Some(serde_json::Value::String(_)) => serde_json::Value::String(v.to_string()),
            Some(serde_json::Value::Bool(_)) => match v.to_lowercase().as_str() {
                "1" | "true" | "on" | "y" | "yes" | "co" | "có" => serde_json::Value::Bool(true),
                "0" | "false" | "off" | "n" | "no" | "khong" | "không" => serde_json::Value::Bool(false),
                _ => return false,
            },
            Some(serde_json::Value::Number(_)) => match v.parse::<i64>() {
                Ok(n) if n >= 0 => serde_json::Value::from(n),
                _ => return false,
            },
            _ => return false,
        };
        map.insert(key.to_string(), new);
        match serde_json::from_value(serde_json::Value::Object(map)) {
            Ok(p) => {
                *self = p;
                true
            }
            Err(_) => false,
        }
    }

    /// The shared fields as JSON, to remember for the next extract.
    pub fn shared_json(&self) -> String {
        let Ok(serde_json::Value::Object(map)) = serde_json::to_value(self) else { return String::new() };
        let kept: serde_json::Map<String, serde_json::Value> =
            map.into_iter().filter(|(k, _)| SHARED.contains(&k.as_str())).collect();
        serde_json::Value::Object(kept).to_string()
    }

    /// Defaults with the remembered shared fields on top.
    pub fn with_shared(json: &str) -> Self {
        let mut v = serde_json::to_value(Self::default()).unwrap_or_default();
        if let (Some(map), Ok(serde_json::Value::Object(saved))) =
            (v.as_object_mut(), serde_json::from_str::<serde_json::Value>(json))
        {
            for (k, val) in saved {
                if SHARED.contains(&k.as_str()) {
                    map.insert(k, val);
                }
            }
        }
        serde_json::from_value(v).unwrap_or_default()
    }

    pub fn side(&self) -> bool {
        self.placement != "here"
    }
}

/// Today as dd/mm/yyyy, Vietnam time (UTC+7).
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 7 * 3600;
    let (y, m, d) = civil(secs.div_euclid(86_400));
    format!("{d:02}/{m:02}/{y}")
}

/// Days since 1970-01-01 → (year, month, day) (H. Hinnant's algorithm).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

// ── XDATA ───────────────────────────────────────────────────────────────────

fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                let mut buf = [0u16; 2];
                c.encode_utf16(&mut buf).iter().map(|u| format!("\\u{u:04x}")).collect()
            }
        })
        .collect()
}

/// The source polyline's XDATA strings: `TL:<id>` then the JSON in pieces.
pub fn params_to_xdata(p: &Params) -> Vec<String> {
    let json: Vec<char> = ascii(&serde_json::to_string(p).unwrap_or_default()).chars().collect();
    let mut out = vec![format!("TL:{}", p.id)];
    out.extend(json.chunks(240).map(|c| c.iter().collect::<String>()));
    out
}

/// What a `CADMINI_TL` record says.
#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    /// The source polyline, with its parameters (None: unreadable JSON).
    Source(String, Option<Params>),
    /// One entity of a sheet.
    Sheet(String),
}

pub fn tag_from_xdata(strings: &[String]) -> Option<Tag> {
    let first = strings.first()?;
    match first.strip_prefix("TL:") {
        Some(id) => Some(Tag::Source(id.to_string(), serde_json::from_str(&strings[1..].concat()).ok())),
        None => Some(Tag::Sheet(first.clone())),
    }
}

/// A new sheet id: ten hex digits.
pub fn new_id(seed: u64) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut x = t ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    format!("{:010x}", x & 0xff_ffff_ffff)
}

// ── Geometry ────────────────────────────────────────────────────────────────

/// (length, signed segment area) of a → b with a's bulge.
fn seg(a: &Vtx, b: &Vtx) -> (f64, f64) {
    let c = (b[0] - a[0]).hypot(b[1] - a[1]);
    let bu = a[2];
    if bu == 0.0 || c == 0.0 {
        return (c, 0.0);
    }
    let th = 4.0 * bu.abs().atan();
    let r = c / (2.0 * (th / 2.0).sin());
    (r * th, (r * r / 2.0 * (th - th.sin())).copysign(bu))
}

/// Signed area (positive: counter-clockwise), arcs included.
pub fn ring_area(ring: &[Vtx]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (&ring[i], &ring[(i + 1) % n]);
            (a[0] * b[1] - b[0] * a[1]) / 2.0 + seg(a, b).1
        })
        .sum()
}

pub fn edge_lengths(ring: &[Vtx]) -> Vec<f64> {
    let n = ring.len();
    (0..n).map(|i| seg(&ring[i], &ring[(i + 1) % n]).0).collect()
}

/// Numbering order: clockwise from the north-west corner, as cadastral maps.
pub fn order_ring(ring: &[Vtx], renumber: bool) -> Vec<Vtx> {
    let mut r = ring.to_vec();
    if !renumber || r.is_empty() {
        return r;
    }
    let n = r.len();
    if ring_area(&r) > 0.0 {
        // Reversed, the bulge of j → j-1 is minus that of j-1 → j.
        r = (0..n).rev().map(|i| [ring[i][0], ring[i][1], -ring[(i + n - 1) % n][2]]).collect();
    }
    let cx = r.iter().map(|p| p[0]).sum::<f64>() / n as f64;
    let cy = r.iter().map(|p| p[1]).sum::<f64>() / n as f64;
    let mut k = 0;
    let mut best = f64::NEG_INFINITY;
    for (i, p) in r.iter().enumerate() {
        let v = (p[1] - cy) - (p[0] - cx);
        if v > best {
            best = v;
            k = i;
        }
    }
    r.rotate_left(k);
    r
}

pub fn centroid(ring: &[Vtx]) -> Xy {
    let n = ring.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (&ring[i], &ring[(i + 1) % n]);
        let f = p[0] * q[1] - q[0] * p[1];
        a += f;
        cx += (p[0] + q[0]) * f;
        cy += (p[1] + q[1]) * f;
    }
    if a.abs() < 1e-12 {
        return [
            ring.iter().map(|p| p[0]).sum::<f64>() / n.max(1) as f64,
            ring.iter().map(|p| p[1]).sum::<f64>() / n.max(1) as f64,
        ];
    }
    [cx / (3.0 * a), cy / (3.0 * a)]
}

/// A number with `nd` decimals, a decimal comma when `comma`.
pub fn fmt(v: f64, nd: usize, comma: bool) -> String {
    let s = format!("{v:.nd$}");
    if comma { s.replace('.', ",") } else { s }
}

/// The smallest standard scale that holds the parcel (plus 30 % for its
/// numbers and lengths) in the map frame.
pub fn auto_scale(w: f64, h: f64, map_mm: Xy, unit: &str) -> i64 {
    let per_mm = if unit == "m" { 1000.0 } else { 1.0 };
    let need = (w * 1.3 * per_mm / map_mm[0]).max(h * 1.3 * per_mm / map_mm[1]).max(1.0);
    SCALES
        .iter()
        .copied()
        .find(|s| *s as f64 >= need)
        .unwrap_or(((need / 1000.0).ceil() * 1000.0) as i64)
}

pub fn paper_size(p: &Params) -> Xy {
    let (_, a, b) = PAPERS_MM.iter().find(|(n, ..)| n.eq_ignore_ascii_case(&p.paper)).unwrap_or(&PAPERS_MM[0]);
    if p.landscape { [*b, *a] } else { [*a, *b] }
}

/// A rectangle (x0, y0, x1, y1).
pub type Rect = [f64; 4];

/// The sheet's areas, in mm from the paper's lower-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub paper: Xy,
    pub frame: Rect,
    pub info: Rect,
    pub map: Rect,
    pub table: Rect,
    pub sign: Rect,
}

pub fn layout(p: &Params) -> Layout {
    let [w, h] = paper_size(p);
    // A 20 mm left margin for binding.
    let (x0, y0, x1, y1) = (20.0, 10.0, w - 10.0, h - 10.0);
    let big = p.paper.eq_ignore_ascii_case("A3");
    if !p.landscape {
        let bottom = if big { 120.0 } else { 85.0 };
        let split = x0 + (x1 - x0) * 0.55;
        Layout {
            paper: [w, h],
            frame: [x0, y0, x1, y1],
            info: [x0 + 4.0, y1 - 68.0, x1 - 4.0, y1 - 32.0],
            map: [x0 + 2.0, y0 + bottom + 2.0, x1 - 2.0, y1 - 72.0],
            table: [x0 + 2.0, y0 + 2.0, split - 2.0, y0 + bottom - 2.0],
            sign: [split + 2.0, y0 + 2.0, x1 - 2.0, y0 + bottom - 2.0],
        }
    } else {
        let split = x0 + (x1 - x0) * 0.58;
        let sign_h = if big { 52.0 } else { 40.0 };
        Layout {
            paper: [w, h],
            frame: [x0, y0, x1, y1],
            info: [split + 4.0, y1 - 76.0, x1 - 2.0, y1 - 32.0],
            map: [x0 + 2.0, y0 + 2.0, split, y1 - 30.0],
            table: [split + 4.0, y0 + sign_h + 2.0, x1 - 2.0, y1 - 80.0],
            sign: [split + 4.0, y0 + 2.0, x1 - 2.0, y0 + sign_h],
        }
    }
}

pub fn info_lines(p: &Params, area: f64) -> Vec<String> {
    let c = p.comma;
    let mut lines = vec![
        format!("Thửa đất số: {};   Tờ bản đồ số: {}", p.so_thua, p.to_ban_do),
        format!("Diện tích: {} m²", fmt(area, 1, c)),
        format!("Mục đích sử dụng: {}", p.muc_dich),
        format!("Người sử dụng đất: {}", p.chu),
        format!("Địa chỉ thửa đất: {}", p.dia_chi),
        format!("Hệ tọa độ: {}", p.he_toa_do),
        format!("Ngày trích xuất: {}", p.ngay),
    ];
    if !p.ghi_chu.is_empty() {
        lines.push(format!("Ghi chú: {}", p.ghi_chu));
    }
    lines
}

/// "Đất trồng lúa (LUC)" → "LUC"; a short code stays as it is.
pub fn land_code(muc_dich: &str) -> String {
    let s = muc_dich.trim();
    if s.ends_with(')') {
        if let Some(k) = s.rfind('(') {
            return s[k + 1..s.len() - 1].trim().to_string();
        }
    }
    if s.chars().count() <= 6 { s.to_string() } else { String::new() }
}

/// Scale, the sheet's lower-left corner (model units) and the shift of the
/// parcel's copy when the sheet is beside the drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub layout: Layout,
    pub scale: i64,
    /// Model units per paper mm.
    pub k: f64,
    pub base: Xy,
    pub shift: Xy,
}

impl Plan {
    /// The paper's outline in model units.
    pub fn sheet_box(&self) -> Rect {
        let [w, h] = self.layout.paper;
        [self.base[0], self.base[1], self.base[0] + w * self.k, self.base[1] + h * self.k]
    }

    /// The map frame in the source drawing's coordinates (before the shift).
    pub fn map_box_at_source(&self) -> Rect {
        let [mx0, my0, mx1, my1] = self.layout.map;
        let (k, [bx, by], [dx, dy]) = (self.k, self.base, self.shift);
        [bx + mx0 * k - dx, by + my0 * k - dy, bx + mx1 * k - dx, by + my1 * k - dy]
    }
}

/// `extents`: the drawing's extents (x0, y0, x1, y1), for a new sheet beside it.
pub fn plan(ring: &[Vtx], p: &Params, extents: Option<Rect>) -> Plan {
    let lay = layout(p);
    let [mx0, my0, mx1, my1] = lay.map;
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in ring {
        xmin = xmin.min(q[0]);
        xmax = xmax.max(q[0]);
        ymin = ymin.min(q[1]);
        ymax = ymax.max(q[1]);
    }
    let scale = if p.scale > 0 { p.scale } else { auto_scale(xmax - xmin, ymax - ymin, [mx1 - mx0, my1 - my0], &p.unit) };
    let k = if p.unit == "m" { scale as f64 / 1000.0 } else { scale as f64 };
    let (pcx, pcy) = ((xmax + xmin) / 2.0, (ymax + ymin) / 2.0);
    let (mcx, mcy) = ((mx0 + mx1) / 2.0, (my0 + my1) / 2.0);
    let base = if !p.side() {
        [pcx - mcx * k, pcy - mcy * k]
    } else if let Some(b) = p.base {
        b
    } else {
        let right = extents.map_or(xmax, |e| e[2]);
        [right + 0.15 * lay.paper[0] * k, pcy - lay.paper[1] / 2.0 * k]
    };
    let shift = [base[0] + mcx * k - pcx, base[1] + mcy * k - pcy];
    Plan { layout: lay, scale, k, base, shift }
}

// ── The sheet's drawing ─────────────────────────────────────────────────────

/// One drawing item, in model units. Line weights in 1/100 mm, -1 by layer.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Text { at: Xy, s: String, h: f64, center: bool, layer: &'static str, rot_deg: f64 },
    Line { a: Xy, b: Xy, layer: &'static str, lw: i16 },
    Poly { pts: Vec<Vtx>, closed: bool, layer: &'static str, lw: i16 },
    Circle { c: Xy, r: f64, layer: &'static str },
    /// ANSI31 (45° lines) `spacing` apart.
    Hatch { pts: Vec<Xy>, layer: &'static str, spacing: f64 },
}

struct Sheet {
    base: Xy,
    k: f64,
    items: Vec<Item>,
}

impl Sheet {
    fn p(&self, x: f64, y: f64) -> Xy {
        [self.base[0] + x * self.k, self.base[1] + y * self.k]
    }

    /// Text at paper mm (`model`: at model coordinates), height in paper mm.
    fn text(&mut self, at: Xy, s: &str, h: f64, center: bool, layer: &'static str, rot_deg: f64, model: bool) {
        if s.is_empty() {
            return;
        }
        let at = if model { at } else { self.p(at[0], at[1]) };
        self.items.push(Item::Text { at, s: s.to_string(), h: h * self.k, center, layer, rot_deg });
    }

    fn ptext(&mut self, x: f64, y: f64, s: &str, h: f64) {
        self.text([x, y], s, h, true, L_TEXT, 0.0, false);
    }

    fn line(&mut self, a: Xy, b: Xy, layer: &'static str, lw: i16, model: bool) {
        let (a, b) = if model { (a, b) } else { (self.p(a[0], a[1]), self.p(b[0], b[1])) };
        self.items.push(Item::Line { a, b, layer, lw });
    }

    fn rect(&mut self, r: Rect, lw: i16) {
        let [x0, y0, x1, y1] = r;
        let pts = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|q| {
            let m = self.p(q[0], q[1]);
            [m[0], m[1], 0.0]
        });
        self.items.push(Item::Poly { pts: pts.to_vec(), closed: true, layer: L_FRAME, lw });
    }
}

fn left_normal(a: &Vtx, b: &Vtx, sgn: f64) -> Xy {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy).max(1e-12);
    [-dy / l * sgn, dx / l * sgn]
}

/// The sheet for the closed ring `ring0` (source coordinates). `extra`: the
/// parcel's parts taken by GPMB, drawn hatched red. Returns the items, the
/// plan and the numbered vertices.
pub fn build_sheet(ring0: &[Vtx], p: &Params, extents: Option<Rect>, extra: &[Vec<Xy>]) -> (Vec<Item>, Plan, Vec<Vtx>) {
    let ring = order_ring(ring0, p.renumber);
    let area = ring_area(&ring).abs();
    let c = p.comma;
    let pl = plan(&ring, p, extents);
    let lay = pl.layout;
    let [dx, dy] = pl.shift;
    let k = pl.k;
    let mut sh = Sheet { base: pl.base, k, items: Vec::new() };
    let side = p.side();

    // Frame and heading.
    let [fx0, _fy0, fx1, fy1] = lay.frame;
    sh.rect(lay.frame, 50);
    let half = (fx0 + fx1) / 2.0;
    if !p.don_vi.is_empty() {
        let x = (fx0 + half) / 2.0;
        sh.ptext(x, fy1 - 7.0, &p.don_vi.to_uppercase(), 2.6);
        sh.line([x - 12.0, fy1 - 10.0], [x + 12.0, fy1 - 10.0], L_FRAME, 18, false);
    }
    let rx = (half + fx1) / 2.0;
    sh.ptext(rx, fy1 - 7.0, "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", 2.6);
    sh.ptext(rx, fy1 - 12.0, "Độc lập - Tự do - Hạnh phúc", 2.8);
    sh.line([rx - 22.0, fy1 - 14.5], [rx + 22.0, fy1 - 14.5], L_FRAME, 18, false);
    let title = if p.tieu_de.is_empty() { "TRÍCH LỤC THỬA ĐẤT" } else { p.tieu_de.as_str() };
    sh.ptext(half, fy1 - 23.0, title, 5.0);

    // The parcel's data.
    let [ix0, iy0, _ix1, iy1] = lay.info;
    let lines = info_lines(p, area);
    let step = 5.5f64.min((iy1 - iy0) / lines.len().max(1) as f64);
    for (i, s) in lines.iter().enumerate() {
        sh.text([ix0, iy1 - 3.0 - i as f64 * step], s, 2.8, false, L_TEXT, 0.0, false);
    }

    // The map: parcel, vertex numbers, edges, label, north arrow, scale.
    let [mx0, my0, mx1, my1] = lay.map;
    sh.rect(lay.map, 25);
    let moved: Vec<Vtx> = if side {
        let m: Vec<Vtx> = ring.iter().map(|q| [q[0] + dx, q[1] + dy, q[2]]).collect();
        sh.items.push(Item::Poly { pts: m.clone(), closed: true, layer: L_PARCEL, lw: 50 });
        m
    } else {
        ring.clone()
    };
    let (sx, sy) = if side { (dx, dy) } else { (0.0, 0.0) };
    for part in extra {
        let mut pts: Vec<Xy> = part.iter().map(|q| [q[0] + sx, q[1] + sy]).collect();
        if pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
        if pts.len() < 3 {
            continue;
        }
        let as_vtx: Vec<Vtx> = pts.iter().map(|q| [q[0], q[1], 0.0]).collect();
        sh.items.push(Item::Hatch { pts: pts.clone(), layer: L_TAKE, spacing: 2.0 * k });
        sh.items.push(Item::Poly { pts: as_vtx.clone(), closed: true, layer: L_TAKE, lw: 35 });
        let a = ring_area(&as_vtx).abs();
        let cc = centroid(&as_vtx);
        // Below the parcel's own label (number / area / land use).
        sh.text([cc[0], cc[1] - 9.0 * k], &format!("Thu hồi {} m²", fmt(a, 1, c)), 2.2, true, L_TAKE, 0.0, true);
    }
    let n = moved.len();
    let lens = edge_lengths(&ring);
    // Outward normals: a clockwise ring has the outside on its left.
    let sgn = if ring_area(&moved) < 0.0 { 1.0 } else { -1.0 };
    for i in 0..n {
        let (a, b, prev) = (&moved[i], &moved[(i + 1) % n], &moved[(i + n - 1) % n]);
        let n1 = left_normal(prev, a, sgn);
        let n2 = left_normal(a, b, sgn);
        let (nx, ny) = (n1[0] + n2[0], n1[1] + n2[1]);
        let l = nx.hypot(ny);
        let l = if l == 0.0 { 1.0 } else { l };
        sh.text([a[0] + nx / l * 3.2 * k, a[1] + ny / l * 3.2 * k], &(i + 1).to_string(), 2.5, true, L_VERTEX, 0.0, true);
        sh.items.push(Item::Circle { c: [a[0], a[1]], r: 0.5 * k, layer: L_VERTEX });
        if p.edge_len && lens[i] > 0.0 {
            let mut ang = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
            if ang > 90.0 {
                ang -= 180.0;
            } else if ang <= -90.0 {
                ang += 180.0;
            }
            let m = [(a[0] + b[0]) / 2.0 + n2[0] * 1.8 * k, (a[1] + b[1]) / 2.0 + n2[1] * 1.8 * k];
            sh.text(m, &fmt(lens[i], 2, c), 2.0, true, L_EDGE, ang, true);
        }
    }
    let [cx, cy] = centroid(&moved);
    if !p.so_thua.is_empty() {
        sh.text([cx, cy + 2.2 * k], &p.so_thua, 3.0, true, L_TEXT, 0.0, true);
        sh.line([cx - 6.0 * k, cy], [cx + 6.0 * k, cy], L_TEXT, -1, true);
        sh.text([cx, cy - 2.2 * k], &fmt(area, 1, c), 2.5, true, L_TEXT, 0.0, true);
        sh.text([cx, cy - 5.6 * k], &land_code(&p.muc_dich), 2.5, true, L_TEXT, 0.0, true);
    } else {
        sh.text([cx, cy], &format!("S = {} m²", fmt(area, 1, c)), 2.5, true, L_TEXT, 0.0, true);
    }
    let (ax, ay) = (mx1 - 8.0, my1 - 12.0);
    let arrow = [[ax, ay + 6.0], [ax - 2.5, ay - 4.0], [ax, ay - 1.5], [ax + 2.5, ay - 4.0]].map(|q| {
        let m = sh.p(q[0], q[1]);
        [m[0], m[1], 0.0]
    });
    sh.items.push(Item::Poly { pts: arrow.to_vec(), closed: true, layer: L_FRAME, lw: -1 });
    sh.ptext(ax, ay + 9.0, "B", 3.0);
    sh.ptext((mx0 + mx1) / 2.0, my0 + 3.0, &format!("Tỷ lệ 1:{}", pl.scale), 2.5);

    // The coordinate table.
    table(&mut sh, lay.table, &ring, &lens, area, c);

    // Signatures.
    let [sx0, sy0, sx1, sy1] = lay.sign;
    let when = match parse_date(&p.ngay) {
        Some((d, m, y)) => format!("{}, ngày {d:02} tháng {m:02} năm {y}", p.dia_danh),
        None => format!("{}, ngày ..... tháng ..... năm .......", p.dia_danh),
    };
    let when = when.trim_start_matches([',', ' ']).to_string();
    sh.ptext((sx0 + sx1) / 2.0, sy1 - 4.0, &when, 2.6);
    let (q1, q2) = (sx0 + (sx1 - sx0) * 0.25, sx0 + (sx1 - sx0) * 0.75);
    // A narrow signature column (A4 portrait): smaller words.
    let th = if sx1 - sx0 > 95.0 { 2.6 } else { 2.2 };
    sh.ptext(q1, sy1 - 10.0, "NGƯỜI TRÍCH LỤC", th);
    sh.ptext(q2, sy1 - 10.0, "THỦ TRƯỞNG ĐƠN VỊ", th);
    sh.ptext(q1, sy0 + 6.0, &p.nguoi_lap, 2.6);
    sh.ptext(q2, sy0 + 6.0, &p.thu_truong, 2.6);
    (sh.items, pl, ring)
}

fn parse_date(s: &str) -> Option<(u32, u32, i64)> {
    let mut it = s.trim().split('/');
    let d: u32 = it.next()?.trim().parse().ok()?;
    let m: u32 = it.next()?.trim().parse().ok()?;
    let y: i64 = it.next()?.trim().parse().ok()?;
    ((1..=31).contains(&d) && (1..=12).contains(&m) && it.next().is_none()).then_some((d, m, y))
}

fn table(sh: &mut Sheet, r: Rect, ring: &[Vtx], lens: &[f64], area: f64, comma: bool) {
    let [x0, y0, x1, y1] = r;
    let n = ring.len();
    let head = 7.0;
    let mut rh = 5.0;
    // Room for the area line below.
    let usable = (y1 - y0) - head - 7.0;
    let mut fit = ((usable / rh).floor() as usize).max(1);
    if n > fit {
        // Smaller rows to fit one column first; more columns only at the smallest.
        rh = 3.6f64.max(usable / n as f64);
        fit = if rh * n as f64 <= usable + 1e-6 { n } else { ((usable / rh).floor() as usize).max(1) };
    }
    let groups = n.div_ceil(fit).max(1);
    let gw = (x1 - x0) / groups as f64;
    let cols = [0.14, 0.32, 0.32, 0.22];
    let names = ["Điểm", "X (m)", "Y (m)", "Cạnh (m)"];
    let th = 2.5f64.min(rh * 0.55);
    for g in 0..groups {
        let rows: Vec<usize> = (g * fit..n.min((g + 1) * fit)).collect();
        let gx = x0 + g as f64 * gw;
        let top = y1;
        let bot = y1 - head - rows.len() as f64 * rh;
        sh.rect([gx, bot, gx + gw, top], 25);
        sh.line([gx, top - head], [gx + gw, top - head], L_FRAME, 18, false);
        let mut cx = gx;
        for (w, nm) in cols.iter().zip(names) {
            sh.ptext(cx + w * gw / 2.0, top - head / 2.0, nm, 2.5);
            cx += w * gw;
            if cx < gx + gw - 1e-6 {
                sh.line([cx, bot], [cx, top], L_FRAME, 18, false);
            }
        }
        for (j, &i) in rows.iter().enumerate() {
            let yy = top - head - (j as f64 + 0.5) * rh;
            // X = North (y), Y = East (x).
            let vals = [(i + 1).to_string(), fmt(ring[i][1], 2, comma), fmt(ring[i][0], 2, comma), fmt(lens[i], 2, comma)];
            let mut cx = gx;
            for (w, v) in cols.iter().zip(vals) {
                sh.text([cx + w * gw / 2.0, yy], &v, th, true, L_TEXT, 0.0, false);
                cx += w * gw;
            }
        }
    }
    let tail = y1 - head - n.min(fit) as f64 * rh;
    sh.text(
        [x0, tail - 4.5],
        &format!("Diện tích: S = {} m²   (cạnh i: từ điểm i đến điểm i+1)", fmt(area, 1, comma)),
        2.5,
        false,
        L_TEXT,
        0.0,
        false,
    );
}

/// Several sheets in a grid right of the drawing (GPMB B4): `cols` per row,
/// from the drawing's top. Returns each sheet's lower-left corner.
pub fn grid_bases(sizes: &[Xy], extents: Option<Rect>, cols: usize) -> Vec<Xy> {
    let (x0, y_top) = extents.map_or((0.0, 0.0), |e| (e[2], e[3]));
    let mut out = Vec::with_capacity(sizes.len());
    let (mut gap, mut x, mut y, mut row_h, mut col) = (0.0, 0.0, 0.0, 0.0f64, 0usize);
    for (n, [w, h]) in sizes.iter().copied().enumerate() {
        if n == 0 {
            gap = w * 0.08;
            x = x0 + w * 0.3;
            y = y_top - h;
        }
        if col >= cols.max(1) {
            col = 0;
            x = x0 + w * 0.3;
            y -= row_h + gap;
            row_h = 0.0;
        }
        out.push([x, y]);
        x += w + gap;
        row_h = row_h.max(h);
        col += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, w: f64) -> Vec<Vtx> {
        vec![[x, y, 0.0], [x + w, y, 0.0], [x + w, y + w, 0.0], [x, y + w, 0.0]]
    }

    #[test]
    fn numbering_runs_clockwise_from_the_north_west() {
        let r = order_ring(&square(0.0, 0.0, 10.0), true);
        assert_eq!(r[0], [0.0, 10.0, 0.0]);
        assert_eq!(r[1], [10.0, 10.0, 0.0]);
        assert!(ring_area(&r) < 0.0);
    }

    #[test]
    fn arcs_count_in_area_and_length() {
        // A half disc of radius 5: the chord (10, 0) → (0, 0) bulges out by 1.
        let r = vec![[0.0, 0.0, 1.0], [10.0, 0.0, 0.0]];
        let a = ring_area(&r).abs();
        assert!((a - std::f64::consts::PI * 12.5).abs() < 1e-9, "{a}");
        let l = edge_lengths(&r);
        assert!((l[0] - 5.0 * std::f64::consts::PI).abs() < 1e-9);
        // Reversing keeps the arc on the same side.
        let o = order_ring(&r, true);
        assert!((ring_area(&o).abs() - a).abs() < 1e-9);
    }

    #[test]
    fn scale_and_layout_match_cadmini() {
        let p = Params::default();
        let lay = layout(&p);
        assert_eq!(lay.paper, [210.0, 297.0]);
        assert_eq!(lay.map, [22.0, 97.0, 198.0, 215.0]);
        // 40 m × 30 m in a 176 × 108 mm map needs 1:361 → 1:500.
        assert_eq!(auto_scale(40.0, 30.0, [176.0, 108.0], "m"), 500);
        assert_eq!(auto_scale(4000.0, 3000.0, [176.0, 108.0], "m"), 37000);
        assert_eq!(land_code("Đất trồng lúa (LUC)"), "LUC");
        assert_eq!(land_code("ONT"), "ONT");
        assert_eq!(fmt(1234.5, 1, true), "1234,5");
    }

    #[test]
    fn a_sheet_has_every_part() {
        let p = Params { so_thua: "12".into(), muc_dich: "Đất ở tại nông thôn (ONT)".into(), ngay: "05/11/2026".into(), ..Default::default() };
        let (items, pl, ring) = build_sheet(&square(500.0, 1000.0, 20.0), &p, Some([0.0, 0.0, 600.0, 1100.0]), &[]);
        assert_eq!(pl.scale, 250);
        assert_eq!(ring.len(), 4);
        // Beside the drawing.
        assert!(pl.base[0] > 600.0);
        let texts: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                Item::Text { s, .. } => Some(s.as_str()),
                _ => None,
            })
            .collect();
        for want in ["TRÍCH LỤC THỬA ĐẤT", "1", "4", "20,00", "400,0", "ONT", "Tỷ lệ 1:250", "NGƯỜI TRÍCH LỤC"] {
            assert!(texts.contains(&want), "{want} missing: {texts:?}");
        }
        assert!(texts.iter().any(|t| t.contains("ngày 05 tháng 11 năm 2026")));
        // The table: X is North. Vertex 1 is the north-west corner (500, 1020).
        assert!(texts.contains(&"1020,00") && texts.contains(&"500,00"));
        // The parcel's copy sits in the map frame.
        let copy = items.iter().find_map(|i| match i {
            Item::Poly { pts, layer: L_PARCEL, .. } => Some(pts.clone()),
            _ => None,
        });
        let b = pl.sheet_box();
        let m = pl.layout.map;
        for q in copy.unwrap() {
            assert!(q[0] > b[0] + m[0] * pl.k && q[0] < b[0] + m[2] * pl.k);
            assert!(q[1] > b[1] + m[1] * pl.k && q[1] < b[1] + m[3] * pl.k);
        }
        // In place: the sheet goes round the parcel.
        let here = Params { placement: "here".into(), ..p };
        let (_, pl2, _) = build_sheet(&square(500.0, 1000.0, 20.0), &here, None, &[]);
        assert_eq!(pl2.shift, [0.0, 0.0]);
    }

    #[test]
    fn params_round_trip_and_cadmini_json() {
        let p = Params { id: "abc".into(), chu: "Nguyễn Văn An".into(), base: Some([1.0, 2.0]), ..Default::default() };
        let x = params_to_xdata(&p);
        assert_eq!(x[0], "TL:abc");
        assert!(x.iter().all(|s| s.is_ascii() && s.len() <= 240));
        assert_eq!(tag_from_xdata(&x), Some(Tag::Source("abc".into(), Some(p.clone()))));
        assert_eq!(tag_from_xdata(&["abc".into()]), Some(Tag::Sheet("abc".into())));
        // CadMini's own JSON (Python, ensure_ascii; base may be missing).
        let py = r#"{"tieu_de": "TRÍCH LỤC THỬA ĐẤT", "so_thua": "7", "paper": "A3", "landscape": true, "scale": 500, "id": "0123456789"}"#;
        let q: Params = serde_json::from_str(py).unwrap();
        assert_eq!(q.tieu_de, "TRÍCH LỤC THỬA ĐẤT");
        assert_eq!((q.paper.as_str(), q.landscape, q.scale, q.base), ("A3", true, 500, None));
        // Field editing and the remembered fields.
        let mut r = Params::default();
        assert!(r.set("so_thua", "15"));
        assert!(r.set("landscape", "có"));
        assert!(r.set("scale", "1000"));
        assert!(!r.set("scale", "abc"));
        assert!(!r.set("nothing", "1"));
        r.nguoi_lap = "Lê C".into();
        let s = Params::with_shared(&r.shared_json());
        assert_eq!((s.landscape, s.nguoi_lap.as_str(), s.so_thua.as_str(), s.scale), (true, "Lê C", "", 0));
    }

    #[test]
    fn long_rings_fold_into_columns() {
        let ring: Vec<Vtx> = (0..80)
            .map(|i| {
                let t = i as f64 / 80.0 * std::f64::consts::TAU;
                [100.0 * t.cos(), 100.0 * t.sin(), 0.0]
            })
            .collect();
        let (items, _, _) = build_sheet(&ring, &Params::default(), None, &[]);
        let heads = items.iter().filter(|i| matches!(i, Item::Text { s, .. } if s == "Điểm")).count();
        assert!(heads >= 2, "{heads}");
    }

    #[test]
    fn grid_places_rows_of_sheets() {
        let b = grid_bases(&[[10.0, 20.0]; 3], Some([0.0, 0.0, 100.0, 50.0]), 2);
        assert_eq!(b[0], [103.0, 30.0]);
        assert!((b[1][0] - 113.8).abs() < 1e-9);
        assert_eq!(b[2][0], 103.0);
        assert!((b[2][1] - (30.0 - 20.8)).abs() < 1e-9);
    }

    #[test]
    fn dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_760), (2026, 11, 3));
        assert_eq!(parse_date("3/11/2026"), Some((3, 11, 2026)));
        assert_eq!(parse_date("hôm nay"), None);
    }
}
