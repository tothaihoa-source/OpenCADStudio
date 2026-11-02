//! CadMini Studio: WSCURRENT and the 2D / 3D workspace switch.
//!
//! `WSCURRENT 2D` / `WSCURRENT 3D` (or WS2D / WS3D) pick a workspace;
//! `WSCURRENT` alone switches to the other one. In the 2D workspace the
//! 3D-only commands are refused with a hint to switch.

use crate::app::workspace::{self, UiStyle, Workspace};
use crate::app::{Message, OpenCADStudio};
use iced::Task;

impl OpenCADStudio {
    /// Show the workspace's tabs and menus (no view change).
    pub(in crate::app) fn apply_workspace_chrome(&mut self) {
        self.ribbon
            .set_workspace_hidden(self.workspace.hidden_modules());
    }

    /// Switch to `ws`: chrome, then the view the workspace expects.
    pub(in crate::app) fn set_workspace(&mut self, ws: Workspace) -> Task<Message> {
        let changed = self.workspace != ws;
        self.workspace = ws;
        self.apply_workspace_chrome();
        self.persist_settings_if_changed();
        self.command_line.push_info(
            crate::tf!("Workspace: {name}", name = crate::t!(ws.title())).as_ref(),
        );
        if !changed {
            return Task::none();
        }
        match ws {
            // Plan view, flat display.
            Workspace::Drafting2D => Task::batch([
                Task::done(Message::MenuCommand("-VIEW TOP".to_string())),
                Task::done(Message::MenuCommand("VSCURRENT 2".to_string())),
            ]),
            // A view that shows depth, as 3D modeling starts in.
            Workspace::Modeling3D => {
                Task::done(Message::MenuCommand("-VIEW SWISO".to_string()))
            }
        }
    }

    /// Classic: pull-down menus and toolbars, no ribbon. Ribbon: the ribbon
    /// with the menus kept above it, so this switch stays one click away.
    pub(in crate::app) fn set_ui_style(&mut self, style: UiStyle) {
        self.ui_style = style;
        self.show_menu_bar = true;
        self.ribbon_closed = style == UiStyle::Classic;
        self.persist_settings_if_changed();
        self.command_line.push_info(
            crate::tf!("Interface: {name}", name = crate::t!(style.title())).as_ref(),
        );
    }

    pub(super) fn dispatch_workspace(&mut self, cmd: &str, _i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim().trim_start_matches(['_', '.']);
        let mut words = trimmed.splitn(2, char::is_whitespace);
        let verb = words.next().unwrap_or("").to_uppercase();
        let arg = words.next().unwrap_or("").trim();
        match verb.as_str() {
            "WSCURRENT" | "WORKSPACE" => {
                let target = if arg.is_empty() {
                    if self.workspace.is_3d() {
                        Workspace::Drafting2D
                    } else {
                        Workspace::Modeling3D
                    }
                } else if let Some(ws) = Workspace::parse(arg) {
                    ws
                } else {
                    self.command_line
                        .push_error(crate::t!("Workspace must be 2D or 3D.").as_ref());
                    return Some(Task::none());
                };
                Some(self.set_workspace(target))
            }
            "WS2D" => Some(self.set_workspace(Workspace::Drafting2D)),
            "UISTYLE" => {
                let style = if arg.is_empty() {
                    match self.ui_style {
                        UiStyle::Classic => UiStyle::Ribbon,
                        UiStyle::Ribbon => UiStyle::Classic,
                    }
                } else if let Some(style) = UiStyle::parse(arg) {
                    style
                } else {
                    self.command_line.push_error(
                        crate::t!("UISTYLE: enter CLASSIC or RIBBON.").as_ref(),
                    );
                    return Some(Task::none());
                };
                self.set_ui_style(style);
                Some(Task::none())
            }
            "WS3D" => Some(self.set_workspace(Workspace::Modeling3D)),
            _ if !self.workspace.is_3d() && workspace::is_3d_command(&verb) => {
                self.command_line.push_error(
                    crate::tf!(
                        "{cmd} is a 3D command. Switch to the 3D workspace first (WS3D).",
                        cmd = verb
                    )
                    .as_ref(),
                );
                Some(Task::none())
            }
            _ => None,
        }
    }
}

inventory::submit!(crate::command::CommandRegistration {
    names: &["WSCURRENT", "WORKSPACE", "WS2D", "WS3D", "UISTYLE"],
});

#[cfg(test)]
mod tests {
    use crate::app::workspace::Workspace;
    use crate::app::OpenCADStudio;

    #[test]
    fn the_2d_workspace_refuses_3d_commands_and_the_3d_one_runs_them() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        let _ = app.dispatch_command("WS2D");
        assert_eq!(app.workspace, Workspace::Drafting2D);
        let _ = app.dispatch_command("BOX");
        assert!(app.tabs[i].active_cmd.is_none());
        let _ = app.dispatch_command("WSCURRENT 3D");
        assert_eq!(app.workspace, Workspace::Modeling3D);
        let _ = app.dispatch_command("BOX");
        assert_eq!(app.tabs[i].active_cmd.as_ref().map(|c| c.name()), Some("BOX"));
    }

    #[test]
    fn a_typed_wscurrent_with_its_answer_switches_once() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        // The menu runs the line as a script would: the bare word must not
        // switch first.
        let _ = app.run_command_line_streaming("WSCURRENT 3D", true);
        assert_eq!(app.workspace, Workspace::Modeling3D);
        let _ = app.run_command_line_streaming("WSCURRENT 2D", true);
        assert_eq!(app.workspace, Workspace::Drafting2D);
    }
}
