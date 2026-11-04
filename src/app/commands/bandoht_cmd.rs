//! BANDOHT (BDHT) — the present-state map inside a boundary, as CadMini;
//! OSMDUONG — roads and places only. Pick a closed polyline (or Enter, then
//! a window), tick the layers in the dialog, and the data downloads and
//! builds on a worker thread; the drawing gets it as one undo step.

use std::sync::Mutex;

use crate::app::commands::cadmini_dialogs::Dialog;
use crate::app::commands::gpmb_cmd::{closed_ring, PickThenRun};
use crate::app::{Message, ModalKind, OpenCADStudio};
use crate::bandoht::{self as B, Item, Xy};
use crate::command::{CadCommand, CmdResult};
use crate::ui::window::cadmini::Msg;
use codec::{EntityType, Handle};
use glam::DVec3;
use iced::widget::{checkbox, column, pick_list, row, text, Space};
use iced::{Element, Fill, Length, Task};

/// A finished run: tab id, its result (items, notes, origin, clear).
type Done = (u64, Result<(Vec<Item>, Vec<String>, Xy, bool), String>);
static DONE: Mutex<Option<Done>> = Mutex::new(None);

pub(in crate::app) struct BandoForm {
    pub(super) opts: B::Options,
    boundary: Vec<Xy>,
    info: String,
    roads_only: bool,
}

/// The second corner of a window boundary.
struct Corners {
    first: Option<DVec3>,
}

impl CadCommand for Corners {
    fn name(&self) -> &'static str {
        "BANDOHT"
    }

    fn prompt(&self) -> String {
        if self.first.is_none() {
            "BANDOHT  Góc thứ nhất của khung:".into()
        } else {
            "BANDOHT  Góc đối diện:".into()
        }
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.first {
            None => {
                self.first = Some(pt);
                CmdResult::NeedPoint
            }
            Some(a) if (pt.x - a.x).abs() >= 1.0 && (pt.y - a.y).abs() >= 1.0 => {
                CmdResult::Dispatch(format!("_BDHTBOX {},{},{},{}", a.x, a.y, pt.x, pt.y))
            }
            Some(_) => CmdResult::NeedPoint,
        }
    }

    fn on_mouse_move(&mut self, pt: DVec3) -> Option<crate::scene::WireModel> {
        let a = self.first?;
        let p = |x: f64, y: f64| [x as f32, y as f32, a.z as f32];
        Some(crate::scene::WireModel::solid(
            "bandoht_window".into(),
            vec![p(a.x, a.y), p(pt.x, a.y), p(pt.x, pt.y), p(a.x, pt.y), p(a.x, a.y)],
            [0.3, 0.9, 0.4, 1.0],
            false,
        ))
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

fn check<'a>(name: &'a str, key: &'static str, on: bool) -> Element<'a, Message> {
    checkbox(on).label(name).text_size(12).on_toggle(move |v| Message::CadMini(Msg::Check(key, v))).into()
}

fn ring_area(r: &[Xy]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f64>().abs() / 2.0
}

pub(in crate::app) fn view(f: &BandoForm) -> Element<'_, Message> {
    let o = &f.opts;
    let roads = column![
        check("Tim đường (đỏ, nét đứt) và mép đường (tím, bo cung R tại nút giao)", "roads", o.roads),
        check("Tên đường, tên ngõ ghi dọc tuyến", "road_names", o.road_names),
        check("Cả lối đi bộ, đường mòn", "paths", o.paths),
    ]
    .spacing(4);
    let mut places = column![check("Lấy địa danh", "places_on", !o.places.is_empty())].spacing(4);
    for (k, _, _, _, label) in B::POI {
        places = places.push(
            row![
                Space::new().width(20),
                checkbox(o.places.contains(*k))
                    .label(*label)
                    .text_size(12)
                    .on_toggle(move |v| Message::CadMini(Msg::Set("place", format!("{}{k}", if v { "+" } else { "-" })))),
            ],
        );
    }
    let state = column![
        row![check("Nhà (OpenStreetMap)", "buildings", o.buildings), check("Mặt nước, sông suối và tên", "water", o.water)]
            .spacing(16),
        row![check("Cây xanh, rừng, công viên", "green", o.green), check("Đất nông nghiệp", "farm", o.farm)].spacing(16),
        check("Đường sắt, đường điện", "rail_power", o.rail_power),
    ]
    .spacing(4);
    let num = |v: f64| if v > 0.0 { format!("{v}") } else { "Tự chọn".into() };
    let strings = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let elev = column![
        check("Đường đồng mức và điểm cao độ (dữ liệu toàn cầu ~30 m, để tham khảo)", "elevation", o.elevation),
        row![
            text("Khoảng cao đều đồng mức (m):").size(12).width(220),
            pick_list(Some(num(o.contour_step)), strings(&["Tự chọn", "0.5", "1", "2", "5", "10", "20"]), |v: &String| v.clone())
                .on_select(|v| Message::CadMini(Msg::Set("contour_step", v)))
                .padding([3, 6])
                .text_size(12)
                .width(120),
        ]
        .spacing(6)
        .align_y(iced::Center),
        row![
            text("Khoảng cách điểm cao độ (m):").size(12).width(220),
            pick_list(Some(num(o.spot_spacing)), strings(&["Tự chọn", "10", "20", "25", "50", "100"]), |v: &String| v.clone())
                .on_select(|v| Message::CadMini(Msg::Set("spot_spacing", v)))
                .padding([3, 6])
                .text_size(12)
                .width(120),
        ]
        .spacing(6)
        .align_y(iced::Center),
    ]
    .spacing(4);
    fn g<'a>(t: &'static str, b: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
        column![text(t).size(13), iced::widget::container(b).padding([4, 8])].spacing(2).into()
    }
    let mut body = column![
        text(f.info.as_str()).size(12),
        g("Đường giao thông (OpenStreetMap)", roads),
        g("Địa danh", places),
    ]
    .spacing(8);
    if !f.roads_only {
        body = body.push(g("Hiện trạng", state)).push(g("Cao độ địa hình (Terrarium/AWS)", elev));
    }
    body.push(check("Xoá dữ liệu đã lấy lần trước trên các lớp này", "clear", o.clear))
        .push(
            text("Nguồn: © OpenStreetMap (ODbL), Terrarium/AWS. Google Maps chỉ dùng làm ảnh nền, không cho lấy dữ liệu đường.")
                .size(11),
        )
        .push(
            row![
                Space::new().width(Fill),
                crate::ui::style::form::dialog_button("Đóng", Message::CadMini(Msg::Act("cancel")), false),
                crate::ui::style::form::dialog_button_styled_opt(
                    "Lấy dữ liệu",
                    Some(Message::CadMini(Msg::Act("ok"))),
                    iced::widget::button::primary,
                ),
            ]
            .spacing(8),
        )
        .width(Length::Fixed(600.0))
        .into()
}

impl OpenCADStudio {
    fn bando_options(&self, roads_only: bool) -> B::Options {
        let saved: Option<B::Options> = serde_json::from_str(&self.bandoht_options).ok();
        match (saved, roads_only) {
            (Some(o), false) => o,
            (Some(o), true) => B::Options { buildings: false, water: false, green: false, farm: false, rail_power: false, elevation: false, ..o },
            (None, true) => B::Options::roads_only(),
            (None, false) => B::Options::default(),
        }
    }

    fn bando_open(&mut self, i: usize, boundary: Vec<Xy>, roads_only: bool) -> Task<Message> {
        let Some(crs) = super::gis_cmd::drawing_crs(&self.tabs[i].scene.document) else {
            self.command_line.push_error("BANDOHT: bản vẽ chưa có hệ toạ độ — chọn tỉnh trong hộp thoại GISCRS rồi chạy lại.");
            self.open_crs_dialog(i);
            return Task::none();
        };
        let area = ring_area(&boundary);
        if area < 100.0 {
            self.command_line.push_error("BANDOHT: ranh quá nhỏ (dưới 100 m²).");
            return Task::none();
        }
        let info = format!("Phạm vi: {:.2} ha · {}", area / 10_000.0, crs.label());
        let f = BandoForm { opts: self.bando_options(roads_only), boundary, info, roads_only };
        self.cadmini_dialog = Some(Dialog::Bando(f));
        self.active_modal = Some(ModalKind::CadMini);
        Task::none()
    }

    pub(in crate::app) fn bando_msg(&mut self, i: usize, msg: Msg) -> Task<Message> {
        let Some(Dialog::Bando(f)) = self.cadmini_dialog.as_mut() else { return Task::none() };
        let o = &mut f.opts;
        match msg {
            Msg::Check(key, v) => match key {
                "roads" => o.roads = v,
                "road_names" => o.road_names = v,
                "paths" => o.paths = v,
                "buildings" => o.buildings = v,
                "water" => o.water = v,
                "green" => o.green = v,
                "farm" => o.farm = v,
                "rail_power" => o.rail_power = v,
                "elevation" => o.elevation = v,
                "clear" => o.clear = v,
                "places_on" => {
                    o.places = if v { B::POI.iter().map(|p| p.0.to_string()).collect() } else { Default::default() }
                }
                _ => {}
            },
            Msg::Set("place", v) => {
                let (on, key) = v.split_at(1);
                if on == "+" {
                    o.places.insert(key.to_string());
                } else {
                    o.places.remove(key);
                }
            }
            Msg::Set("contour_step", v) => o.contour_step = v.parse().unwrap_or(0.0),
            Msg::Set("spot_spacing", v) => o.spot_spacing = v.parse().unwrap_or(0.0),
            Msg::Act("ok") => {
                let (opts, boundary, roads_only) = (f.opts.clone(), f.boundary.clone(), f.roads_only);
                if !roads_only {
                    self.bandoht_options = serde_json::to_string(&opts).unwrap_or_default();
                    self.persist_settings_if_changed();
                }
                self.cadmini_dialog = None;
                self.close_active_modal();
                return self.bando_start(i, opts, boundary);
            }
            _ => {}
        }
        Task::none()
    }

    fn bando_start(&mut self, i: usize, opts: B::Options, boundary: Vec<Xy>) -> Task<Message> {
        let Some(crs) = super::gis_cmd::drawing_crs(&self.tabs[i].scene.document) else { return Task::none() };
        let tab = self.tabs[i].id;
        self.command_line.push_info("BANDOHT: đang tải dữ liệu bản đồ (chạy nền, vẫn vẽ được)…");
        #[cfg(not(target_arch = "wasm32"))]
        {
            Task::perform(
                async move {
                    let (tx, rx) = iced::futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let r = run(&crs, &boundary, &opts);
                        *DONE.lock().unwrap_or_else(|e| e.into_inner()) = Some((tab, r));
                        let _ = tx.send(());
                    });
                    let _ = rx.await;
                },
                |_| Message::Command("_BDHTDONE".into()),
            )
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (crs, tab, opts, boundary);
            Task::none()
        }
    }

    /// The finished run into the drawing, one undo step.
    fn bando_apply(&mut self) -> Task<Message> {
        let Some((tab, r)) = DONE.lock().unwrap_or_else(|e| e.into_inner()).take() else { return Task::none() };
        let Some(i) = self.tabs.iter().position(|t| t.id == tab) else {
            self.command_line.push_error("BANDOHT: bản vẽ đã đóng.");
            return Task::none();
        };
        let (items, notes, origin, clear) = match r {
            Ok(v) => v,
            Err(e) => {
                self.command_line.push_error(&format!("BANDOHT: {e}"));
                return Task::none();
            }
        };
        self.push_undo_snapshot(i, "BANDOHT");
        let used: std::collections::BTreeSet<&str> = items.iter().map(|it| it.layer()).collect();
        self.bando_layers(i, &used);
        if clear {
            let set: Vec<String> = used.iter().map(|l| l.to_lowercase()).collect();
            let old: Vec<Handle> = super::gpmb_cmd::model_entities(&self.tabs[i].scene)
                .iter()
                .filter(|e| set.contains(&e.common().layer.to_lowercase()))
                .map(|e| e.common().handle)
                .collect();
            if !old.is_empty() {
                self.tabs[i].scene.erase_entities(&old);
            }
        }
        let n_fill = items.iter().filter(|it| matches!(it, Item::Hatch { .. })).count();
        let entities: Vec<EntityType> = items.iter().map(|it| entity_of(it, origin)).collect();
        let n = entities.len();
        let handles = self.tabs[i].scene.add_entities(entities);
        self.tabs[i].dirty = true;
        self.command_line.push_output(&format!("BANDOHT: đã vẽ {n} đối tượng. {} (Ctrl+Z để bỏ)", notes.join("; ")));
        let fills: Vec<Handle> = handles.into_iter().take(n_fill).collect();
        if fills.is_empty() {
            Task::none()
        } else {
            self.apply_cmd_result(CmdResult::Relaunch("DRAWORDER BACK".to_string(), fills))
        }
    }

    fn bando_layers(&mut self, i: usize, used: &std::collections::BTreeSet<&str>) {
        crate::io::linetypes::ensure_catalog_linetype(&mut self.tabs[i].scene.document, "DASHED");
        let scene = &mut self.tabs[i].scene;
        let dashed = scene.document.line_types.get("DASHED").is_some();
        for (name, color) in B::LAYERS {
            if !used.contains(name) || scene.document.layers.contains(name) {
                continue;
            }
            scene.ensure_layer(name);
            if let Some(layer) = scene.document.layers.get_mut(name) {
                layer.color = codec::types::Color::from_index(*color);
                if *name == B::L_TIM && dashed {
                    layer.line_type = "DASHED".into();
                }
            }
        }
        self.sync_registered_layers(i);
    }

    pub(super) fn dispatch_bandoht(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim().trim_start_matches(['_', '.']);
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_uppercase(), r.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };
        match verb.as_str() {
            "BANDOHT" | "BDHT" | "OSMDUONG" if rest.is_empty() => {
                self.bando_roads_only = verb == "OSMDUONG";
                let c: Box<dyn CadCommand> = Box::new(PickThenRun {
                    name: "BANDOHT",
                    prompt: "BANDOHT  Chọn ranh (polyline kín, đường tròn), Enter — Enter không chọn: vẽ khung:",
                    run: "_BDHTRUN",
                    selected: Vec::new(),
                });
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(c);
                Some(Task::none())
            }
            "BDHTRUN" => {
                if let Ok(h) = u64::from_str_radix(rest, 16) {
                    self.tabs[i].scene.deselect_all();
                    self.tabs[i].scene.select_entity(Handle::new(h), false);
                }
                let ring = self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| closed_ring(e));
                self.tabs[i].scene.deselect_all();
                match ring {
                    Some(r) => Some(self.bando_open(i, r, self.bando_roads_only)),
                    None => {
                        let c: Box<dyn CadCommand> = Box::new(Corners { first: None });
                        self.command_line.push_info(&c.prompt());
                        self.tabs[i].active_cmd = Some(c);
                        Some(Task::none())
                    }
                }
            }
            "BDHTBOX" => {
                let v: Vec<f64> = rest.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if v.len() != 4 {
                    return Some(Task::none());
                }
                let (x0, y0, x1, y1) = (v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3]));
                Some(self.bando_open(i, vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], self.bando_roads_only))
            }
            "BDHTDONE" => Some(self.bando_apply()),
            _ => None,
        }
    }
}

fn entity_of(it: &Item, o: Xy) -> EntityType {
    use codec::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    use codec::types::{Color, Vector2, Vector3};
    let set = |mut e: EntityType, layer: &str| {
        e.common_mut().layer = layer.to_string();
        e
    };
    match it {
        Item::Pline { layer, pts, closed } => {
            let mut pl = codec::entities::LwPolyline::from_points(pts.iter().map(|p| Vector2::new(p[0] + o[0], p[1] + o[1])).collect());
            for (v, p) in pl.vertices.iter_mut().zip(pts) {
                v.bulge = p[2];
            }
            pl.is_closed = *closed;
            set(EntityType::LwPolyline(pl), layer)
        }
        Item::Text { layer, at, s, h, rot_deg, left } => {
            let mut t = codec::entities::Text::new();
            t.value = s.clone();
            t.height = *h;
            t.rotation = rot_deg.to_radians();
            let p = Vector3::new(at[0] + o[0], at[1] + o[1], 0.0);
            t.insertion_point = p;
            t.horizontal_alignment = if *left { H::Left } else { H::Center };
            t.vertical_alignment = V::Middle;
            t.alignment_point = Some(p);
            set(EntityType::Text(t), layer)
        }
        Item::Circle { layer, c, r } => {
            let mut ci = codec::entities::Circle::new();
            ci.center = Vector3::new(c[0] + o[0], c[1] + o[1], 0.0);
            ci.radius = *r;
            set(EntityType::Circle(ci), layer)
        }
        Item::Hatch { layer, rings, color } => {
            use codec::entities::{BoundaryEdge, BoundaryPath, BoundaryPathFlags, PolylineEdge};
            let mut h = codec::entities::Hatch::new();
            for (k, ring) in rings.iter().enumerate() {
                let bits = if k == 0 { 1 | 2 | 16 } else { 2 };
                let mut path = BoundaryPath::with_flags(BoundaryPathFlags::from_bits(bits));
                path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
                    ring.iter().map(|p| Vector2::new(p[0] + o[0], p[1] + o[1])).collect(),
                    true,
                )));
                h.paths.push(path);
            }
            h.pattern = codec::entities::HatchPattern::new("SOLID");
            h.pattern_type = codec::entities::HatchPatternType::Predefined;
            h.is_solid = true;
            let mut e = set(EntityType::Hatch(Box::new(h)), layer);
            e.common_mut().color = Color::from_index(*color);
            e
        }
    }
}

/// Download and build: the items in local coordinates, notes, the origin.
#[cfg(not(target_arch = "wasm32"))]
fn run(
    crs: &crate::gis::crs::DrawingCrs,
    ring: &[Xy],
    opts: &B::Options,
) -> Result<(Vec<Item>, Vec<String>, Xy, bool), String> {
    let say = |_: &str| {};
    run_with(crs, ring, opts, &|q| B::fetch_overpass(q, &say), &mut |ll| B::terrain_sample(ll, &say))
}

/// `run` with the downloads given (tests pass canned data).
pub(in crate::app) fn run_with(
    crs: &crate::gis::crs::DrawingCrs,
    ring: &[Xy],
    opts: &B::Options,
    overpass: &dyn Fn(&str) -> Result<serde_json::Value, String>,
    terrain: &mut dyn FnMut(&[Xy]) -> Result<Vec<f64>, String>,
) -> Result<(Vec<Item>, Vec<String>, Xy, bool), String> {
    let origin = [ring[0][0].round(), ring[0][1].round()];
    let local: Vec<geo::Coord<f64>> = ring.iter().chain(std::iter::once(&ring[0])).map(|p| geo::coord! { x: p[0] - origin[0], y: p[1] - origin[1] }).collect();
    let boundary = geo::MultiPolygon::new(vec![geo::Polygon::new(geo::LineString::new(local), vec![])]);
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in ring {
        x0 = x0.min(p[0] - 80.0);
        y0 = y0.min(p[1] - 80.0);
        x1 = x1.max(p[0] + 80.0);
        y1 = y1.max(p[1] + 80.0);
    }
    let corners: Vec<(f64, f64)> = [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [(x0 + x1) / 2.0, y0], [(x0 + x1) / 2.0, y1]]
        .iter()
        .map(|p| crs.to_wgs84(p[0], p[1]))
        .collect();
    let bbox = [
        corners.iter().map(|c| c.0).fold(f64::MAX, f64::min),
        corners.iter().map(|c| c.1).fold(f64::MAX, f64::min),
        corners.iter().map(|c| c.0).fold(f64::MIN, f64::max),
        corners.iter().map(|c| c.1).fold(f64::MIN, f64::max),
    ];
    if (bbox[2] - bbox[0]) * (bbox[3] - bbox[1]) > 0.06 {
        return Err("Phạm vi quá rộng (hơn khoảng 25 × 25 km) — chia nhỏ ranh để lấy từng phần.".into());
    }
    let to_xy = |lon: f64, lat: f64| {
        let (x, y) = crs.from_wgs84(lon, lat);
        [x - origin[0], y - origin[1]]
    };
    let parts = opts.osm_parts();
    let js = if parts.is_empty() { serde_json::json!({"elements": []}) } else { overpass(&B::overpass_query(bbox, &parts))? };
    let osm = B::Osm::parse(&js, &to_xy);
    let mut items = Vec::new();
    let mut notes = B::build(&osm, &boundary, opts, &mut items);
    if opts.elevation {
        let mut heights = |pts: &[Xy]| {
            let ll: Vec<Xy> = pts
                .iter()
                .map(|p| {
                    let (lon, lat) = crs.to_wgs84(p[0] + origin[0], p[1] + origin[1]);
                    [lon, lat]
                })
                .collect();
            terrain(&ll)
        };
        let (zmin, zmax, step) = B::elevation(&boundary, &mut heights, opts, &mut items)?;
        notes.push(format!("Cao độ {zmin:.1}–{zmax:.1} m, đồng mức cách {step} m (SRTM/Copernicus ~30 m, tham khảo)"));
    }
    Ok((items, notes, origin, opts.clear))
}

inventory::submit!(crate::command::CommandRegistration { names: &["BANDOHT", "BDHT", "OSMDUONG"] });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_map_lands_in_the_drawing() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        let crs = crate::gis::crs::DrawingCrs::parse_user("Hà Tĩnh").unwrap();
        let (cx, cy) = (542_100.0, 2_029_700.0);
        let ring = vec![[cx - 150.0, cy - 100.0], [cx + 150.0, cy - 100.0], [cx + 150.0, cy + 150.0], [cx - 150.0, cy + 150.0]];
        // A road through the middle and a house, given in lon/lat.
        let ll = |x: f64, y: f64| {
            let (lon, lat) = crs.to_wgs84(cx + x, cy + y);
            serde_json::json!({"lon": lon, "lat": lat})
        };
        let js = serde_json::json!({"elements": [
            {"type": "way", "id": 1, "tags": {"highway": "primary", "name": "Quốc lộ 1"}, "nodes": [1, 2],
             "geometry": [ll(-300.0, 0.0), ll(300.0, 0.0)]},
            {"type": "way", "id": 2, "tags": {"building": "yes"}, "nodes": [3, 4, 5, 6, 3],
             "geometry": [ll(20.0, 20.0), ll(40.0, 20.0), ll(40.0, 35.0), ll(20.0, 35.0), ll(20.0, 20.0)]},
        ]});
        let opts = B::Options { elevation: true, ..B::Options::default() };
        let r = run_with(&crs, &ring, &opts, &|q| {
            assert!(q.contains("highway"));
            Ok(js.clone())
        }, &mut |ll| Ok(ll.iter().map(|p| 10.0 + (p[0] - 105.0) * 1000.0).collect()))
        .unwrap();
        assert!(r.1.iter().any(|n| n.contains("đoạn đường")), "{:?}", r.1);
        *DONE.lock().unwrap() = Some((app.tabs[i].id, Ok(r)));
        let _ = app.dispatch_command("_BDHTDONE");
        let ents = super::super::gpmb_cmd::model_entities(&app.tabs[i].scene);
        let on = |l: &str| ents.iter().filter(|e| e.common().layer == l).count();
        assert!(on(B::L_TIM) >= 1 && on(B::L_MEP) >= 2 && on(B::L_NHA) == 1, "{} {} {}", on(B::L_TIM), on(B::L_MEP), on(B::L_NHA));
        assert!(ents.iter().any(|e| matches!(e, EntityType::Text(t) if t.value == "Quốc lộ 1")));
        // In drawing coordinates: the house's corner lands within a few cm.
        let house = ents.iter().find(|e| e.common().layer == B::L_NHA).unwrap();
        let EntityType::LwPolyline(pl) = house else { panic!() };
        assert!(pl.vertices.iter().any(|v| (v.location.x - (cx + 20.0)).abs() < 0.05 && (v.location.y - (cy + 20.0)).abs() < 0.05));
        // Contours from the sloping terrain.
        assert!(on(B::L_DM) + on(B::L_DM_CAI) > 0);
        let layer = app.tabs[i].scene.document.layers.get(B::L_MEP).unwrap();
        assert_eq!(layer.color, codec::types::Color::from_index(6));
        // One undo step takes it all away.
        let _ = app.update(Message::Undo);
        let ents = super::super::gpmb_cmd::model_entities(&app.tabs[i].scene);
        assert!(ents.iter().all(|e| e.common().layer != B::L_TIM));
    }

    #[test]
    fn the_dialog_needs_a_coordinate_system() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let _ = app.dispatch_command("_BDHTBOX 0,0,100,100");
        assert!(matches!(app.cadmini_dialog, Some(Dialog::Crs(_))));
        let _ = app.dispatch_command("GISCRS Hà Tĩnh");
        app.cadmini_dialog = None;
        app.active_modal = None;
        let _ = app.dispatch_command("_BDHTBOX 0,0,100,100");
        let Some(Dialog::Bando(f)) = &app.cadmini_dialog else { panic!() };
        assert!(f.info.contains("1.00 ha"));
        let i = app.active_tab;
        let _ = app.bando_msg(i, Msg::Check("water", false));
        let _ = app.bando_msg(i, Msg::Set("place", "-quan_an".into()));
        let Some(Dialog::Bando(f)) = &app.cadmini_dialog else { panic!() };
        assert!(!f.opts.water && !f.opts.places.contains("quan_an"));
    }
}
