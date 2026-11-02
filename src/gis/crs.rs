//! VN-2000 coordinate system (CadMini Studio GIS).
//!
//! - Transverse Mercator on the WGS84 ellipsoid: 3° zones (k0 = 0.9999) for
//!   provincial maps, 6° zones (k0 = 0.9996); false easting 500 000 m.
//! - To WGS84 with the 7 parameters of Decision 05/2007/QĐ-BTNMT
//!   (position-vector convention, as PROJ's `+towgs84`).
//! - Central meridians of the 34 provinces after 1 July 2025 (Circular
//!   24/2025/TT-BNNMT) and of the 63 former provinces (Circular 973/2001).
//!
//! The drawing keeps its system in the custom property `CADMINI_CRS`
//! (`VN2000;<lon0>;<zone>;<name>`), the same text CadMini writes.

/// Custom drawing property holding the system.
pub const HEADER_KEY: &str = "CADMINI_CRS";

/// VN-2000 → WGS84: tx, ty, tz (m), rx, ry, rz (arc seconds), scale (ppm).
pub const TOWGS84: [f64; 7] = [
    -191.904_414_29,
    -39.303_182_79,
    -111.450_328_35,
    -0.009_288_36,
    0.019_754_79,
    -0.004_273_72,
    0.252_906_278,
];

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;

/// Province, central meridian degrees, minutes, merged from.
pub const PROVINCES_2025: &[(&str, u32, u32, &str)] = &[
    ("An Giang", 104, 45, "An Giang + Kiên Giang"),
    ("Bắc Ninh", 107, 0, "Bắc Ninh + Bắc Giang"),
    ("Cà Mau", 104, 30, "Cà Mau + Bạc Liêu"),
    ("Cao Bằng", 105, 45, ""),
    ("Đắk Lắk", 108, 30, "Đắk Lắk + Phú Yên"),
    ("Điện Biên", 103, 0, ""),
    ("Đồng Nai", 107, 45, "Đồng Nai + Bình Phước"),
    ("Đồng Tháp", 105, 0, "Đồng Tháp + Tiền Giang"),
    ("Gia Lai", 108, 15, "Gia Lai + Bình Định"),
    ("Hà Tĩnh", 105, 30, ""),
    ("Hưng Yên", 105, 30, "Hưng Yên + Thái Bình"),
    ("Khánh Hòa", 108, 15, "Khánh Hòa + Ninh Thuận"),
    ("Lai Châu", 104, 45, ""),
    ("Lạng Sơn", 107, 15, ""),
    ("Lào Cai", 104, 45, "Lào Cai + Yên Bái"),
    ("Lâm Đồng", 107, 45, "Lâm Đồng + Đắk Nông + Bình Thuận"),
    ("Nghệ An", 104, 45, ""),
    ("Ninh Bình", 105, 0, "Ninh Bình + Hà Nam + Nam Định"),
    ("Phú Thọ", 104, 45, "Phú Thọ + Vĩnh Phúc + Hòa Bình"),
    ("Quảng Ngãi", 108, 0, "Quảng Ngãi + Kon Tum"),
    ("Quảng Ninh", 107, 45, ""),
    ("Quảng Trị", 106, 0, "Quảng Trị + Quảng Bình"),
    ("Sơn La", 104, 0, ""),
    ("Tây Ninh", 105, 45, "Tây Ninh + Long An"),
    ("Thái Nguyên", 106, 30, "Thái Nguyên + Bắc Kạn"),
    ("Thanh Hóa", 105, 0, ""),
    ("TP Cần Thơ", 105, 0, "Cần Thơ + Sóc Trăng + Hậu Giang"),
    ("TP Đà Nẵng", 107, 45, "Đà Nẵng + Quảng Nam"),
    ("TP Hà Nội", 105, 0, ""),
    ("TP Hải Phòng", 105, 45, "Hải Phòng + Hải Dương"),
    ("TP Hồ Chí Minh", 105, 45, "TP HCM + Bà Rịa - Vũng Tàu + Bình Dương"),
    ("TP Huế", 107, 0, ""),
    ("Tuyên Quang", 106, 0, "Tuyên Quang + Hà Giang"),
    ("Vĩnh Long", 105, 30, "Vĩnh Long + Bến Tre + Trà Vinh"),
];

/// The 63 provinces before 1 July 2025.
pub const PROVINCES_OLD: &[(&str, u32, u32)] = &[
    ("An Giang", 104, 45), ("Bà Rịa - Vũng Tàu", 107, 45), ("Bạc Liêu", 105, 0),
    ("Bắc Giang", 107, 0), ("Bắc Kạn", 106, 30), ("Bắc Ninh", 105, 30), ("Bến Tre", 105, 45),
    ("Bình Dương", 105, 45), ("Bình Định", 108, 15), ("Bình Phước", 106, 15),
    ("Bình Thuận", 108, 30), ("Cà Mau", 104, 30), ("Cao Bằng", 105, 45), ("Cần Thơ", 105, 0),
    ("Đà Nẵng", 107, 45), ("Đắk Lắk", 108, 30), ("Đắk Nông", 108, 30), ("Điện Biên", 103, 0),
    ("Đồng Nai", 107, 45), ("Đồng Tháp", 105, 0), ("Gia Lai", 108, 30), ("Hà Giang", 105, 30),
    ("Hà Nam", 105, 0), ("Hà Nội", 105, 0), ("Hà Tĩnh", 105, 30), ("Hải Dương", 105, 30),
    ("Hải Phòng", 105, 45), ("Hậu Giang", 105, 0), ("Hòa Bình", 106, 0), ("Hưng Yên", 105, 30),
    ("Khánh Hòa", 108, 15), ("Kiên Giang", 104, 30), ("Kon Tum", 107, 30), ("Lai Châu", 104, 45),
    ("Lạng Sơn", 107, 15), ("Lào Cai", 104, 45), ("Lâm Đồng", 107, 45), ("Long An", 105, 45),
    ("Nam Định", 105, 30), ("Nghệ An", 104, 45), ("Ninh Bình", 105, 0), ("Ninh Thuận", 108, 15),
    ("Phú Thọ", 104, 45), ("Phú Yên", 108, 30), ("Quảng Bình", 106, 0), ("Quảng Nam", 107, 45),
    ("Quảng Ngãi", 108, 0), ("Quảng Ninh", 107, 45), ("Quảng Trị", 106, 15),
    ("Sóc Trăng", 105, 30), ("Sơn La", 104, 0), ("Tây Ninh", 105, 30), ("Thái Bình", 105, 30),
    ("Thái Nguyên", 106, 30), ("Thanh Hóa", 105, 0), ("Thừa Thiên Huế", 107, 0),
    ("Tiền Giang", 105, 45), ("TP Hồ Chí Minh", 105, 45), ("Trà Vinh", 105, 30),
    ("Tuyên Quang", 106, 0), ("Vĩnh Long", 105, 30), ("Vĩnh Phúc", 105, 0), ("Yên Bái", 104, 45),
];

/// A drawing's VN-2000 system.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingCrs {
    /// Central meridian, degrees.
    pub lon0: f64,
    /// Zone width: 3 or 6 degrees.
    pub zone: u8,
    /// Province or note.
    pub name: String,
}

impl Default for DrawingCrs {
    fn default() -> Self {
        Self { lon0: 105.5, zone: 3, name: String::new() }
    }
}

impl DrawingCrs {
    pub fn k0(&self) -> f64 {
        if self.zone == 3 { 0.9999 } else { 0.9996 }
    }

    pub fn label(&self) -> String {
        let d = self.lon0.trunc();
        let m = ((self.lon0 - d) * 60.0).round();
        let name = if self.name.is_empty() { String::new() } else { format!("{} ", self.name) };
        format!("VN-2000 {name}KTT {d}°{m:02}′ múi {}°", self.zone)
    }

    pub fn proj4(&self) -> String {
        let t = TOWGS84.map(|v| format!("{v}")).join(",");
        format!(
            "+proj=tmerc +lat_0=0 +lon_0={} +k={} +x_0=500000 +y_0=0 +ellps=WGS84 +towgs84={t} +units=m +no_defs",
            self.lon0,
            self.k0()
        )
    }

    /// ESRI WKT for a shapefile's `.prj`.
    pub fn wkt_esri(&self) -> String {
        format!(
            "PROJCS[\"VN_2000_KTT_{lon}_{zone}deg\",GEOGCS[\"GCS_VN_2000\",DATUM[\"D_Vietnam_2000\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]],PROJECTION[\"Transverse_Mercator\"],PARAMETER[\"False_Easting\",500000.0],PARAMETER[\"False_Northing\",0.0],PARAMETER[\"Central_Meridian\",{lon}],PARAMETER[\"Scale_Factor\",{k}],PARAMETER[\"Latitude_Of_Origin\",0.0],UNIT[\"Meter\",1.0]]",
            lon = self.lon0,
            zone = self.zone,
            k = self.k0()
        )
    }

    pub fn to_text(&self) -> String {
        format!("VN2000;{};{};{}", self.lon0, self.zone, self.name)
    }

    pub fn from_text(text: &str) -> Option<Self> {
        let mut parts = text.splitn(4, ';');
        if parts.next()? != "VN2000" {
            return None;
        }
        let lon0: f64 = parts.next()?.trim().parse().ok()?;
        let zone: u8 = parts.next()?.trim().parse().ok()?;
        let name = parts.next().unwrap_or("").to_string();
        (lon0.is_finite() && (zone == 3 || zone == 6)).then_some(Self { lon0, zone, name })
    }

    /// Parse what the user types at GISCRS: a province name (new or former),
    /// or a central meridian such as `105.5`, `105 30`, `105°30'`, with an
    /// optional zone (`/6` or ` 6d`).
    pub fn parse_user(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let folded = fold(text);
        if let Some(&(name, d, m, _)) =
            PROVINCES_2025.iter().find(|(n, ..)| fold(n) == folded || fold(n).ends_with(&folded) && folded.len() > 3)
        {
            return Some(Self { lon0: d as f64 + m as f64 / 60.0, zone: 3, name: name.to_string() });
        }
        if let Some(&(name, d, m)) = PROVINCES_OLD.iter().find(|(n, ..)| fold(n) == folded) {
            return Some(Self {
                lon0: d as f64 + m as f64 / 60.0,
                zone: 3,
                name: format!("{name} (cũ)"),
            });
        }
        let (meridian, zone) = match text.split_once('/') {
            Some((a, z)) => (a, z.trim().trim_end_matches(['d', 'D', '°']).parse::<u8>().ok()?),
            None => (text, 3),
        };
        let nums: Vec<f64> = meridian
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        let lon0 = match nums.as_slice() {
            [d] => *d,
            [d, m] => d + m / 60.0,
            [d, m, s] => d + m / 60.0 + s / 3600.0,
            _ => return None,
        };
        ((100.0..=120.0).contains(&lon0) && (zone == 3 || zone == 6))
            .then_some(Self { lon0, zone, name: String::new() })
    }

    /// (lon, lat) WGS84 degrees from VN-2000 grid metres.
    pub fn to_wgs84(&self, x: f64, y: f64) -> (f64, f64) {
        let (lon, lat) = tm_inverse(x, y, self.lon0, self.k0());
        let p = geodetic_to_ecef(lon, lat, 0.0);
        let q = helmert(p, false);
        let (lon, lat, _) = ecef_to_geodetic(q);
        (lon, lat)
    }

    /// VN-2000 grid metres from WGS84 (lon, lat) degrees.
    pub fn from_wgs84(&self, lon: f64, lat: f64) -> (f64, f64) {
        let p = geodetic_to_ecef(lon, lat, 0.0);
        let q = helmert(p, true);
        let (lon, lat, _) = ecef_to_geodetic(q);
        tm_forward(lon, lat, self.lon0, self.k0())
    }
}

/// Lower-case, without Vietnamese marks, for loose name matching.
pub fn fold(s: &str) -> String {
    const FROM: &str = "àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ";
    const TO: &str = "aaaaaaaaaaaaaaaaaeeeeeeeeeeeiiiiiooooooooooooooooouuuuuuuuuuuyyyyyd";
    let map: Vec<(char, char)> = FROM.chars().zip(TO.chars()).collect();
    s.to_lowercase()
        .chars()
        .map(|c| map.iter().find(|(f, _)| *f == c).map_or(c, |(_, t)| *t))
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .trim_start_matches("tp")
        .to_string()
}

/// `18°21′09.04″N`.
pub fn dms(v: f64, pos: char, neg: char) -> String {
    let h = if v >= 0.0 { pos } else { neg };
    let v = v.abs();
    let d = v.trunc();
    let m = ((v - d) * 60.0).trunc();
    let s = (v - d - m / 60.0) * 3600.0;
    format!("{d}°{m:02}′{s:05.2}″{h}")
}

/// Parse `lat, lon` in decimal degrees (also `lat lon`). Swapped when the
/// first number cannot be a latitude.
pub fn parse_latlon(text: &str) -> Option<(f64, f64)> {
    let nums: Vec<f64> = text
        .split([',', ';', ' ', '\t'])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let [a, b] = nums.as_slice() else { return None };
    let (lat, lon) = if a.abs() > 90.0 { (*b, *a) } else { (*a, *b) };
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

// ── Transverse Mercator, Krüger series to n⁶ (Karney 2011) ──────────────────

struct TmConsts {
    a_hat: f64,
    alpha: [f64; 6],
    beta: [f64; 6],
    e: f64,
}

fn tm_consts() -> &'static TmConsts {
    static C: std::sync::OnceLock<TmConsts> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let n = F / (2.0 - F);
        let (n2, n3, n4, n5, n6) = (n * n, n.powi(3), n.powi(4), n.powi(5), n.powi(6));
        let a_hat = A / (1.0 + n) * (1.0 + n2 / 4.0 + n4 / 64.0 + n6 / 256.0);
        let alpha = [
            n / 2.0 - 2.0 * n2 / 3.0 + 5.0 * n3 / 16.0 + 41.0 * n4 / 180.0 - 127.0 * n5 / 288.0
                + 7891.0 * n6 / 37800.0,
            13.0 * n2 / 48.0 - 3.0 * n3 / 5.0 + 557.0 * n4 / 1440.0 + 281.0 * n5 / 630.0
                - 1983433.0 * n6 / 1935360.0,
            61.0 * n3 / 240.0 - 103.0 * n4 / 140.0 + 15061.0 * n5 / 26880.0
                + 167603.0 * n6 / 181440.0,
            49561.0 * n4 / 161280.0 - 179.0 * n5 / 168.0 + 6601661.0 * n6 / 7257600.0,
            34729.0 * n5 / 80640.0 - 3418889.0 * n6 / 1995840.0,
            212378941.0 * n6 / 319334400.0,
        ];
        let beta = [
            n / 2.0 - 2.0 * n2 / 3.0 + 37.0 * n3 / 96.0 - n4 / 360.0 - 81.0 * n5 / 512.0
                + 96199.0 * n6 / 604800.0,
            n2 / 48.0 + n3 / 15.0 - 437.0 * n4 / 1440.0 + 46.0 * n5 / 105.0
                - 1118711.0 * n6 / 3870720.0,
            17.0 * n3 / 480.0 - 37.0 * n4 / 840.0 - 209.0 * n5 / 4480.0 + 5569.0 * n6 / 90720.0,
            4397.0 * n4 / 161280.0 - 11.0 * n5 / 504.0 - 830251.0 * n6 / 7257600.0,
            4583.0 * n5 / 161280.0 - 108847.0 * n6 / 3991680.0,
            20648693.0 * n6 / 638668800.0,
        ];
        TmConsts { a_hat, alpha, beta, e: (F * (2.0 - F)).sqrt() }
    })
}

fn taupf(tau: f64, e: f64) -> f64 {
    let tau1 = tau.hypot(1.0);
    let sig = (e * (e * tau / tau1).atanh()).sinh();
    sig.hypot(1.0) * tau - sig * tau1
}

fn tauf(taup: f64, e: f64) -> f64 {
    let e2m = 1.0 - e * e;
    let mut tau = taup / e2m;
    for _ in 0..8 {
        let taupa = taupf(tau, e);
        let dtau = (taup - taupa) * (1.0 + e2m * tau * tau)
            / (e2m * tau.hypot(1.0) * taupa.hypot(1.0));
        tau += dtau;
        if dtau.abs() < 1e-14 * tau.abs().max(1.0) {
            break;
        }
    }
    tau
}

/// Grid (x east, y north) metres from (lon, lat) degrees.
pub fn tm_forward(lon: f64, lat: f64, lon0: f64, k0: f64) -> (f64, f64) {
    let c = tm_consts();
    let phi = lat.to_radians();
    let lam = (lon - lon0).to_radians();
    let t = taupf(phi.tan(), c.e);
    let xi_p = t.atan2(lam.cos());
    let eta_p = (lam.sin() / t.hypot(1.0)).atanh();
    let (mut xi, mut eta) = (xi_p, eta_p);
    for (j, a) in c.alpha.iter().enumerate() {
        let k = 2.0 * (j + 1) as f64;
        xi += a * (k * xi_p).sin() * (k * eta_p).cosh();
        eta += a * (k * xi_p).cos() * (k * eta_p).sinh();
    }
    (500_000.0 + k0 * c.a_hat * eta, k0 * c.a_hat * xi)
}

/// (lon, lat) degrees from grid metres.
pub fn tm_inverse(x: f64, y: f64, lon0: f64, k0: f64) -> (f64, f64) {
    let c = tm_consts();
    let xi = y / (k0 * c.a_hat);
    let eta = (x - 500_000.0) / (k0 * c.a_hat);
    let (mut xi_p, mut eta_p) = (xi, eta);
    for (j, b) in c.beta.iter().enumerate() {
        let k = 2.0 * (j + 1) as f64;
        xi_p -= b * (k * xi).sin() * (k * eta).cosh();
        eta_p -= b * (k * xi).cos() * (k * eta).sinh();
    }
    let taup = xi_p.sin() / eta_p.sinh().hypot(xi_p.cos());
    let lam = eta_p.sinh().atan2(xi_p.cos());
    let phi = tauf(taup, c.e).atan();
    (lon0 + lam.to_degrees(), phi.to_degrees())
}

// ── Datum shift ─────────────────────────────────────────────────────────────

fn geodetic_to_ecef(lon: f64, lat: f64, h: f64) -> [f64; 3] {
    let e2 = F * (2.0 - F);
    let (phi, lam) = (lat.to_radians(), lon.to_radians());
    let n = A / (1.0 - e2 * phi.sin().powi(2)).sqrt();
    [
        (n + h) * phi.cos() * lam.cos(),
        (n + h) * phi.cos() * lam.sin(),
        (n * (1.0 - e2) + h) * phi.sin(),
    ]
}

fn ecef_to_geodetic(p: [f64; 3]) -> (f64, f64, f64) {
    let e2 = F * (2.0 - F);
    let [x, y, z] = p;
    let lam = y.atan2(x);
    let r = x.hypot(y);
    let mut phi = z.atan2(r * (1.0 - e2));
    let mut h = 0.0;
    for _ in 0..10 {
        let n = A / (1.0 - e2 * phi.sin().powi(2)).sqrt();
        h = r / phi.cos() - n;
        let next = z.atan2(r * (1.0 - e2 * n / (n + h)));
        if (next - phi).abs() < 1e-15 {
            phi = next;
            break;
        }
        phi = next;
    }
    (lam.to_degrees(), phi.to_degrees(), h)
}

/// VN-2000 → WGS84 (`inverse` false) or back, position-vector convention.
fn helmert(p: [f64; 3], inverse: bool) -> [f64; 3] {
    let sec = std::f64::consts::PI / (180.0 * 3600.0);
    let [tx, ty, tz, rx, ry, rz, s] = TOWGS84;
    let (rx, ry, rz) = (rx * sec, ry * sec, rz * sec);
    let m = 1.0 + s * 1e-6;
    let [x, y, z] = p;
    if !inverse {
        [
            tx + m * (x - rz * y + ry * z),
            ty + m * (rz * x + y - rx * z),
            tz + m * (-ry * x + rx * y + z),
        ]
    } else {
        let (x, y, z) = ((x - tx) / m, (y - ty) / m, (z - tz) / m);
        // The transpose of the small rotation.
        [x + rz * y - ry * z, -rz * x + y + rx * z, ry * x - rx * y + z]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values from PROJ (pyproj) with the same +towgs84.
    #[test]
    fn grid_to_wgs84_matches_proj() {
        let c = DrawingCrs { lon0: 105.5, zone: 3, name: String::new() };
        for (x, y, lon, lat) in [
            (560000.0, 2030000.0, 106.069_581_299_500_65, 18.352_510_809_686_887),
            (480000.0, 1980000.0, 105.313_077_961_420_65, 17.901_480_311_930_392),
            (500000.0, 2000000.0, 105.501_843_055_131_69, 18.082_289_379_107_36),
        ] {
            let (a, b) = c.to_wgs84(x, y);
            assert!((a - lon).abs() < 1e-8 && (b - lat).abs() < 1e-8, "{a} {b}");
        }
        let c = DrawingCrs { lon0: 105.0, zone: 6, name: String::new() };
        let (a, b) = c.to_wgs84(600000.0, 1200000.0);
        assert!((a - 105.916_684_160_749_82).abs() < 1e-8);
        assert!((b - 10.853_105_607_573_237).abs() < 1e-8);
    }

    #[test]
    fn wgs84_to_grid_matches_proj() {
        let c = DrawingCrs { lon0: 107.75, zone: 3, name: String::new() };
        let (x, y) = c.from_wgs84(107.9, 16.05);
        assert!((x - 515_855.122_589_538_9).abs() < 1e-3, "{x}");
        assert!((y - 1_775_117.481_067_723).abs() < 1e-3, "{y}");
    }

    #[test]
    fn round_trip() {
        let c = DrawingCrs::default();
        let (lon, lat) = c.to_wgs84(543_210.0, 2_012_345.0);
        let (x, y) = c.from_wgs84(lon, lat);
        // PROJ itself comes back within a millimetre: the datum shift
        // drops the ellipsoidal height both ways.
        assert!((x - 543_210.0).abs() < 2e-3 && (y - 2_012_345.0).abs() < 2e-3, "{x} {y}");
    }

    #[test]
    fn parses_user_input() {
        let ht = DrawingCrs::parse_user("ha tinh").unwrap();
        assert_eq!(ht.lon0, 105.5);
        assert_eq!(ht.name, "Hà Tĩnh");
        assert_eq!(DrawingCrs::parse_user("Đà Nẵng").unwrap().lon0, 107.75);
        assert_eq!(DrawingCrs::parse_user("105 45").unwrap().lon0, 105.75);
        let z6 = DrawingCrs::parse_user("105/6").unwrap();
        assert_eq!((z6.lon0, z6.zone), (105.0, 6));
        assert!(DrawingCrs::parse_user("xyz").is_none());
        let text = ht.to_text();
        assert_eq!(DrawingCrs::from_text(&text), Some(ht));
    }

    #[test]
    fn latlon_and_dms() {
        assert_eq!(parse_latlon("18.35, 106.07"), Some((18.35, 106.07)));
        assert_eq!(parse_latlon("106.07 18.35"), Some((18.35, 106.07)));
        assert_eq!(dms(18.5, 'N', 'S'), "18°30′00.00″N");
    }
}
