//! CadMini Studio classic toolbars (UISTYLE CLASSIC), laid out as in
//! AutoCAD 2007 and CadMini: Standard / Dimension / GIS along the top under
//! the menus, Draw down the left edge of the drawing and Modify down the
//! right. Icons and labels come from the ribbon definitions, so both
//! interface styles show the same pictures; every button runs its command
//! through the same message as a ribbon button.

use iced::widget::{button, column, container, row, text, tooltip, Space};
use iced::{Background, Border, Element, Length, Theme};

use crate::app::Message;
use crate::modules::{IconKind, ModuleEvent, ToolDef};

const ICON: f32 = 18.0;
const BTN: f32 = 26.0;

/// (command, label, glyph shown when the ribbon has no icon for it).
type Entry = (&'static str, &'static str, &'static str);

const STANDARD: &[Entry] = &[
    ("PAN", "Pan", "✋"),
    ("ZOOM WINDOW", "Zoom Window", "⊡"),
    ("ZOOM PREVIOUS", "Zoom Previous", "↶"),
    ("ZOOM EXTENTS", "Zoom Extents", "⤢"),
    ("REGEN", "Regen", "⟳"),
];

const PROPERTIES: &[Entry] = &[
    ("LAYER", "Layer Properties", "▤"),
    ("PROPERTIES", "Properties", "☰"),
    ("MATCHPROP", "Match Properties", "🖌"),
    ("STYLE", "Text Style", "A"),
    ("DIMSTYLE", "Dimension Style", "↔"),
];

const DIMENSION: &[Entry] = &[
    ("DIMLINEAR", "Linear", "⟷"),
    ("DIMALIGNED", "Aligned", "⤡"),
    ("DIMRADIUS", "Radius", "R"),
    ("DIMDIAMETER", "Diameter", "Ø"),
    ("DIMANGULAR", "Angular", "∠"),
    ("DIMCONTINUE", "Continue", "⇥"),
    ("DIMBASELINE", "Baseline", "⇤"),
    ("MLEADER", "Multileader", "↗"),
];

const INQUIRY: &[Entry] = &[
    ("DIST", "Distance", "D"),
    ("AREA", "Area", "▱"),
    ("ID", "ID Point", "•"),
];

const GIS: &[Entry] = &[
    ("GISCRS", "GIS Coordinate System (VN-2000)", "🌐"),
    ("GISSAT", "Satellite Background On/Off", "🛰"),
    ("GISSOURCE", "Background Source", "🗺"),
    ("GISLL", "Point Lat/Long", "⌖"),
    ("GISIMPORT", "Import GIS Data", "⇩"),
    ("GISEXPORT", "Export GIS Data", "⇧"),
    ("GPMB", "Land Clearance (GPMB)", "▦"),
    ("TRICHLUC", "Parcel Extract Sheet (TRICHLUC)", "▤"),
    ("XUAT", "Export Window to PDF/JPG/PNG (XUAT)", "⎙"),
    ("BANDOHT", "Present-State Map (BANDOHT)", "🗺"),
];

const FADE: &[Entry] = &[
    ("HATCHFADE", "Hatch Fade", "▨"),
    ("LINEFADE", "Line Fade", "╱"),
    ("HATCHB", "Hatch Boundary to Polyline", "▭"),
];

const DRAW: &[Entry] = &[
    ("LINE", "Line", "╱"),
    ("XLINE", "Construction Line", "⟋"),
    ("PLINE", "Polyline", "⌇"),
    ("POLYGON", "Polygon", "⬡"),
    ("RECTANG", "Rectangle", "▭"),
    ("ARC", "Arc", "◜"),
    ("CIRCLE", "Circle", "◯"),
    ("REVCLOUD", "Revision Cloud", "☁"),
    ("SPLINE", "Spline", "∿"),
    ("ELLIPSE", "Ellipse", "⬭"),
    ("INSERT", "Insert Block", "⊞"),
    ("BLOCK", "Make Block", "▣"),
    ("POINT", "Point", "·"),
    ("HATCH", "Hatch", "▨"),
    ("REGION", "Region", "◩"),
    ("TABLE", "Table", "▦"),
    ("MTEXT", "Multiline Text", "A"),
];

const MODELING: &[Entry] = &[
    ("BOX", "Box", "▢"),
    ("CYLINDER", "Cylinder", "⌭"),
    ("SPHERE", "Sphere", "●"),
    ("CONE", "Cone", "△"),
    ("EXTRUDE", "Extrude", "⇑"),
    ("REVOLVE", "Revolve", "↻"),
    ("SWEEP", "Sweep", "∫"),
    ("LOFT", "Loft", "≋"),
    ("PRESSPULL", "Presspull", "⇕"),
    ("UNION", "Union", "∪"),
    ("SUBTRACT", "Subtract", "∖"),
    ("INTERSECT", "Intersect", "∩"),
    ("SLICE", "Slice", "⫽"),
    ("3DORBIT", "Orbit", "⟲"),
];

const MODIFY: &[Entry] = &[
    ("ERASE", "Erase", "✕"),
    ("COPY", "Copy", "⧉"),
    ("MIRROR", "Mirror", "⇋"),
    ("OFFSET", "Offset", "⫴"),
    ("ARRAYRECT", "Array", "⁘"),
    ("MOVE", "Move", "✥"),
    ("ROTATE", "Rotate", "↻"),
    ("SCALE", "Scale", "⤡"),
    ("STRETCH", "Stretch", "⇲"),
    ("TRIM", "Trim", "✂"),
    ("EXTEND", "Extend", "⇥"),
    ("BREAK", "Break", "⌿"),
    ("JOIN", "Join", "⊷"),
    ("CHAMFER", "Chamfer", "◺"),
    ("FILLET", "Fillet", "◟"),
    ("EXPLODE", "Explode", "✳"),
];

fn tool(&(command, label, glyph): &Entry) -> ToolDef {
    let icon = crate::modules::registry::ribbon_commands()
        .get(command)
        .map(|(_, icon)| *icon)
        .and_then(|icon| match icon {
            IconKind::Svg(_) => Some(icon),
            IconKind::Glyph(_) => None,
        })
        .unwrap_or(IconKind::Glyph(glyph));
    ToolDef { id: command, label, icon, event: ModuleEvent::Command(command.to_string()) }
}

fn icon_el(icon: IconKind) -> Element<'static, Message> {
    match icon {
        IconKind::Glyph(s) => text(s).size(ICON * 0.8).into(),
        IconKind::Svg(bytes) => crate::ui::icons::semantic(bytes, ICON),
    }
}

fn tip_panel(label: &'static str, command: &'static str) -> Element<'static, Message> {
    let label = crate::t!(label).replace('\n', " ");
    container(text(format!("{label} ({command})")).size(11))
        .padding([2, 6])
        .style(|theme: &Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(Background::Color(palette.background.strong.color)),
                border: Border {
                    color: palette.background.neutral.color,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                text_color: Some(palette.background.strong.text),
                ..Default::default()
            }
        })
        .into()
}

fn tool_button(entry: &Entry, tip_at: tooltip::Position) -> Element<'static, Message> {
    let t = tool(entry);
    let btn = button(container(icon_el(t.icon)).center(Length::Fill))
        .on_press(Message::RibbonToolClick { tool_id: t.id.to_string(), event: t.event.clone() })
        .width(Length::Fixed(BTN))
        .height(Length::Fixed(BTN))
        .padding(0)
        .style(|theme: &Theme, status| {
            let palette = theme.palette();
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hovered.then_some(Background::Color(palette.background.strong.color)),
                border: Border { radius: 3.0.into(), ..Default::default() },
                text_color: palette.background.base.text,
                ..Default::default()
            }
        });
    tooltip(btn, tip_panel(t.label, entry.0), tip_at).gap(4).into()
}

fn rule(vertical: bool) -> Element<'static, Message> {
    let (w, h) = if vertical { (BTN - 6.0, 1.0) } else { (1.0, BTN - 6.0) };
    container(Space::new())
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background.neutral.color)),
            ..Default::default()
        })
        .into()
}

fn panel_style(theme: &Theme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border { color: palette.background.neutral.color, width: 1.0, radius: 4.0.into() },
        ..Default::default()
    }
}

// ── Toolbars: docked in the top strip or floating over the drawing ─────────

/// One toolbar's place: docked in the strip under the menus, or floating
/// over the drawing at (x, y) from its top-left corner (top-right when
/// `from_right`), with its buttons in `cols` columns.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BarState {
    pub id: String,
    pub floating: bool,
    pub x: f32,
    pub y: f32,
    pub cols: u16,
    pub from_right: bool,
    /// Moved by the user: keeps its place in both workspaces.
    pub moved: bool,
}

impl Default for BarState {
    fn default() -> Self {
        Self { id: String::new(), floating: false, x: 0.0, y: 0.0, cols: 1, from_right: false, moved: false }
    }
}

/// The toolbars: id, title, tools.
const BARS: &[(&str, &str, &[Entry])] = &[
    ("standard", "Standard", STANDARD),
    ("properties", "Properties", PROPERTIES),
    ("dimension", "Dimension", DIMENSION),
    ("inquiry", "Inquiry", INQUIRY),
    ("fade", "Fade", FADE),
    ("gis", "GIS", GIS),
    ("draw", "Draw", DRAW),
    ("modeling", "Modeling", MODELING),
    ("modify", "Modify", MODIFY),
];

fn bar_tools(id: &str) -> &'static [Entry] {
    BARS.iter().find(|(b, ..)| *b == id).map(|(.., t)| *t).unwrap_or(&[])
}

/// Whether a toolbar belongs to the workspace.
fn shown_in(id: &str, is_3d: bool) -> bool {
    match id {
        "gis" => !is_3d,
        "modeling" => is_3d,
        _ => true,
    }
}

/// The toolbars as they first appear: the small ones docked on top, Draw
/// (and Modeling in 3D) down the left edge, Modify down the right.
pub fn default_layout() -> Vec<BarState> {
    BARS.iter()
        .map(|(id, _, tools)| {
            let docked = !matches!(*id, "draw" | "modeling" | "modify");
            BarState {
                id: id.to_string(),
                floating: !docked,
                x: if *id == "modeling" { 4.0 + BTN + 10.0 } else { 4.0 },
                y: 30.0,
                cols: if docked { tools.len() as u16 } else { 1 },
                from_right: *id == "modify",
                moved: false,
            }
        })
        .collect()
}

/// `layout` with any toolbar it lacks (an older settings file) added.
pub fn complete(mut layout: Vec<BarState>) -> Vec<BarState> {
    layout.retain(|b| BARS.iter().any(|(id, ..)| *id == b.id));
    for d in default_layout() {
        if !layout.iter().any(|b| b.id == d.id) {
            layout.push(d);
        }
    }
    layout
}

/// A drag in progress: which toolbar, moving or resizing, and where it
/// started (set by the first pointer move).
#[derive(Clone, Debug, Default)]
pub struct Drag {
    pub id: String,
    pub resize: bool,
    pub start: Option<(iced::Point, BarState)>,
}

/// Toolbar drag messages.
#[derive(Clone, Debug)]
pub enum ToolbarMsg {
    /// Press on a toolbar's grip (move) or its corner (resize).
    Start { id: &'static str, resize: bool },
    /// Pointer over the drawing, in drawing-area pixels.
    Move(iced::Point),
    End,
}

/// Apply a pointer move to the toolbar being dragged.
pub fn drag_to(layout: &mut [BarState], drag: &mut Drag, p: iced::Point) {
    let Some(bar) = layout.iter_mut().find(|b| b.id == drag.id) else { return };
    let Some((origin, start)) = drag.start.clone() else {
        if !bar.floating {
            // Torn off the top strip: it floats under the pointer.
            bar.floating = true;
            bar.from_right = false;
            bar.x = (p.x - 6.0).max(0.0);
            bar.y = (p.y - 6.0).max(0.0);
            bar.cols = bar_tools(&bar.id).len().max(1) as u16;
        }
        bar.moved = true;
        drag.start = Some((p, bar.clone()));
        return;
    };
    let (dx, dy) = (p.x - origin.x, p.y - origin.y);
    if drag.resize {
        let n = bar_tools(&bar.id).len().max(1) as f32;
        let width = start.cols as f32 * BTN + if start.from_right { -dx } else { dx };
        bar.cols = (width / BTN).round().clamp(1.0, n) as u16;
    } else {
        bar.x = (start.x + if start.from_right { -dx } else { dx }).max(0.0);
        bar.y = (start.y + dy).max(0.0);
    }
}

/// End a drag: a floating toolbar let go at the top edge docks again.
pub fn drop_bar(layout: &mut [BarState], drag: &Drag) {
    if drag.resize {
        return;
    }
    if let Some(bar) = layout.iter_mut().find(|b| b.id == drag.id) {
        if bar.floating && bar.y < 6.0 {
            bar.floating = false;
            bar.cols = bar_tools(&bar.id).len() as u16;
        }
    }
}

fn grip(id: &'static str, title: &'static str, vertical_bar: bool, cols: usize) -> Element<'static, Message> {
    let mark = if vertical_bar { "⋯" } else { "⋮" };
    let span = cols as f32 * (BTN + 1.0) - 1.0;
    let handle = container(text(mark).size(11))
        .center_x(if vertical_bar { Length::Fixed(span) } else { Length::Fixed(10.0) })
        .center_y(if vertical_bar { Length::Fixed(10.0) } else { Length::Fixed(BTN) })
        .width(if vertical_bar { Length::Fixed(span) } else { Length::Fixed(10.0) })
        .height(if vertical_bar { Length::Fixed(10.0) } else { Length::Fixed(BTN) });
    let area = iced::widget::mouse_area(handle)
        .on_press(Message::ClassicToolbar(ToolbarMsg::Start { id, resize: false }))
        .interaction(iced::mouse::Interaction::Grab);
    tooltip(area, tip_text(title), tooltip::Position::Top).gap(2).into()
}

fn tip_text(s: &'static str) -> Element<'static, Message> {
    container(text(crate::t!(s).into_owned()).size(11))
        .padding([2, 6])
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background.strong.color)),
            ..Default::default()
        })
        .into()
}

fn corner(id: &'static str) -> Element<'static, Message> {
    iced::widget::mouse_area(
        container(text("◢").size(8)).width(Length::Fixed(10.0)).height(Length::Fixed(10.0)),
    )
    .on_press(Message::ClassicToolbar(ToolbarMsg::Start { id, resize: true }))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

/// One toolbar: grip, buttons in `cols` columns, and (floating) the corner
/// that changes the number of columns.
fn bar_view(id: &'static str, title: &'static str, tools: &'static [Entry], state: &BarState) -> Element<'static, Message> {
    let cols = state.cols.max(1) as usize;
    let vertical_bar = cols < tools.len();
    let tip_at = if state.from_right { tooltip::Position::Left } else { tooltip::Position::Right };
    let tip_at = if state.floating { tip_at } else { tooltip::Position::Bottom };
    let mut grid = column![].spacing(1);
    for chunk in tools.chunks(cols) {
        let mut r = row![].spacing(1);
        for entry in chunk {
            r = r.push(tool_button(entry, tip_at));
        }
        grid = grid.push(r);
    }
    let body: Element<'static, Message> = if vertical_bar {
        column![grip(id, title, true, cols), grid].spacing(1).align_x(iced::Center).into()
    } else {
        row![grip(id, title, false, cols), grid].spacing(1).align_y(iced::Center).into()
    };
    let body: Element<'static, Message> = if state.floating {
        column![body, container(corner(id)).width(Length::Shrink)]
            .align_x(if state.from_right { iced::Left } else { iced::Right })
            .into()
    } else {
        body
    };
    let panel = container(body).padding(2);
    if state.floating {
        panel.style(panel_style).into()
    } else {
        panel.into()
    }
}

fn bar_def(id: &str) -> Option<(&'static str, &'static str, &'static [Entry])> {
    BARS.iter().find(|(b, ..)| *b == id).copied()
}

/// The strip under the menus with the docked toolbars.
pub fn top_bar(layout: &[BarState], is_3d: bool, dragging: bool) -> Element<'static, Message> {
    let mut items = row![].spacing(2).align_y(iced::Center);
    let mut first = true;
    for state in layout.iter().filter(|b| !b.floating && shown_in(&b.id, is_3d)) {
        let Some((id, title, tools)) = bar_def(&state.id) else { continue };
        if !first {
            items = items.push(container(rule(false)).padding([0, 3]));
        }
        first = false;
        items = items.push(bar_view(id, title, tools, state));
    }
    let strip = container(items).padding([0, 2]).height(Length::Fixed(BTN + 2.0));
    if dragging {
        iced::widget::mouse_area(strip).on_release(Message::ClassicToolbar(ToolbarMsg::End)).into()
    } else {
        strip.into()
    }
}

/// The floating toolbars over the drawing, then (while dragging) the layer
/// that follows the pointer.
pub fn overlays(layout: &[BarState], is_3d: bool, dragging: bool) -> Vec<Element<'static, Message>> {
    let mut out = Vec::new();
    for state in layout.iter().filter(|b| b.floating && shown_in(&b.id, is_3d)) {
        let Some((id, title, tools)) = bar_def(&state.id) else { continue };
        // Below the ViewCube in 3D until the user places it.
        let y = if is_3d && state.from_right && !state.moved { state.y.max(170.0) } else { state.y };
        let padding = if state.from_right {
            iced::Padding { right: state.x, top: y, ..iced::Padding::ZERO }
        } else {
            iced::Padding { left: state.x, top: y, ..iced::Padding::ZERO }
        };
        out.push(
            container(iced::widget::opaque(bar_view(id, title, tools, state)))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(if state.from_right { iced::Right } else { iced::Left })
                .align_y(iced::Top)
                .padding(padding)
                .into(),
        );
    }
    if dragging {
        out.push(
            iced::widget::mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_move(|p| Message::ClassicToolbar(ToolbarMsg::Move(p)))
                .on_release(Message::ClassicToolbar(ToolbarMsg::End))
                .interaction(iced::mouse::Interaction::Grabbing)
                .into(),
        );
    }
    out
}

/// Every command a classic toolbar can run.
pub fn all_commands() -> impl Iterator<Item = &'static str> {
    [STANDARD, PROPERTIES, DIMENSION, INQUIRY, GIS, FADE, DRAW, MODELING, MODIFY]
        .into_iter()
        .flatten()
        .map(|e| e.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_toolbar_command_runs() {
        for command in all_commands() {
            assert!(
                crate::ui::menu_bar::implemented(command),
                "{command} is not a registered command"
            );
        }
    }

    #[test]
    fn workspace_picks_gis_or_modeling() {
        assert!(shown_in("gis", false) && !shown_in("gis", true));
        assert!(shown_in("modeling", true) && !shown_in("modeling", false));
    }

    #[test]
    fn tear_off_move_resize_and_dock_again() {
        let mut layout = default_layout();
        let mut drag = Drag { id: "dimension".into(), resize: false, start: None };
        drag_to(&mut layout, &mut drag, iced::Point::new(200.0, 100.0));
        let bar = layout.iter().find(|b| b.id == "dimension").unwrap();
        assert!(bar.floating);
        assert_eq!((bar.x, bar.y), (194.0, 94.0));
        drag_to(&mut layout, &mut drag, iced::Point::new(250.0, 120.0));
        let bar = layout.iter().find(|b| b.id == "dimension").unwrap();
        assert_eq!((bar.x, bar.y), (244.0, 114.0));
        // Narrow it to two columns.
        let mut resize = Drag { id: "dimension".into(), resize: true, start: None };
        drag_to(&mut layout, &mut resize, iced::Point::new(400.0, 200.0));
        let full = layout.iter().find(|b| b.id == "dimension").unwrap().cols as f32;
        drag_to(&mut layout, &mut resize, iced::Point::new(400.0 - (full - 2.0) * BTN, 200.0));
        assert_eq!(layout.iter().find(|b| b.id == "dimension").unwrap().cols, 2);
        // Back to the top edge: docked, one row again.
        let mut back = Drag { id: "dimension".into(), resize: false, start: None };
        drag_to(&mut layout, &mut back, iced::Point::new(250.0, 120.0));
        drag_to(&mut layout, &mut back, iced::Point::new(250.0, 0.0));
        drop_bar(&mut layout, &back);
        let bar = layout.iter().find(|b| b.id == "dimension").unwrap();
        assert!(!bar.floating);
        assert_eq!(bar.cols as usize, DIMENSION.len());
    }

    #[test]
    fn a_right_edge_toolbar_moves_from_the_right() {
        let mut layout = default_layout();
        let mut drag = Drag { id: "modify".into(), resize: false, start: None };
        drag_to(&mut layout, &mut drag, iced::Point::new(900.0, 100.0));
        drag_to(&mut layout, &mut drag, iced::Point::new(880.0, 110.0));
        let bar = layout.iter().find(|b| b.id == "modify").unwrap();
        assert!(bar.from_right);
        assert_eq!((bar.x, bar.y), (24.0, 40.0));
    }

    #[test]
    fn an_old_layout_gets_the_missing_toolbars() {
        let layout = complete(vec![BarState { id: "draw".into(), floating: true, ..Default::default() }]);
        assert_eq!(layout.len(), BARS.len());
    }
}
