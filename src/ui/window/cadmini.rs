//! Messages of CadMini Studio's dialogs (GPMB, TRICHLUC, XUAT, GISCRS);
//! their state and views live in `app::commands::cadmini_dialogs`.

/// One edit or press in a CadMini dialog. Fields and actions are named by
/// their key.
#[derive(Clone, Debug)]
pub enum Msg {
    /// A text field or a pick list changed.
    Set(&'static str, String),
    /// A check box changed.
    Check(&'static str, bool),
    /// A button.
    Act(&'static str),
    /// GPMB: a parcel layer ticked / unticked.
    Layer(usize, bool),
    /// GPMB: a parcel's cell edited (parcel handle, field, value).
    Cell(u64, &'static str, String),
    /// GPMB: show a parcel on the drawing.
    Zoom(u64),
}
