//! CadMini Studio GIS commands (2D workspace):
//!
//! - GISCRS    set the drawing's VN-2000 system (province or central meridian)
//! - GISLL     latitude / longitude of picked points
//! - GISGOTO   zoom to a latitude / longitude
//! - GISIMPORT GeoJSON, KML/KMZ or Shapefile into the drawing (one undo step)
//! - GISEXPORT the drawing (or the selection) to GeoJSON, KML/KMZ or Shapefile
//!
//! The system is kept in the drawing's custom property `CADMINI_CRS`, the same
//! one CadMini (Python) uses, so both read each other's drawings.

use crate::app::{Message, OpenCADStudio};
use crate::command::{CadCommand, CmdResult, FreeTextValuePromptCommand};
use crate::gis::crs::{self, DrawingCrs, HEADER_KEY};
use crate::gis::formats::{self, Coords, Feature, Geom, Xy};
use codec::EntityType;
use glam::DVec3;
use iced::Task;

/// GISLL: report each picked point in VN-2000 and WGS84.
struct GisLlCommand {
    crs: DrawingCrs,
}

impl CadCommand for GisLlCommand {
    fn name(&self) -> &'static str {
        "GISLL"
    }

    fn prompt(&self) -> String {
        crate::t!("GISLL  Specify point (Enter to end):").into_owned()
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        let (lon, lat) = self.crs.to_wgs84(pt.x, pt.y);
        CmdResult::Measurement(format!(
            "X = {:.3}  Y = {:.3}  →  {:.8}, {:.8}  ({} {})",
            pt.x,
            pt.y,
            lat,
            lon,
            crs::dms(lat, 'N', 'S'),
            crs::dms(lon, 'E', 'W'),
        ))
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

/// GMAP: open Google Maps at a picked point.
struct GmapCommand {
    crs: DrawingCrs,
}

impl CadCommand for GmapCommand {
    fn name(&self) -> &'static str {
        "GMAP"
    }

    fn prompt(&self) -> String {
        crate::t!("GMAP  Specify point to open in Google Maps:").into_owned()
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        let (lon, lat) = self.crs.to_wgs84(pt.x, pt.y);
        CmdResult::Dispatch(format!(
            "_GMAPOPEN https://www.google.com/maps/search/?api=1&query={lat:.7},{lon:.7}"
        ))
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

/// The drawing's system, if it has one.
pub(in crate::app) fn drawing_crs(doc: &codec::Document) -> Option<DrawingCrs> {
    doc.summary_info
        .custom_properties
        .iter()
        .find(|(k, _)| k == HEADER_KEY)
        .and_then(|(_, v)| DrawingCrs::from_text(v))
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "GIS".into())
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    if let Some(bytes) = crate::scene::model::pdf_raster::source_bytes(path) {
        return Ok(bytes.to_vec());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

/// Read a GIS file into features and the kind of their coordinates.
fn read_gis(path: &str) -> Result<(Vec<Feature>, Coords), String> {
    let lower = path.to_lowercase();
    let layer = formats::layer_name(&file_stem(path));
    let bytes = read_file(path)?;
    if lower.ends_with(".kmz") {
        Ok((formats::read_kml(&formats::kmz_text(&bytes)?, &layer)?, Coords::LonLat))
    } else if lower.ends_with(".kml") {
        Ok((formats::read_kml(&String::from_utf8_lossy(&bytes), &layer)?, Coords::LonLat))
    } else if lower.ends_with(".shp") {
        let sibling = |ext: &str| {
            let p = std::path::Path::new(path).with_extension(ext);
            read_file(&p.to_string_lossy())
                .or_else(|_| read_file(&p.with_extension(ext.to_uppercase()).to_string_lossy()))
                .ok()
        };
        let dbf = sibling("dbf");
        let geographic = sibling("prj")
            .map(|b| formats::prj_is_geographic(&String::from_utf8_lossy(&b)))
            .unwrap_or(false);
        let features = formats::read_shapefile(&bytes, dbf.as_deref(), &layer)?;
        // No .prj: decide by the numbers.
        let lonlat = geographic
            || features.iter().all(|f| match &f.geom {
                Some(Geom::Point(p)) => p[0].abs() <= 180.0 && p[1].abs() <= 90.0,
                Some(Geom::Line(l)) => l.iter().all(|p| p[0].abs() <= 180.0),
                Some(Geom::Polygon(r)) => r.iter().flatten().all(|p| p[0].abs() <= 180.0),
                None => true,
            });
        Ok((features, if lonlat { Coords::LonLat } else { Coords::Grid }))
    } else {
        formats::read_geojson(&String::from_utf8_lossy(&bytes), &layer)
    }
}

fn polyline(points: &[Xy], closed: bool) -> codec::entities::LwPolyline {
    let mut pl = codec::entities::LwPolyline::from_points(
        points.iter().map(|p| codec::types::Vector2::new(p[0], p[1])).collect(),
    );
    pl.is_closed = closed;
    pl
}

/// Drawing entities for one feature.
fn feature_entities(f: &Feature, text_height: f64) -> Vec<EntityType> {
    let color = f
        .color
        .map(|[r, g, b]| codec::types::Color::Rgb { r, g, b })
        .unwrap_or(codec::types::Color::ByLayer);
    let mut out: Vec<EntityType> = Vec::new();
    let mut label_at: Option<Xy> = None;
    match &f.geom {
        Some(Geom::Point(p)) => {
            let mut point = codec::entities::Point::new();
            point.location = codec::types::Vector3::new(p[0], p[1], 0.0);
            out.push(EntityType::Point(point));
            label_at = Some([p[0] + text_height * 0.5, p[1] + text_height * 0.5]);
        }
        Some(Geom::Line(l)) if l.len() >= 2 => {
            out.push(EntityType::LwPolyline(polyline(l, false)));
            label_at = Some(l[l.len() / 2]);
        }
        Some(Geom::Polygon(rings)) => {
            for r in rings {
                out.push(EntityType::LwPolyline(polyline(r, true)));
            }
            if let Some(outer) = rings.first() {
                let n = outer.len() as f64;
                label_at = Some([
                    outer.iter().map(|p| p[0]).sum::<f64>() / n,
                    outer.iter().map(|p| p[1]).sum::<f64>() / n,
                ]);
            }
        }
        _ => {}
    }
    if let (Some(name), Some(at)) = (f.name(), label_at) {
        let mut text = codec::entities::Text::new();
        text.value = name.to_string();
        text.insertion_point = codec::types::Vector3::new(at[0], at[1], 0.0);
        text.height = text_height;
        out.push(EntityType::Text(text));
    }
    for e in &mut out {
        let common = e.common_mut();
        common.layer = f.layer.clone();
        common.color = color;
    }
    out
}

fn vec2(v: &codec::types::Vector2) -> Xy {
    [v.x, v.y]
}

/// The features an entity exports as (grid coordinates).
fn entity_feature(e: &EntityType) -> Option<Feature> {
    let common = e.common();
    let mut props = Vec::new();
    let geom = match e {
        EntityType::Point(p) => Geom::Point([p.location.x, p.location.y]),
        EntityType::Line(l) => Geom::Line(vec![[l.start.x, l.start.y], [l.end.x, l.end.y]]),
        EntityType::LwPolyline(pl) => {
            let pts = flatten_lw(pl);
            if pl.is_closed && pts.len() >= 3 {
                Geom::Polygon(vec![pts])
            } else {
                Geom::Line(pts)
            }
        }
        EntityType::Circle(c) => {
            let n = 72;
            Geom::Polygon(vec![(0..n)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / n as f64;
                    [c.center.x + c.radius * a.cos(), c.center.y + c.radius * a.sin()]
                })
                .collect()])
        }
        EntityType::Text(t) => {
            props.push(("name".to_string(), t.value.clone()));
            Geom::Point([t.insertion_point.x, t.insertion_point.y])
        }
        EntityType::MText(t) => {
            props.push(("name".to_string(), t.value.clone()));
            Geom::Point([t.insertion_point.x, t.insertion_point.y])
        }
        EntityType::Insert(ins) => {
            props.push(("block".to_string(), ins.block_name.clone()));
            Geom::Point([ins.insert_point.x, ins.insert_point.y])
        }
        _ => return None,
    };
    let color = common.color.rgb().map(|(r, g, b)| [r, g, b]);
    Some(Feature { geom: Some(geom), layer: common.layer.clone(), props, color })
}

/// A lightweight polyline's points, arcs split into short chords.
fn flatten_lw(pl: &codec::entities::LwPolyline) -> Vec<Xy> {
    let v = &pl.vertices;
    let mut out = Vec::new();
    let n = v.len();
    for k in 0..n {
        let a = vec2(&v[k].location);
        out.push(a);
        let last = k + 1 == n;
        if last && !pl.is_closed {
            break;
        }
        let b = vec2(&v[(k + 1) % n].location);
        let bulge = v[k].bulge;
        if bulge.abs() > 1e-9 {
            // Arc through a→b with included angle 4·atan(bulge).
            let theta = 4.0 * bulge.atan();
            let chord = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            if chord > 0.0 {
                let r = chord / (2.0 * (theta / 2.0).sin());
                let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let d = r * (theta / 2.0).cos();
                let nx = -(b[1] - a[1]) / chord;
                let ny = (b[0] - a[0]) / chord;
                let c = [mid[0] + nx * d, mid[1] + ny * d];
                let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
                let steps = ((theta.abs() / (std::f64::consts::PI / 36.0)).ceil() as usize).max(2);
                for s in 1..steps {
                    let t = a0 + theta * s as f64 / steps as f64;
                    out.push([c[0] + r.abs() * t.cos(), c[1] + r.abs() * t.sin()]);
                }
            }
        }
    }
    out
}

impl OpenCADStudio {
    fn gis_require_crs(&mut self, i: usize) -> Option<DrawingCrs> {
        let found = drawing_crs(&self.tabs[i].scene.document);
        if found.is_none() {
            self.command_line.push_error(
                crate::t!("This drawing has no coordinate system yet. Run GISCRS first (e.g. GISCRS Hà Tĩnh).")
                    .as_ref(),
            );
        }
        found
    }

    fn gis_set_crs(&mut self, i: usize, value: &str) -> Task<Message> {
        let value = value.trim();
        if value.is_empty() {
            return Task::none();
        }
        if value == "?" {
            let list: Vec<String> = crs::PROVINCES_2025
                .iter()
                .map(|(n, d, m, _)| format!("{n} {d}°{m:02}′"))
                .collect();
            self.command_line.push_output(&list.join(" · "));
            return Task::none();
        }
        let Some(system) = DrawingCrs::parse_user(value) else {
            self.command_line.push_error(
                crate::tf!("GISCRS: \"{value}\" is not a province or a central meridian (100–120°).").as_ref(),
            );
            return Task::none();
        };
        self.push_undo_snapshot(i, "GISCRS");
        let props = &mut self.tabs[i].scene.document.summary_info.custom_properties;
        let text = system.to_text();
        match props.iter_mut().find(|(k, _)| k == HEADER_KEY) {
            Some(entry) => entry.1 = text,
            None => props.push((HEADER_KEY.to_string(), text)),
        }
        self.tabs[i].dirty = true;
        self.command_line
            .push_output(crate::tf!("GISCRS: {}", system.label()).as_ref());
        // The background follows the new system.
        self.basemap.invalidate();
        Task::none()
    }

    fn gis_goto(&mut self, i: usize, value: &str) -> Task<Message> {
        let Some(system) = self.gis_require_crs(i) else {
            return Task::none();
        };
        let Some((lat, lon)) = crs::parse_latlon(value) else {
            if !value.trim().is_empty() {
                self.command_line
                    .push_error(crate::t!("GISGOTO: enter latitude, longitude in degrees.").as_ref());
            }
            return Task::none();
        };
        let (x, y) = system.from_wgs84(lon, lat);
        self.command_line
            .push_output(&format!("GISGOTO: {lat:.8}, {lon:.8}  →  X = {x:.3}  Y = {y:.3}"));
        let half = DVec3::new(250.0, 250.0, 0.0);
        let c = DVec3::new(x, y, 0.0);
        self.handle_zoom_to_window(c - half, c + half);
        Task::none()
    }

    fn gis_import(&mut self, i: usize, path: &str) -> Task<Message> {
        let (features, coords) = match read_gis(path) {
            Ok(r) => r,
            Err(e) => {
                self.command_line
                    .push_error(crate::tf!("GISIMPORT: cannot read \"{path}\": {e}").as_ref());
                return Task::none();
            }
        };
        if features.is_empty() {
            self.command_line
                .push_info(crate::t!("GISIMPORT: the file holds no features.").as_ref());
            return Task::none();
        }
        let system = if coords == Coords::LonLat {
            match self.gis_require_crs(i) {
                Some(s) => Some(s),
                None => return Task::none(),
            }
        } else {
            None
        };
        let to_grid = |p: Xy| match &system {
            Some(s) => {
                let (x, y) = s.from_wgs84(p[0], p[1]);
                [x, y]
            }
            None => p,
        };
        let features: Vec<Feature> = features
            .into_iter()
            .map(|f| Feature { geom: f.geom.as_ref().map(|g| g.map(&to_grid)), ..f })
            .collect();
        // Label height from the data's extent.
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for f in &features {
            let pts: Vec<Xy> = match &f.geom {
                Some(Geom::Point(p)) => vec![*p],
                Some(Geom::Line(l)) => l.clone(),
                Some(Geom::Polygon(r)) => r.iter().flatten().copied().collect(),
                None => vec![],
            };
            for p in pts {
                lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                hi = [hi[0].max(p[0]), hi[1].max(p[1])];
            }
        }
        let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
        let text_height = if extent.is_finite() && extent > 0.0 {
            (extent / 400.0).clamp(0.5, 50.0)
        } else {
            2.0
        };
        self.push_undo_snapshot(i, "GISIMPORT");
        let mut layers: Vec<String> = Vec::new();
        let mut entities = Vec::new();
        for f in &features {
            if !layers.contains(&f.layer) {
                layers.push(f.layer.clone());
            }
            entities.extend(feature_entities(f, text_height));
        }
        let scene = &mut self.tabs[i].scene;
        for layer in &layers {
            scene.ensure_layer(layer);
        }
        let count = entities.len();
        let handles = scene.add_entities(entities);
        self.sync_registered_layers(i);
        self.tabs[i].dirty = true;
        self.tabs[i].scene.zoom_to_entities(&handles);
        self.command_line.push_output(
            crate::tf!(
                "GISIMPORT: {} feature(s), {count} object(s) on {} layer(s){}.",
                features.len(),
                layers.len(),
                if system.is_some() { ", converted from WGS84 to VN-2000" } else { "" }
            )
            .as_ref(),
        );
        Task::none()
    }

    fn gis_export(&mut self, i: usize, path: &str) -> Task<Message> {
        let Some(system) = self.gis_require_crs(i) else {
            return Task::none();
        };
        let scene = &self.tabs[i].scene;
        let selected: Vec<EntityType> =
            scene.selected_entities().into_iter().map(|(_, e)| e.clone()).collect();
        let entities: Vec<EntityType> = if selected.is_empty() {
            let model = scene.model_space_block_handle();
            scene
                .document
                .block_records
                .iter()
                .find(|br| br.handle == model)
                .map(|br| {
                    br.entity_handles
                        .iter()
                        .filter_map(|h| scene.document.get_entity(*h).cloned())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            selected
        };
        let features: Vec<Feature> = entities.iter().filter_map(entity_feature).collect();
        if features.is_empty() {
            self.command_line
                .push_info(crate::t!("GISEXPORT: nothing to export (points, lines, polylines, circles, texts, blocks).").as_ref());
            return Task::none();
        }
        let lower = path.to_lowercase();
        let lonlat = || -> Vec<Feature> {
            features
                .iter()
                .map(|f| Feature {
                    geom: f.geom.as_ref().map(|g| {
                        g.map(&|p: Xy| {
                            let (lon, lat) = system.to_wgs84(p[0], p[1]);
                            [lon, lat]
                        })
                    }),
                    ..f.clone()
                })
                .collect()
        };
        let title = file_stem(path);
        let result: Result<Vec<String>, String> = (|| {
            let write = |p: &std::path::Path, bytes: &[u8]| -> Result<String, String> {
                std::fs::write(p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
                Ok(p.display().to_string())
            };
            let path = std::path::Path::new(path);
            if lower.ends_with(".kml") {
                Ok(vec![write(path, formats::write_kml(&lonlat(), &title).as_bytes())?])
            } else if lower.ends_with(".kmz") {
                let kmz = formats::write_kmz(&formats::write_kml(&lonlat(), &title))?;
                Ok(vec![write(path, &kmz)?])
            } else if lower.ends_with(".shp") {
                // One shapefile per geometry kind.
                let mut written = Vec::new();
                for (suffix, kind) in [("_point", "Point"), ("_line", "LineString"), ("_polygon", "Polygon")] {
                    let group: Vec<&Feature> = features
                        .iter()
                        .filter(|f| f.geom.as_ref().is_some_and(|g| g.kind() == kind))
                        .collect();
                    if group.is_empty() {
                        continue;
                    }
                    let stem = path.with_file_name(format!("{title}{suffix}"));
                    let files = formats::write_shapefile(&group);
                    written.push(write(&stem.with_extension("shp"), &files.shp)?);
                    write(&stem.with_extension("shx"), &files.shx)?;
                    write(&stem.with_extension("dbf"), &files.dbf)?;
                    write(&stem.with_extension("cpg"), b"UTF-8")?;
                    write(&stem.with_extension("prj"), system.wkt_esri().as_bytes())?;
                }
                Ok(written)
            } else {
                Ok(vec![write(path, formats::write_geojson(&lonlat()).as_bytes())?])
            }
        })();
        match result {
            Ok(files) => self.command_line.push_output(
                crate::tf!("GISEXPORT: {} feature(s) → {}", features.len(), files.join(", ")).as_ref(),
            ),
            Err(e) => self.command_line.push_error(crate::tf!("GISEXPORT: {e}").as_ref()),
        }
        Task::none()
    }

    pub(super) fn dispatch_gis(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim();
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_uppercase(), r.trim().trim_matches('"')),
            None => (trimmed.to_ascii_uppercase(), ""),
        };
        let verb = verb.trim_start_matches('.').to_string();
        match verb.as_str() {
            "GISCRS" | "_GISCRSSET" if !rest.is_empty() => Some(self.gis_set_crs(i, rest)),
            "_GISCRSSET" | "_GISGOTOLL" | "_GISIMPORTFILE" | "_GISEXPORTFILE" if rest.is_empty() => {
                Some(Task::none())
            }
            "GISCRS" => {
                let current = drawing_crs(&self.tabs[i].scene.document)
                    .map(|c| c.label())
                    .unwrap_or_else(|| crate::t!("none").into_owned());
                self.command_line
                    .push_output(crate::tf!("GISCRS  Current system: {current}").as_ref());
                let c = FreeTextValuePromptCommand::new(
                    "_GISCRSSET",
                    "Province or central meridian (Hà Tĩnh, 105.5, 105 30, 105/6; ? lists provinces):",
                );
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISLL" => {
                let Some(system) = self.gis_require_crs(i) else {
                    return Some(Task::none());
                };
                let c = GisLlCommand { crs: system };
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISGOTO" | "_GISGOTOLL" if !rest.is_empty() => Some(self.gis_goto(i, rest)),
            "GISGOTO" => {
                if self.gis_require_crs(i).is_none() {
                    return Some(Task::none());
                }
                let c = FreeTextValuePromptCommand::new(
                    "_GISGOTOLL",
                    "GISGOTO  Latitude, longitude (e.g. 18.3525, 106.0696):",
                );
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISIMPORT" | "_GISIMPORTFILE" if !rest.is_empty() => Some(self.gis_import(i, rest)),
            "GISIMPORT" if crate::sys::file_dia() => {
                let filters = vec![
                    (
                        "GIS (*.geojson, *.json, *.kml, *.kmz, *.shp)".to_string(),
                        ["geojson", "json", "kml", "kmz", "shp", "GEOJSON", "JSON", "KML", "KMZ", "SHP"]
                            .map(String::from)
                            .to_vec(),
                    ),
                    ("All Files".to_string(), vec!["*".to_string()]),
                ];
                let title = crate::t!("Import GIS Data").into_owned();
                Some(Task::perform(pick_gis(title, filters), |path| match path {
                    Some(path) => Message::Command(format!("_GISIMPORTFILE {path}")),
                    None => Message::Noop,
                }))
            }
            "GISIMPORT" => {
                let c = FreeTextValuePromptCommand::new("_GISIMPORTFILE", "GISIMPORT  GIS file:");
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISEXPORT" | "_GISEXPORTFILE" if !rest.is_empty() => Some(self.gis_export(i, rest)),
            "GISEXPORT" => {
                if self.gis_require_crs(i).is_none() {
                    return Some(Task::none());
                }
                if crate::sys::file_dia() {
                    let name = format!("{}.geojson", self.tabs[i].tab_display_name());
                    Some(Task::perform(
                        async move {
                            let handle = crate::sys::file_dialog()
                                .set_title(crate::t!("Export GIS Data").as_ref())
                                .add_filter("GeoJSON (*.geojson)", &["geojson"])
                                .add_filter("Google Earth (*.kmz)", &["kmz"])
                                .add_filter("KML (*.kml)", &["kml"])
                                .add_filter("Shapefile VN-2000 (*.shp)", &["shp"])
                                .set_file_name(&name)
                                .save_file()
                                .await?;
                            Some(crate::sys::handle_path(&handle).to_string_lossy().into_owned())
                        },
                        |path| match path {
                            Some(path) => Message::Command(format!("_GISEXPORTFILE {path}")),
                            None => Message::Noop,
                        },
                    ))
                } else {
                    let c = FreeTextValuePromptCommand::new(
                        "_GISEXPORTFILE",
                        "GISEXPORT  File (.geojson, .kml, .kmz or .shp):",
                    );
                    self.command_line.push_info(&c.prompt());
                    self.tabs[i].active_cmd = Some(Box::new(c));
                    Some(Task::none())
                }
            }
            // ── Map background (GISSAT) ──────────────────────────────────
            "GISSAT" | "SAT" => {
                let on = match rest.to_ascii_uppercase().as_str() {
                    "ON" | "1" => true,
                    "OFF" | "0" => false,
                    _ => !self.basemap.on,
                };
                Some(self.set_basemap_on(i, on))
            }
            "GISSOURCE" if rest.is_empty() => {
                let list: Vec<String> = crate::gis::tiles::sources()
                    .iter()
                    .enumerate()
                    .map(|(n, s)| format!("{} {} [{}]", n + 1, s.name, s.key))
                    .collect();
                self.command_line.push_output(&list.join("  ·  "));
                let c = FreeTextValuePromptCommand::new(
                    "_GISSOURCESET",
                    "GISSOURCE  Background source (number or key):",
                );
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISSOURCE" | "_GISSOURCESET" => {
                let all = crate::gis::tiles::sources();
                let key = rest
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| all.get(n.wrapping_sub(1)))
                    .or_else(|| all.iter().find(|s| s.key.eq_ignore_ascii_case(rest)))
                    .map(|s| s.key.to_string())
                    .or_else(|| rest.eq_ignore_ascii_case("custom").then(|| "custom".to_string()));
                match key {
                    Some(key) => Some(self.set_basemap_source(i, &key)),
                    None => {
                        if !rest.is_empty() {
                            self.command_line.push_error(
                                crate::tf!("GISSOURCE: no source \"{rest}\".").as_ref(),
                            );
                        }
                        Some(Task::none())
                    }
                }
            }
            "GISURL" if rest.is_empty() => {
                let c = FreeTextValuePromptCommand::new(
                    "_GISURLSET",
                    "GISURL  Tile URL with {z} {x} {y}:",
                );
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISURL" | "_GISURLSET" => {
                if !rest.contains("{z}") || !rest.contains("{x}") || !rest.contains("{y}") {
                    self.command_line.push_error(
                        crate::t!("GISURL: the address needs {z}, {x} and {y}.").as_ref(),
                    );
                    return Some(Task::none());
                }
                self.basemap.custom_url = rest.to_string();
                Some(self.set_basemap_source(i, "custom"))
            }
            "GISOPACITY" if rest.is_empty() => {
                let c = FreeTextValuePromptCommand::new(
                    "_GISOPACITYSET",
                    "GISOPACITY  Background strength 10–100 %:",
                );
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "GISOPACITY" | "_GISOPACITYSET" => {
                match rest.trim_end_matches('%').trim().parse::<f32>() {
                    Ok(v) if (10.0..=100.0).contains(&v) => {
                        self.basemap.opacity = v / 100.0;
                        self.basemap.invalidate();
                        self.persist_settings_if_changed();
                        self.command_line
                            .push_output(&format!("GISOPACITY: {v:.0}%"));
                        Some(self.basemap_tick())
                    }
                    _ => {
                        self.command_line
                            .push_error(crate::t!("GISOPACITY: enter 10 to 100.").as_ref());
                        Some(Task::none())
                    }
                }
            }
            "GMAP" => {
                let Some(system) = self.gis_require_crs(i) else {
                    return Some(Task::none());
                };
                let c = GmapCommand { crs: system };
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "_GMAPOPEN" => Some(crate::sys::open_url(rest, self.main_window)),
            _ => None,
        }
    }
}

/// Pick a GIS file; a shapefile's companions are registered beside it so
/// the web build reads them as well.
async fn pick_gis(title: String, filters: Vec<(String, Vec<String>)>) -> Option<String> {
    super::dgn_import::pick(title, filters).await
}

inventory::submit!(crate::command::CommandRegistration {
    names: &[
        "GISCRS", "GISLL", "GISGOTO", "GISIMPORT", "GISEXPORT", "GISSAT", "SAT", "GISSOURCE",
        "GISURL", "GISOPACITY", "GMAP",
    ],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_bulged_polyline_flattens_round() {
        let mut pl = codec::entities::LwPolyline::from_points(vec![
            codec::types::Vector2::new(0.0, 0.0),
            codec::types::Vector2::new(10.0, 0.0),
        ]);
        pl.is_closed = true;
        pl.vertices[0].bulge = 1.0;
        pl.vertices[1].bulge = 1.0;
        let pts = flatten_lw(&pl);
        assert!(pts.len() > 10);
        for p in pts {
            let r = ((p[0] - 5.0).powi(2) + p[1].powi(2)).sqrt();
            assert!((r - 5.0).abs() < 1e-9, "{r}");
        }
    }

    #[test]
    fn features_become_entities_on_their_layer() {
        let f = Feature {
            geom: Some(Geom::Polygon(vec![vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]]])),
            layer: "Ranh giới".into(),
            props: vec![("name".into(), "Lô A".into())],
            color: Some([255, 0, 0]),
        };
        let ents = feature_entities(&f, 1.0);
        assert_eq!(ents.len(), 2);
        assert!(ents.iter().all(|e| e.common().layer == "Ranh giới"));
        assert!(matches!(&ents[1], EntityType::Text(t) if t.value == "Lô A"));
    }
}

// ── Map background ──────────────────────────────────────────────────────────

impl OpenCADStudio {
    fn set_basemap_on(&mut self, i: usize, on: bool) -> Task<Message> {
        if on && drawing_crs(&self.tabs[i].scene.document).is_none() {
            self.command_line.push_error(
                crate::t!("GISSAT: set the drawing's coordinate system first (GISCRS).").as_ref(),
            );
            self.basemap.on = true;
            self.persist_settings_if_changed();
            return self.dispatch_gis("GISCRS", i).unwrap_or_else(Task::none);
        }
        self.basemap.on = on;
        self.basemap.invalidate();
        self.persist_settings_if_changed();
        if on {
            let source = self.basemap.source();
            self.command_line.push_output(&format!(
                "GISSAT: {} — {}",
                source.name, source.attribution
            ));
        } else {
            for tab in &mut self.tabs {
                if tab.scene.basemap_images.take().is_some() {
                    tab.scene.camera_generation += 1;
                }
            }
            self.command_line.push_output("GISSAT: off");
        }
        self.basemap_tick()
    }

    fn set_basemap_source(&mut self, i: usize, key: &str) -> Task<Message> {
        self.basemap.source_key = key.to_string();
        self.basemap.reset();
        self.persist_settings_if_changed();
        let source = self.basemap.source();
        self.command_line.push_output(&format!(
            "GISSOURCE: {} {}",
            source.name,
            if source.attribution.is_empty() { String::new() } else { format!("— {}", source.attribution) }
        ));
        if self.basemap.on {
            self.basemap_tick()
        } else {
            self.set_basemap_on(i, true)
        }
    }

    /// Bring the active drawing's map background in step with its view, and
    /// start downloading the tiles it lacks.
    pub(in crate::app) fn basemap_tick(&mut self) -> Task<Message> {
        let i = self.active_tab;
        let Some(tab) = self.tabs.get_mut(i) else {
            return Task::none();
        };
        let crs = drawing_crs(&tab.scene.document);
        let show = self.basemap.on
            && !self.workspace.is_3d()
            && !tab.is_start
            && tab.scene.current_layout == "Model"
            && crs.is_some();
        let Some(crs) = crs.filter(|_| show) else {
            if tab.scene.basemap_images.take().is_some() {
                tab.scene.camera_generation += 1;
            }
            return Task::none();
        };
        let (center, size, px) = tab.scene.view_window();
        if !self.basemap.needs_build(center, size, px) && tab.scene.basemap_images.is_some() {
            return Task::none();
        }
        let (models, missing) = self.basemap.build(&crs, center, size, px);
        tab.scene.basemap_images = Some(std::sync::Arc::new(models));
        tab.scene.camera_generation += 1;
        fetch_tiles(self.basemap.source(), missing)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_tiles(
    source: crate::gis::tiles::TileSource,
    keys: Vec<crate::gis::tiles::TileKey>,
) -> Task<Message> {
    Task::batch(keys.into_iter().map(|key| {
        let source = source.clone();
        let source_key = source.key.to_string();
        Task::perform(
            async move {
                let (tx, rx) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(crate::gis::tiles::load(&source, key));
                });
                rx.await.ok().flatten()
            },
            move |tile| Message::BasemapTile(source_key.clone(), key, tile),
        )
    }))
}

#[cfg(target_arch = "wasm32")]
fn fetch_tiles(
    _source: crate::gis::tiles::TileSource,
    _keys: Vec<crate::gis::tiles::TileKey>,
) -> Task<Message> {
    Task::none()
}
