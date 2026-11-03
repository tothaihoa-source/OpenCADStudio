//! XUAT (EXPORTWIN) — a window of the drawing to PDF / JPG / PNG, as CadMini:
//! pick two corners, set the page up on the command line (format, paper,
//! orientation, scale, colour, satellite background, resolution), Save.
//!
//! The page is plotted like PLOT does (vector PDF, line weights); the
//! satellite / map background is laid under it bent onto VN-2000; a JPG or
//! PNG is that page rasterised at the chosen resolution. The tiles download
//! and the file is written on a worker thread.

use crate::app::{Message, OpenCADStudio};
use crate::command::{CadCommand, CmdOption, CmdResult, FreeTextValuePromptCommand};
use crate::io::pdf_export::PdfPageInput;
use glam::DVec3;
use iced::Task;
use serde::{Deserialize, Serialize};

pub const PAPERS: &[(&str, f64, f64)] = &[
    ("A4", 210.0, 297.0),
    ("A3", 297.0, 420.0),
    ("A2", 420.0, 594.0),
    ("A1", 594.0, 841.0),
    ("A0", 841.0, 1189.0),
];
/// Raster pages above this many pixels lower their resolution.
const MAX_PIXELS: f64 = 60_000_000.0;

/// XUAT's options, remembered between runs (CadMini's keys).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// "pdf", "jpg" or "png".
    pub fmt: String,
    pub paper: String,
    /// "auto", "portrait" or "landscape".
    pub orient: String,
    /// 0: fit the paper; else 1:scale.
    pub scale: u32,
    /// "m" or "mm".
    pub unit: String,
    pub margin: f64,
    /// Black lines (monochrome) or the drawing's colours.
    pub monochrome: bool,
    pub dpi: u32,
    pub satellite: bool,
    pub open: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fmt: "pdf".into(),
            paper: "A3".into(),
            orient: "auto".into(),
            scale: 0,
            unit: "m".into(),
            margin: 5.0,
            monochrome: true,
            dpi: 200,
            satellite: false,
            open: true,
        }
    }
}

/// The page for a window: paper size (mm), mm of paper per drawing unit,
/// and where the window's lower-left corner lands (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Page {
    pub paper: [f64; 2],
    pub mm_per_unit: f64,
    pub target: [f64; 2],
    /// The scale's denominator (1:n).
    pub scale: f64,
}

impl Options {
    pub fn paper_mm(&self, window: [f64; 4]) -> [f64; 2] {
        let (_, a, b) = PAPERS.iter().find(|(n, ..)| n.eq_ignore_ascii_case(&self.paper)).unwrap_or(&PAPERS[1]);
        let landscape = match self.orient.as_str() {
            "landscape" => true,
            "portrait" => false,
            _ => window[2] - window[0] > window[3] - window[1],
        };
        if landscape { [*b, *a] } else { [*a, *b] }
    }

    pub fn page(&self, window: [f64; 4]) -> Page {
        let paper = self.paper_mm(window);
        let (w, h) = ((window[2] - window[0]).max(1e-9), (window[3] - window[1]).max(1e-9));
        let per_unit_mm = if self.unit == "m" { 1000.0 } else { 1.0 };
        let mm_per_unit = if self.scale > 0 {
            per_unit_mm / f64::from(self.scale)
        } else {
            let m = self.margin.max(0.0);
            ((paper[0] - 2.0 * m) / w).min((paper[1] - 2.0 * m) / h).max(1e-12)
        };
        let target = [(paper[0] - w * mm_per_unit) / 2.0, (paper[1] - h * mm_per_unit) / 2.0];
        Page { paper, mm_per_unit, target, scale: per_unit_mm / mm_per_unit }
    }

    /// The raster resolution that keeps the page under the pixel budget.
    pub fn raster_dpi(&self, paper: [f64; 2]) -> u32 {
        let mut dpi = f64::from(self.dpi.max(50));
        while dpi > 72.0 && (paper[0] * dpi / 25.4) * (paper[1] * dpi / 25.4) > MAX_PIXELS {
            dpi *= 0.8;
        }
        dpi.round() as u32
    }

    /// Drawing units a background pixel spans: ~0.2 mm of paper for a PDF,
    /// the raster's pixel otherwise.
    pub fn background_units_per_px(&self, page: &Page) -> f64 {
        let mm = if self.fmt == "pdf" { 0.2 } else { (25.4 / f64::from(self.raster_dpi(page.paper))).max(0.08) };
        mm / page.mm_per_unit
    }

    fn summary(&self, window: [f64; 4], has_crs: bool) -> String {
        let page = self.page(window);
        let landscape = page.paper[0] > page.paper[1];
        let scale = if self.scale > 0 { format!("1:{}", self.scale) } else { format!("vừa khổ ≈1:{:.0}", page.scale) };
        format!(
            "{} · {} {}{} · {} · {}{}{}",
            self.fmt.to_uppercase(),
            self.paper,
            if landscape { "ngang" } else { "đứng" },
            if self.orient == "auto" { " (tự động)" } else { "" },
            scale,
            if self.monochrome { "đen trắng" } else { "màu" },
            if self.fmt == "pdf" { String::new() } else { format!(" · {} dpi", self.dpi) },
            if has_crs && self.satellite { " · nền vệ tinh" } else { "" },
        )
    }
}

/// The window being exported and its options.
pub(in crate::app) struct Pending {
    tab: u64,
    window: [f64; 4],
}

/// XUAT's first step: two corners.
struct Corners {
    first: Option<DVec3>,
}

impl CadCommand for Corners {
    fn name(&self) -> &'static str {
        "XUAT"
    }

    fn prompt(&self) -> String {
        if self.first.is_none() {
            crate::t!("XUAT  First corner of the area to export:").into_owned()
        } else {
            crate::t!("XUAT  Opposite corner:").into_owned()
        }
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.first {
            None => {
                self.first = Some(pt);
                CmdResult::NeedPoint
            }
            Some(a) => {
                let (x0, y0, x1, y1) = (a.x.min(pt.x), a.y.min(pt.y), a.x.max(pt.x), a.y.max(pt.y));
                if x1 - x0 <= 0.0 || y1 - y0 <= 0.0 {
                    return CmdResult::NeedPoint;
                }
                CmdResult::Dispatch(format!("_XUATBOX {x0},{y0},{x1},{y1}"))
            }
        }
    }

    fn on_mouse_move(&mut self, pt: DVec3) -> Option<crate::scene::WireModel> {
        let a = self.first?;
        let p = |x: f64, y: f64| [x as f32, y as f32, a.z as f32];
        Some(crate::scene::WireModel::solid(
            "xuat_window".into(),
            vec![p(a.x, a.y), p(pt.x, a.y), p(pt.x, pt.y), p(a.x, pt.y), p(a.x, a.y)],
            [1.0, 0.8, 0.2, 1.0],
            false,
        ))
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

const MENU: &[(&str, &str)] = &[
    ("Save", "SAVE"),
    ("PDF/JPG/PNG", "FORMAT"),
    ("Paper", "PAPER"),
    ("Orientation", "ORIENT"),
    ("Scale", "SCALE"),
    ("Mono/Color", "COLOR"),
    ("Satellite", "SAT"),
    ("DPI", "DPI"),
];

/// The set-up menu, its options as buttons; SCALE and DPI take a value.
struct Menu {
    summary: String,
    value: Option<&'static str>,
}

impl CadCommand for Menu {
    fn name(&self) -> &'static str {
        "XUAT"
    }

    fn prompt(&self) -> String {
        match self.value {
            Some("SCALE") => crate::t!("XUAT  Scale 1:<n> (0 = fit the paper):").into_owned(),
            Some(_) => crate::t!("XUAT  Image resolution, dpi (100–600):").into_owned(),
            None => format!("XUAT  {}  <Save> (hoặc gõ tên file):", self.summary),
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        if self.value.is_some() {
            return Vec::new();
        }
        MENU.iter().map(|(label, kw)| CmdOption::new(label, kw)).collect()
    }

    fn wants_text_input(&self) -> bool {
        true
    }

    /// As typed: a file name keeps its case and spaces.
    fn input_kind(&self) -> crate::command::InputKind {
        crate::command::InputKind::FreeText
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        if let Some(kw) = self.value {
            return Some(CmdResult::Dispatch(format!("_XUATSET {kw} {}", t.trim_start_matches("1:"))));
        }
        // A file name typed straight in: save there.
        let low = t.to_lowercase();
        if [".pdf", ".jpg", ".jpeg", ".png"].iter().any(|e| low.ends_with(e)) {
            return Some(CmdResult::Dispatch(format!("_XUATFILE {t}")));
        }
        let t = t.strip_prefix('_').unwrap_or(t);
        let up = t.to_uppercase();
        let exact = MENU.iter().find(|(l, k)| k.eq_ignore_ascii_case(&up) || l.eq_ignore_ascii_case(t));
        let prefix: Vec<_> =
            MENU.iter().filter(|(l, k)| k.starts_with(&up) || l.to_uppercase().starts_with(&up)).collect();
        let Some((_, kw)) = exact.or(if prefix.len() == 1 { Some(prefix[0]) } else { None }) else {
            return Some(CmdResult::NeedPoint);
        };
        if matches!(*kw, "SCALE" | "DPI") {
            self.value = Some(kw);
            return Some(CmdResult::NeedPoint);
        }
        Some(CmdResult::Dispatch(format!("_XUATSET {kw}")))
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.value.take().is_some() {
            return CmdResult::NeedPoint;
        }
        CmdResult::Dispatch("_XUATSET SAVE".into())
    }
}

fn cycle(list: &[&str], current: &str) -> String {
    let k = list.iter().position(|v| v.eq_ignore_ascii_case(current)).map_or(0, |k| (k + 1) % list.len());
    list[k].to_string()
}

/// Render the page (with the background under it) to `path`: the PDF, or
/// it rasterised. Blocking. Returns the line to report.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn write_export(
    mut page_input: PdfPageInput,
    opts: &Options,
    page: Page,
    window: [f64; 4],
    background: Option<(crate::gis::crs::DrawingCrs, crate::gis::tiles::TileSource, f32)>,
    path: &str,
) -> Result<String, String> {
    let started = std::time::Instant::now();
    let mut extra = String::new();
    if let Some((crs, source, opacity)) = background {
        let upp = opts.background_units_per_px(&page);
        let (models, missing) = crate::gis::basemap::export_models(&crs, &source, window, upp, opacity);
        extra = format!(", {} ô ảnh nền {}", models.len(), source.name);
        if missing > 0 {
            extra.push_str(&format!(" ({missing} ô không tải được)"));
        }
        let n = models.len();
        let images = &mut page_input.content.images;
        images.splice(
            0..0,
            models.into_iter().map(|image| crate::io::pdf_export::PlotImage { image, clips: Vec::new() }),
        );
        page_input.content.group_splits.images += n;
    }
    let bytes = crate::io::pdf_export::render_pdf_bytes(std::slice::from_ref(&page_input), None)?;
    let p = std::path::Path::new(path);
    if opts.fmt == "pdf" {
        std::fs::write(p, &bytes).map_err(|e| e.to_string())?;
    } else {
        let dpi = opts.raster_dpi(page.paper);
        let pages =
            crate::scene::model::pdf_raster::rasterize_bytes_pages(std::sync::Arc::new(bytes), dpi as f32, 1);
        let r = pages.first().ok_or("không raster hoá được trang")?;
        let img = image::RgbaImage::from_raw(r.width, r.height, r.pixels.as_ref().clone())
            .ok_or("ảnh sai kích thước")?;
        // On white paper.
        let mut rgb = image::RgbImage::new(r.width, r.height);
        for (o, px) in rgb.pixels_mut().zip(img.pixels()) {
            let a = u16::from(px[3]);
            let mix = |c: u8| ((u16::from(c) * a + 255 * (255 - a)) / 255) as u8;
            *o = image::Rgb([mix(px[0]), mix(px[1]), mix(px[2])]);
        }
        let file = std::fs::File::create(p).map_err(|e| e.to_string())?;
        let mut w = std::io::BufWriter::new(file);
        if opts.fmt == "jpg" {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 92)
                .encode_image(&rgb)
                .map_err(|e| e.to_string())?;
        } else {
            rgb.write_to(&mut w, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        }
        extra.push_str(&format!(", {dpi} dpi, {}×{} điểm ảnh", r.width, r.height));
    }
    let landscape = page.paper[0] > page.paper[1];
    Ok(format!(
        "XUAT: đã xuất {path} ({} {}, tỉ lệ 1:{:.0}{extra}) trong {:.1} s.",
        opts.paper,
        if landscape { "ngang" } else { "đứng" },
        page.scale,
        started.elapsed().as_secs_f64()
    ))
}

impl OpenCADStudio {
    fn xuat_options(&self) -> Options {
        serde_json::from_str(&self.xuat_options).unwrap_or_default()
    }

    fn xuat_store(&mut self, o: &Options) {
        self.xuat_options = serde_json::to_string(o).unwrap_or_default();
        self.persist_settings_if_changed();
    }

    fn xuat_show_menu(&mut self, i: usize) -> Task<Message> {
        let Some(pd) = &self.xuat else { return Task::none() };
        let has_crs = super::gis_cmd::drawing_crs(&self.tabs[i].scene.document).is_some();
        let c: Box<dyn CadCommand> = Box::new(Menu { summary: self.xuat_options().summary(pd.window, has_crs), value: None });
        self.command_line.push_info(&c.prompt());
        self.tabs[i].active_cmd = Some(c);
        Task::none()
    }

    /// The page and what the worker needs, from the drawing as it stands.
    fn xuat_job(
        &self,
        i: usize,
        window: [f64; 4],
        opts: &Options,
    ) -> (PdfPageInput, Page, Option<(crate::gis::crs::DrawingCrs, crate::gis::tiles::TileSource, f32)>) {
        let page = opts.page(window);
        let input = self.window_plot_page(window, page.paper, page.mm_per_unit, page.target, opts.monochrome);
        let background = if opts.satellite {
            super::gis_cmd::drawing_crs(&self.tabs[i].scene.document)
                .map(|crs| (crs, self.basemap.source(), self.basemap.opacity))
        } else {
            None
        };
        (input, page, background)
    }

    fn xuat_save(&mut self, i: usize, path: &str) -> Task<Message> {
        let Some(pd) = self.xuat.take() else { return Task::none() };
        if pd.tab != self.tabs[i].id {
            return Task::none();
        }
        let opts = self.xuat_options();
        let path = {
            let ext = if opts.fmt == "jpg" { "jpg" } else { opts.fmt.as_str() };
            let low = path.to_lowercase();
            if low.ends_with(&format!(".{ext}")) || (ext == "jpg" && low.ends_with(".jpeg")) {
                path.to_string()
            } else {
                format!("{path}.{ext}")
            }
        };
        let (input, page, background) = self.xuat_job(i, pd.window, &opts);
        if opts.satellite && background.is_none() {
            self.command_line.push_info(crate::t!("XUAT: the drawing has no coordinate system (GISCRS): no satellite background.").as_ref());
        }
        self.command_line.push_info(&format!("XUAT: đang xuất {path}…"));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let window = pd.window;
            let open = opts.open;
            Task::perform(
                async move {
                    let (tx, rx) = iced::futures::channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let r = write_export(input, &opts, page, window, background, &path);
                        let _ = tx.send((r, path));
                    });
                    rx.await.unwrap_or((Err("dừng giữa chừng".into()), String::new()))
                },
                move |(r, path)| match r {
                    Ok(line) => Message::Command(format!("_XUATDONE {}{line}\n{path}", if open { "1" } else { "0" })),
                    Err(e) => Message::Command(format!("_XUATDONE E{e}")),
                },
            )
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (input, page, background, path);
            Task::none()
        }
    }

    pub(super) fn dispatch_xuat(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        let trimmed = cmd.trim().trim_start_matches(['_', '.']);
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_ascii_uppercase(), r.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };
        match verb.as_str() {
            "XUAT" | "EXPORTWIN" if rest.is_empty() => {
                let c: Box<dyn CadCommand> = Box::new(Corners { first: None });
                self.command_line.push_info(&c.prompt());
                self.tabs[i].active_cmd = Some(c);
                Some(Task::none())
            }
            "XUATBOX" => {
                let v: Vec<f64> = rest.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if v.len() != 4 {
                    return Some(Task::none());
                }
                let window = [v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])];
                self.xuat = Some(Pending { tab: self.tabs[i].id, window });
                Some(self.xuat_show_menu(i))
            }
            "XUATSET" => {
                if self.xuat.is_none() {
                    return Some(Task::none());
                }
                let (key, value) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(k, v)| (k, v.trim()));
                let mut o = self.xuat_options();
                match key.to_ascii_uppercase().as_str() {
                    "SAVE" | "" => {
                        if crate::sys::file_dia() {
                            let ext = o.fmt.clone();
                            let name = format!("{}_xuat.{ext}", self.tabs[i].tab_display_name());
                            let filter = match ext.as_str() {
                                "jpg" => ("JPG (*.jpg)", vec!["jpg", "jpeg"]),
                                "png" => ("PNG (*.png)", vec!["png"]),
                                _ => ("PDF (*.pdf)", vec!["pdf"]),
                            };
                            return Some(Task::perform(
                                async move {
                                    let handle = crate::sys::file_dialog()
                                        .set_title("XUAT")
                                        .add_filter(filter.0, &filter.1)
                                        .set_file_name(&name)
                                        .save_file()
                                        .await?;
                                    Some(crate::sys::handle_path(&handle).to_string_lossy().into_owned())
                                },
                                |path| match path {
                                    Some(path) => Message::Command(format!("_XUATFILE {path}")),
                                    None => Message::Noop,
                                },
                            ));
                        }
                        let c: Box<dyn CadCommand> =
                            Box::new(FreeTextValuePromptCommand::new("_XUATFILE", "XUAT  File (.pdf/.jpg/.png):"));
                        self.command_line.push_info(&c.prompt());
                        self.tabs[i].active_cmd = Some(c);
                        return Some(Task::none());
                    }
                    "FORMAT" => o.fmt = cycle(&["pdf", "jpg", "png"], &o.fmt),
                    "PAPER" => o.paper = cycle(&PAPERS.iter().map(|p| p.0).collect::<Vec<_>>(), &o.paper),
                    "ORIENT" => o.orient = cycle(&["auto", "portrait", "landscape"], &o.orient),
                    "COLOR" => o.monochrome = !o.monochrome,
                    "SAT" => o.satellite = !o.satellite,
                    "SCALE" => match value.parse::<u32>() {
                        Ok(n) => o.scale = n,
                        Err(_) => self.command_line.push_error(&format!("XUAT: tỉ lệ không hợp lệ: {value}")),
                    },
                    "DPI" => match value.trim_end_matches(|c: char| c.is_alphabetic()).trim().parse::<u32>() {
                        Ok(n) => o.dpi = n.clamp(50, 1200),
                        Err(_) => self.command_line.push_error(&format!("XUAT: dpi không hợp lệ: {value}")),
                    },
                    "PAPERSET" => {
                        if PAPERS.iter().any(|p| p.0.eq_ignore_ascii_case(value)) {
                            o.paper = value.to_uppercase();
                        }
                    }
                    "FORMATSET" => {
                        let v = value.to_lowercase();
                        if ["pdf", "jpg", "png"].contains(&v.as_str()) {
                            o.fmt = v;
                        }
                    }
                    _ => {}
                }
                self.xuat_store(&o);
                Some(self.xuat_show_menu(i))
            }
            "XUATFILE" if !rest.is_empty() => {
                // A typed name's extension picks the format.
                let low = rest.to_lowercase();
                if let Some(f) = [("pdf", "pdf"), ("jpg", "jpg"), ("jpeg", "jpg"), ("png", "png")]
                    .iter()
                    .find(|(e, _)| low.ends_with(&format!(".{e}")))
                    .map(|(_, f)| f.to_string())
                {
                    let mut o = self.xuat_options();
                    if o.fmt != f {
                        o.fmt = f;
                        self.xuat_store(&o);
                    }
                }
                Some(self.xuat_save(i, rest))
            }
            "XUATDONE" => {
                if let Some(e) = rest.strip_prefix('E') {
                    self.command_line.push_error(&format!("XUAT: không xuất được: {e}"));
                    return Some(Task::none());
                }
                let open = rest.starts_with('1');
                let body = rest.get(1..).unwrap_or("");
                let (line, path) = body.split_once('\n').unwrap_or((body, ""));
                self.command_line.push_output(line);
                if open && !path.is_empty() {
                    return Some(crate::sys::open_url(&format!("file://{path}"), self.main_window));
                }
                Some(Task::none())
            }
            _ => None,
        }
    }
}

inventory::submit!(crate::command::CommandRegistration { names: &["XUAT", "EXPORTWIN"] });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_fits_or_keeps_the_scale() {
        let o = Options::default();
        // 400 × 200 m, A3 auto → landscape; fits 410 × 287 mm usable → 1:975.6.
        let p = o.page([0.0, 0.0, 400.0, 200.0]);
        assert_eq!(p.paper, [420.0, 297.0]);
        assert!((p.mm_per_unit - 410.0 / 400.0).abs() < 1e-12);
        assert!((p.target[0] - 5.0).abs() < 1e-9);
        assert!((p.scale - 1000.0 / 1.025).abs() < 1e-6);
        let o = Options { scale: 500, orient: "portrait".into(), paper: "A4".into(), ..Options::default() };
        let p = o.page([0.0, 0.0, 50.0, 50.0]);
        assert_eq!(p.paper, [210.0, 297.0]);
        assert!((p.mm_per_unit - 2.0).abs() < 1e-12);
        assert_eq!(p.target, [55.0, 98.5]);
        // An A0 raster keeps under the pixel budget.
        let o = Options { dpi: 600, ..Options::default() };
        assert!(o.raster_dpi([841.0, 1189.0]) < 300);
        assert_eq!(o.raster_dpi([210.0, 297.0]), 600);
        assert_eq!(cycle(&["pdf", "jpg", "png"], "png"), "pdf");
    }

    fn add_box(app: &mut OpenCADStudio) {
        let i = app.active_tab;
        let mut pl = codec::entities::LwPolyline::from_points(
            [[542_100.0, 2_029_700.0], [542_160.0, 2_029_700.0], [542_160.0, 2_029_740.0], [542_100.0, 2_029_740.0]]
                .iter()
                .map(|p| codec::types::Vector2::new(p[0], p[1]))
                .collect(),
        );
        pl.is_closed = true;
        app.tabs[i].scene.add_entity(codec::EntityType::LwPolyline(pl));
    }

    #[test]
    fn a_window_goes_to_pdf_png_and_jpg() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        add_box(&mut app);
        let _ = app.dispatch_command("_XUATBOX 542090,2029690,542170,2029750");
        assert!(app.xuat.is_some());
        let _ = app.dispatch_command("_XUATSET PAPER");
        assert_eq!(app.xuat_options().paper, "A2");
        let _ = app.dispatch_command("_XUATSET SCALE 1000");
        assert_eq!(app.xuat_options().scale, 1000);
        let window = app.xuat.as_ref().unwrap().window;
        for fmt in ["pdf", "png", "jpg"] {
            let o = Options { fmt: fmt.into(), paper: "A4".into(), dpi: 100, ..Options::default() };
            let (input, page, bg) = app.xuat_job(i, window, &o);
            assert!(bg.is_none());
            let path = std::env::temp_dir().join(format!("xuat-test-{}.{fmt}", std::process::id()));
            let line = write_export(input, &o, page, window, None, path.to_str().unwrap()).unwrap();
            assert!(line.contains("A4"), "{line}");
            let bytes = std::fs::read(&path).unwrap();
            match fmt {
                "pdf" => assert!(bytes.starts_with(b"%PDF")),
                "png" => {
                    let img = image::load_from_memory(&bytes).unwrap().to_rgb8();
                    // A4 landscape at 100 dpi, the box's lines in black on white.
                    assert!((img.width() as i32 - 1169).abs() <= 2 && (img.height() as i32 - 827).abs() <= 2, "{}x{}", img.width(), img.height());
                    assert!(img.pixels().any(|p| p[0] < 80));
                    assert!(img.pixels().filter(|p| p[0] > 250).count() > 500_000);
                }
                _ => assert!(bytes.starts_with(&[0xFF, 0xD8])),
            }
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn the_satellite_lies_under_the_lines() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        add_box(&mut app);
        let crs = crate::gis::crs::DrawingCrs::parse_user("Hà Tĩnh").unwrap();
        // Tiles in the disk cache under a source of the test's own.
        let mut source = crate::gis::tiles::source("custom", "http://127.0.0.1:9/{z}/{x}/{y}.png");
        source.key = "xuat_test";
        let window = [542_090.0, 2_029_690.0, 542_170.0, 2_029_750.0];
        let o = Options { fmt: "png".into(), paper: "A4".into(), dpi: 60, satellite: true, ..Options::default() };
        let page = o.page(window);
        let (_, keys) = crate::gis::basemap::export_tiles(&crs, &source, window, o.background_units_per_px(&page), 400).unwrap();
        let mut png = Vec::new();
        image::RgbImage::from_pixel(256, 256, image::Rgb([20, 160, 40]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        // Not one colour, or it reads as "no imagery".
        let mut img = image::load_from_memory(&png).unwrap().to_rgb8();
        img.put_pixel(0, 0, image::Rgb([0, 0, 0]));
        png.clear();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let mut dirs = Vec::new();
        for k in &keys {
            let p = crate::gis::tiles::cache_path(source.key, *k).unwrap();
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, &png).unwrap();
            dirs.push(p);
        }
        let (input, page, _) = app.xuat_job(i, window, &Options { satellite: false, ..o.clone() });
        let path = std::env::temp_dir().join(format!("xuat-sat-{}.png", std::process::id()));
        let line = write_export(input, &o, page, window, Some((crs, source.clone(), 1.0)), path.to_str().unwrap()).unwrap();
        assert!(line.contains(&format!("{} ô ảnh nền", keys.len())), "{line}");
        let out = image::open(&path).unwrap().to_rgb8();
        let green = out.pixels().filter(|p| p[1] > 120 && p[0] < 60 && p[2] < 80).count();
        // The window (80 × 60 m fitted in A4 landscape) is mostly green;
        // the margins outside it stay white; the box's lines are dark.
        assert!(green as f64 > 0.6 * f64::from(out.width() * out.height()), "{green}");
        assert!(out.get_pixel(1, 1)[0] > 240);
        assert!(out.pixels().any(|p| p[0] < 30 && p[1] < 30));
        let _ = std::fs::remove_file(&path);
        if let Some(root) = crate::gis::tiles::cache_path(source.key, (0, 0, 0)).and_then(|p| p.parent()?.parent().map(|d| d.to_path_buf())) {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn the_background_is_laid_under_the_drawing() {
        // Tiles from the disk cache: a synthetic one where the window is.
        let crs = crate::gis::crs::DrawingCrs::parse_user("Hà Tĩnh").unwrap();
        let source = crate::gis::tiles::source("osm", "");
        let window = [542_100.0, 2_029_700.0, 542_160.0, 2_029_740.0];
        let (z, keys) = crate::gis::basemap::export_tiles(&crs, &source, window, 0.5, 400).unwrap();
        assert!(z >= 17 && !keys.is_empty());
        let (z2, few) = crate::gis::basemap::export_tiles(&crs, &source, window, 0.01, 4).unwrap();
        assert!(few.len() <= 4 && z2 <= source.max_zoom);
    }
}
