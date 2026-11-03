//! TRICHLUC — the parcel extract sheet, as CadMini (see `crate::trichluc`).
//!
//! - TRICHLUC   pick a closed polyline (or any part of a sheet, to edit it),
//!              then set the sheet up on the command line and Draw
//! - TLPDF      every extract sheet of the drawing into one PDF, a page each,
//!              true paper size and scale
//! - GPMB4      (GPMB B4) a sheet for every parcel GPMB B3 found taken, in a
//!              grid right of the drawing, with the taken parts hatched
//!
//! The sheet's parameters live in the polyline's XDATA (`CADMINI_TL`); each
//! sheet entity carries the sheet id, so re-running TRICHLUC on either redraws
//! that sheet in place.

use crate::app::commands::gpmb_cmd::{hatch, model_entities, on_layer, PickThenRun};
use crate::app::{Message, OpenCADStudio};
use crate::command::{CadCommand, CmdOption, CmdResult, FreeTextValuePromptCommand};
use crate::gpmb as G;
use crate::trichluc::{self as T, Params, Tag, Vtx, Xy};
use codec::{EntityType, Handle};
use glam::DVec3;
use iced::Task;

/// The sheet being set up: its source polyline and parameters.
pub(in crate::app) struct Pending {
    tab: u64,
    src: u64,
    params: Params,
    editing: bool,
}

const MENU: &[(&str, &str)] = &[
    ("Draw", "DRAW"),
    ("Parcel Info", "INFO"),
    ("Signatures", "SIGN"),
    ("A4/A3", "PAPER"),
    ("Portrait/Landscape", "ORIENT"),
    ("Scale", "SCALE"),
    ("Beside/In Place", "PLACE"),
    ("Neighbors", "NEIGHBORS"),
    ("Edge Lengths", "EDGES"),
];

const INFO_FIELDS: &[(&str, &str)] = &[
    ("so_thua", "Parcel number (số thửa)"),
    ("to_ban_do", "Map sheet (tờ bản đồ)"),
    ("muc_dich", "Land use (mục đích sử dụng)"),
    ("chu", "Land user (người sử dụng đất)"),
    ("dia_chi", "Address (địa chỉ thửa đất)"),
    ("ghi_chu", "Note (ghi chú)"),
];

const SIGN_FIELDS: &[(&str, &str)] = &[
    ("tieu_de", "Title (tiêu đề)"),
    ("don_vi", "Office (đơn vị)"),
    ("dia_danh", "Place (địa danh)"),
    ("ngay", "Date dd/mm/yyyy (ngày)"),
    ("nguoi_lap", "Drawn by (người trích lục)"),
    ("thu_truong", "Head of office (thủ trưởng)"),
    ("he_toa_do", "Coordinate system (hệ tọa độ)"),
];

/// The set-up menu: the sheet as it stands, its options as buttons.
struct Menu {
    summary: String,
    /// The SCALE option is waiting for its value.
    scale: bool,
}

impl CadCommand for Menu {
    fn name(&self) -> &'static str {
        "TRICHLUC"
    }

    fn prompt(&self) -> String {
        if self.scale {
            return crate::t!("TRICHLUC  Scale 1:<n> (0 = smallest that fits):").into_owned();
        }
        format!("TRICHLUC  {}  <Draw>:", self.summary)
    }

    fn options(&self) -> Vec<CmdOption> {
        if self.scale {
            return Vec::new();
        }
        MENU.iter().map(|(label, kw)| CmdOption::new(label, kw)).collect()
    }

    fn wants_text_input(&self) -> bool {
        true
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        if self.scale {
            let n = t.trim_start_matches("1:");
            return Some(CmdResult::Dispatch(format!("_TLSET scale {n}")));
        }
        let t = t.strip_prefix('_').unwrap_or(t);
        let up = t.to_uppercase();
        let exact = MENU.iter().find(|(l, k)| k.eq_ignore_ascii_case(&up) || l.eq_ignore_ascii_case(t));
        let prefix: Vec<_> = MENU
            .iter()
            .filter(|(l, k)| k.starts_with(&up) || l.to_uppercase().starts_with(&up))
            .collect();
        let Some((_, kw)) = exact.or(if prefix.len() == 1 { Some(prefix[0]) } else { None }) else {
            return Some(CmdResult::NeedPoint);
        };
        if *kw == "SCALE" {
            self.scale = true;
            return Some(CmdResult::NeedPoint);
        }
        Some(CmdResult::Dispatch(format!("_TLSET {kw}")))
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.scale {
            self.scale = false;
            return CmdResult::NeedPoint;
        }
        CmdResult::Dispatch("_TLSET DRAW".into())
    }
}

/// Ask a list of fields in turn; Enter keeps the value shown.
struct Fields {
    fields: &'static [(&'static str, &'static str)],
    params: Params,
    step: usize,
}

impl Fields {
    fn advance(&mut self) -> CmdResult {
        self.step += 1;
        if self.step >= self.fields.len() {
            let json = serde_json::to_string(&self.params).unwrap_or_default();
            return CmdResult::Dispatch(format!("_TLJSON {json}"));
        }
        CmdResult::NeedPoint
    }
}

impl CadCommand for Fields {
    fn name(&self) -> &'static str {
        "TRICHLUC"
    }

    fn prompt(&self) -> String {
        let (key, label) = self.fields[self.step.min(self.fields.len() - 1)];
        let mut s = format!("TRICHLUC  {} <{}>:", crate::t!(label), self.params.get(key));
        if key == "muc_dich" {
            s.push_str("  (");
            s.push_str(&T::LAND_USES.iter().map(|u| T::land_code(u)).collect::<Vec<_>>().join(" "));
            s.push(')');
        }
        s
    }

    fn input_kind(&self) -> crate::command::InputKind {
        crate::command::InputKind::FreeText
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let (key, _) = self.fields[self.step];
        let mut v = text.trim().to_string();
        // A bare land-use code becomes its full name.
        if key == "muc_dich" {
            if let Some(full) = T::LAND_USES.iter().find(|u| T::land_code(u).eq_ignore_ascii_case(&v)) {
                v = full.to_string();
            }
        }
        // "-" clears a field.
        if v == "-" {
            v.clear();
        }
        self.params.set(key, &v);
        Some(self.advance())
    }

    fn on_enter(&mut self) -> CmdResult {
        self.advance()
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }
}

// ── Reading ─────────────────────────────────────────────────────────────────

/// A closed LWPOLYLINE's vertices with bulges (a last vertex on the first
/// one closes it).
fn ring_of(e: &EntityType) -> Option<Vec<Vtx>> {
    let EntityType::LwPolyline(pl) = e else { return None };
    let mut pts: Vec<Vtx> = pl.vertices.iter().map(|v| [v.location.x, v.location.y, v.bulge]).collect();
    let mut closed = pl.is_closed;
    if pts.len() > 1 {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        if (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9 {
            pts.pop();
            closed = true;
        }
    }
    (closed && pts.len() >= 3).then_some(pts)
}

fn tag_of(e: &EntityType) -> Option<Tag> {
    let record = e.common().extended_data.get_record(T::APPID)?;
    let strings: Vec<String> = record
        .values
        .iter()
        .filter_map(|v| match v {
            codec::xdata::XDataValue::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    T::tag_from_xdata(&strings)
}

fn set_tag(e: &mut EntityType, strings: Vec<String>) {
    let mut record = codec::xdata::ExtendedDataRecord::new(T::APPID);
    for s in strings {
        record.add_value(codec::xdata::XDataValue::String(s));
    }
    e.common_mut().extended_data.upsert_record(record);
}

fn gpmb_data(e: &EntityType) -> Option<G::ParcelData> {
    let record = e.common().extended_data.get_record(G::APPID)?;
    let strings: Vec<String> = record
        .values
        .iter()
        .filter_map(|v| match v {
            codec::xdata::XDataValue::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    G::data_from_xdata(&strings)
}

/// The points that bound an entity worth copying as a neighbour.
fn key_points(e: &EntityType) -> Option<Vec<Xy>> {
    Some(match e {
        EntityType::Line(l) => vec![[l.start.x, l.start.y], [l.end.x, l.end.y]],
        EntityType::LwPolyline(pl) => pl.vertices.iter().map(|v| [v.location.x, v.location.y]).collect(),
        EntityType::Circle(c) => vec![
            [c.center.x - c.radius, c.center.y - c.radius],
            [c.center.x + c.radius, c.center.y + c.radius],
        ],
        EntityType::Arc(a) => vec![[a.center.x - a.radius, a.center.y - a.radius], [a.center.x + a.radius, a.center.y + a.radius]],
        EntityType::Text(t) => {
            // The words' extent, roughly: 0.8 height a character.
            use codec::entities::TextHorizontalAlignment as H;
            let w = t.value.chars().count() as f64 * t.height * 0.8;
            let at = match (&t.horizontal_alignment, t.alignment_point) {
                (H::Left, _) | (_, None) => t.insertion_point,
                (_, Some(a)) => a,
            };
            let (x0, x1) = match t.horizontal_alignment {
                H::Left => (at.x, at.x + w),
                H::Right => (at.x - w, at.x),
                _ => (at.x - w / 2.0, at.x + w / 2.0),
            };
            vec![[x0, at.y - t.height], [x1, at.y + t.height]]
        }
        EntityType::MText(t) => {
            // Any attachment: the words may lie on every side of the point.
            let plain = crate::app::commands::gpmb_cmd::plain_mtext(&t.value);
            let w = if t.rectangle_width > 0.0 { t.rectangle_width } else { plain.chars().count() as f64 * t.height * 0.8 };
            let h = t.height * (1.0 + t.value.matches("\\P").count() as f64) * 1.7;
            let p = t.insertion_point;
            vec![[p.x - w, p.y - h], [p.x + w, p.y + h]]
        }
        EntityType::Point(p) => vec![[p.location.x, p.location.y]],
        EntityType::Insert(i) if i.attributes.is_empty() => vec![[i.insert_point.x, i.insert_point.y]],
        _ => return None,
    })
}

/// Whether `q` lies inside the ring (its chords).
fn inside(ring: &[Vtx], q: Xy) -> bool {
    let n = ring.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
            c = !c;
        }
    }
    c
}

// ── Writing ─────────────────────────────────────────────────────────────────

fn entity_of(item: &T::Item) -> EntityType {
    use codec::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let lw = |e: &mut EntityType, lw: i16| {
        if lw > 0 {
            e.common_mut().line_weight = codec::types::LineWeight::Value(lw);
        }
    };
    match item {
        T::Item::Text { at, s, h, center, layer, rot_deg } => {
            let mut t = codec::entities::Text::new();
            t.value = s.clone();
            t.height = *h;
            t.rotation = rot_deg.to_radians();
            t.insertion_point = codec::types::Vector3::new(at[0], at[1], 0.0);
            if *center {
                t.horizontal_alignment = H::Center;
                t.vertical_alignment = V::Middle;
                t.alignment_point = Some(codec::types::Vector3::new(at[0], at[1], 0.0));
            }
            on_layer(EntityType::Text(t), layer, None)
        }
        T::Item::Line { a, b, layer, lw: w } => {
            let mut l = codec::entities::Line::new();
            l.start = codec::types::Vector3::new(a[0], a[1], 0.0);
            l.end = codec::types::Vector3::new(b[0], b[1], 0.0);
            let mut e = on_layer(EntityType::Line(l), layer, None);
            lw(&mut e, *w);
            e
        }
        T::Item::Poly { pts, closed, layer, lw: w } => {
            let mut pl = codec::entities::LwPolyline::from_points(
                pts.iter().map(|p| codec::types::Vector2::new(p[0], p[1])).collect(),
            );
            for (v, p) in pl.vertices.iter_mut().zip(pts) {
                v.bulge = p[2];
            }
            pl.is_closed = *closed;
            let mut e = on_layer(EntityType::LwPolyline(pl), layer, None);
            lw(&mut e, *w);
            e
        }
        T::Item::Circle { c, r, layer } => {
            let mut ci = codec::entities::Circle::new();
            ci.center = codec::types::Vector3::new(c[0], c[1], 0.0);
            ci.radius = *r;
            on_layer(EntityType::Circle(ci), layer, None)
        }
        T::Item::Hatch { pts, layer, spacing } => {
            let ring: Vec<geo::Coord<f64>> = pts.iter().map(|p| geo::coord! { x: p[0], y: p[1] }).collect();
            let poly = geo::Polygon::new(geo::LineString::new(ring), Vec::new());
            hatch(&poly, layer, "ANSI31", &[45.0], *spacing)
        }
    }
}

impl OpenCADStudio {
    fn tl_ensure_layers(&mut self, i: usize) {
        let scene = &mut self.tabs[i].scene;
        for (name, color) in T::LAYERS {
            if !scene.document.layers.contains(name) {
                scene.ensure_layer(name);
                if let Some(layer) = scene.document.layers.get_mut(name) {
                    layer.color = codec::types::Color::from_index(*color);
                }
            }
        }
        self.sync_registered_layers(i);
    }

    /// The source polyline and parameters behind the picked entities.
    fn tl_resolve(&self, i: usize, picked: &[(Handle, EntityType)]) -> Option<(Handle, Option<Params>)> {
        let model = model_entities(&self.tabs[i].scene);
        for (h, e) in picked {
            match tag_of(e) {
                Some(Tag::Source(_, Some(p))) if ring_of(e).is_some() => return Some((*h, Some(p))),
                Some(Tag::Sheet(id)) => {
                    let src = model.iter().find_map(|m| match tag_of(m) {
                        Some(Tag::Source(sid, Some(p))) if sid == id && ring_of(m).is_some() => {
                            Some((m.common().handle, Some(p)))
                        }
                        _ => None,
                    });
                    if src.is_some() {
                        return src;
                    }
                }
                _ if ring_of(e).is_some() => return Some((*h, None)),
                _ => {}
            }
        }
        None
    }

    fn tl_show_menu(&mut self, i: usize) -> Task<Message> {
        let Some(pd) = &self.trichluc else { return Task::none() };
        let p = &pd.params;
        let scale = if p.scale > 0 { format!("1:{}", p.scale) } else { "1:auto".into() };
        let summary = format!(
            "thửa {} tờ {} · {} {} · {} · {}{}{}",
            if p.so_thua.is_empty() { "?" } else { &p.so_thua },
            if p.to_ban_do.is_empty() { "?" } else { &p.to_ban_do },
            p.paper,
            if p.landscape { "ngang" } else { "đứng" },
            scale,
            if p.side() { "bên cạnh" } else { "tại chỗ" },
            if p.side() && p.neighbors { ", chép thửa liền kề" } else { "" },
            if p.edge_len { ", ghi cạnh" } else { "" },
        );
        let c: Box<dyn CadCommand> = Box::new(Menu { summary, scale: false });
        self.command_line.push_info(&c.prompt());
        self.tabs[i].active_cmd = Some(c);
        Task::none()
    }

    /// Draw (or redraw) one sheet, without an undo step of its own. Returns
    /// the plan, or None when the source is gone or not closed.
    fn tl_commit(
        &mut self,
        i: usize,
        src: Handle,
        p: &mut Params,
        editing: bool,
        extra: &[Vec<Xy>],
    ) -> Option<T::Plan> {
        let src_entity = self.tabs[i].scene.document.get_entity(src)?.clone();
        let ring = ring_of(&src_entity)?;
        self.tl_ensure_layers(i);
        if editing {
            let old: Vec<Handle> = model_entities(&self.tabs[i].scene)
                .iter()
                .filter(|e| matches!(tag_of(e), Some(Tag::Sheet(id)) if id == p.id))
                .map(|e| e.common().handle)
                .collect();
            if !old.is_empty() {
                self.tabs[i].scene.erase_entities(&old);
            }
        }
        let extents = if p.side() && p.base.is_none() {
            self.tabs[i]
                .scene
                .model_space_extents()
                .map(|(a, b)| [a.x as f64, a.y as f64, b.x as f64, b.y as f64])
        } else {
            None
        };
        let (items, plan, _) = T::build_sheet(&ring, p, extents, extra);
        let tag = vec![p.id.clone()];
        let mut made: Vec<EntityType> = items
            .iter()
            .map(|it| {
                let mut e = entity_of(it);
                set_tag(&mut e, tag.clone());
                e
            })
            .collect();
        // The neighbours: what lies wholly in the map frame, copied beside.
        if p.side() && p.neighbors {
            let [x0, y0, x1, y1] = plan.map_box_at_source();
            let delta = DVec3::new(plan.shift[0], plan.shift[1], 0.0);
            let own_label = !p.so_thua.is_empty();
            for e in model_entities(&self.tabs[i].scene) {
                if made.len() > 5000 {
                    break;
                }
                let h = e.common().handle;
                let layer = e.common().layer.to_lowercase();
                if h == src || tag_of(&e).is_some() || layer.starts_with("tl_") || layer.starts_with("gpmb_") {
                    continue;
                }
                let Some(pts) = key_points(&e) else { continue };
                // The parcel's own words: the sheet writes its label itself.
                if own_label && matches!(e, EntityType::Text(_) | EntityType::MText(_)) {
                    let at = match &e {
                        EntityType::Text(t) => [t.insertion_point.x, t.insertion_point.y],
                        EntityType::MText(t) => [t.insertion_point.x, t.insertion_point.y],
                        _ => unreachable!(),
                    };
                    if inside(&ring, at) {
                        continue;
                    }
                }
                if pts.is_empty() || !pts.iter().all(|q| q[0] >= x0 && q[0] <= x1 && q[1] >= y0 && q[1] <= y1) {
                    continue;
                }
                let mut c = e.clone();
                c.common_mut().handle = Handle::NULL;
                crate::scene::view::dispatch::apply_transform(&mut c, &crate::command::EntityTransform::Translate(delta));
                set_tag(&mut c, tag.clone());
                made.push(c);
            }
        }
        self.tabs[i].scene.add_entities(made);
        if p.side() {
            p.base = Some(plan.base);
        }
        let mut src_entity = src_entity;
        set_tag(&mut src_entity, T::params_to_xdata(p));
        self.tabs[i].scene.update_entity(src_entity);
        self.tabs[i].dirty = true;
        Some(plan)
    }

    fn tl_zoom(&mut self, b: [f64; 4]) {
        let (mx, my) = ((b[2] - b[0]) * 0.04, (b[3] - b[1]) * 0.04);
        self.handle_zoom_to_window(DVec3::new(b[0] - mx, b[1] - my, 0.0), DVec3::new(b[2] + mx, b[3] + my, 0.0));
    }

    fn tl_draw(&mut self, i: usize) -> Task<Message> {
        let Some(pd) = self.trichluc.take() else { return Task::none() };
        if pd.tab != self.tabs[i].id {
            return Task::none();
        }
        let mut p = pd.params;
        let src = Handle::new(pd.src);
        // A parcel GPMB B3 found taken: its taken parts, hatched.
        let extra: Vec<Vec<Xy>> = self
            .gpmb
            .get(&pd.tab)
            .and_then(|st| st.rows.iter().find(|r| r.handle == pd.src))
            .map(|r| r.parts.iter().filter_map(|g| G::rings(g).into_iter().next()).collect())
            .unwrap_or_default();
        self.push_undo_snapshot(i, "TRICHLUC");
        let Some(plan) = self.tl_commit(i, src, &mut p, pd.editing, &extra) else {
            self.discard_last_undo_entry(i);
            self.command_line.push_error(crate::t!("TRICHLUC: the source polyline is gone or no longer closed.").as_ref());
            return Task::none();
        };
        self.trichluc_defaults = p.shared_json();
        self.persist_settings_if_changed();
        let ring = self.tabs[i].scene.document.get_entity(src).and_then(ring_of).unwrap_or_default();
        self.tabs[i].scene.deselect_all();
        self.tl_zoom(plan.sheet_box());
        self.command_line.push_output(&format!(
            "TRICHLUC: {} bản trích lục — khổ {} {}, tỉ lệ 1:{}, {} đỉnh, S = {} m². Chạy lại TRICHLUC và chọn bản vẽ để sửa; TLPDF xuất PDF.",
            if pd.editing { "đã cập nhật" } else { "đã tạo" },
            p.paper,
            if p.landscape { "ngang" } else { "đứng" },
            plan.scale,
            ring.len(),
            T::fmt(T::ring_area(&ring).abs(), 1, p.comma),
        ));
        Task::none()
    }

    /// Every sheet of the drawing: its paper box (model units) and size (mm),
    /// top row first, left to right.
    fn tl_sheets(&self, i: usize) -> Vec<([f64; 4], Xy)> {
        let mut out: Vec<([f64; 4], Xy)> = model_entities(&self.tabs[i].scene)
            .iter()
            .filter_map(|e| match (tag_of(e), ring_of(e)) {
                (Some(Tag::Source(_, Some(p))), Some(ring)) => {
                    let plan = T::plan(&ring, &p, None);
                    Some((plan.sheet_box(), plan.layout.paper))
                }
                _ => None,
            })
            .collect();
        // Rows of a grid share their bottom edge.
        out.sort_by(|a, b| {
            let row = |x: &([f64; 4], Xy)| (x.0[1] * 1000.0).round();
            row(b).total_cmp(&row(a)).then(a.0[0].total_cmp(&b.0[0]))
        });
        out
    }

    fn tl_pdf(&mut self, i: usize, path: &str) -> Task<Message> {
        let sheets = self.tl_sheets(i);
        if sheets.is_empty() {
            self.command_line.push_error(crate::t!("TLPDF: the drawing has no extract sheet (TRICHLUC).").as_ref());
            return Task::none();
        }
        let pages: Vec<_> = sheets.iter().map(|(b, paper)| self.sheet_plot_page(*b, *paper)).collect();
        match crate::io::pdf_export::export_pdf_pages(&pages, std::path::Path::new(path), None) {
            Ok(()) => {
                self.command_line.push_output(&format!("TLPDF: {} trang → {path}", pages.len()));
                crate::sys::open_url(&format!("file://{path}"), self.main_window)
            }
            Err(e) => {
                self.command_line.push_error(&format!("TLPDF: không xuất được {path}: {e}"));
                Task::none()
            }
        }
    }

    /// GPMB B4: a sheet for each parcel B3 found taken.
    fn gpmb_step4(&mut self, i: usize) -> Task<Message> {
        let id = self.tabs[i].id;
        let rows: Vec<G::Row> = self.gpmb.get(&id).map(|st| st.rows.clone()).unwrap_or_default();
        if rows.is_empty() {
            self.command_line.push_error(crate::t!("GPMB: run B3 first.").as_ref());
            return Task::none();
        }
        let base = Params::with_shared(&self.trichluc_defaults);
        let extents = self.tabs[i]
            .scene
            .model_space_extents()
            .map(|(a, b)| [a.x as f64, a.y as f64, b.x as f64, b.y as f64]);
        // Each sheet's parameters and size, then their places in a grid.
        let mut jobs: Vec<(Handle, Params, bool, Vec<Vec<Xy>>, Xy)> = Vec::new();
        for r in &rows {
            let h = Handle::new(r.handle);
            let Some(e) = self.tabs[i].scene.document.get_entity(h) else { continue };
            let Some(ring) = ring_of(e) else { continue };
            let old = match tag_of(e) {
                Some(Tag::Source(_, Some(p))) => Some(p),
                _ => None,
            };
            let c = base.comma;
            let mut p = Params {
                so_thua: r.data.so_thua.clone(),
                to_ban_do: r.data.to_ban_do.clone(),
                chu: r.data.chu.clone(),
                muc_dich: r.data.loai_dat.clone(),
                dia_chi: if r.data.dia_chi.is_empty() { base.dia_chi.clone() } else { r.data.dia_chi.clone() },
                ghi_chu: format!(
                    "Diện tích thu hồi {} m² ({}); còn lại {} m²",
                    T::fmt(r.dt_thu_hoi, 1, c),
                    r.ghi_chu.to_lowercase(),
                    T::fmt(r.dt_con_lai, 1, c)
                ),
                placement: "side".into(),
                // Only the parcel itself, not its neighbours.
                neighbors: false,
                base: old.as_ref().and_then(|o| o.base),
                ..base.clone()
            };
            p.id = old.as_ref().map_or_else(|| T::new_id(r.handle), |o| o.id.clone());
            let plan = T::plan(&ring, &Params { base: None, ..p.clone() }, extents);
            let [w, hh] = plan.layout.paper;
            let extra: Vec<Vec<Xy>> = r.parts.iter().filter_map(|g| G::rings(g).into_iter().next()).collect();
            jobs.push((h, p, old.is_some(), extra, [w * plan.k, hh * plan.k]));
        }
        if jobs.is_empty() {
            self.command_line.push_error(crate::t!("GPMB B4: no taken parcel is a closed polyline.").as_ref());
            return Task::none();
        }
        let bases = T::grid_bases(&jobs.iter().map(|j| j.4).collect::<Vec<_>>(), extents, 5);
        self.push_undo_snapshot(i, "GPMB4");
        let mut all = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut n = 0;
        for ((h, mut p, editing, extra, _), b) in jobs.into_iter().zip(bases) {
            // A sheet drawn before stays where it is.
            p.base = Some(p.base.unwrap_or(b));
            if let Some(plan) = self.tl_commit(i, h, &mut p, editing, &extra) {
                let s = plan.sheet_box();
                all = [all[0].min(s[0]), all[1].min(s[1]), all[2].max(s[2]), all[3].max(s[3])];
                n += 1;
            }
        }
        self.tabs[i].scene.deselect_all();
        if n > 0 {
            self.tl_zoom(all);
        }
        self.command_line.push_output(&format!(
            "GPMB B4: {n} bản trích lục thửa thu hồi (phần thu hồi gạch đỏ). TLPDF xuất tất cả ra một file PDF; TRICHLUC trên một tờ để sửa."
        ));
        Task::none()
    }

    pub(super) fn dispatch_trichluc(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim().trim_start_matches(['_', '.']);
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_uppercase(), r.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };
        match verb.as_str() {
            "TRICHLUC" | "TL" if rest.is_empty() => {
                let c: Box<dyn CadCommand> = Box::new(PickThenRun {
                    name: "TRICHLUC",
                    prompt: "TRICHLUC  Pick a closed polyline (or an extract sheet to edit), Enter:",
                    run: "_TLRUN",
                    selected: Vec::new(),
                });
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(c);
                Some(Task::none())
            }
            "TLRUN" => {
                // A script names the polyline by its handle.
                if let Ok(h) = u64::from_str_radix(rest.trim_start_matches("0x").trim_start_matches("0X"), 16) {
                    self.tabs[i].scene.deselect_all();
                    self.tabs[i].scene.select_entity(Handle::new(h), false);
                }
                let picked: Vec<(Handle, EntityType)> =
                    self.tabs[i].scene.selected_entities().into_iter().map(|(h, e)| (h, e.clone())).collect();
                self.tabs[i].scene.deselect_all();
                let Some((src, saved)) = self.tl_resolve(i, &picked) else {
                    self.command_line.push_error(crate::t!("TRICHLUC: no closed polyline was picked.").as_ref());
                    return Some(Task::none());
                };
                let editing = saved.is_some();
                let params = saved.unwrap_or_else(|| {
                    let mut p = Params::with_shared(&self.trichluc_defaults);
                    p.id = T::new_id(src.value());
                    // What GPMB knows of the parcel.
                    let data = self.tabs[i].scene.document.get_entity(src).and_then(gpmb_data).or_else(|| {
                        self.gpmb
                            .get(&self.tabs[i].id)
                            .and_then(|st| st.parcels.iter().find(|q| q.handle == src.value()))
                            .map(|q| q.data.clone())
                    });
                    if let Some(d) = data {
                        p.so_thua = d.so_thua;
                        p.to_ban_do = d.to_ban_do;
                        p.chu = d.chu;
                        p.muc_dich = T::LAND_USES
                            .iter()
                            .find(|u| T::land_code(u).eq_ignore_ascii_case(&d.loai_dat))
                            .map_or(d.loai_dat, |u| u.to_string());
                        p.dia_chi = d.dia_chi;
                        p.ghi_chu = d.ghi_chu;
                    }
                    p
                });
                self.trichluc = Some(Pending { tab: self.tabs[i].id, src: src.value(), params, editing });
                Some(self.tl_show_menu(i))
            }
            "TLSET" => {
                let Some(pd) = self.trichluc.as_mut() else { return Some(Task::none()) };
                let (key, value) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(k, v)| (k, v.trim()));
                let p = &mut pd.params;
                match key.to_ascii_uppercase().as_str() {
                    "DRAW" | "" => return Some(self.tl_draw(i)),
                    "INFO" | "SIGN" => {
                        let fields = if key.eq_ignore_ascii_case("INFO") { INFO_FIELDS } else { SIGN_FIELDS };
                        let c: Box<dyn CadCommand> = Box::new(Fields { fields, params: p.clone(), step: 0 });
                        self.command_line.push_info(&c.prompt());
                        self.tabs[i].active_cmd = Some(c);
                        return Some(Task::none());
                    }
                    "PAPER" => {
                        p.paper = if p.paper.eq_ignore_ascii_case("A4") { "A3".into() } else { "A4".into() };
                        p.base = None;
                    }
                    "ORIENT" => {
                        p.landscape = !p.landscape;
                        p.base = None;
                    }
                    "PLACE" => {
                        p.placement = if p.side() { "here".into() } else { "side".into() };
                        p.base = None;
                    }
                    "NEIGHBORS" => p.neighbors = !p.neighbors,
                    "EDGES" => p.edge_len = !p.edge_len,
                    _ => {
                        if !p.set(&key.to_lowercase(), value) {
                            self.command_line.push_error(&format!("TRICHLUC: không đặt được {key} = {value}"));
                        }
                    }
                }
                Some(self.tl_show_menu(i))
            }
            "TLJSON" => {
                if let (Some(pd), Ok(p)) = (self.trichluc.as_mut(), serde_json::from_str::<Params>(rest)) {
                    pd.params = p;
                }
                Some(self.tl_show_menu(i))
            }
            "TLPDF" if rest.is_empty() => {
                if self.tl_sheets(i).is_empty() {
                    self.command_line.push_error(crate::t!("TLPDF: the drawing has no extract sheet (TRICHLUC).").as_ref());
                    return Some(Task::none());
                }
                if crate::sys::file_dia() {
                    let name = format!("{}_TrichLuc.pdf", self.tabs[i].tab_display_name());
                    Some(Task::perform(
                        async move {
                            let handle = crate::sys::file_dialog()
                                .set_title("TLPDF")
                                .add_filter("PDF (*.pdf)", &["pdf"])
                                .set_file_name(&name)
                                .save_file()
                                .await?;
                            Some(crate::sys::handle_path(&handle).to_string_lossy().into_owned())
                        },
                        |path| match path {
                            Some(path) => Message::Command(format!("_TLPDFFILE {path}")),
                            None => Message::Noop,
                        },
                    ))
                } else {
                    let c: Box<dyn CadCommand> = Box::new(FreeTextValuePromptCommand::new("_TLPDFFILE", "TLPDF  PDF file:"));
                    self.command_line.push_info(&c.prompt());
                    self.tabs[i].active_cmd = Some(c);
                    Some(Task::none())
                }
            }
            "TLPDFFILE" if !rest.is_empty() => {
                let path = if rest.to_lowercase().ends_with(".pdf") { rest.to_string() } else { format!("{rest}.pdf") };
                Some(self.tl_pdf(i, &path))
            }
            "GPMB4" if rest.is_empty() => Some(self.gpmb_step4(i)),
            "GPMB" if rest.eq_ignore_ascii_case("B4") => Some(self.gpmb_step4(i)),
            _ => None,
        }
    }
}

inventory::submit!(crate::command::CommandRegistration { names: &["TRICHLUC", "TLPDF"] });

#[cfg(test)]
mod tests {
    use super::*;

    fn add_ring(app: &mut OpenCADStudio, pts: &[[f64; 3]]) -> Handle {
        let i = app.active_tab;
        let mut pl = codec::entities::LwPolyline::from_points(
            pts.iter().map(|p| codec::types::Vector2::new(p[0], p[1])).collect(),
        );
        for (v, p) in pl.vertices.iter_mut().zip(pts) {
            v.bulge = p[2];
        }
        pl.is_closed = true;
        app.tabs[i].scene.add_entity(EntityType::LwPolyline(pl))
    }

    fn sheet_count(app: &OpenCADStudio) -> usize {
        model_entities(&app.tabs[app.active_tab].scene)
            .iter()
            .filter(|e| matches!(tag_of(e), Some(Tag::Sheet(_))))
            .count()
    }

    fn texts(app: &OpenCADStudio) -> Vec<String> {
        model_entities(&app.tabs[app.active_tab].scene)
            .iter()
            .filter_map(|e| match e {
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_sheet_is_drawn_edited_and_redrawn() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        let h = add_ring(&mut app, &[[0.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 15.0, 0.4], [0.0, 15.0, 0.0]]);
        // A neighbour wholly inside the map frame is copied.
        let mut t = codec::entities::Text::new();
        t.value = "Bên".into();
        t.height = 1.0;
        t.insertion_point = codec::types::Vector3::new(21.5, 5.0, 0.0);
        app.tabs[i].scene.add_entity(EntityType::Text(t.clone()));
        // The parcel's own words are not copied once it has a number.
        t.value = "Chủ cũ".into();
        t.insertion_point = codec::types::Vector3::new(8.0, 5.0, 0.0);
        app.tabs[i].scene.add_entity(EntityType::Text(t.clone()));
        // One whose words run out of the frame stays behind.
        t.value = "Thửa ngoài khung".into();
        t.insertion_point = codec::types::Vector3::new(22.0, 5.0, 0.0);
        app.tabs[i].scene.add_entity(EntityType::Text(t));
        let _ = app.dispatch_command(&format!("_TLRUN {:X}", h.value()));
        assert!(app.trichluc.is_some());
        let _ = app.dispatch_command("_TLSET so_thua 12");
        let _ = app.dispatch_command("_TLSET ORIENT");
        let _ = app.dispatch_command("_TLSET DRAW");
        assert!(app.trichluc.is_none());
        let n = sheet_count(&app);
        assert!(n > 40, "{n}");
        let all = texts(&app);
        assert!(all.iter().filter(|s| s.as_str() == "Bên").count() == 2, "neighbour copied");
        assert_eq!(all.iter().filter(|s| s.as_str() == "Chủ cũ").count(), 1);
        assert_eq!(all.iter().filter(|s| s.as_str() == "Thửa ngoài khung").count(), 1);
        assert!(all.iter().any(|s| s == "12"));
        // The arc stays an arc in the copy.
        let copies: Vec<EntityType> = model_entities(&app.tabs[i].scene)
            .into_iter()
            .filter(|e| e.common().layer == T::L_PARCEL)
            .collect();
        assert!(matches!(&copies[0], EntityType::LwPolyline(pl) if pl.vertices.iter().any(|v| v.bulge != 0.0)));
        let src = app.tabs[i].scene.document.get_entity(h).unwrap().clone();
        let Some(Tag::Source(id, Some(p))) = tag_of(&src) else { panic!("no params") };
        assert!(p.landscape && p.base.is_some() && p.so_thua == "12");
        // Picking any part of the sheet edits it; redrawing replaces it.
        let part = model_entities(&app.tabs[i].scene)
            .into_iter()
            .find(|e| matches!(tag_of(e), Some(Tag::Sheet(s)) if s == id))
            .unwrap()
            .common()
            .handle;
        let _ = app.dispatch_command(&format!("_TLRUN {:X}", part.value()));
        assert!(app.trichluc.as_ref().is_some_and(|pd| pd.editing && pd.params.so_thua == "12"));
        let _ = app.dispatch_command("_TLSET so_thua 13");
        let _ = app.dispatch_command("_TLSET DRAW");
        assert_eq!(sheet_count(&app), n);
        let all = texts(&app);
        assert!(all.iter().any(|s| s == "13") && !all.iter().any(|s| s == "12"));
        // The shared fields are remembered.
        assert!(Params::with_shared(&app.trichluc_defaults).landscape);
        // One PDF page per sheet.
        assert_eq!(app.tl_sheets(i).len(), 1);
        let path = std::env::temp_dir().join(format!("tl-test-{}.pdf", std::process::id()));
        let pages: Vec<_> = app.tl_sheets(i).iter().map(|(b, paper)| app.sheet_plot_page(*b, *paper)).collect();
        crate::io::pdf_export::export_pdf_pages(&pages, &path, None).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn gpmb_b4_makes_a_sheet_per_taken_parcel() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        let sq = |x: f64| [[x, 0.0, 0.0], [x + 10.0, 0.0, 0.0], [x + 10.0, 10.0, 0.0], [x, 10.0, 0.0]];
        add_ring(&mut app, &sq(0.0));
        add_ring(&mut app, &sq(10.0));
        add_ring(&mut app, &sq(30.0));
        let b = {
            let mut pl = codec::entities::LwPolyline::from_points(
                [[5.0, -1.0], [25.0, -1.0], [25.0, 11.0], [5.0, 11.0]]
                    .iter()
                    .map(|p| codec::types::Vector2::new(p[0], p[1]))
                    .collect(),
            );
            pl.is_closed = true;
            pl.common.layer = "RANH".into();
            app.tabs[i].scene.add_entity(EntityType::LwPolyline(pl))
        };
        let _ = app.dispatch_command("GPMB1");
        app.tabs[i].scene.deselect_all();
        app.tabs[i].scene.select_entity(b, false);
        let _ = app.dispatch_command("_GPMB3RUN");
        let _ = app.dispatch_command("GPMB4");
        assert_eq!(app.tl_sheets(i).len(), 2);
        let hatched = model_entities(&app.tabs[i].scene)
            .iter()
            .filter(|e| e.common().layer == T::L_TAKE && matches!(e, EntityType::Hatch(_)))
            .count();
        assert_eq!(hatched, 2);
        // Again: the same two sheets, redrawn.
        let n = sheet_count(&app);
        let _ = app.dispatch_command("GPMB4");
        assert_eq!(app.tl_sheets(i).len(), 2);
        assert_eq!(sheet_count(&app), n);
    }
}
