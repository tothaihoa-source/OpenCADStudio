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
            let storage = crate::entities::curve::ocs_plane(hatch.normal, hatch.elevation);
            let plane = crate::command::WorkingPlane::new(
                DVec3::from_array(storage.origin),
                DVec3::from_array(storage.x_axis),
                DVec3::from_array(storage.y_axis),
            );
            let rings = crate::scene::hatch_boundary_rings(hatch);
            for entity in crate::scene::boundary_entities(&rings, plane) {
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
