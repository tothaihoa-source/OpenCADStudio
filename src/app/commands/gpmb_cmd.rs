//! GPMB — land clearance in four steps, as CadMini (see `crate::gpmb`).
//!
//! - GPMB          the steps as buttons: Layers / B1 / B2 / B3 / Edit / Table / Excel / B4
//! - GPMBLAYERS    pick an object on each parcel layer (none: every layer)
//! - GPMB1         gather each closed parcel's texts; "?" where data is missing
//! - GPMB2         close open parcels, trace parcels from loose lines, mark
//!                 dangling ends, write each parcel's area
//! - GPMB3         pick the GPMB boundary: taken parts (red hatch), the
//!                 commune's land (green hatch), labels
//! - GPMBEDIT      pick a parcel, fill in its data (kept in the drawing)
//! - GPMBTABLE     the summary table in the drawing
//! - GPMBXLSX      the summary table as an Excel workbook
//!
//! Each step is one undo step. Parcel data lives in the polyline's XDATA
//! (`CADMINI_GPMB`), the same as CadMini, so both read each other's work.

use std::collections::HashMap;

use crate::app::{Message, OpenCADStudio};
use crate::command::{CadCommand, CmdResult, FreeTextValuePromptCommand, KeywordCommand};
use crate::gpmb::{self as G, Xy};
use codec::{EntityType, Handle};
use geo::Area;
use glam::DVec3;
use iced::Task;

/// What GPMB remembers for a drawing between its steps.
#[derive(Default)]
pub(in crate::app) struct GpmbState {
    pub layers: Vec<String>,
    pub tol: f64,
    pub min_area: f64,
    pub parcels: Vec<G::Parcel>,
    pub rings: HashMap<u64, Vec<Xy>>,
    pub boundary: Option<u64>,
    pub rows: Vec<G::Row>,
    pub out_parts: Vec<geo::Polygon<f64>>,
    pub total: f64,
    /// B3 was started from the GPMB dialog: show it again when done.
    pub reopen: bool,
}

impl GpmbState {
    fn new() -> Self {
        Self { tol: 0.05, min_area: 0.1, ..Default::default() }
    }
}

/// Gathers a selection (Enter confirms), then dispatches `run`.
pub(super) struct PickThenRun {
    pub name: &'static str,
    pub prompt: &'static str,
    pub run: &'static str,
    pub selected: Vec<Handle>,
}

impl CadCommand for PickThenRun {
    fn name(&self) -> &'static str {
        self.name
    }

    fn prompt(&self) -> String {
        crate::t!(self.prompt).into_owned()
    }

    fn is_selection_gathering(&self) -> bool {
        true
    }

    fn on_selection_complete(&mut self, handles: Vec<Handle>) -> CmdResult {
        self.selected = handles;
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.selected.is_empty() {
            return CmdResult::Dispatch(format!("{} ", self.run));
        }
        CmdResult::Relaunch(self.run.to_string(), std::mem::take(&mut self.selected))
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }
}

/// GPMBTABLE: the table's top-left corner.
struct TableCorner;

impl CadCommand for TableCorner {
    fn name(&self) -> &'static str {
        "GPMBTABLE"
    }

    fn prompt(&self) -> String {
        crate::t!("GPMBTABLE  Top-left corner of the summary table:").into_owned()
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        CmdResult::Dispatch(format!("_GPMBTABLEAT {},{}", pt.x, pt.y))
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

/// GPMBEDIT: ask each field in turn, Enter keeps the value shown.
struct EditParcel {
    handle: u64,
    data: G::ParcelData,
    step: usize,
}

const EDIT_FIELDS: &[&str] = &[
    "Parcel number (số thửa)",
    "Map sheet (tờ bản đồ)",
    "Owner (chủ sử dụng)",
    "Land use (loại đất)",
    "Address (địa chỉ)",
    "Note (ghi chú)",
];

impl EditParcel {
    fn field(&mut self, k: usize) -> &mut String {
        match k {
            0 => &mut self.data.so_thua,
            1 => &mut self.data.to_ban_do,
            2 => &mut self.data.chu,
            3 => &mut self.data.loai_dat,
            4 => &mut self.data.dia_chi,
            _ => &mut self.data.ghi_chu,
        }
    }

    fn advance(&mut self) -> CmdResult {
        self.step += 1;
        if self.step >= EDIT_FIELDS.len() {
            let json = serde_json::to_string(&self.data).unwrap_or_default();
            return CmdResult::Dispatch(format!("_GPMBSAVE {:X} {json}", self.handle));
        }
        CmdResult::NeedPoint
    }
}

impl CadCommand for EditParcel {
    fn name(&self) -> &'static str {
        "GPMBEDIT"
    }

    fn prompt(&self) -> String {
        let k = self.step.min(EDIT_FIELDS.len() - 1);
        let current = match k {
            0 => &self.data.so_thua,
            1 => &self.data.to_ban_do,
            2 => &self.data.chu,
            3 => &self.data.loai_dat,
            4 => &self.data.dia_chi,
            _ => &self.data.ghi_chu,
        };
        format!("GPMBEDIT  {} <{}>:", crate::t!(EDIT_FIELDS[k]), current)
    }

    fn input_kind(&self) -> crate::command::InputKind {
        crate::command::InputKind::FreeText
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let k = self.step;
        *self.field(k) = text.trim().to_string();
        Some(self.advance())
    }

    fn on_enter(&mut self) -> CmdResult {
        self.advance()
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }
}

// ── Reading the drawing ─────────────────────────────────────────────────────

pub(super) fn model_entities(scene: &crate::scene::Scene) -> Vec<EntityType> {
    let model = scene.model_space_block_handle();
    scene
        .document
        .block_records
        .iter()
        .find(|br| br.handle == model)
        .map(|br| br.entity_handles.iter().filter_map(|h| scene.document.get_entity(*h).cloned()).collect())
        .unwrap_or_default()
}

fn own_layer(layer: &str) -> bool {
    let l = layer.to_lowercase();
    l.starts_with("gpmb_") || l.starts_with("tl_")
}

/// The closed outline of a parcel entity, arcs split.
pub(super) fn closed_ring(e: &EntityType) -> Option<Vec<Xy>> {
    match e {
        EntityType::LwPolyline(pl) => {
            let mut pts = super::gis_cmd::flatten_lw(pl);
            let closed_by_points = pts.len() > 3 && pts.first() == pts.last();
            if !(pl.is_closed || closed_by_points) {
                return None;
            }
            if closed_by_points {
                pts.pop();
            }
            (pts.len() >= 3 && G::ring_area(&pts) > 0.0).then_some(pts)
        }
        EntityType::Circle(c) => Some(
            (0..72)
                .map(|k| {
                    let a = std::f64::consts::TAU * f64::from(k) / 72.0;
                    [c.center.x + c.radius * a.cos(), c.center.y + c.radius * a.sin()]
                })
                .collect(),
        ),
        _ => None,
    }
}

/// An open chain of a line entity (for B2).
fn open_chain(e: &EntityType) -> Option<Vec<Xy>> {
    match e {
        EntityType::Line(l) => Some(vec![[l.start.x, l.start.y], [l.end.x, l.end.y]]),
        EntityType::LwPolyline(pl) if !pl.is_closed => {
            let pts = super::gis_cmd::flatten_lw(pl);
            (pts.len() >= 2).then_some(pts)
        }
        EntityType::Arc(a) => {
            let (mut s, e) = (a.start_angle, a.end_angle);
            if e < s {
                s -= std::f64::consts::TAU;
            }
            let n = (((e - s) / (std::f64::consts::PI / 36.0)).ceil() as usize).max(2);
            Some(
                (0..=n)
                    .map(|k| {
                        let t = s + (e - s) * k as f64 / n as f64;
                        [a.center.x + a.radius * t.cos(), a.center.y + a.radius * t.sin()]
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// MTEXT without its formatting codes.
pub(super) fn plain_mtext(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '\\' => match chars.next() {
                Some('P') | Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some('{') => out.push('{'),
                Some('}') => out.push('}'),
                Some('L' | 'l' | 'O' | 'o' | 'K' | 'k') => {}
                Some(_) => {
                    // \fArial|b0; \H2x; \C1; \A1; … up to the semicolon.
                    for d in chars.by_ref() {
                        if d == ';' {
                            break;
                        }
                    }
                }
                None => {}
            },
            _ => out.push(c),
        }
    }
    out.trim().to_string()
}

fn text_point(t: &codec::entities::Text) -> Xy {
    use codec::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let justified = !matches!(t.horizontal_alignment, H::Left) || !matches!(t.vertical_alignment, V::Baseline);
    match (justified, t.alignment_point) {
        (true, Some(p)) => [p.x, p.y],
        _ => [t.insertion_point.x, t.insertion_point.y],
    }
}

fn labels(entities: &[EntityType]) -> Vec<G::Label> {
    let mut out = Vec::new();
    for e in entities {
        if own_layer(&e.common().layer) {
            // Texts GPMB or the extract sheets wrote: not parcel data.
            continue;
        }
        match e {
            EntityType::Text(t) if !t.value.trim().is_empty() => out.push(G::Label {
                at: text_point(t),
                text: t.value.trim().to_string(),
                height: t.height.max(1e-6),
                tag: String::new(),
            }),
            EntityType::MText(t) => {
                let s = plain_mtext(&t.value);
                if !s.is_empty() {
                    out.push(G::Label {
                        at: [t.insertion_point.x, t.insertion_point.y],
                        text: s,
                        height: t.height.max(1e-6),
                        tag: String::new(),
                    });
                }
            }
            EntityType::Insert(ins) => {
                for a in &ins.attributes {
                    let s = a.value.trim();
                    if !s.is_empty() {
                        out.push(G::Label {
                            at: [a.insertion_point.x, a.insertion_point.y],
                            text: s.to_string(),
                            height: a.height.max(1e-6),
                            tag: a.tag.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

pub(super) fn saved_data(e: &EntityType) -> Option<G::ParcelData> {
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

pub(super) fn set_saved_data(e: &mut EntityType, d: &G::ParcelData) {
    let mut record = codec::xdata::ExtendedDataRecord::new(G::APPID);
    for s in G::data_to_xdata(d) {
        record.add_value(codec::xdata::XDataValue::String(s));
    }
    e.common_mut().extended_data.upsert_record(record);
}

// ── Writing ─────────────────────────────────────────────────────────────────

pub(super) fn on_layer(mut e: EntityType, layer: &str, color: Option<i16>) -> EntityType {
    let c = e.common_mut();
    c.layer = layer.to_string();
    c.color = color.map_or(codec::types::Color::ByLayer, codec::types::Color::from_index);
    e
}

fn text(s: &str, at: Xy, h: f64, layer: &str, color: Option<i16>) -> EntityType {
    use codec::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let mut t = codec::entities::Text::new();
    t.value = s.to_string();
    t.height = h;
    t.horizontal_alignment = H::Center;
    t.vertical_alignment = V::Middle;
    t.insertion_point = codec::types::Vector3::new(at[0], at[1], 0.0);
    t.alignment_point = Some(codec::types::Vector3::new(at[0], at[1], 0.0));
    on_layer(EntityType::Text(t), layer, color)
}

fn left_text(s: &str, at: Xy, h: f64, layer: &str) -> EntityType {
    use codec::entities::{TextHorizontalAlignment as H, TextVerticalAlignment as V};
    let mut t = codec::entities::Text::new();
    t.value = s.to_string();
    t.height = h;
    t.horizontal_alignment = H::Left;
    t.vertical_alignment = V::Middle;
    t.insertion_point = codec::types::Vector3::new(at[0], at[1], 0.0);
    t.alignment_point = Some(codec::types::Vector3::new(at[0], at[1], 0.0));
    on_layer(EntityType::Text(t), layer, None)
}

fn polyline(ring: &[Xy], layer: &str) -> EntityType {
    let mut pl = codec::entities::LwPolyline::from_points(
        ring.iter().map(|p| codec::types::Vector2::new(p[0], p[1])).collect(),
    );
    pl.is_closed = true;
    on_layer(EntityType::LwPolyline(pl), layer, None)
}

fn line(a: Xy, b: Xy, layer: &str) -> EntityType {
    let mut l = codec::entities::Line::new();
    l.start = codec::types::Vector3::new(a[0], a[1], 0.0);
    l.end = codec::types::Vector3::new(b[0], b[1], 0.0);
    on_layer(EntityType::Line(l), layer, None)
}

/// A hatch over `poly` with ISO-like line families (`angles_deg`) `spacing`
/// apart, whatever the drawing's units.
pub(super) fn hatch(poly: &geo::Polygon<f64>, layer: &str, name: &str, angles_deg: &[f64], spacing: f64) -> EntityType {
    use codec::entities::{BoundaryEdge, BoundaryPath, BoundaryPathFlags, HatchPattern, HatchPatternLine, PolylineEdge};
    let mut h = codec::entities::Hatch::new();
    for (k, ring) in G::rings(poly).iter().enumerate() {
        let bits = if k == 0 { 1 | 2 | 16 } else { 2 };
        let mut path = BoundaryPath::with_flags(BoundaryPathFlags::from_bits(bits));
        path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
            ring.iter().map(|p| codec::types::Vector2::new(p[0], p[1])).collect(),
            true,
        )));
        h.paths.push(path);
    }
    let mut pattern = HatchPattern::new(name);
    for a in angles_deg {
        let r = a.to_radians();
        pattern.lines.push(HatchPatternLine {
            angle: r,
            base_point: codec::types::Vector2::new(0.0, 0.0),
            // Perpendicular to the lines, `spacing` apart.
            offset: codec::types::Vector2::new(-spacing * r.sin(), spacing * r.cos()),
            dash_lengths: Vec::new(),
        });
    }
    h.pattern = pattern;
    h.is_solid = false;
    h.pattern_type = codec::entities::HatchPatternType::Predefined;
    h.pattern_scale = spacing / 3.175;
    on_layer(EntityType::Hatch(Box::new(h)), layer, None)
}

impl OpenCADStudio {
    pub(super) fn gpmb_state(&mut self, i: usize) -> &mut GpmbState {
        let id = self.tabs[i].id;
        self.gpmb.entry(id).or_insert_with(GpmbState::new)
    }

    fn gpmb_ensure_layers(&mut self, i: usize) {
        let scene = &mut self.tabs[i].scene;
        for (name, color) in G::LAYERS {
            if !scene.document.layers.contains(name) {
                scene.ensure_layer(name);
                if let Some(layer) = scene.document.layers.get_mut(name) {
                    layer.color = codec::types::Color::from_index(*color);
                }
            }
        }
        self.sync_registered_layers(i);
    }

    fn gpmb_clear(&mut self, i: usize, layers: &[&str]) {
        let set: Vec<String> = layers.iter().map(|l| l.to_lowercase()).collect();
        let scene = &self.tabs[i].scene;
        let model = scene.model_space_block_handle();
        let handles: Vec<Handle> = scene
            .document
            .block_records
            .iter()
            .find(|br| br.handle == model)
            .map(|br| {
                br.entity_handles
                    .iter()
                    .copied()
                    .filter(|h| {
                        scene
                            .document
                            .get_entity(*h)
                            .is_some_and(|e| set.contains(&e.common().layer.to_lowercase()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !handles.is_empty() {
            self.tabs[i].scene.erase_entities(&handles);
        }
    }

    fn gpmb_add(&mut self, i: usize, entities: Vec<EntityType>) -> Vec<Handle> {
        if entities.is_empty() {
            return Vec::new();
        }
        let handles = self.tabs[i].scene.add_entities(entities);
        self.tabs[i].dirty = true;
        handles
    }

    /// With no parcel layer chosen, take the layer holding the most closed
    /// shapes (the parcels, not the one GPMB boundary) and say so.
    pub(super) fn gpmb_default_layers(&mut self, i: usize) {
        if !self.gpmb_state(i).layers.is_empty() {
            return;
        }
        let mut count: HashMap<String, usize> = HashMap::new();
        for e in model_entities(&self.tabs[i].scene) {
            let layer = e.common().layer.clone();
            if !own_layer(&layer) && closed_ring(&e).is_some() {
                *count.entry(layer).or_default() += 1;
            }
        }
        if let Some((layer, n)) = count.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))) {
            self.command_line.push_info(&format!(
                "GPMB: layer thửa = {layer} ({n} hình kín). Đổi bằng GPMBLAYERS."
            ));
            self.gpmb_state(i).layers = vec![layer];
        }
    }

    /// The parcel layers' closed outlines and the drawing's labels.
    fn gpmb_read(&mut self, i: usize) -> (Vec<G::ParcelShape>, Vec<G::Label>) {
        self.gpmb_default_layers(i);
        let layers: Vec<String> = self.gpmb_state(i).layers.iter().map(|l| l.to_lowercase()).collect();
        let entities = model_entities(&self.tabs[i].scene);
        let on_parcel_layer = |e: &EntityType| {
            let l = e.common().layer.to_lowercase();
            if layers.is_empty() { !own_layer(&l) || l == G::L_NEW.to_lowercase() } else { layers.contains(&l) }
        };
        let shapes = entities
            .iter()
            .filter(|e| on_parcel_layer(e))
            .filter_map(|e| {
                closed_ring(e).map(|ring| G::ParcelShape {
                    handle: e.common().handle.value(),
                    ring,
                    saved: saved_data(e),
                })
            })
            .collect();
        (shapes, labels(&entities))
    }

    /// B1: gather the texts; mark parcels missing data.
    pub(super) fn gpmb_step1(&mut self, i: usize, undo: bool) -> Task<Message> {
        let (shapes, labels) = self.gpmb_read(i);
        if shapes.is_empty() {
            self.command_line.push_error(
                crate::t!("GPMB: no closed parcel on the parcel layers (GPMBLAYERS to choose them).").as_ref(),
            );
            return Task::none();
        }
        let started = std::time::Instant::now();
        let parcels = G::extract(&shapes, &labels);
        let rings: HashMap<u64, Vec<Xy>> = shapes.into_iter().map(|s| (s.handle, s.ring)).collect();
        if undo {
            self.push_undo_snapshot(i, "GPMB1");
        }
        self.gpmb_ensure_layers(i);
        self.gpmb_clear(i, &[G::L_MISSING]);
        let h = G::text_height(parcels.iter().map(|p| p.area));
        let marks: Vec<EntityType> = parcels
            .iter()
            .filter(|p| !p.missing.is_empty())
            .filter_map(|p| {
                let ring = rings.get(&p.handle)?;
                let c = G::inner_point(&G::polygon(ring));
                Some(text("?", [c[0] + h * 1.5, c[1] + h * 1.2], h * 0.9, G::L_MISSING, None))
            })
            .collect();
        self.gpmb_add(i, marks);
        let missing = parcels.iter().filter(|p| !p.missing.is_empty()).count();
        let n = parcels.len();
        let st = self.gpmb_state(i);
        st.parcels = parcels;
        st.rings = rings;
        self.command_line.push_output(&format!(
            "GPMB B1: {n} thửa kín, {missing} thửa thiếu thông tin (dấu ? layer {}) — {:.1} s.",
            G::L_MISSING,
            started.elapsed().as_secs_f64()
        ));
        Task::none()
    }

    /// B2: close the small gaps, trace parcels from loose lines, mark the
    /// dangling ends, then B1 again and write the areas.
    pub(super) fn gpmb_step2(&mut self, i: usize) -> Task<Message> {
        self.gpmb_default_layers(i);
        let tol = self.gpmb_state(i).tol;
        let layers: Vec<String> = self.gpmb_state(i).layers.iter().map(|l| l.to_lowercase()).collect();
        let entities = model_entities(&self.tabs[i].scene);
        let on_parcel_layer = |e: &EntityType| {
            let l = e.common().layer.to_lowercase();
            if layers.is_empty() { !own_layer(&l) || l == G::L_NEW.to_lowercase() } else { layers.contains(&l) || l == G::L_NEW.to_lowercase() }
        };
        self.push_undo_snapshot(i, "GPMB2");
        self.gpmb_ensure_layers(i);
        // Close open polylines whose ends are within the tolerance.
        let mut closed = 0;
        let mut still_open = 0;
        for e in entities.iter().filter(|e| on_parcel_layer(e)) {
            let EntityType::LwPolyline(pl) = e else { continue };
            if pl.is_closed || pl.vertices.len() < 3 {
                continue;
            }
            let (a, b) = (pl.vertices[0].location, pl.vertices[pl.vertices.len() - 1].location);
            if (a.x - b.x).hypot(a.y - b.y) <= tol {
                let mut fixed = pl.clone();
                if fixed.vertices.len() > 3 {
                    fixed.vertices.pop();
                }
                fixed.is_closed = true;
                self.tabs[i].scene.update_entity(EntityType::LwPolyline(fixed));
                closed += 1;
            } else {
                still_open += 1;
            }
        }
        let entities = model_entities(&self.tabs[i].scene);
        let mut lines = Vec::new();
        let mut existing = Vec::new();
        for e in entities.iter().filter(|e| on_parcel_layer(e)) {
            if let Some(r) = closed_ring(e) {
                existing.push(r);
            } else if let Some(c) = open_chain(e) {
                lines.push(c);
            }
        }
        let (new, ends) = G::trace(&lines, &existing, tol);
        self.gpmb_clear(i, &[G::L_ERROR]);
        let mut made: Vec<EntityType> = new.iter().map(|r| polyline(r, G::L_NEW)).collect();
        let size = G::text_height(new.iter().map(|r| G::ring_area(r)).chain(existing.iter().map(|r| G::ring_area(r))));
        for p in &ends {
            made.push(line([p[0] - size, p[1] - size], [p[0] + size, p[1] + size], G::L_ERROR));
            made.push(line([p[0] - size, p[1] + size], [p[0] + size, p[1] - size], G::L_ERROR));
        }
        self.gpmb_add(i, made);
        {
            let st = self.gpmb_state(i);
            if !new.is_empty() && !st.layers.iter().any(|l| l.eq_ignore_ascii_case(G::L_NEW)) {
                st.layers.push(G::L_NEW.to_string());
            }
        }
        self.command_line.push_output(&format!(
            "GPMB B2: đóng {closed} thửa hở, dựng thêm {} thửa từ đường rời (layer {}); {} đầu mút treo cần xem lại (dấu X layer {}); {still_open} polyline hở khe lớn hơn {tol} m.",
            new.len(),
            G::L_NEW,
            ends.len(),
            G::L_ERROR
        ));
        let task = self.gpmb_step1(i, false);
        // Each parcel's area, under its number.
        self.gpmb_clear(i, &[G::L_AREA]);
        let st = self.gpmb_state(i);
        let h = G::text_height(st.parcels.iter().map(|p| p.area));
        let labels: Vec<EntityType> = st
            .parcels
            .iter()
            .filter_map(|p| {
                let ring = st.rings.get(&p.handle)?;
                let c = G::inner_point(&G::polygon(ring));
                Some(text(&format!("{:.1} m²", p.area), [c[0], c[1] - h * 1.3], h * 0.75, G::L_AREA, None))
            })
            .collect();
        let n = labels.len();
        self.gpmb_add(i, labels);
        self.command_line
            .push_output(&format!("GPMB B2: đã ghi diện tích lên {n} thửa (layer {}).", G::L_AREA));
        task
    }

    /// B3 with the boundary picked: taken parts, the commune's land, labels.
    fn gpmb_step3(&mut self, i: usize) -> Task<Message> {
        let picked: Vec<(Handle, EntityType)> = self.tabs[i]
            .scene
            .selected_entities()
            .into_iter()
            .map(|(h, e)| (h, e.clone()))
            .collect();
        let Some((bh, boundary)) = picked.iter().find_map(|(h, e)| closed_ring(e).map(|r| (*h, r))) else {
            self.command_line
                .push_error(crate::t!("GPMB B3: pick a closed polyline as the GPMB boundary.").as_ref());
            return Task::none();
        };
        if self.gpmb_state(i).parcels.is_empty() {
            let _ = self.gpmb_step1(i, true);
        }
        let started = std::time::Instant::now();
        let st = self.gpmb_state(i);
        let parcels: Vec<(G::Parcel, Vec<Xy>)> = st
            .parcels
            .iter()
            .filter(|p| p.handle != bh.value())
            .filter_map(|p| Some((p.clone(), st.rings.get(&p.handle)?.clone())))
            .collect();
        let (mut rows, out_parts, total) = G::compute(&parcels, &boundary, st.min_area, 0.002);
        for r in &mut rows {
            if r.data.so_thua.is_empty() {
                r.data.so_thua = format!("?{}", r.stt);
            }
        }
        self.push_undo_snapshot(i, "GPMB3");
        self.gpmb_ensure_layers(i);
        self.gpmb_clear(i, &[G::L_TAKE, G::L_OUT, G::L_LABEL]);
        let areas = rows.iter().flat_map(|r| r.parts.iter().map(|p| p.unsigned_area()));
        let h = G::text_height(areas.chain(out_parts.iter().map(|p| p.unsigned_area())));
        let spacing = (h * 1.2).max(0.05);
        let mut hatches = Vec::new();
        let mut made = Vec::new();
        for r in &rows {
            for (k, p) in r.parts.iter().enumerate() {
                hatches.push(hatch(p, G::L_TAKE, "ANSI31", &[45.0], spacing));
                for ring in G::rings(p).iter().take(1) {
                    let mut e = polyline(ring, G::L_TAKE);
                    e.common_mut().line_weight = codec::types::LineWeight::Value(35);
                    made.push(e);
                }
                let c = G::inner_point(p);
                let name = if r.parts.len() > 1 {
                    format!("TH {}.{}", r.data.so_thua, k + 1)
                } else {
                    format!("TH {}", r.data.so_thua)
                };
                made.push(text(
                    &format!("{name}: {:.1} m²", p.unsigned_area()),
                    [c[0], c[1] - h * 2.6],
                    h * 0.8,
                    G::L_LABEL,
                    Some(1),
                ));
            }
        }
        for p in &out_parts {
            hatches.push(hatch(p, G::L_OUT, "ANSI37", &[45.0, 135.0], spacing * 1.6));
            let c = G::inner_point(p);
            made.push(text(
                &format!("UBND quản lý: {:.1} m²", p.unsigned_area()),
                c,
                h * 0.8,
                G::L_LABEL,
                Some(3),
            ));
        }
        let hatch_handles = self.gpmb_add(i, hatches);
        self.gpmb_add(i, made);
        let s = G::summary(&rows, &out_parts, total);
        let st = self.gpmb_state(i);
        st.boundary = Some(bh.value());
        st.rows = rows;
        st.out_parts = out_parts;
        st.total = total;
        self.tabs[i].scene.deselect_all();
        self.command_line.push_output(&format!(
            "GPMB B3: {} thửa bị thu hồi, trong thửa {:.1} m², UBND quản lý {:.1} m², tổng ranh {:.1} m² (chênh {:.2} m²) — {:.1} s.",
            s.so_thua,
            s.thu_hoi_trong_thua,
            s.ngoai_thua,
            s.tong_ranh,
            s.chenh_lech,
            started.elapsed().as_secs_f64()
        ));
        // The hatches under the parcel lines and numbers.
        if hatch_handles.is_empty() {
            Task::none()
        } else {
            self.apply_cmd_result(CmdResult::Relaunch("DRAWORDER BACK".to_string(), hatch_handles))
        }
    }

    fn gpmb_table(&mut self, i: usize, at: Xy) -> Task<Message> {
        let st = self.gpmb_state(i);
        if st.rows.is_empty() {
            self.command_line.push_error(crate::t!("GPMB: run B3 first.").as_ref());
            return Task::none();
        }
        let data = G::table(&st.rows, &st.out_parts, st.total);
        let h = G::text_height(st.rows.iter().flat_map(|r| r.parts.iter().map(|p| p.unsigned_area()))) * 0.8;
        // CadMini's column widths, in text heights.
        let widths = [3.5, 5.0, 4.0, 16.0, 5.0, 7.0, 8.0, 7.0, 13.0].map(|w: f64| w * h);
        let row_h = h * 2.0;
        let total_w: f64 = widths.iter().sum();
        self.push_undo_snapshot(i, "GPMBTABLE");
        self.gpmb_ensure_layers(i);
        let mut made = Vec::new();
        let rows = data.len();
        for r in 0..=rows {
            let y = at[1] - r as f64 * row_h;
            made.push(line([at[0], y], [at[0] + total_w, y], G::L_TABLE));
        }
        let mut x = at[0];
        for c in 0..=widths.len() {
            made.push(line([x, at[1]], [x, at[1] - rows as f64 * row_h], G::L_TABLE));
            if c < widths.len() {
                x += widths[c];
            }
        }
        for (r, row) in data.iter().enumerate() {
            let y = at[1] - (r as f64 + 0.5) * row_h;
            let mut x = at[0];
            for (c, cell) in row.iter().enumerate() {
                if !cell.is_empty() {
                    if r == 0 || c == 0 || (5..=7).contains(&c) {
                        made.push(text(cell, [x + widths[c] / 2.0, y], h * 0.8, G::L_TABLE, None));
                    } else {
                        made.push(left_text(cell, [x + h * 0.4, y], h * 0.8, G::L_TABLE));
                    }
                }
                x += widths[c];
            }
        }
        self.gpmb_add(i, made);
        self.command_line.push_output(&format!("GPMB: bảng tổng hợp {} dòng (layer {}).", rows, G::L_TABLE));
        Task::none()
    }

    fn gpmb_xlsx(&mut self, i: usize, path: &str) -> Task<Message> {
        let st = self.gpmb_state(i);
        if st.rows.is_empty() {
            self.command_line.push_error(crate::t!("GPMB: run B3 first.").as_ref());
            return Task::none();
        }
        match write_xlsx(path, &st.rows, &st.out_parts, st.total) {
            Ok(()) => {
                self.command_line.push_output(&format!("GPMB: đã xuất {path}"));
                crate::sys::open_url(&format!("file://{path}"), self.main_window)
            }
            Err(e) => {
                self.command_line.push_error(&format!("GPMB: không xuất được {path}: {e}"));
                Task::none()
            }
        }
    }

    pub(super) fn dispatch_gpmb(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim().trim_start_matches(['_', '.']);
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_uppercase(), r.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };
        let start = |app: &mut Self, c: Box<dyn CadCommand>| {
            app.command_line.push_info(&c.prompt());
            app.tabs[i].active_cmd = Some(c);
            Some(Task::none())
        };
        match (verb.as_str(), rest.to_ascii_uppercase().as_str()) {
            ("GPMB", "") => {
                self.open_gpmb_dialog(i);
                Some(Task::none())
            }
            ("GPMBSTEPS", "") => start(
                self,
                Box::new(
                    KeywordCommand::new(
                        "GPMB",
                        "GPMB  [Layers / B1 / B2 / B3 / Edit / Table / Excel / B4]:",
                        vec![
                            ("Layers", "LAYERS", None),
                            ("B1 Clean", "B1", None),
                            ("B2 Trace", "B2", None),
                            ("B3 Compute", "B3", None),
                            ("Edit Parcel", "EDIT", None),
                            ("Table", "TABLE", None),
                            ("Excel", "EXCEL", None),
                            ("B4 Extracts", "B4", None),
                        ],
                    )
                    .with_default("B1"),
                ),
            ),
            ("GPMB", "LAYERS") | ("GPMBLAYERS", "") => start(
                self,
                Box::new(PickThenRun {
                    name: "GPMBLAYERS",
                    prompt: "GPMBLAYERS  Pick an object on each parcel layer (Enter: every layer):",
                    run: "_GPMBLAYERSET",
                    selected: Vec::new(),
                }),
            ),
            ("GPMBLAYERSET", _) => {
                let layers: Vec<String> = {
                    let mut l: Vec<String> = self.tabs[i]
                        .scene
                        .selected_entities()
                        .iter()
                        .map(|(_, e)| e.common().layer.clone())
                        .collect();
                    l.sort();
                    l.dedup();
                    l
                };
                self.tabs[i].scene.deselect_all();
                let text = if layers.is_empty() { "mọi layer".to_string() } else { layers.join(", ") };
                let st = self.gpmb_state(i);
                st.layers = layers;
                st.parcels.clear();
                self.command_line.push_output(&format!("GPMB: layer thửa = {text}"));
                Some(Task::none())
            }
            ("GPMB", "B1") | ("GPMB1", "") => Some(self.gpmb_step1(i, true)),
            ("GPMB", "B2") | ("GPMB2", "") => Some(self.gpmb_step2(i)),
            ("GPMB", "B3") | ("GPMB3", "") => start(
                self,
                Box::new(PickThenRun {
                    name: "GPMB3",
                    prompt: "GPMB B3  Pick the GPMB boundary (closed polyline), Enter:",
                    run: "_GPMB3RUN",
                    selected: Vec::new(),
                }),
            ),
            ("GPMB3RUN", _) => {
                // A script names the boundary by its handle.
                if let Ok(h) = u64::from_str_radix(rest.trim_start_matches("0X").trim_start_matches("0x"), 16) {
                    self.tabs[i].scene.deselect_all();
                    self.tabs[i].scene.select_entity(Handle::new(h), false);
                }
                let t = self.gpmb_step3(i);
                if std::mem::take(&mut self.gpmb_state(i).reopen) {
                    self.open_gpmb_dialog(i);
                }
                Some(t)
            }
            ("GPMB", "EDIT") | ("GPMBEDIT", "") => start(
                self,
                Box::new(PickThenRun {
                    name: "GPMBEDIT",
                    prompt: "GPMBEDIT  Pick a parcel, Enter:",
                    run: "_GPMBEDITRUN",
                    selected: Vec::new(),
                }),
            ),
            ("GPMBEDITRUN", _) => {
                let picked = self.tabs[i]
                    .scene
                    .selected_entities()
                    .into_iter()
                    .find(|(_, e)| closed_ring(e).is_some())
                    .map(|(h, e)| (h, saved_data(e)));
                self.tabs[i].scene.deselect_all();
                let Some((h, saved)) = picked else {
                    self.command_line.push_error(crate::t!("GPMBEDIT: pick a closed parcel.").as_ref());
                    return Some(Task::none());
                };
                // What B1 found, then what was saved.
                let found = self.gpmb_state(i).parcels.iter().find(|p| p.handle == h.value()).map(|p| p.data.clone());
                let data = saved.or(found).unwrap_or_default();
                start(self, Box::new(EditParcel { handle: h.value(), data, step: 0 }))
            }
            ("GPMBSAVE", _) => {
                let mut parts = rest.splitn(2, ' ');
                let handle = parts.next().and_then(|h| u64::from_str_radix(h, 16).ok());
                let data: Option<G::ParcelData> = parts.next().and_then(|j| serde_json::from_str(j).ok());
                let (Some(handle), Some(data)) = (handle, data) else { return Some(Task::none()) };
                let Some(mut e) = self.tabs[i].scene.document.get_entity(Handle::new(handle)).cloned() else {
                    return Some(Task::none());
                };
                self.push_undo_snapshot(i, "GPMBEDIT");
                set_saved_data(&mut e, &data);
                self.tabs[i].scene.update_entity(e);
                self.tabs[i].dirty = true;
                if let Some(p) = self.gpmb_state(i).parcels.iter_mut().find(|p| p.handle == handle) {
                    p.data = data;
                    p.check();
                }
                self.command_line.push_output("GPMBEDIT: đã lưu thông tin thửa (trong bản vẽ).");
                Some(Task::none())
            }
            ("GPMB", "TABLE") | ("GPMBTABLE", "") => start(self, Box::new(TableCorner)),
            ("GPMBTABLEAT", _) => {
                let mut it = rest.split(',').filter_map(|v| v.trim().parse::<f64>().ok());
                match (it.next(), it.next()) {
                    (Some(x), Some(y)) => Some(self.gpmb_table(i, [x, y])),
                    _ => Some(Task::none()),
                }
            }
            ("GPMB", "EXCEL") | ("GPMBXLSX", "") => {
                if self.gpmb_state(i).rows.is_empty() {
                    self.command_line.push_error(crate::t!("GPMB: run B3 first.").as_ref());
                    return Some(Task::none());
                }
                if crate::sys::file_dia() {
                    let name = format!("{}_GPMB.xlsx", self.tabs[i].tab_display_name());
                    Some(Task::perform(
                        async move {
                            let handle = crate::sys::file_dialog()
                                .set_title("GPMB → Excel")
                                .add_filter("Excel (*.xlsx)", &["xlsx"])
                                .set_file_name(&name)
                                .save_file()
                                .await?;
                            Some(crate::sys::handle_path(&handle).to_string_lossy().into_owned())
                        },
                        |path| match path {
                            Some(path) => Message::Command(format!("_GPMBXLSXFILE {path}")),
                            None => Message::Noop,
                        },
                    ))
                } else {
                    start(self, Box::new(FreeTextValuePromptCommand::new("_GPMBXLSXFILE", "GPMBXLSX  Excel file:")))
                }
            }
            ("GPMBXLSXFILE", _) if !rest.is_empty() => {
                let path = if rest.to_lowercase().ends_with(".xlsx") { rest.to_string() } else { format!("{rest}.xlsx") };
                Some(self.gpmb_xlsx(i, &path))
            }
            _ => None,
        }
    }
}

fn write_xlsx(path: &str, rows: &[G::Row], out: &[geo::Polygon<f64>], total: f64) -> Result<(), String> {
    use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};
    let data = G::table(rows, out, total);
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("GPMB").map_err(|e| e.to_string())?;
    let title = Format::new().set_bold().set_font_size(13);
    let head = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let cell = Format::new().set_border(FormatBorder::Thin).set_align(FormatAlign::VerticalCenter);
    let wrap = cell.clone().set_text_wrap();
    let num = cell.clone().set_num_format("#,##0.0");
    let bold = cell.clone().set_bold();
    let bold_num = num.clone().set_bold();
    ws.write_string_with_format(0, 0, "BẢNG TỔNG HỢP DIỆN TÍCH GPMB", &title).map_err(|e| e.to_string())?;
    let n_rows = rows.len();
    for (r, row) in data.iter().enumerate() {
        let xr = (r + 2) as u32;
        let summary_row = r > n_rows;
        for (c, v) in row.iter().enumerate() {
            let c16 = c as u16;
            let res = if r == 0 {
                ws.write_string_with_format(xr, c16, v, &head)
            } else if (5..=7).contains(&c) && !v.is_empty() {
                let f = if summary_row { &bold_num } else { &num };
                ws.write_number_with_format(xr, c16, v.parse::<f64>().unwrap_or(0.0), f)
            } else if r > 0 && c == 0 && !v.is_empty() {
                ws.write_number_with_format(xr, c16, v.parse::<f64>().unwrap_or(0.0), &cell)
            } else {
                let f = if summary_row { &bold } else if c == 3 { &wrap } else { &cell };
                ws.write_string_with_format(xr, c16, v, f)
            };
            res.map_err(|e| e.to_string())?;
        }
    }
    for (c, w) in [6.0, 9.0, 7.0, 28.0, 9.0, 13.0, 14.0, 13.0, 24.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w).map_err(|e| e.to_string())?;
    }
    wb.save(path).map_err(|e| e.to_string())
}

inventory::submit!(crate::command::CommandRegistration {
    names: &["GPMB", "GPMBLAYERS", "GPMB1", "GPMB2", "GPMB3", "GPMB4", "GPMBEDIT", "GPMBTABLE", "GPMBXLSX"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtext_codes_are_stripped() {
        assert_eq!(plain_mtext(r"{\fArial|b0;\H2.5x;Nguyễn Văn An}\PONT"), "Nguyễn Văn An ONT");
    }

    fn add_square(app: &mut OpenCADStudio, x: f64, y: f64, w: f64) -> Handle {
        let i = app.active_tab;
        let mut pl = codec::entities::LwPolyline::from_points(
            [[x, y], [x + w, y], [x + w, y + w], [x, y + w]]
                .iter()
                .map(|p| codec::types::Vector2::new(p[0], p[1]))
                .collect(),
        );
        pl.is_closed = true;
        app.tabs[i].scene.add_entity(EntityType::LwPolyline(pl))
    }

    fn add_text(app: &mut OpenCADStudio, x: f64, y: f64, s: &str) {
        let i = app.active_tab;
        let mut t = codec::entities::Text::new();
        t.value = s.into();
        t.height = 1.0;
        t.insertion_point = codec::types::Vector3::new(x, y, 0.0);
        app.tabs[i].scene.add_entity(EntityType::Text(t));
    }

    fn on(app: &OpenCADStudio, layer: &str) -> Vec<EntityType> {
        model_entities(&app.tabs[app.active_tab].scene)
            .into_iter()
            .filter(|e| e.common().layer == layer)
            .collect()
    }

    #[test]
    fn the_steps_run_on_a_small_map() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        add_square(&mut app, 0.0, 0.0, 10.0);
        add_square(&mut app, 10.0, 0.0, 10.0);
        add_text(&mut app, 5.0, 6.0, "1");
        add_text(&mut app, 5.0, 4.0, "Nguyễn Văn An");
        add_text(&mut app, 5.0, 2.0, "ONT");
        add_text(&mut app, 15.0, 5.0, "2");
        // B1: parcel 2 lacks owner and land use.
        let _ = app.dispatch_command("GPMB1");
        assert_eq!(app.gpmb_state(i).parcels.len(), 2);
        assert_eq!(on(&app, G::L_MISSING).len(), 1);
        // B3 with a boundary over half of parcel 1, all of 2, and 50 m² outside.
        let b = {
            let mut pl = codec::entities::LwPolyline::from_points(
                [[5.0, 0.0], [25.0, 0.0], [25.0, 10.0], [5.0, 10.0]]
                    .iter()
                    .map(|p| codec::types::Vector2::new(p[0], p[1]))
                    .collect(),
            );
            pl.is_closed = true;
            pl.common.layer = "RANH".into();
            app.tabs[i].scene.add_entity(EntityType::LwPolyline(pl))
        };
        app.tabs[i].scene.deselect_all();
        app.tabs[i].scene.select_entity(b, false);
        let _ = app.dispatch_command("_GPMB3RUN");
        let st = app.gpmb_state(i);
        assert_eq!(st.rows.len(), 2);
        assert!((st.rows[0].dt_thu_hoi - 50.0).abs() < 1e-6);
        assert!((st.rows[1].dt_thu_hoi - 100.0).abs() < 1e-6);
        assert!((G::summary(&st.rows, &st.out_parts, st.total).ngoai_thua - 50.0).abs() < 1e-6);
        assert_eq!(on(&app, G::L_TAKE).iter().filter(|e| matches!(e, EntityType::Hatch(_))).count(), 2);
        assert_eq!(on(&app, G::L_OUT).len(), 1);
        // Table and workbook.
        let _ = app.dispatch_command("_GPMBTABLEAT 0,-10");
        assert!(!on(&app, G::L_TABLE).is_empty());
        let path = std::env::temp_dir().join(format!("gpmb-test-{}.xlsx", std::process::id()));
        let st = app.gpmb_state(i);
        write_xlsx(path.to_str().unwrap(), &st.rows, &st.out_parts, st.total).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn edited_data_is_kept_in_the_drawing() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        let h = add_square(&mut app, 0.0, 0.0, 10.0);
        let d = G::ParcelData { so_thua: "7".into(), chu: "Trần Thị Bình".into(), loai_dat: "CLN".into(), ..Default::default() };
        let _ = app.dispatch_command(&format!("_GPMBSAVE {:X} {}", h.value(), serde_json::to_string(&d).unwrap()));
        let e = app.tabs[i].scene.document.get_entity(h).unwrap().clone();
        assert_eq!(saved_data(&e), Some(d));
        let _ = app.dispatch_command("GPMB1");
        let p = &app.gpmb_state(i).parcels[0];
        assert_eq!(p.data.chu, "Trần Thị Bình");
        assert!(p.missing.is_empty());
    }
}
