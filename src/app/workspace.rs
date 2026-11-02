//! CadMini Studio: separate 2D drafting and 3D modeling workspaces.
//!
//! The 2D workspace shows the drafting tools and the GIS tools, keeps the
//! view looking straight down and hides the 3D chrome (ViewCube, the Model
//! tab, the 3D menus). The 3D workspace shows the modeling tools and hides
//! the GIS ones. WSCURRENT switches between them.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Workspace {
    /// 2D drafting + GIS.
    #[default]
    Drafting2D,
    /// 3D modeling.
    Modeling3D,
}

impl Workspace {
    pub fn is_3d(self) -> bool {
        self == Workspace::Modeling3D
    }

    /// Parse the answer to WSCURRENT: `2D`, `3D`, or a workspace name.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_uppercase();
        match s.as_str() {
            "2D" | "2" | "2D DRAFTING" | "DRAFTING" | "2D DRAFTING & ANNOTATION" | "GIS" => {
                Some(Workspace::Drafting2D)
            }
            "3D" | "3" | "3D MODELING" | "MODELING" => Some(Workspace::Modeling3D),
            _ => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Workspace::Drafting2D => "2D Drafting & GIS",
            Workspace::Modeling3D => "3D Modeling",
        }
    }

    /// Ribbon tabs (module ids) this workspace keeps out of sight.
    pub fn hidden_modules(self) -> &'static [&'static str] {
        match self {
            Workspace::Drafting2D => &["model"],
            Workspace::Modeling3D => &["gis"],
        }
    }

    /// Top-level menus this workspace keeps out of the menu bar.
    pub fn hidden_menus(self) -> &'static [&'static str] {
        match self {
            Workspace::Drafting2D => &["Modeling"],
            Workspace::Modeling3D => &["GIS"],
        }
    }

    /// Submenus this workspace keeps out of the menus.
    pub fn hidden_submenus(self) -> &'static [&'static str] {
        match self {
            Workspace::Drafting2D => &["3D Views", "Visual Styles"],
            Workspace::Modeling3D => &[],
        }
    }
}

/// Commands that only make sense with a 3D view. In the 2D workspace they
/// are refused with a hint to switch.
pub const THREE_D_COMMANDS: &[&str] = &[
    "BOX", "CYLINDER", "CONE", "SPHERE", "PYRAMID", "WEDGE", "TORUS", "POLYSOLID",
    "EXTRUDE", "REVOLVE", "LOFT", "SWEEP", "PRESSPULL", "UNION", "SUBTRACT", "INTERSECT",
    "FILLETEDGE", "CHAMFEREDGE", "SHELL", "SLICE", "SECTIONPLANE", "3DORBIT", "ORBIT",
    "3DO", "3O", "3DFORBIT", "3DCORBIT", "3DMOVE", "3DROTATE", "3DALIGN", "3DMIRROR",
    "3DARRAY", "VIEW_ISO", "PERSPECTIVE",
];

/// Whether `verb` is a 3D-only command.
pub fn is_3d_command(verb: &str) -> bool {
    let verb = verb.trim_start_matches(['_', '.']).to_uppercase();
    THREE_D_COMMANDS.contains(&verb.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_answers() {
        assert_eq!(Workspace::parse("2d"), Some(Workspace::Drafting2D));
        assert_eq!(Workspace::parse(" 3D "), Some(Workspace::Modeling3D));
        assert_eq!(Workspace::parse("x"), None);
    }

    #[test]
    fn three_d_commands() {
        assert!(is_3d_command("_extrude"));
        assert!(!is_3d_command("LINE"));
    }
}

/// CadMini Studio: how the commands are laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UiStyle {
    /// Pull-down menus and classic toolbars (Draw on the left, Modify on the
    /// right, Standard / Dimension on top), as CadMini and AutoCAD 2007.
    #[default]
    Classic,
    /// The Open CAD Studio ribbon (menus kept above it).
    Ribbon,
}

impl UiStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "CLASSIC" | "C" | "TOOLBAR" | "TOOLBARS" | "MENU" => Some(UiStyle::Classic),
            "RIBBON" | "R" => Some(UiStyle::Ribbon),
            _ => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            UiStyle::Classic => "Classic menus and toolbars",
            UiStyle::Ribbon => "Ribbon",
        }
    }
}
