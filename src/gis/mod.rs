//! CadMini Studio GIS: the VN-2000 coordinate system and GIS file formats.
//! The commands that use them live in `app::commands::gis_cmd`.

pub mod crs;
pub mod formats;

pub use crs::DrawingCrs;
pub use formats::{Coords, Feature, Geom};
