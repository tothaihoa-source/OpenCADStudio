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
    ("GISLL", "Point Lat/Long", "⌖"),
    ("GISIMPORT", "Import GIS Data", "⇩"),
    ("GISEXPORT", "Export GIS Data", "⇧"),
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

/// The groups of the top toolbar row for a workspace.
pub fn top_groups(is_3d: bool) -> Vec<&'static [Entry]> {
    let mut groups: Vec<&'static [Entry]> = vec![STANDARD, PROPERTIES, DIMENSION, INQUIRY, FADE];
    if !is_3d {
        groups.push(GIS);
    }
    groups
}

/// The top toolbar row, under the menus.
pub fn top_bar(is_3d: bool) -> Element<'static, Message> {
    let mut items = row![].spacing(1).align_y(iced::Center);
    for (k, group) in top_groups(is_3d).into_iter().enumerate() {
        if k > 0 {
            items = items.push(container(rule(false)).padding([0, 4]));
        }
        for entry in group {
            items = items.push(tool_button(entry, tooltip::Position::Bottom));
        }
    }
    container(items)
        .padding([2, 6])
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(crate::ui::style::chrome::bar(theme))),
            ..Default::default()
        })
        .into()
}

/// The tools of the left edge: Draw, and the modeling tools in 3D.
pub fn left_groups(is_3d: bool) -> Vec<&'static [Entry]> {
    if is_3d { vec![DRAW, MODELING] } else { vec![DRAW] }
}

fn vertical(groups: Vec<&'static [Entry]>, tip_at: tooltip::Position) -> Element<'static, Message> {
    let mut col = column![].spacing(1).align_x(iced::Center);
    for (k, group) in groups.into_iter().enumerate() {
        if k > 0 {
            col = col.push(container(rule(true)).padding([3, 0]));
        }
        for entry in group {
            col = col.push(tool_button(entry, tip_at));
        }
    }
    let panel = container(iced::widget::scrollable(col).direction(
        iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(0).scroller_width(0),
        ),
    ))
    .padding(2)
    .style(panel_style);
    iced::widget::opaque(panel)
}

/// Overlay: the Draw toolbar down the left edge of the drawing.
pub fn left_overlay(is_3d: bool) -> Element<'static, Message> {
    container(vertical(left_groups(is_3d), tooltip::Position::Right))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Left)
        .align_y(iced::Top)
        .padding(iced::Padding { left: 4.0, top: 40.0, bottom: 40.0, right: 0.0 })
        .into()
}

/// Overlay: the Modify toolbar down the right edge of the drawing; in 3D it
/// starts below the ViewCube.
pub fn right_overlay(is_3d: bool) -> Element<'static, Message> {
    container(vertical(vec![MODIFY], tooltip::Position::Left))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Right)
        .align_y(iced::Top)
        .padding(iced::Padding {
            right: 4.0,
            top: if is_3d { 170.0 } else { 40.0 },
            bottom: 40.0,
            left: 0.0,
        })
        .into()
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
    fn gis_shows_in_2d_and_modeling_in_3d() {
        assert!(top_groups(false).iter().any(|g| std::ptr::eq(*g, GIS)));
        assert!(!top_groups(true).iter().any(|g| std::ptr::eq(*g, GIS)));
        assert_eq!(left_groups(true).len(), 2);
    }
}
