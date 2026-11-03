//! The satellite / map background under a VN-2000 drawing (GISSAT), as in
//! CadMini: XYZ tiles at the zoom that matches the screen, kept on disk,
//! each tile bent onto VN-2000 with an 8×8 grid and drawn as a raster
//! under the drawing. While a sharp tile loads, the coarser ones already
//! in memory show in its place.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::crs::DrawingCrs;
use super::tiles::{self, Tile, TileKey, TileSource};
use crate::scene::model::image_model::{ImageModel, ImageQuadVertex};

const GRID: usize = 8;
const MAX_TILES: usize = 600;
/// More tiles than this in view: too far out to fetch.
const MAX_IN_VIEW: usize = 250;
/// Tiles downloading at once.
pub const MAX_PARALLEL: usize = 8;

#[derive(Default)]
pub struct Basemap {
    pub on: bool,
    pub source_key: String,
    pub custom_url: String,
    /// 0.1 – 1.0.
    pub opacity: f32,
    /// Loaded tiles; `None` = the server has no image there.
    tiles: HashMap<TileKey, Option<Tile>>,
    order: VecDeque<TileKey>,
    pending: HashSet<TileKey>,
    /// Tiles arrived since the last build.
    pub dirty: bool,
    /// The view the models were last built for.
    last_view: Option<[i64; 6]>,
}

impl Basemap {
    pub fn new(on: bool, source_key: &str, custom_url: &str, opacity: f32) -> Self {
        Self {
            on,
            source_key: source_key.to_string(),
            custom_url: custom_url.to_string(),
            opacity: opacity.clamp(0.1, 1.0),
            ..Default::default()
        }
    }

    pub fn source(&self) -> TileSource {
        tiles::source(&self.source_key, &self.custom_url)
    }

    /// Forget every tile (another source).
    pub fn reset(&mut self) {
        self.tiles.clear();
        self.order.clear();
        self.pending.clear();
        self.last_view = None;
        self.dirty = true;
    }

    /// Force the next build even when the view did not move.
    pub fn invalidate(&mut self) {
        self.last_view = None;
    }

    pub fn receive(&mut self, source_key: &str, key: TileKey, tile: Option<Tile>) {
        if source_key != self.source().key {
            return;
        }
        self.pending.remove(&key);
        self.tiles.insert(key, tile);
        self.order.push_back(key);
        while self.tiles.len() > MAX_TILES {
            match self.order.pop_front() {
                Some(old) => {
                    self.tiles.remove(&old);
                }
                None => break,
            }
        }
        self.dirty = true;
    }

    /// Whether the view moved, or tiles arrived, since the last build.
    pub fn needs_build(&self, center: [f64; 2], size: [f64; 2], px: f64) -> bool {
        self.dirty || self.last_view != Some(view_key(center, size, px))
    }

    /// The background for the view (centre, size in drawing metres, metres
    /// per screen pixel), and the tiles still to download, at most enough
    /// to fill the download slots.
    pub fn build(
        &mut self,
        crs: &DrawingCrs,
        center: [f64; 2],
        size: [f64; 2],
        px: f64,
    ) -> (Vec<ImageModel>, Vec<TileKey>) {
        self.dirty = false;
        self.last_view = Some(view_key(center, size, px));
        let source = self.source();
        // The view's outline in Web Mercator.
        let (hw, hh) = (size[0] / 2.0, size[1] / 2.0);
        let mut mx = Vec::new();
        let mut my = Vec::new();
        for fx in [-1.0, 0.0, 1.0] {
            for fy in [-1.0, 0.0, 1.0] {
                let (lon, lat) = crs.to_wgs84(center[0] + fx * hw, center[1] + fy * hh);
                let (x, y) = tiles::lonlat_to_merc(lon, lat);
                mx.push(x);
                my.push(y);
            }
        }
        let (lon, lat) = crs.to_wgs84(center[0], center[1]);
        if !(lon.is_finite() && lat.is_finite()) || lat.abs() > 85.0 || px <= 0.0 {
            return (Vec::new(), Vec::new());
        }
        let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let z = tiles::zoom_for(px, lat, source.max_zoom);
        let need = tiles::tiles_for(min(&mx), min(&my), max(&mx), max(&my), z);
        if need.len() > MAX_IN_VIEW {
            return (Vec::new(), Vec::new());
        }
        let mut fine: Vec<(TileKey, Tile)> = Vec::new();
        let mut coarse: Vec<(TileKey, Tile)> = Vec::new();
        let mut coarse_seen: HashSet<TileKey> = HashSet::new();
        let mut missing = Vec::new();
        for &key in &need {
            match self.tiles.get(&key) {
                Some(Some(tile)) => {
                    fine.push((key, tile.clone()));
                    continue;
                }
                Some(None) => {}
                None => {
                    if !self.pending.contains(&key) {
                        missing.push(key);
                    }
                }
            }
            // Not (yet) here: the nearest loaded parent stands in.
            let mut parent = key;
            for _ in 0..5 {
                if parent.0 == 0 {
                    break;
                }
                parent = (parent.0 - 1, parent.1 / 2, parent.2 / 2);
                match self.tiles.get(&parent) {
                    Some(Some(tile)) => {
                        if coarse_seen.insert(parent) {
                            coarse.push((parent, tile.clone()));
                        }
                        break;
                    }
                    Some(None) => continue,
                    None => {
                        // Ask for the parent of a tile the server lacks, so
                        // something shows there.
                        if matches!(self.tiles.get(&key), Some(None))
                            && !self.pending.contains(&parent)
                            && !missing.contains(&parent)
                        {
                            missing.push(parent);
                        }
                        break;
                    }
                }
            }
        }
        // The tile under the centre first.
        let (cx, cy) = tiles::lonlat_to_merc(lon, lat);
        missing.sort_by(|a, b| {
            let d = |k: &TileKey| {
                let (x0, y0, x1, y1) = tiles::tile_bounds(*k);
                ((x0 + x1) / 2.0 - cx).hypot((y0 + y1) / 2.0 - cy)
            };
            d(a).total_cmp(&d(b))
        });
        let free = MAX_PARALLEL.saturating_sub(self.pending.len());
        missing.truncate(free);
        for key in &missing {
            self.pending.insert(*key);
        }
        coarse.sort_by_key(|(k, _)| k.0);
        let mut models = Vec::with_capacity(coarse.len() + fine.len());
        for (k, (key, tile)) in coarse.iter().chain(fine.iter()).enumerate() {
            // Coarser tiles lie deeper; the sharp ones on top of them.
            let depth = -0.99 + 0.0001 * (key.0 as f32) + 1e-7 * (k as f32);
            models.push(tile_model(crs, *key, tile, self.opacity, depth));
        }
        (models, missing)
    }
}

fn view_key(center: [f64; 2], size: [f64; 2], px: f64) -> [i64; 6] {
    let q = |v: f64| (v / px.max(1e-9)).round() as i64;
    [q(center[0]), q(center[1]), q(size[0]), q(size[1]), (px * 1e6).round() as i64, 0]
}

fn split(v: f64) -> (f32, f32) {
    let hi = v as f32;
    (hi, (v - f64::from(hi)) as f32)
}

/// One tile as a raster bent onto VN-2000.
pub fn tile_model(crs: &DrawingCrs, key: TileKey, tile: &Tile, opacity: f32, depth: f32) -> ImageModel {
    let (mx0, my0, mx1, my1) = tiles::tile_bounds(key);
    let mut grid = vec![[0.0f64; 2]; (GRID + 1) * (GRID + 1)];
    for j in 0..=GRID {
        for i in 0..=GRID {
            let u = i as f64 / GRID as f64;
            let v = j as f64 / GRID as f64;
            let (lon, lat) = tiles::merc_to_lonlat(mx0 + u * (mx1 - mx0), my1 - v * (my1 - my0));
            let (x, y) = crs.from_wgs84(lon, lat);
            grid[j * (GRID + 1) + i] = [x, y];
        }
    }
    let vertex = |i: usize, j: usize| {
        let [x, y] = grid[j * (GRID + 1) + i];
        let (xh, xl) = split(x);
        let (yh, yl) = split(y);
        ImageQuadVertex {
            pos: [xh, yh, 0.0],
            uv: [i as f32 / GRID as f32, j as f32 / GRID as f32],
            pos_low: [xl, yl, 0.0],
        }
    };
    let mut verts = Vec::with_capacity(GRID * GRID * 6);
    for j in 0..GRID {
        for i in 0..GRID {
            let (a, b, c, d) = (vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1), vertex(i, j + 1));
            verts.extend([a, d, c, a, c, b]);
        }
    }
    let corner = |i: usize, j: usize| {
        let v = vertex(i, j);
        (v.pos, v.pos_low)
    };
    let (c0, l0) = corner(0, GRID);
    let (c1, l1) = corner(GRID, GRID);
    let (c2, l2) = corner(GRID, 0);
    let (c3, l3) = corner(0, 0);
    ImageModel {
        render_instance: None,
        file_path: format!("basemap:{}/{}/{}", key.0, key.1, key.2),
        pixels: Arc::clone(&tile.rgba),
        width: tile.width,
        height: tile.height,
        opacity,
        corners: [c0, c1, c2, c3],
        corners_low: [l0, l1, l2, l3],
        draw_depth: depth,
        verts,
        pixelated: false,
        use_alpha: false,
    }
}

/// The tiles an export of `window` (drawing units) needs at `units_per_px`
/// (the output's resolution), coarser while there would be more than
/// `max_tiles`: the zoom, the keys.
pub fn export_tiles(
    crs: &DrawingCrs,
    source: &TileSource,
    window: [f64; 4],
    units_per_px: f64,
    max_tiles: usize,
) -> Option<(u8, Vec<TileKey>)> {
    let [x0, y0, x1, y1] = window;
    let (mut mx, mut my) = (Vec::new(), Vec::new());
    for fx in [0.0, 0.5, 1.0] {
        for fy in [0.0, 0.5, 1.0] {
            let (lon, lat) = crs.to_wgs84(x0 + fx * (x1 - x0), y0 + fy * (y1 - y0));
            if !(lon.is_finite() && lat.is_finite()) || lat.abs() > 85.0 {
                return None;
            }
            let (x, y) = tiles::lonlat_to_merc(lon, lat);
            mx.push(x);
            my.push(y);
        }
    }
    let (_, lat) = crs.to_wgs84((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut z = tiles::zoom_for(units_per_px.max(1e-6), lat, source.max_zoom);
    loop {
        let keys = tiles::tiles_for(min(&mx), min(&my), max(&mx), max(&my), z);
        if keys.len() <= max_tiles || z == 0 {
            return Some((z, keys));
        }
        z -= 1;
    }
}

/// The background of an export: every tile it needs, loaded (disk cache,
/// else downloaded, `parallel` at a time), as rasters bent onto VN-2000
/// under the drawing. Blocking: call from a worker thread. Returns the
/// rasters and how many tiles could not be had.
#[cfg(not(target_arch = "wasm32"))]
pub fn export_models(
    crs: &DrawingCrs,
    source: &TileSource,
    window: [f64; 4],
    units_per_px: f64,
    opacity: f32,
) -> (Vec<ImageModel>, usize) {
    let Some((_, keys)) = export_tiles(crs, source, window, units_per_px, 400) else {
        return (Vec::new(), 0);
    };
    let mut loaded: Vec<(TileKey, Option<Tile>)> = Vec::with_capacity(keys.len());
    for chunk in keys.chunks(MAX_PARALLEL) {
        std::thread::scope(|scope| {
            let handles: Vec<_> =
                chunk.iter().map(|&key| scope.spawn(move || (key, tiles::load(source, key)))).collect();
            for h in handles {
                if let Ok(r) = h.join() {
                    loaded.push(r);
                }
            }
        });
    }
    let missing = loaded.iter().filter(|(_, t)| t.is_none()).count();
    let models = loaded
        .iter()
        .enumerate()
        .filter_map(|(k, (key, tile))| {
            tile.as_ref().map(|t| tile_model(crs, *key, t, opacity, -0.99 + 1e-7 * k as f32))
        })
        .collect();
    (models, missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile() -> Tile {
        Tile { rgba: Arc::new(vec![0u8; 4 * 4 * 4]), width: 4, height: 4 }
    }

    #[test]
    fn builds_loaded_tiles_and_asks_for_the_rest() {
        let crs = DrawingCrs::parse_user("Hà Tĩnh").unwrap();
        let mut map = Basemap::new(true, "esri_imagery", "", 1.0);
        let center = [542_131.0, 2_029_730.0];
        let (models, missing) = map.build(&crs, center, [400.0, 300.0], 0.5);
        assert!(models.is_empty());
        assert!(!missing.is_empty() && missing.len() <= MAX_PARALLEL);
        // Nothing asked twice while pending.
        map.invalidate();
        let (_, again) = map.build(&crs, center, [400.0, 300.0], 0.5);
        assert!(again.iter().all(|k| !missing.contains(k)));
        for key in &missing {
            map.receive("esri_imagery", *key, Some(tile()));
        }
        assert!(map.needs_build(center, [400.0, 300.0], 0.5));
        let (models, _) = map.build(&crs, center, [400.0, 300.0], 0.5);
        assert_eq!(models.len(), missing.len());
        // A tile lands where VN-2000 says: the view centre is inside the
        // bounding box of one of them.
        let inside = |v: &[f64], c: f64| {
            v.iter().cloned().fold(f64::MAX, f64::min) <= c
                && c <= v.iter().cloned().fold(f64::MIN, f64::max)
        };
        let holds_centre = |m: &ImageModel| {
            let xs: Vec<f64> =
                m.verts.iter().map(|v| f64::from(v.pos[0]) + f64::from(v.pos_low[0])).collect();
            let ys: Vec<f64> =
                m.verts.iter().map(|v| f64::from(v.pos[1]) + f64::from(v.pos_low[1])).collect();
            inside(&xs, center[0]) && inside(&ys, center[1])
        };
        assert!(models.iter().any(holds_centre));
        let m = &models[0];
        assert_eq!(m.verts.len(), GRID * GRID * 6);
    }

    #[test]
    fn a_missing_tile_shows_its_loaded_parent() {
        let crs = DrawingCrs::parse_user("Hà Tĩnh").unwrap();
        let mut map = Basemap::new(true, "osm", "", 1.0);
        let center = [542_131.0, 2_029_730.0];
        let (_, missing) = map.build(&crs, center, [100.0, 100.0], 0.25);
        let key = missing[0];
        let parent = (key.0 - 1, key.1 / 2, key.2 / 2);
        map.receive("osm", parent, Some(tile()));
        let (models, _) = map.build(&crs, center, [100.0, 100.0], 0.25);
        assert!(models.iter().any(|m| m.file_path == format!("basemap:{}/{}/{}", parent.0, parent.1, parent.2)));
    }
}
