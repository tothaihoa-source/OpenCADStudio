//! GIS file formats (CadMini Studio): GeoJSON, KML/KMZ and ESRI Shapefile.
//!
//! Readers return [`Feature`]s in the file's own coordinates and say which
//! kind those are; writers take features already in the coordinates the
//! format wants (lon/lat for GeoJSON and KML, grid metres for Shapefile).

use std::io::{Read, Write};

pub type Xy = [f64; 2];

#[derive(Clone, Debug, PartialEq)]
pub enum Geom {
    Point(Xy),
    Line(Vec<Xy>),
    /// Outer ring first, then holes. Rings are open (no repeated first point).
    Polygon(Vec<Vec<Xy>>),
}

impl Geom {
    pub fn kind(&self) -> &'static str {
        match self {
            Geom::Point(_) => "Point",
            Geom::Line(_) => "LineString",
            Geom::Polygon(_) => "Polygon",
        }
    }

    pub fn map(&self, f: &impl Fn(Xy) -> Xy) -> Geom {
        match self {
            Geom::Point(p) => Geom::Point(f(*p)),
            Geom::Line(l) => Geom::Line(l.iter().map(|p| f(*p)).collect()),
            Geom::Polygon(r) => {
                Geom::Polygon(r.iter().map(|ring| ring.iter().map(|p| f(*p)).collect()).collect())
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feature {
    pub geom: Option<Geom>,
    /// Target layer (from the file name, a KML folder or a `layer` property).
    pub layer: String,
    pub props: Vec<(String, String)>,
    pub color: Option<[u8; 3]>,
}

impl Feature {
    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// A label for the feature: its name-like property.
    pub fn name(&self) -> Option<&str> {
        ["name", "ten", "tên", "label", "text", "NAME", "TEN"]
            .iter()
            .find_map(|k| self.prop(k))
            .filter(|s| !s.trim().is_empty())
    }
}

/// What a file's coordinates are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coords {
    /// WGS84 longitude, latitude.
    LonLat,
    /// Projected metres (taken as the drawing's VN-2000).
    Grid,
}

/// A layer name the drawing accepts.
pub fn layer_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if "<>/\\\":;?*|=`,".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() { "GIS".to_string() } else { cleaned.chars().take(255).collect() }
}

fn open_ring(mut r: Vec<Xy>) -> Vec<Xy> {
    if r.len() > 1 && r.first() == r.last() {
        r.pop();
    }
    r
}

fn signed_area(r: &[Xy]) -> f64 {
    let n = r.len();
    (0..n).map(|i| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0
}

// ── GeoJSON ─────────────────────────────────────────────────────────────────

fn json_xy(v: &serde_json::Value) -> Option<Xy> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
}

fn json_line(v: &serde_json::Value) -> Vec<Xy> {
    v.as_array().map(|a| a.iter().filter_map(json_xy).collect()).unwrap_or_default()
}

fn json_poly(v: &serde_json::Value) -> Vec<Vec<Xy>> {
    v.as_array()
        .map(|a| a.iter().map(|r| open_ring(json_line(r))).filter(|r| r.len() >= 3).collect())
        .unwrap_or_default()
}

fn json_geoms(g: &serde_json::Value, out: &mut Vec<Geom>) {
    let c = &g["coordinates"];
    match g["type"].as_str().unwrap_or("") {
        "Point" => out.extend(json_xy(c).map(Geom::Point)),
        "MultiPoint" => out.extend(json_line(c).into_iter().map(Geom::Point)),
        "LineString" => out.push(Geom::Line(json_line(c))),
        "MultiLineString" => {
            out.extend(c.as_array().into_iter().flatten().map(|l| Geom::Line(json_line(l))))
        }
        "Polygon" => out.push(Geom::Polygon(json_poly(c))),
        "MultiPolygon" => {
            out.extend(c.as_array().into_iter().flatten().map(|p| Geom::Polygon(json_poly(p))))
        }
        "GeometryCollection" => {
            for g in g["geometries"].as_array().into_iter().flatten() {
                json_geoms(g, out);
            }
        }
        _ => {}
    }
}

fn json_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// GeoJSON features. Coordinates are lon/lat unless the file names a
/// projected `crs` (old GeoJSON) or holds numbers no longitude can be.
pub fn read_geojson(text: &str, layer: &str) -> Result<(Vec<Feature>, Coords), String> {
    let root: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let items: Vec<&serde_json::Value> = match root["type"].as_str() {
        Some("FeatureCollection") => root["features"].as_array().into_iter().flatten().collect(),
        Some("Feature") => vec![&root],
        Some(_) => vec![],
        None => return Err("not a GeoJSON file".into()),
    };
    let mut out = Vec::new();
    let bare;
    let items = if items.is_empty() && root["coordinates"].is_array() {
        bare = serde_json::json!({ "type": "Feature", "geometry": root.clone(), "properties": {} });
        vec![&bare]
    } else {
        items
    };
    for f in items {
        let props: Vec<(String, String)> = f["properties"]
            .as_object()
            .map(|o| o.iter().map(|(k, v)| (k.clone(), json_text(v))).collect())
            .unwrap_or_default();
        let lyr = props
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("layer"))
            .map(|(_, v)| layer_name(v))
            .unwrap_or_else(|| layer.to_string());
        let mut geoms = Vec::new();
        json_geoms(&f["geometry"], &mut geoms);
        for g in geoms {
            out.push(Feature { geom: Some(g), layer: lyr.clone(), props: props.clone(), color: None });
        }
    }
    let projected_crs = root["crs"]["properties"]["name"]
        .as_str()
        .is_some_and(|n| !n.contains("4326") && !n.contains("CRS84"));
    let big = out.iter().any(|f| match &f.geom {
        Some(Geom::Point(p)) => p[0].abs() > 360.0 || p[1].abs() > 90.0,
        Some(Geom::Line(l)) => l.iter().any(|p| p[0].abs() > 360.0),
        Some(Geom::Polygon(r)) => r.iter().flatten().any(|p| p[0].abs() > 360.0),
        None => false,
    });
    Ok((out, if projected_crs || big { Coords::Grid } else { Coords::LonLat }))
}

fn fmt_xy(p: Xy) -> String {
    format!("[{:.9},{:.9}]", p[0], p[1])
}

/// GeoJSON (RFC 7946) from lon/lat features: outer rings counter-clockwise.
pub fn write_geojson(features: &[Feature]) -> String {
    let mut parts = Vec::with_capacity(features.len());
    for f in features {
        let Some(geom) = &f.geom else { continue };
        let coords = match geom {
            Geom::Point(p) => fmt_xy(*p),
            Geom::Line(l) => format!("[{}]", l.iter().map(|p| fmt_xy(*p)).collect::<Vec<_>>().join(",")),
            Geom::Polygon(rings) => {
                let rs: Vec<String> = rings
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let mut r = r.clone();
                        let ccw = signed_area(&r) > 0.0;
                        if (i == 0) != ccw {
                            r.reverse();
                        }
                        if let Some(first) = r.first().copied() {
                            r.push(first);
                        }
                        format!("[{}]", r.iter().map(|p| fmt_xy(*p)).collect::<Vec<_>>().join(","))
                    })
                    .collect();
                format!("[{}]", rs.join(","))
            }
        };
        let mut props = serde_json::Map::new();
        props.insert("layer".into(), serde_json::Value::String(f.layer.clone()));
        for (k, v) in &f.props {
            props.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        parts.push(format!(
            "{{\"type\":\"Feature\",\"properties\":{},\"geometry\":{{\"type\":\"{}\",\"coordinates\":{}}}}}",
            serde_json::Value::Object(props),
            geom.kind(),
            coords
        ));
    }
    format!("{{\"type\":\"FeatureCollection\",\"features\":[\n{}\n]}}\n", parts.join(",\n"))
}

// ── KML / KMZ ───────────────────────────────────────────────────────────────

fn kml_coords(text: &str) -> Vec<Xy> {
    text.split_whitespace()
        .filter_map(|tok| {
            let mut it = tok.split(',');
            Some([it.next()?.parse().ok()?, it.next()?.parse().ok()?])
        })
        .collect()
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.tag_name().name() == name)
}

fn child_text(n: roxmltree::Node, name: &str) -> String {
    child(n, name).and_then(|c| c.text()).unwrap_or("").trim().to_string()
}

fn kml_geoms(n: roxmltree::Node, out: &mut Vec<Geom>) {
    for g in n.children().filter(|c| c.is_element()) {
        match g.tag_name().name() {
            "Point" => {
                if let Some(p) = kml_coords(&child_text(g, "coordinates")).first() {
                    out.push(Geom::Point(*p));
                }
            }
            "LineString" | "LinearRing" => {
                out.push(Geom::Line(kml_coords(&child_text(g, "coordinates"))))
            }
            "Polygon" => {
                let ring = |b: roxmltree::Node| {
                    child(b, "LinearRing")
                        .map(|r| open_ring(kml_coords(&child_text(r, "coordinates"))))
                        .unwrap_or_default()
                };
                let mut rings = Vec::new();
                if let Some(o) = child(g, "outerBoundaryIs") {
                    rings.push(ring(o));
                }
                for i in g.children().filter(|c| c.tag_name().name() == "innerBoundaryIs") {
                    rings.push(ring(i));
                }
                rings.retain(|r| r.len() >= 3);
                if !rings.is_empty() {
                    out.push(Geom::Polygon(rings));
                }
            }
            "MultiGeometry" => kml_geoms(g, out),
            _ => {}
        }
    }
}

/// `aabbggrr` → rgb.
fn kml_rgb(text: &str) -> Option<[u8; 3]> {
    let t = text.trim().trim_start_matches('#');
    if t.len() != 8 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some([byte(6)?, byte(4)?, byte(2)?])
}

/// KML placemarks; each takes the name of the folder it is in as its layer.
pub fn read_kml(text: &str, layer: &str) -> Result<Vec<Feature>, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    // Style id → line colour.
    let mut styles = std::collections::HashMap::new();
    for s in doc.descendants().filter(|n| n.tag_name().name() == "Style") {
        if let (Some(id), Some(c)) = (
            s.attribute("id"),
            child(s, "LineStyle").map(|l| child_text(l, "color")).and_then(|c| kml_rgb(&c)),
        ) {
            styles.insert(format!("#{id}"), c);
        }
    }
    let mut out = Vec::new();
    for pm in doc.descendants().filter(|n| n.tag_name().name() == "Placemark") {
        let folder = pm
            .ancestors()
            .skip(1)
            .find(|a| a.tag_name().name() == "Folder")
            .map(|f| child_text(f, "name"))
            .filter(|s| !s.is_empty());
        let mut props = Vec::new();
        let name = child_text(pm, "name");
        if !name.is_empty() {
            props.push(("name".to_string(), name));
        }
        let desc = child_text(pm, "description");
        if !desc.is_empty() {
            props.push(("description".to_string(), desc));
        }
        for d in pm.descendants().filter(|n| matches!(n.tag_name().name(), "Data" | "SimpleData")) {
            let key = d.attribute("name").unwrap_or("").to_string();
            let value = child(d, "value").and_then(|v| v.text()).or(d.text()).unwrap_or("").trim();
            if !key.is_empty() {
                props.push((key, value.to_string()));
            }
        }
        let color = child(pm, "Style")
            .and_then(|s| child(s, "LineStyle"))
            .and_then(|l| kml_rgb(&child_text(l, "color")))
            .or_else(|| styles.get(&child_text(pm, "styleUrl")).copied());
        let lyr = layer_name(folder.as_deref().unwrap_or(layer));
        let mut geoms = Vec::new();
        kml_geoms(pm, &mut geoms);
        for g in geoms {
            out.push(Feature { geom: Some(g), layer: lyr.clone(), props: props.clone(), color });
        }
    }
    Ok(out)
}

/// The `doc.kml` inside a KMZ.
pub fn kmz_text(bytes: &[u8]) -> Result<String, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let name = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .find(|n| n.to_lowercase().ends_with(".kml"))
        .ok_or("no .kml in the KMZ")?;
    let mut text = String::new();
    zip.by_name(&name).map_err(|e| e.to_string())?.read_to_string(&mut text).map_err(|e| e.to_string())?;
    Ok(text)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn kml_ring(r: &[Xy], close: bool) -> String {
    let mut pts: Vec<String> = r.iter().map(|p| format!("{:.9},{:.9},0", p[0], p[1])).collect();
    if close && r.len() > 1 && r.first() != r.last() {
        pts.push(format!("{:.9},{:.9},0", r[0][0], r[0][1]));
    }
    pts.join(" ")
}

/// KML from lon/lat features, one folder per layer.
pub fn write_kml(features: &[Feature], title: &str) -> String {
    let mut by_layer: std::collections::BTreeMap<&str, Vec<&Feature>> = Default::default();
    for f in features {
        by_layer.entry(f.layer.as_str()).or_default().push(f);
    }
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n");
    s.push_str(&format!("<name>{}</name>\n", xml_escape(title)));
    for (layer, list) in by_layer {
        s.push_str(&format!("<Folder><name>{}</name>\n", xml_escape(layer)));
        for f in list {
            let Some(g) = &f.geom else { continue };
            s.push_str("<Placemark>");
            if let Some(n) = f.name() {
                s.push_str(&format!("<name>{}</name>", xml_escape(n)));
            }
            if let Some([r, g, b]) = f.color {
                s.push_str(&format!(
                    "<Style><LineStyle><color>ff{b:02x}{g:02x}{r:02x}</color><width>2</width></LineStyle><PolyStyle><color>40{b:02x}{g:02x}{r:02x}</color></PolyStyle></Style>"
                ));
            }
            if !f.props.is_empty() {
                s.push_str("<ExtendedData>");
                for (k, v) in &f.props {
                    s.push_str(&format!(
                        "<Data name=\"{}\"><value>{}</value></Data>",
                        xml_escape(k),
                        xml_escape(v)
                    ));
                }
                s.push_str("</ExtendedData>");
            }
            match g {
                Geom::Point(p) => s.push_str(&format!(
                    "<Point><coordinates>{:.9},{:.9},0</coordinates></Point>",
                    p[0], p[1]
                )),
                Geom::Line(l) => s.push_str(&format!(
                    "<LineString><tessellate>1</tessellate><coordinates>{}</coordinates></LineString>",
                    kml_ring(l, false)
                )),
                Geom::Polygon(rings) => {
                    s.push_str("<Polygon>");
                    for (i, r) in rings.iter().enumerate() {
                        let tag = if i == 0 { "outerBoundaryIs" } else { "innerBoundaryIs" };
                        s.push_str(&format!(
                            "<{tag}><LinearRing><coordinates>{}</coordinates></LinearRing></{tag}>",
                            kml_ring(r, true)
                        ));
                    }
                    s.push_str("</Polygon>");
                }
            }
            s.push_str("</Placemark>\n");
        }
        s.push_str("</Folder>\n");
    }
    s.push_str("</Document></kml>\n");
    s
}

/// A KMZ holding `doc.kml`.
pub fn write_kmz(kml: &str) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("doc.kml", options).map_err(|e| e.to_string())?;
        zip.write_all(kml.as_bytes()).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

// ── ESRI Shapefile ──────────────────────────────────────────────────────────

fn le_i32(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be_i32(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn le_f64(b: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// The records of a DBF table, as (field, text) pairs.
pub fn read_dbf(b: &[u8]) -> Vec<Vec<(String, String)>> {
    let (Some(n), Some(header_len), Some(rec_len)) = (
        le_i32(b, 4),
        b.get(8..10).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize),
        b.get(10..12).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize),
    ) else {
        return Vec::new();
    };
    let mut fields = Vec::new();
    let mut at = 32;
    while at + 32 <= b.len() && b[at] != 0x0D {
        let name_bytes = &b[at..at + 11];
        let end = name_bytes.iter().position(|&c| c == 0).unwrap_or(11);
        let name = String::from_utf8_lossy(&name_bytes[..end]).trim().to_string();
        fields.push((name, b[at + 16] as usize));
        at += 32;
    }
    let mut out = Vec::new();
    for r in 0..n.max(0) as usize {
        let start = header_len + r * rec_len;
        let Some(rec) = b.get(start..start + rec_len) else { break };
        if rec[0] == b'*' {
            out.push(Vec::new());
            continue;
        }
        let mut off = 1;
        let mut row = Vec::new();
        for (name, len) in &fields {
            let raw = rec.get(off..off + len).unwrap_or(&[]);
            let text = match std::str::from_utf8(raw) {
                Ok(s) => s.to_string(),
                // Old Vietnamese tables: TCVN3/VNI bytes; keep them readable as Latin-1.
                Err(_) => raw.iter().map(|&c| c as char).collect(),
            };
            row.push((name.clone(), text.trim().to_string()));
            off += len;
        }
        out.push(row);
    }
    out
}

/// Shapefile records (`.shp` bytes) with their attributes (`.dbf`).
pub fn read_shapefile(shp: &[u8], dbf: Option<&[u8]>, layer: &str) -> Result<Vec<Feature>, String> {
    if be_i32(shp, 0) != Some(9994) {
        return Err("not a shapefile".into());
    }
    let rows = dbf.map(read_dbf).unwrap_or_default();
    let mut out = Vec::new();
    let mut at = 100;
    let mut index = 0usize;
    while at + 8 <= shp.len() {
        let len = be_i32(shp, at + 4).unwrap_or(0).max(0) as usize * 2;
        let rec = at + 8;
        at = rec + len;
        let props = rows.get(index).cloned().unwrap_or_default();
        index += 1;
        let Some(kind) = le_i32(shp, rec) else { break };
        let lyr = props
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("layer"))
            .map(|(_, v)| layer_name(v))
            .filter(|s| s != "GIS")
            .unwrap_or_else(|| layer.to_string());
        let mut push = |g: Geom| {
            out.push(Feature { geom: Some(g), layer: lyr.clone(), props: props.clone(), color: None })
        };
        match kind % 10 {
            // Point, PointZ (11), PointM (21).
            1 => {
                if let (Some(x), Some(y)) = (le_f64(shp, rec + 4), le_f64(shp, rec + 12)) {
                    push(Geom::Point([x, y]));
                }
            }
            // MultiPoint.
            8 => {
                let n = le_i32(shp, rec + 36).unwrap_or(0).max(0) as usize;
                for k in 0..n {
                    let p = rec + 40 + k * 16;
                    if let (Some(x), Some(y)) = (le_f64(shp, p), le_f64(shp, p + 8)) {
                        push(Geom::Point([x, y]));
                    }
                }
            }
            // PolyLine (3), Polygon (5) and their Z / M forms.
            3 | 5 => {
                let parts = le_i32(shp, rec + 36).unwrap_or(0).max(0) as usize;
                let points = le_i32(shp, rec + 40).unwrap_or(0).max(0) as usize;
                let starts: Vec<usize> = (0..parts)
                    .filter_map(|k| le_i32(shp, rec + 44 + k * 4).map(|v| v.max(0) as usize))
                    .collect();
                let base = rec + 44 + parts * 4;
                let pt = |k: usize| -> Option<Xy> {
                    Some([le_f64(shp, base + k * 16)?, le_f64(shp, base + k * 16 + 8)?])
                };
                let rings: Vec<Vec<Xy>> = (0..parts)
                    .map(|k| {
                        let end = starts.get(k + 1).copied().unwrap_or(points);
                        (starts[k]..end.min(points)).filter_map(pt).collect()
                    })
                    .collect();
                if kind % 10 == 3 {
                    for r in rings {
                        push(Geom::Line(r));
                    }
                } else {
                    // Clockwise rings start polygons; counter-clockwise ones are holes.
                    let mut polys: Vec<Vec<Vec<Xy>>> = Vec::new();
                    for r in rings.into_iter().map(open_ring).filter(|r| r.len() >= 3) {
                        if signed_area(&r) < 0.0 || polys.is_empty() {
                            polys.push(vec![r]);
                        } else {
                            polys.last_mut().unwrap().push(r);
                        }
                    }
                    for p in polys {
                        push(Geom::Polygon(p));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Whether a `.prj` describes longitude/latitude rather than a projection.
pub fn prj_is_geographic(prj: &str) -> bool {
    let t = prj.trim_start().to_uppercase();
    t.starts_with("GEOGCS") || t.starts_with("GEOGCRS")
}

/// The files of one shapefile: `.shp`, `.shx`, `.dbf`.
pub struct ShapefileBytes {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
}

/// One shapefile from features of one geometry kind (grid coordinates).
/// Attributes: LAYER, NAME, then up to 20 other properties.
pub fn write_shapefile(features: &[&Feature]) -> ShapefileBytes {
    let kind: i32 = match features.iter().find_map(|f| f.geom.as_ref()) {
        Some(Geom::Point(_)) => 1,
        Some(Geom::Line(_)) => 3,
        _ => 5,
    };
    let mut records: Vec<Vec<u8>> = Vec::new();
    let mut bbox = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let grow = |p: Xy, bbox: &mut [f64; 4]| {
        bbox[0] = bbox[0].min(p[0]);
        bbox[1] = bbox[1].min(p[1]);
        bbox[2] = bbox[2].max(p[0]);
        bbox[3] = bbox[3].max(p[1]);
    };
    for f in features {
        let mut rec = Vec::new();
        rec.extend(kind.to_le_bytes());
        match &f.geom {
            Some(Geom::Point(p)) => {
                grow(*p, &mut bbox);
                rec.extend(p[0].to_le_bytes());
                rec.extend(p[1].to_le_bytes());
            }
            Some(g) => {
                let parts: Vec<Vec<Xy>> = match g {
                    Geom::Line(l) => vec![l.clone()],
                    Geom::Polygon(rings) => rings
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            let mut r = r.clone();
                            // Shapefile: outer rings clockwise, holes counter-clockwise.
                            if (i == 0) != (signed_area(&r) < 0.0) {
                                r.reverse();
                            }
                            if let Some(first) = r.first().copied() {
                                r.push(first);
                            }
                            r
                        })
                        .collect(),
                    Geom::Point(_) => unreachable!(),
                };
                let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
                for p in parts.iter().flatten() {
                    grow(*p, &mut b);
                    grow(*p, &mut bbox);
                }
                for v in b {
                    rec.extend(v.to_le_bytes());
                }
                rec.extend((parts.len() as i32).to_le_bytes());
                rec.extend((parts.iter().map(Vec::len).sum::<usize>() as i32).to_le_bytes());
                let mut start = 0i32;
                for p in &parts {
                    rec.extend(start.to_le_bytes());
                    start += p.len() as i32;
                }
                for p in parts.iter().flatten() {
                    rec.extend(p[0].to_le_bytes());
                    rec.extend(p[1].to_le_bytes());
                }
            }
            None => {
                rec.clear();
                rec.extend(0i32.to_le_bytes());
            }
        }
        records.push(rec);
    }
    if !bbox[0].is_finite() {
        bbox = [0.0; 4];
    }
    let header = |file_len_words: i32| {
        let mut h = Vec::with_capacity(100);
        h.extend(9994i32.to_be_bytes());
        h.extend([0u8; 20]);
        h.extend(file_len_words.to_be_bytes());
        h.extend(1000i32.to_le_bytes());
        h.extend(kind.to_le_bytes());
        for v in bbox {
            h.extend(v.to_le_bytes());
        }
        h.extend([0u8; 32]);
        h
    };
    let shp_len: usize = 100 + records.iter().map(|r| 8 + r.len()).sum::<usize>();
    let mut shp = header((shp_len / 2) as i32);
    let mut shx = header(((100 + 8 * records.len()) / 2) as i32);
    let mut offset = 100usize;
    for (i, r) in records.iter().enumerate() {
        shp.extend(((i + 1) as i32).to_be_bytes());
        shp.extend(((r.len() / 2) as i32).to_be_bytes());
        shp.extend(r);
        shx.extend(((offset / 2) as i32).to_be_bytes());
        shx.extend(((r.len() / 2) as i32).to_be_bytes());
        offset += 8 + r.len();
    }
    ShapefileBytes { shp, shx, dbf: write_dbf(features) }
}

fn write_dbf(features: &[&Feature]) -> Vec<u8> {
    // Columns: LAYER, NAME, then the other property keys in first-seen order.
    let mut keys: Vec<String> = Vec::new();
    for f in features {
        for (k, _) in &f.props {
            let up = k.to_uppercase();
            if up != "LAYER" && up != "NAME" && !keys.iter().any(|x| x.eq_ignore_ascii_case(k)) && keys.len() < 20 {
                keys.push(k.clone());
            }
        }
    }
    let value = |f: &Feature, key: &str| -> String {
        match key {
            "LAYER" => f.layer.clone(),
            "NAME" => f.name().unwrap_or("").to_string(),
            k => f.prop(k).unwrap_or("").to_string(),
        }
    };
    let mut columns: Vec<(String, String)> =
        vec![("LAYER".into(), "LAYER".into()), ("NAME".into(), "NAME".into())];
    let mut used: Vec<String> = vec!["LAYER".into(), "NAME".into()];
    for k in &keys {
        let mut short: String = k.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(10).collect();
        if short.is_empty() {
            short = "F".into();
        }
        let base = short.clone();
        let mut n = 1;
        while used.iter().any(|u| u.eq_ignore_ascii_case(&short)) {
            short = format!("{}{n}", &base[..base.len().min(8)]);
            n += 1;
        }
        used.push(short.clone());
        columns.push((short, k.clone()));
    }
    let widths: Vec<usize> = columns
        .iter()
        .map(|(_, key)| {
            features.iter().map(|f| value(f, key).len()).max().unwrap_or(1).clamp(1, 254)
        })
        .collect();
    let rec_len = 1 + widths.iter().sum::<usize>();
    let header_len = 32 + 32 * columns.len() + 1;
    let mut b = Vec::new();
    b.push(0x03);
    b.extend([126u8, 1, 1]);
    b.extend((features.len() as u32).to_le_bytes());
    b.extend((header_len as u16).to_le_bytes());
    b.extend((rec_len as u16).to_le_bytes());
    b.extend([0u8; 17]);
    b.push(0x00); // language driver: none (text is UTF-8, see .cpg)
    b.extend([0u8; 2]);
    for ((name, _), w) in columns.iter().zip(&widths) {
        let mut field = [0u8; 32];
        for (i, c) in name.bytes().take(10).enumerate() {
            field[i] = c;
        }
        field[11] = b'C';
        field[16] = *w as u8;
        b.extend(field);
    }
    b.push(0x0D);
    for f in features {
        b.push(b' ');
        for ((_, key), w) in columns.iter().zip(&widths) {
            let v = value(f, key);
            // Cut on a character boundary.
            let mut cut = v.len().min(*w);
            while !v.is_char_boundary(cut) {
                cut -= 1;
            }
            b.extend(&v.as_bytes()[..cut]);
            b.extend(std::iter::repeat_n(b' ', w - cut));
        }
    }
    b.push(0x1A);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Feature {
        Feature {
            geom: Some(Geom::Polygon(vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]])),
            layer: "Thửa đất".into(),
            props: vec![("name".into(), "Thửa 12".into()), ("dien_tich".into(), "100".into())],
            color: None,
        }
    }

    #[test]
    fn geojson_round_trip() {
        let text = write_geojson(&[square()]);
        let (back, coords) = read_geojson(&text, "x").unwrap();
        assert_eq!(coords, Coords::LonLat);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].layer, "Thửa đất");
        let Some(Geom::Polygon(r)) = &back[0].geom else { panic!() };
        assert_eq!(r[0].len(), 4);
        assert!(signed_area(&r[0]) > 0.0);
    }

    #[test]
    fn kml_round_trip() {
        let text = write_kml(&[square()], "test");
        let back = read_kml(&text, "x").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].layer, "Thửa đất");
        assert_eq!(back[0].name(), Some("Thửa 12"));
        let kmz = write_kmz(&text).unwrap();
        assert_eq!(kmz_text(&kmz).unwrap(), text);
    }

    #[test]
    fn shapefile_round_trip() {
        let f = square();
        let line = Feature { geom: Some(Geom::Line(vec![[1.0, 2.0], [3.0, 4.0]])), ..Default::default() };
        let files = write_shapefile(&[&f]);
        let back = read_shapefile(&files.shp, Some(&files.dbf), "x").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].layer, "Thửa đất");
        assert_eq!(back[0].prop("NAME"), Some("Thửa 12"));
        assert_eq!(back[0].prop("dien_tich"), Some("100"));
        let Some(Geom::Polygon(r)) = &back[0].geom else { panic!() };
        assert_eq!(r[0].len(), 4);
        let files = write_shapefile(&[&line]);
        let back = read_shapefile(&files.shp, Some(&files.dbf), "dg").unwrap();
        assert_eq!(back[0].geom, line.geom);
        assert_eq!(back[0].layer, "dg");
        assert_eq!(files.shx.len(), 108);
    }
}
