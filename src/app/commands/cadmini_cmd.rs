//! CadMini Studio commands brought over from CadMini:
//!
//! - HATCHFADE [0–90]  draw hatches fainter (no value: switch 0 ↔ 50 %)
//! - LINEFADE  [0–90]  draw lines and text fainter (no value: switch 0 ↔ 50 %)
//! - HATCHB / HATCHGENERATEBOUNDARY  closed polylines along the boundaries
//!   of the selected hatches, on the current layer (one undo step)
//!
//! - TPY / TRANSPARENCYDISPLAY  show or hide object and layer transparency

use crate::app::{Message, OpenCADStudio};
use crate::command::{CadCommand, CmdResult};
use crate::scene::display_fade;
use codec::Handle;
use glam::DVec3;
use iced::Task;

/// Gathers a selection (Enter confirms), then runs `run` on it.
struct SelectThenRun {
    name: &'static str,
    run: &'static str,
    selected: Vec<Handle>,
}

impl CadCommand for SelectThenRun {
    fn name(&self) -> &'static str {
        self.name
    }

    fn prompt(&self) -> String {
        crate::t!("%{name}  Select hatch objects, then press Enter:", name = self.name).into_owned()
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
            return CmdResult::Cancel;
        }
        CmdResult::Relaunch(self.run.to_string(), std::mem::take(&mut self.selected))
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }
}

/// One vertex of a boundary: where it is and the bulge of the segment that
/// leaves it.
type BVertex = ([f64; 2], f64);

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// A boundary edge as vertices from its start, with its end point. Arcs stay
/// arcs (one vertex with its bulge); ellipses and splines are split.
fn edge_vertices(edge: &codec::entities::BoundaryEdge) -> Option<(Vec<BVertex>, [f64; 2])> {
    use codec::entities::BoundaryEdge;
    match edge {
        BoundaryEdge::Line(l) => Some((vec![([l.start.x, l.start.y], 0.0)], [l.end.x, l.end.y])),
        BoundaryEdge::Polyline(p) => {
            let mut v: Vec<BVertex> = p.vertices.iter().map(|v| ([v.x, v.y], v.z)).collect();
            let end = if p.is_closed { v.first()?.0 } else { v.last()?.0 };
            if !p.is_closed {
                v.pop();
            }
            Some((v, end))
        }
        _ => {
            let kernel_curve = crate::entities::hatch::edge_curve(edge)?;
            {
                if let BoundaryEdge::CircularArc(_) = edge {
                    // Counter-clockwise from start to end in the kernel's form.
                    if let Some((centre, r, a0, a1)) = arc_parts(edge) {
                        let mut sweep = (a1 - a0).rem_euclid(std::f64::consts::TAU);
                        if sweep < 1e-12 {
                            sweep = std::f64::consts::TAU;
                        }
                        let at = |a: f64| [centre[0] + r * a.cos(), centre[1] + r * a.sin()];
                        if sweep >= std::f64::consts::TAU - 1e-9 {
                            // A whole circle: two half arcs.
                            return Some((vec![(at(a0), 1.0), (at(a0 + std::f64::consts::PI), 1.0)], at(a0)));
                        }
                        return Some((vec![(at(a0), (sweep / 4.0).tan())], at(a1)));
                    }
                }
                let pts: Vec<[f64; 2]> = kernel_curve
                    .tessellate_angle(kernel::tessellation::DEFAULT_ANGLE)
                    .into_iter()
                    .map(|p| [p[0], p[1]])
                    .collect();
                let end = *pts.last()?;
                Some((pts[..pts.len() - 1].iter().map(|p| (*p, 0.0)).collect(), end))
            }
        }
    }
}

/// Centre, radius and the counter-clockwise start / end angles of an arc
/// edge, as the hatch draws it.
fn arc_parts(edge: &codec::entities::BoundaryEdge) -> Option<([f64; 2], f64, f64, f64)> {
    let codec::entities::BoundaryEdge::CircularArc(a) = edge else { return None };
    let (s, e) = if a.counter_clockwise {
        (a.start_angle, a.end_angle)
    } else {
        (std::f64::consts::TAU - a.end_angle, std::f64::consts::TAU - a.start_angle)
    };
    Some(([a.center.x, a.center.y], a.radius, s, e))
}

/// The edge walked the other way: end first, every bulge negated.
fn reversed((verts, end): (Vec<BVertex>, [f64; 2])) -> (Vec<BVertex>, [f64; 2]) {
    let start = verts.first().map(|v| v.0).unwrap_or(end);
    let mut points: Vec<[f64; 2]> = verts.iter().map(|v| v.0).collect();
    points.push(end);
    let bulges: Vec<f64> = verts.iter().map(|v| v.1).collect();
    let n = bulges.len();
    let out: Vec<BVertex> = (0..n).map(|k| (points[n - k], -bulges[n - 1 - k])).collect();
    (out, start)
}

/// Closed polylines along every loop of a hatch: lines, arcs (as bulges)
/// and polyline loops kept as they are, in the hatch's own plane.
pub(crate) fn hatch_boundary_polylines(hatch: &codec::entities::Hatch) -> Vec<codec::EntityType> {
    let mut out = Vec::new();
    for path in &hatch.paths {
        let edges: Vec<(Vec<BVertex>, [f64; 2])> = path.edges.iter().filter_map(edge_vertices).collect();
        if edges.is_empty() {
            continue;
        }
        let mut verts: Vec<BVertex> = Vec::new();
        let mut cur: Option<[f64; 2]> = None;
        for (k, edge) in edges.iter().enumerate() {
            let start = edge.0.first().map(|v| v.0).unwrap_or(edge.1);
            let end = edge.1;
            let flip = match cur {
                // Walk on from where the last edge ended.
                Some(c) => dist(c, end) < dist(c, start),
                // The first edge: walk towards the next one.
                None => edges.get(k + 1).is_some_and(|next| {
                    let ns = next.0.first().map(|v| v.0).unwrap_or(next.1);
                    let near = |p: [f64; 2]| dist(p, ns).min(dist(p, next.1));
                    near(start) < near(end)
                }),
            };
            let (v, e) = if flip { reversed(edge.clone()) } else { edge.clone() };
            verts.extend(v);
            cur = Some(e);
        }
        // A vertex followed by one at the same place starts a segment of no
        // length: drop it (the next one carries the real segment).
        let n = verts.len();
        let verts: Vec<BVertex> = (0..n)
            .filter(|&k| n < 2 || dist(verts[k].0, verts[(k + 1) % n].0) > 1e-9)
            .map(|k| verts[k])
            .collect();
        if verts.len() < 2 {
            continue;
        }
        let mut pl = codec::entities::LwPolyline::from_points(
            verts.iter().map(|v| codec::types::Vector2::new(v.0[0], v.0[1])).collect(),
        );
        for (vertex, (_, bulge)) in pl.vertices.iter_mut().zip(&verts) {
            vertex.bulge = *bulge;
        }
        pl.is_closed = true;
        pl.normal = hatch.normal;
        pl.elevation = hatch.elevation;
        out.push(codec::EntityType::LwPolyline(pl));
    }
    out
}

impl OpenCADStudio {
    /// Redraw every drawing after a display-only change.
    fn redraw_all_tabs(&mut self) {
        for tab in &mut self.tabs {
            tab.scene.camera_generation += 1;
        }
    }

    fn set_fade(&mut self, hatch: bool, arg: &str) -> Task<Message> {
        let current = if hatch { display_fade::hatch_fade() } else { display_fade::line_fade() };
        let value = if arg.is_empty() {
            if current > 0 { 0 } else { 50 }
        } else {
            match arg.trim_end_matches('%').trim().parse::<u32>() {
                Ok(v) if v <= display_fade::MAX_FADE => v,
                _ => {
                    self.command_line.push_error(
                        crate::t!("Enter a fade from 0 to 90 (percent).").as_ref(),
                    );
                    return Task::none();
                }
            }
        };
        if hatch {
            display_fade::set_hatch_fade(value);
        } else {
            display_fade::set_line_fade(value);
        }
        self.redraw_all_tabs();
        self.persist_settings_if_changed();
        let what = if hatch { crate::t!("Hatch fade") } else { crate::t!("Line fade") };
        let state = if value == 0 { crate::t!("off").into_owned() } else { format!("{value}%") };
        self.command_line.push_output(&format!("<{what}: {state}>"));
        Task::none()
    }

    fn hatch_boundaries(&mut self, i: usize) -> Task<Message> {
        let hatches: Vec<codec::entities::Hatch> = self.tabs[i]
            .scene
            .selected_entities()
            .into_iter()
            .filter_map(|(_, e)| match e {
                codec::EntityType::Hatch(h) => Some((**h).clone()),
                _ => None,
            })
            .collect();
        if hatches.is_empty() {
            self.command_line
                .push_error(crate::t!("HATCHB: no hatch among the selected objects.").as_ref());
            return Task::none();
        }
        self.push_undo_snapshot(i, "HATCHB");
        let mut made = 0usize;
        for hatch in &hatches {
            for entity in hatch_boundary_polylines(hatch) {
                if self.commit_entity_handle(entity).is_some() {
                    made += 1;
                }
            }
        }
        if made == 0 {
            self.discard_last_undo_entry(i);
        }
        self.tabs[i].scene.deselect_all();
        self.tabs[i].dirty |= made > 0;
        let layer = self.tabs[i].active_layer.clone();
        self.command_line.push_output(
            crate::tf!(
                "HATCHB: {made} boundary polyline(s) from {} hatch(es) on layer {layer}.",
                hatches.len()
            )
            .as_ref(),
        );
        Task::none()
    }

    pub(super) fn dispatch_cadmini(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim().trim_start_matches(['_', '.']);
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_uppercase(), r.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };
        match verb.as_str() {
            "HATCHFADE" | "LINEFADE" if rest.is_empty() => {
                let (run, prompt) = if verb == "HATCHFADE" {
                    ("_HATCHFADESET", "HATCHFADE  Hatch fade 0–90 % (Enter switches off ↔ 50 %):")
                } else {
                    ("_LINEFADESET", "LINEFADE  Line fade 0–90 % (Enter switches off ↔ 50 %):")
                };
                let c = crate::command::FreeTextValuePromptCommand::new(run, prompt);
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            "HATCHFADE" | "HATCHFADESET" => Some(self.set_fade(true, rest)),
            "LINEFADESET" => Some(self.set_fade(false, rest)),
            // Layer / object transparency on screen (CadMini's TPY button).
            "TPY" | "TRANSPARENCYDISPLAY" => Some(Task::done(Message::ToggleTransparencyDisplay)),
            "LINEFADE" => Some(self.set_fade(false, rest)),
            "HATCHBRUN" => Some(self.hatch_boundaries(i)),
            "HATCHB" | "HATCHGENERATEBOUNDARY" => {
                let has_hatch = self.tabs[i]
                    .scene
                    .selected_entities()
                    .iter()
                    .any(|(_, e)| matches!(e, codec::EntityType::Hatch(_)));
                if has_hatch {
                    return Some(self.hatch_boundaries(i));
                }
                let c = SelectThenRun { name: "HATCHB", run: "_HATCHBRUN", selected: Vec::new() };
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(Box::new(c));
                Some(Task::none())
            }
            _ => None,
        }
    }
}

inventory::submit!(crate::command::CommandRegistration {
    names: &["HATCHFADE", "LINEFADE", "TPY", "TRANSPARENCYDISPLAY", "HATCHB", "HATCHGENERATEBOUNDARY"],
});

#[cfg(test)]
mod tests {
    use super::*;
    use codec::entities::{BoundaryEdge, BoundaryPath, Hatch, PolylineEdge};
    use codec::types::Vector2;

    fn square_hatch(app: &mut OpenCADStudio) -> Handle {
        let mut hatch = Hatch::new();
        hatch.is_solid = true;
        let mut path = BoundaryPath::new();
        path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
            [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
                .into_iter()
                .map(|p| Vector2::new(p[0], p[1]))
                .collect(),
            true,
        )));
        hatch.paths.push(path);
        let i = app.active_tab;
        app.tabs[i].scene.add_entity(codec::EntityType::Hatch(Box::new(hatch)))
    }

    fn polylines(app: &OpenCADStudio) -> Vec<codec::entities::LwPolyline> {
        app.tabs[app.active_tab]
            .scene
            .document
            .entities()
            .filter_map(|e| match e {
                codec::EntityType::LwPolyline(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hatchb_draws_the_boundary_and_undoes_in_one_step() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let hatch = square_hatch(&mut app);
        let i = app.active_tab;
        app.tabs[i].scene.select_entity(hatch, false);
        let _ = app.dispatch_command("HATCHB");
        let made = polylines(&app);
        assert_eq!(made.len(), 1);
        assert!(made[0].is_closed);
        assert_eq!(made[0].vertices.len(), 4);
        // The hatch stays.
        assert!(app.tabs[i].scene.document.get_entity(hatch).is_some());
        app.undo_active_tab();
        assert!(polylines(&app).is_empty());
    }

    fn hatch_of(edges: Vec<BoundaryEdge>) -> Hatch {
        let mut hatch = Hatch::new();
        let mut path = BoundaryPath::new();
        for e in edges {
            path.add_edge(e);
        }
        hatch.paths.push(path);
        hatch
    }

    fn lw(e: &codec::EntityType) -> &codec::entities::LwPolyline {
        match e {
            codec::EntityType::LwPolyline(p) => p,
            _ => panic!("not a polyline"),
        }
    }

    #[test]
    fn hatchb_keeps_arcs_as_arcs() {
        use codec::entities::{CircularArcEdge, LineEdge};
        // A "D": the line along the bottom, a half circle over it.
        let d = hatch_of(vec![
            BoundaryEdge::Line(LineEdge { start: Vector2::new(0.0, 0.0), end: Vector2::new(10.0, 0.0) }),
            BoundaryEdge::CircularArc(CircularArcEdge {
                center: Vector2::new(5.0, 0.0),
                radius: 5.0,
                start_angle: 0.0,
                end_angle: std::f64::consts::PI,
                counter_clockwise: true,
            }),
        ]);
        let out = hatch_boundary_polylines(&d);
        assert_eq!(out.len(), 1);
        let p = lw(&out[0]);
        assert!(p.is_closed);
        assert_eq!(p.vertices.len(), 2, "no split arc");
        assert_eq!((p.vertices[0].location.x, p.vertices[0].location.y), (0.0, 0.0));
        assert_eq!(p.vertices[0].bulge, 0.0);
        assert!((p.vertices[1].bulge - 1.0).abs() < 1e-9);

        // The same D with the arc stored clockwise, walked the other way.
        let d_cw = hatch_of(vec![
            BoundaryEdge::CircularArc(CircularArcEdge {
                center: Vector2::new(5.0, 0.0),
                radius: 5.0,
                start_angle: std::f64::consts::PI,
                end_angle: std::f64::consts::TAU,
                counter_clockwise: false,
            }),
            BoundaryEdge::Line(LineEdge { start: Vector2::new(10.0, 0.0), end: Vector2::new(0.0, 0.0) }),
        ]);
        let p = lw(&hatch_boundary_polylines(&d_cw)[0]).clone();
        assert_eq!(p.vertices.len(), 2);
        assert!((p.vertices.iter().map(|v| v.bulge.abs()).sum::<f64>() - 1.0).abs() < 1e-9);

        // A whole circle: two half arcs.
        let circle = hatch_of(vec![BoundaryEdge::CircularArc(CircularArcEdge {
            center: Vector2::new(0.0, 0.0),
            radius: 3.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::TAU,
            counter_clockwise: true,
        })]);
        let p = lw(&hatch_boundary_polylines(&circle)[0]).clone();
        assert_eq!(p.vertices.len(), 2);
        assert!(p.vertices.iter().all(|v| (v.bulge - 1.0).abs() < 1e-9));

        // A polyline loop keeps its bulges.
        let mut edge = PolylineEdge::new(
            vec![Vector2::new(0.0, 0.0), Vector2::new(4.0, 0.0), Vector2::new(4.0, 4.0)],
            true,
        );
        edge.vertices[1].z = 0.5;
        let p = lw(&hatch_boundary_polylines(&hatch_of(vec![BoundaryEdge::Polyline(edge)]))[0]).clone();
        assert_eq!(p.vertices.len(), 3);
        assert_eq!(p.vertices[1].bulge, 0.5);
    }

    #[test]
    fn hatchb_without_a_selection_asks_for_one() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let _ = app.dispatch_command("HATCHB");
        let i = app.active_tab;
        assert_eq!(app.tabs[i].active_cmd.as_ref().map(|c| c.name()), Some("HATCHB"));
    }

    #[test]
    fn fades_take_a_value_or_switch() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let _ = app.dispatch_command("LINEFADE 30");
        assert_eq!(display_fade::line_fade(), 30);
        let _ = app.dispatch_command("_LINEFADESET");
        assert_eq!(display_fade::line_fade(), 0);
        let _ = app.dispatch_command("_LINEFADESET");
        assert_eq!(display_fade::line_fade(), 50);
        let _ = app.dispatch_command("LINEFADE 0");
        assert_eq!(display_fade::line_fade(), 0);
    }
}
