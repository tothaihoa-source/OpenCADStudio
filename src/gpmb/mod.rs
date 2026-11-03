//! GPMB (giải phóng mặt bằng — land clearance) in four steps, as CadMini
//! does it. This module is the calculation only; the commands that read the
//! drawing and write the results live in `app::commands::gpmb_cmd`.
//!
//! - B1 clean: each closed parcel gathers the texts inside it → parcel
//!   number, owner, land-use code, area written on the map; missing items
//!   are flagged. Data (including what the user adds) is kept in the
//!   parcel polyline's XDATA (`CADMINI_GPMB`), the same as CadMini.
//! - B2 trace: close open polylines with a small gap; build parcels from
//!   loose lines where there is no closed parcel yet.
//! - B3 compute: intersect each parcel with the GPMB boundary → area taken
//!   / remaining; the part of the boundary in no parcel is managed by the
//!   commune (roads, canals, public land); slivers below the threshold are
//!   digitising error.
//! - B4 sheets: one parcel extract per taken parcel (TRICHLUC).

use geo::{Area, BooleanOps, Contains, InteriorPoint};
use geo::{Coord, LineString, MultiPolygon, Point, Polygon};

pub const APPID: &str = "CADMINI_GPMB";

pub const L_MISSING: &str = "GPMB_THIEU_TT";
pub const L_ERROR: &str = "GPMB_LOI";
pub const L_NEW: &str = "GPMB_THUA_BO";
pub const L_TAKE: &str = "GPMB_THU_HOI";
pub const L_OUT: &str = "GPMB_NGOAI_THUA";
pub const L_LABEL: &str = "GPMB_NHAN";
pub const L_TABLE: &str = "GPMB_BANG";
pub const L_AREA: &str = "GPMB_DIEN_TICH";
/// The layers GPMB writes, with their colours.
pub const LAYERS: &[(&str, i16)] = &[
    (L_MISSING, 1),
    (L_ERROR, 6),
    (L_NEW, 4),
    (L_TAKE, 1),
    (L_OUT, 3),
    (L_LABEL, 7),
    (L_TABLE, 7),
    (L_AREA, 4),
];
pub const UBND: &str = "Đất do UBND quản lý (không thuộc thửa)";

/// Land-use codes (Circular 08/2024 and earlier ones).
const CODES: &[&str] = &[
    "ONT", "ODT", "LUC", "LUK", "LUN", "BHK", "NHK", "HNK", "CLN", "RSX", "RPH", "RDD", "NTS", "LMU",
    "NKH", "TMD", "SKC", "SKK", "SKN", "SKX", "TSC", "DGD", "DYT", "DVH", "DTT", "DGT", "DTL", "DNL",
    "DBV", "DCH", "DKG", "DSH", "DRA", "TON", "TIN", "NTD", "SON", "MNC", "PNK", "BCS", "DCS", "NCS",
    "CQP", "CAN", "DHT", "DKV", "DXH", "DSN", "DCK", "DDT", "DDL", "SKS", "DNG", "TTN", "NNP", "SXN",
    "RST", "LNP", "CSD", "DVD", "DNT", "ODT+CLN", "ONT+CLN",
];

pub type Xy = [f64; 2];

// ── Texts ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Number,
    Area,
    Code,
    Name,
    Other,
}

fn is_int_label(t: &str) -> bool {
    // 1–5 digits, optionally one letter: 12, 205a.
    let mut chars = t.chars().peekable();
    let mut digits = 0;
    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
        chars.next();
        digits += 1;
    }
    if !(1..=5).contains(&digits) {
        return false;
    }
    match (chars.next(), chars.next()) {
        (None, _) => true,
        (Some(c), None) => c.is_ascii_alphabetic(),
        _ => false,
    }
}

fn is_area_label(t: &str) -> bool {
    let lower = t.to_lowercase();
    let body = lower.trim_end_matches("m²").trim_end_matches("m2").trim_end();
    if body.len() == lower.len() && !body.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        return false;
    }
    let mut parts = body.splitn(2, ['.', ',']);
    let int = parts.next().unwrap_or("");
    let frac = parts.next();
    (1..=7).contains(&int.len())
        && int.chars().all(|c| c.is_ascii_digit())
        && frac.is_none_or(|f| (1..=3).contains(&f.len()) && f.chars().all(|c| c.is_ascii_digit()))
}

fn is_code_label(t: &str) -> bool {
    t.split('+')
        .all(|p| (2..=4).contains(&p.len()) && p.chars().all(|c| c.is_ascii_uppercase()))
}

fn has_name_hint(t: &str) -> bool {
    let l = t.to_lowercase();
    ["ô.", "ô ", "bà", "ông", "b.", "hộ", "ubnd", "đ/c", "cty", "công ty", "htx"]
        .iter()
        .any(|h| l.starts_with(h))
}

pub fn classify_text(s: &str) -> TextKind {
    let t = s.trim();
    if t.is_empty() {
        return TextKind::Other;
    }
    if is_int_label(t) {
        return TextKind::Number;
    }
    if is_area_label(t) {
        return TextKind::Area;
    }
    let up = t.to_uppercase();
    if is_code_label(t) && (CODES.contains(&up.as_str()) || t.chars().count() == 3) {
        return TextKind::Code;
    }
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    if letters >= 4 && (t.contains(' ') || has_name_hint(t)) {
        return TextKind::Name;
    }
    TextKind::Other
}

pub fn parse_area(s: &str) -> Option<f64> {
    let t = s.to_lowercase().replace("m²", "").replace("m2", "").trim().replace(',', ".");
    t.parse().ok()
}

// ── Parcels ─────────────────────────────────────────────────────────────────

/// A closed parcel outline from the drawing.
#[derive(Clone, Debug)]
pub struct ParcelShape {
    pub handle: u64,
    /// Outer ring, not repeated at the end; arcs already split.
    pub ring: Vec<Xy>,
    /// Data saved earlier in the polyline's XDATA.
    pub saved: Option<ParcelData>,
}

/// A text, multiline text or block attribute.
#[derive(Clone, Debug)]
pub struct Label {
    pub at: Xy,
    pub text: String,
    pub height: f64,
    /// Attribute tag, empty for plain text.
    pub tag: String,
}

/// What GPMB knows of a parcel; saved as JSON in XDATA with CadMini's keys.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ParcelData {
    pub so_thua: String,
    pub to_ban_do: String,
    pub chu: String,
    pub loai_dat: String,
    pub dt_ghi: Option<f64>,
    pub dia_chi: String,
    pub ghi_chu: String,
}

#[derive(Clone, Debug)]
pub struct Parcel {
    pub handle: u64,
    pub data: ParcelData,
    /// Geometric area.
    pub area: f64,
    pub missing: Vec<&'static str>,
}

impl Parcel {
    pub fn check(&mut self) {
        self.missing = [
            ("số thửa", &self.data.so_thua),
            ("chủ sử dụng", &self.data.chu),
            ("loại đất", &self.data.loai_dat),
        ]
        .into_iter()
        .filter(|(_, v)| v.trim().is_empty())
        .map(|(k, _)| k)
        .collect();
    }
}

pub fn polygon(ring: &[Xy]) -> Polygon<f64> {
    let mut coords: Vec<Coord<f64>> = ring.iter().map(|p| Coord { x: p[0], y: p[1] }).collect();
    if let Some(first) = coords.first().copied() {
        coords.push(first);
    }
    Polygon::new(LineString::from(coords), vec![])
}

pub fn ring_area(ring: &[Xy]) -> f64 {
    polygon(ring).unsigned_area()
}

/// A point inside a polygon, away from its edges where it can.
pub fn inner_point(p: &Polygon<f64>) -> Xy {
    p.interior_point().map(|pt| [pt.x(), pt.y()]).unwrap_or_else(|| {
        let r = &p.exterior().0;
        let n = r.len().max(1) as f64;
        [r.iter().map(|c| c.x).sum::<f64>() / n, r.iter().map(|c| c.y).sum::<f64>() / n]
    })
}

fn centroid(ring: &[Xy]) -> Xy {
    use geo::Centroid;
    polygon(ring).centroid().map(|c| [c.x(), c.y()]).unwrap_or([0.0, 0.0])
}

fn bbox(ring: &[Xy]) -> [f64; 4] {
    ring.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

/// B1: each parcel's data from the texts and block attributes inside it.
/// A text inside nested parcels goes to the smallest one.
pub fn extract(shapes: &[ParcelShape], labels: &[Label]) -> Vec<Parcel> {
    let polys: Vec<Polygon<f64>> = shapes.iter().map(|s| polygon(&s.ring)).collect();
    let areas: Vec<f64> = polys.iter().map(|p| p.unsigned_area()).collect();
    let boxes: Vec<[f64; 4]> = shapes.iter().map(|s| bbox(&s.ring)).collect();
    // Grid index over the parcels' boxes.
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); shapes.len()];
    let index = GridIndex::new(&boxes);
    for (li, l) in labels.iter().enumerate() {
        let pt = Point::new(l.at[0], l.at[1]);
        let mut best: Option<usize> = None;
        for pi in index.query_point(l.at) {
            if polys[pi].contains(&pt) && best.is_none_or(|b| areas[pi] < areas[b]) {
                best = Some(pi);
            }
        }
        if let Some(pi) = best {
            buckets[pi].push(li);
        }
    }
    let mut out = Vec::with_capacity(shapes.len());
    for (i, shape) in shapes.iter().enumerate() {
        let mut d = ParcelData::default();
        let c = centroid(&shape.ring);
        let mut nums: Vec<(f64, f64, &str)> = Vec::new();
        for &li in &buckets[i] {
            let l = &labels[li];
            let s = l.text.trim();
            match l.tag.to_uppercase().as_str() {
                "SO_THUA" | "SOTHUA" | "STT" | "THUA" | "SO" => {
                    d.so_thua = s.to_string();
                    continue;
                }
                "CHU" | "CHU_SD" | "TEN" | "TEN_CHU" | "CHUSUDUNG" => {
                    d.chu = s.to_string();
                    continue;
                }
                "LOAI_DAT" | "LOAIDAT" | "MDSD" | "LOAI" => {
                    d.loai_dat = s.to_string();
                    continue;
                }
                "DIEN_TICH" | "DIENTICH" | "DT" => {
                    d.dt_ghi = parse_area(s);
                    continue;
                }
                "TO" | "TO_BD" | "TOBANDO" => {
                    d.to_ban_do = s.to_string();
                    continue;
                }
                _ => {}
            }
            match classify_text(s) {
                TextKind::Number => {
                    let dist = (l.at[0] - c[0]).hypot(l.at[1] - c[1]);
                    nums.push((dist, -l.height, s));
                }
                TextKind::Area if d.dt_ghi.is_none() => d.dt_ghi = parse_area(s),
                TextKind::Code if d.loai_dat.is_empty() => d.loai_dat = s.to_uppercase(),
                TextKind::Name if d.chu.is_empty() => d.chu = s.to_string(),
                _ => {}
            }
        }
        if d.so_thua.is_empty() && !nums.is_empty() {
            // Nearest the parcel's centre, then the larger text.
            nums.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            d.so_thua = nums[0].2.to_string();
        }
        if let Some(saved) = &shape.saved {
            let pick = |a: &mut String, b: &String| {
                if !b.trim().is_empty() {
                    *a = b.clone();
                }
            };
            pick(&mut d.so_thua, &saved.so_thua);
            pick(&mut d.to_ban_do, &saved.to_ban_do);
            pick(&mut d.chu, &saved.chu);
            pick(&mut d.loai_dat, &saved.loai_dat);
            pick(&mut d.dia_chi, &saved.dia_chi);
            pick(&mut d.ghi_chu, &saved.ghi_chu);
            if saved.dt_ghi.is_some() {
                d.dt_ghi = saved.dt_ghi;
            }
        }
        let mut p = Parcel { handle: shape.handle, data: d, area: areas[i], missing: Vec::new() };
        p.check();
        out.push(p);
    }
    out
}

/// A plain grid over boxes, enough for parcel maps of tens of thousands.
struct GridIndex {
    origin: Xy,
    cell: f64,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<usize>>,
}

impl GridIndex {
    fn new(boxes: &[[f64; 4]]) -> Self {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for x in boxes {
            b = [b[0].min(x[0]), b[1].min(x[1]), b[2].max(x[2]), b[3].max(x[3])];
        }
        if boxes.is_empty() || !b[0].is_finite() {
            return Self { origin: [0.0, 0.0], cell: 1.0, cols: 1, rows: 1, cells: vec![Vec::new()] };
        }
        let n = (boxes.len() as f64).sqrt().ceil().max(1.0);
        let cell = ((b[2] - b[0]).max(b[3] - b[1]) / n).max(1e-6);
        let cols = (((b[2] - b[0]) / cell).floor() as usize + 1).min(4096);
        let rows = (((b[3] - b[1]) / cell).floor() as usize + 1).min(4096);
        let mut cells = vec![Vec::new(); cols * rows];
        let col = |x: f64| (((x - b[0]) / cell).floor().max(0.0) as usize).min(cols - 1);
        let row = |y: f64| (((y - b[1]) / cell).floor().max(0.0) as usize).min(rows - 1);
        for (i, x) in boxes.iter().enumerate() {
            for r in row(x[1])..=row(x[3]) {
                for c in col(x[0])..=col(x[2]) {
                    cells[r * cols + c].push(i);
                }
            }
        }
        Self { origin: [b[0], b[1]], cell, cols, rows, cells }
    }

    fn query_point(&self, p: Xy) -> impl Iterator<Item = usize> + '_ {
        let c = ((p[0] - self.origin[0]) / self.cell).floor();
        let r = ((p[1] - self.origin[1]) / self.cell).floor();
        let ok = c >= 0.0 && r >= 0.0 && (c as usize) < self.cols && (r as usize) < self.rows;
        let list: &[usize] = if ok { &self.cells[r as usize * self.cols + c as usize] } else { &[] };
        list.iter().copied()
    }

    fn query_box(&self, b: [f64; 4]) -> Vec<usize> {
        let col = |x: f64| (((x - self.origin[0]) / self.cell).floor().max(0.0) as usize).min(self.cols - 1);
        let row = |y: f64| (((y - self.origin[1]) / self.cell).floor().max(0.0) as usize).min(self.rows - 1);
        let mut out: Vec<usize> = Vec::new();
        for r in row(b[1])..=row(b[3]) {
            for c in col(b[0])..=col(b[2]) {
                out.extend(&self.cells[r * self.cols + c]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

// ── B2: trace parcels from loose lines ──────────────────────────────────────

/// Regions closed off by `lines` (polylines as point chains), skipping those
/// already covered by a closed parcel; and the dangling ends to check.
pub fn trace(lines: &[Vec<Xy>], existing: &[Vec<Xy>], tol: f64) -> (Vec<Vec<Xy>>, Vec<Xy>) {
    use kernel::geom2d::arrangement::bounded_faces;
    use kernel::geom2d::curve::Line;
    let tol = tol.max(1e-6);
    let mut segments = Vec::new();
    let mut chains: Vec<&Vec<Xy>> = lines.iter().collect();
    chains.extend(existing.iter());
    for (k, chain) in chains.iter().enumerate() {
        let closed = k >= lines.len();
        let n = chain.len();
        let count = if closed { n } else { n.saturating_sub(1) };
        for i in 0..count {
            let (a, b) = (chain[i], chain[(i + 1) % n]);
            if (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-9 {
                segments.push(Line { start: a, end: b });
            }
        }
    }
    let faces = bounded_faces(&segments, kernel::geom2d::Tolerance::new(tol));
    let have: Option<MultiPolygon<f64>> = (!existing.is_empty())
        .then(|| union_all(existing.iter().map(|r| polygon(r)).collect()));
    let mut new = Vec::new();
    for face in faces {
        let p = polygon(&face);
        let a = p.unsigned_area();
        if a <= tol * tol * 10.0 {
            continue;
        }
        if let Some(have) = &have {
            let inter = MultiPolygon::new(vec![p.clone()]).intersection(have).unsigned_area();
            if inter >= a * 0.98 {
                continue;
            }
        }
        new.push(face);
    }
    // Dangling ends: chain ends that meet no other line within the tolerance.
    let mut ends = Vec::new();
    for (k, chain) in lines.iter().enumerate() {
        if chain.len() < 2 {
            continue;
        }
        for end in [chain[0], chain[chain.len() - 1]] {
            let touches = chains.iter().enumerate().any(|(j, other)| {
                let n = other.len();
                let closed = j >= lines.len();
                let count = if closed { n } else { n.saturating_sub(1) };
                (0..count).any(|i| {
                    if j == k && (i == 0 || i + 1 == count) {
                        // Its own end segment.
                        let (a, b) = (other[i], other[(i + 1) % n]);
                        let own = (a == end) || (b == end);
                        if own {
                            return false;
                        }
                    }
                    dist_to_segment(end, other[i], other[(i + 1) % n]) <= tol
                })
            });
            if !touches {
                ends.push(end);
            }
        }
    }
    (new, ends)
}

fn dist_to_segment(p: Xy, a: Xy, b: Xy) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn union_all(polys: Vec<Polygon<f64>>) -> MultiPolygon<f64> {
    // Pairwise, so large maps do not build one huge sweep at once.
    let mut layer: Vec<MultiPolygon<f64>> = polys.into_iter().map(|p| MultiPolygon::new(vec![p])).collect();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len() / 2 + 1);
        let mut it = layer.into_iter();
        while let Some(a) = it.next() {
            match it.next() {
                Some(b) => next.push(a.union(&b)),
                None => next.push(a),
            }
        }
        layer = next;
    }
    layer.pop().unwrap_or_else(|| MultiPolygon::new(vec![]))
}

// ── B3: GPMB areas ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Row {
    pub stt: usize,
    pub handle: u64,
    pub data: ParcelData,
    pub dt_thua: f64,
    pub dt_thu_hoi: f64,
    pub dt_con_lai: f64,
    pub ghi_chu: String,
    /// The taken parts.
    pub parts: Vec<Polygon<f64>>,
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub so_thua: usize,
    pub thu_hoi_trong_thua: f64,
    pub ngoai_thua: f64,
    pub tong_ranh: f64,
    pub chenh_lech: f64,
}

/// Parcel numbers in natural order: 2 before 10, numbers before the rest.
fn sort_key(s: &str) -> (u8, u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    match digits.parse::<u64>() {
        Ok(n) => (0, n, s.to_string()),
        Err(_) => (1, 0, s.to_string()),
    }
}

/// Intersect the parcels with the boundary: the rows of the table, the
/// boundary's parts in no parcel, and the boundary's area.
pub fn compute(
    parcels: &[(Parcel, Vec<Xy>)],
    boundary: &[Xy],
    min_area: f64,
    sliver_ratio: f64,
) -> (Vec<Row>, Vec<Polygon<f64>>, f64) {
    let bpoly = MultiPolygon::new(vec![polygon(boundary)]);
    let total = bpoly.unsigned_area();
    let bb = bbox(boundary);
    let boxes: Vec<[f64; 4]> = parcels.iter().map(|(_, r)| bbox(r)).collect();
    let index = GridIndex::new(&boxes);
    let mut cand = index.query_box(bb);
    cand.retain(|&i| {
        let b = boxes[i];
        b[0] <= bb[2] && b[2] >= bb[0] && b[1] <= bb[3] && b[3] >= bb[1]
    });
    cand.sort_by(|&a, &b| sort_key(&parcels[a].0.data.so_thua).cmp(&sort_key(&parcels[b].0.data.so_thua)));
    let mut rows = Vec::new();
    let mut covered: Vec<Polygon<f64>> = Vec::new();
    for i in cand {
        let (p, ring) = &parcels[i];
        let g = polygon(ring);
        let ga = g.unsigned_area();
        let inter = MultiPolygon::new(vec![g.clone()]).intersection(&bpoly);
        let a = inter.unsigned_area();
        if a <= 0.0 {
            continue;
        }
        covered.push(g);
        if a < min_area || a < ga * sliver_ratio {
            // Touches the boundary, or a digitising sliver.
            continue;
        }
        let parts: Vec<Polygon<f64>> =
            inter.0.into_iter().filter(|x| x.unsigned_area() >= min_area).collect();
        let mut note =
            if a >= ga * 0.999 { "Thu hồi toàn bộ".to_string() } else { "Thu hồi một phần".to_string() };
        if parts.len() > 1 {
            note += &format!(" ({} phần)", parts.len());
        }
        rows.push(Row {
            stt: rows.len() + 1,
            handle: p.handle,
            data: p.data.clone(),
            dt_thua: ga,
            dt_thu_hoi: a,
            dt_con_lai: (ga - a).max(0.0),
            ghi_chu: note,
            parts,
        });
    }
    let outside = if covered.is_empty() {
        bpoly
    } else {
        bpoly.difference(&union_all(covered))
    };
    let out_parts = outside.0.into_iter().filter(|x| x.unsigned_area() >= min_area).collect();
    (rows, out_parts, total)
}

pub fn summary(rows: &[Row], out_parts: &[Polygon<f64>], total: f64) -> Summary {
    let th: f64 = rows.iter().map(|r| r.dt_thu_hoi).sum();
    let ngoai: f64 = out_parts.iter().map(|p| p.unsigned_area()).sum();
    Summary {
        so_thua: rows.len(),
        thu_hoi_trong_thua: th,
        ngoai_thua: ngoai,
        tong_ranh: total,
        chenh_lech: total - th - ngoai,
    }
}

/// The table: header, one line per parcel, then the totals, the commune's
/// part and the boundary's total (CadMini's layout).
pub fn table(rows: &[Row], out_parts: &[Polygon<f64>], total: f64) -> Vec<Vec<String>> {
    let s = summary(rows, out_parts, total);
    let f1 = |v: f64| format!("{v:.1}");
    let mut out = vec![[
        "STT", "Số thửa", "Tờ BĐ", "Chủ sử dụng", "Loại đất", "DT thửa (m²)", "DT thu hồi (m²)",
        "DT còn lại (m²)", "Ghi chú",
    ]
    .map(String::from)
    .to_vec()];
    for r in rows {
        out.push(vec![
            r.stt.to_string(),
            r.data.so_thua.clone(),
            r.data.to_ban_do.clone(),
            r.data.chu.clone(),
            r.data.loai_dat.clone(),
            f1(r.dt_thua),
            f1(r.dt_thu_hoi),
            f1(r.dt_con_lai),
            r.ghi_chu.clone(),
        ]);
    }
    let e = String::new;
    out.push(vec![
        e(),
        e(),
        e(),
        format!("Tổng cộng ({} thửa)", rows.len()),
        e(),
        f1(rows.iter().map(|r| r.dt_thua).sum()),
        f1(s.thu_hoi_trong_thua),
        f1(rows.iter().map(|r| r.dt_con_lai).sum()),
        e(),
    ]);
    out.push(vec![e(), e(), e(), UBND.to_string(), e(), e(), f1(s.ngoai_thua), e(), "UBND quản lý".into()]);
    out.push(vec![e(), e(), e(), "Tổng diện tích GPMB (trong ranh)".into(), e(), e(), f1(s.tong_ranh), e(), e()]);
    out
}

/// Label height from the typical parcel size (median side / 12).
pub fn text_height(areas: impl IntoIterator<Item = f64>) -> f64 {
    let mut sizes: Vec<f64> = areas.into_iter().map(|a| a.max(1e-9).sqrt()).collect();
    if sizes.is_empty() {
        return 10.0 / 12.0;
    }
    sizes.sort_by(f64::total_cmp);
    (sizes[sizes.len() / 2] / 12.0).max(0.05)
}

/// The rings of a polygon: outer, then holes, without the closing point.
pub fn rings(p: &Polygon<f64>) -> Vec<Vec<Xy>> {
    std::iter::once(p.exterior())
        .chain(p.interiors())
        .map(|r| {
            let mut v: Vec<Xy> = r.0.iter().map(|c| [c.x, c.y]).collect();
            if v.len() > 1 && v.first() == v.last() {
                v.pop();
            }
            v
        })
        .collect()
}

// ── Saved data (XDATA, CadMini's format) ────────────────────────────────────

/// XDATA strings: "GPMB", then the JSON in pieces of 240.
pub fn data_to_xdata(d: &ParcelData) -> Vec<String> {
    let json = serde_json::to_string(d).unwrap_or_default();
    // ASCII-only JSON, as CadMini writes it (ensure_ascii).
    let ascii: String = json
        .chars()
        .map(|c| if c.is_ascii() { c.to_string() } else { escape_u(c) })
        .collect();
    let mut out = vec!["GPMB".to_string()];
    let bytes: Vec<char> = ascii.chars().collect();
    for chunk in bytes.chunks(240) {
        out.push(chunk.iter().collect());
    }
    out
}

fn escape_u(c: char) -> String {
    let mut buf = [0u16; 2];
    c.encode_utf16(&mut buf).iter().map(|u| format!("\\u{u:04x}")).collect()
}

pub fn data_from_xdata(strings: &[String]) -> Option<ParcelData> {
    if strings.first().map(String::as_str) != Some("GPMB") {
        return None;
    }
    serde_json::from_str(&strings[1..].concat()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: f64, y: f64, w: f64) -> Vec<Xy> {
        vec![[x, y], [x + w, y], [x + w, y + w], [x, y + w]]
    }

    fn label(x: f64, y: f64, t: &str) -> Label {
        Label { at: [x, y], text: t.into(), height: 1.0, tag: String::new() }
    }

    #[test]
    fn classifies_map_texts() {
        assert_eq!(classify_text("12"), TextKind::Number);
        assert_eq!(classify_text("205a"), TextKind::Number);
        assert_eq!(classify_text("356.5"), TextKind::Area);
        assert_eq!(classify_text("1200,4 m2"), TextKind::Area);
        assert_eq!(classify_text("ONT"), TextKind::Code);
        assert_eq!(classify_text("ODT+CLN"), TextKind::Code);
        assert_eq!(classify_text("Nguyễn Văn An"), TextKind::Name);
        assert_eq!(classify_text("Ô.Hùng"), TextKind::Name);
        assert_eq!(classify_text("x"), TextKind::Other);
        assert_eq!(parse_area("1200,4 m2"), Some(1200.4));
    }

    #[test]
    fn b1_gathers_texts_into_the_smallest_parcel() {
        let shapes = vec![
            ParcelShape { handle: 1, ring: sq(0.0, 0.0, 20.0), saved: None },
            ParcelShape { handle: 2, ring: sq(30.0, 0.0, 20.0), saved: None },
            // Nested inside parcel 1.
            ParcelShape { handle: 3, ring: sq(2.0, 2.0, 5.0), saved: None },
        ];
        let labels = vec![
            label(10.0, 12.0, "12"),
            label(10.0, 10.0, "Nguyễn Văn An"),
            label(10.0, 8.0, "ONT"),
            label(10.0, 6.0, "400.0"),
            label(40.0, 10.0, "13"),
            label(4.0, 4.0, "14"),
        ];
        let p = extract(&shapes, &labels);
        assert_eq!(p[0].data.so_thua, "12");
        assert_eq!(p[0].data.chu, "Nguyễn Văn An");
        assert_eq!(p[0].data.loai_dat, "ONT");
        assert_eq!(p[0].data.dt_ghi, Some(400.0));
        assert!(p[0].missing.is_empty());
        assert_eq!(p[1].data.so_thua, "13");
        assert_eq!(p[1].missing, vec!["chủ sử dụng", "loại đất"]);
        assert_eq!(p[2].data.so_thua, "14");
        assert!((p[0].area - 400.0).abs() < 1e-9);
    }

    #[test]
    fn b3_takes_the_parts_inside_the_boundary() {
        let shapes = vec![
            ParcelShape { handle: 1, ring: sq(0.0, 0.0, 10.0), saved: None },
            ParcelShape { handle: 2, ring: sq(10.0, 0.0, 10.0), saved: None },
            ParcelShape { handle: 3, ring: sq(40.0, 0.0, 10.0), saved: None },
        ];
        let labels = vec![label(5.0, 5.0, "1"), label(15.0, 5.0, "2"), label(45.0, 5.0, "3")];
        let parcels: Vec<(Parcel, Vec<Xy>)> = extract(&shapes, &labels)
            .into_iter()
            .zip(shapes.iter().map(|s| s.ring.clone()))
            .collect();
        // Boundary: x 5..25, y 0..10 → half of parcel 1, all of parcel 2,
        // and 5 × 10 of land in no parcel.
        let boundary = vec![[5.0, 0.0], [25.0, 0.0], [25.0, 10.0], [5.0, 10.0]];
        let (rows, out, total) = compute(&parcels, &boundary, 0.1, 0.002);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].data.so_thua, "1");
        assert!((rows[0].dt_thu_hoi - 50.0).abs() < 1e-6);
        assert!((rows[0].dt_con_lai - 50.0).abs() < 1e-6);
        assert_eq!(rows[0].ghi_chu, "Thu hồi một phần");
        assert!((rows[1].dt_thu_hoi - 100.0).abs() < 1e-6);
        assert_eq!(rows[1].ghi_chu, "Thu hồi toàn bộ");
        assert!((total - 200.0).abs() < 1e-6);
        let s = summary(&rows, &out, total);
        assert!((s.ngoai_thua - 50.0).abs() < 1e-6);
        assert!(s.chenh_lech.abs() < 1e-6);
        let t = table(&rows, &out, total);
        assert_eq!(t.len(), 1 + 2 + 3);
        assert_eq!(t[3][3], "Tổng cộng (2 thửa)");
    }

    #[test]
    fn b3_a_parcel_wholly_inside_leaves_a_hole_for_the_commune() {
        let shapes = vec![ParcelShape { handle: 1, ring: sq(10.0, 10.0, 10.0), saved: None }];
        let parcels: Vec<(Parcel, Vec<Xy>)> = extract(&shapes, &[])
            .into_iter()
            .zip(shapes.iter().map(|s| s.ring.clone()))
            .collect();
        let boundary = sq(0.0, 0.0, 40.0);
        let (rows, out, total) = compute(&parcels, &boundary, 0.1, 0.002);
        assert_eq!(rows.len(), 1);
        let s = summary(&rows, &out, total);
        assert!((s.ngoai_thua - (1600.0 - 100.0)).abs() < 1e-6);
        assert_eq!(rings(&out[0]).len(), 2, "the commune's land has the parcel as a hole");
    }

    #[test]
    fn b2_traces_parcels_from_loose_lines() {
        // A 10 × 10 square drawn as four separate lines, a closed parcel next
        // to it, and a dangling line.
        let lines = vec![
            vec![[0.0, 0.0], [10.0, 0.0]],
            vec![[10.0, 0.0], [10.0, 10.0]],
            vec![[10.0, 10.0], [0.0, 10.0]],
            vec![[0.0, 10.0], [0.0, 0.0]],
            vec![[30.0, 30.0], [35.0, 30.0]],
        ];
        let existing = vec![sq(10.0, 0.0, 10.0)];
        let (new, ends) = trace(&lines, &existing, 0.01);
        assert_eq!(new.len(), 1);
        assert!((ring_area(&new[0]) - 100.0).abs() < 1e-6);
        assert_eq!(ends.len(), 2);
    }

    #[test]
    fn data_round_trips_through_xdata() {
        let d = ParcelData {
            so_thua: "12".into(),
            chu: "Nguyễn Văn An".into(),
            loai_dat: "ONT".into(),
            dt_ghi: Some(400.5),
            ..Default::default()
        };
        let x = data_to_xdata(&d);
        assert_eq!(x[0], "GPMB");
        assert!(x[1..].iter().all(|s| s.is_ascii() && s.len() <= 240));
        assert_eq!(data_from_xdata(&x), Some(d));
    }

    #[test]
    fn text_height_follows_parcel_size() {
        assert!((text_height([144.0, 144.0, 10_000.0]) - 1.0).abs() < 1e-9);
    }
}
