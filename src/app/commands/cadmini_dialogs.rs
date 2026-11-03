//! CadMini Studio's dialogs, laid out as CadMini's: GPMB (one window for
//! the layers, the four steps and the parcel table), TRICHLUC (the sheet's
//! form), XUAT (the export's form) and GISCRS (province, background).
//! The command-line forms stay for scripts.

use std::collections::HashSet;

use crate::app::commands::xuat_cmd::{Options as XuatOptions, PAPERS};
use crate::app::{Message, ModalKind, OpenCADStudio};
use crate::gpmb as G;
use crate::trichluc::{self as T, Params};
use crate::ui::style::form::{dialog_button, dialog_button_styled_opt};
use crate::ui::window::cadmini::Msg;
use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Element, Fill, Length};

pub(in crate::app) enum Dialog {
    Gpmb(GpmbForm),
    Trichluc(TlForm),
    Xuat(XuatForm),
    Crs(CrsForm),
}

impl Dialog {
    pub(in crate::app) fn title(&self) -> &'static str {
        match self {
            Dialog::Gpmb(_) => "Giải phóng mặt bằng (GPMB)",
            Dialog::Trichluc(_) => "Trích lục thửa đất (TRICHLUC)",
            Dialog::Xuat(_) => "Xuất PDF / ảnh theo vùng chọn (XUAT)",
            Dialog::Crs(_) => "Hệ toạ độ và nền bản đồ (GIS)",
        }
    }
}

pub(in crate::app) struct GpmbRow {
    handle: u64,
    data: G::ParcelData,
    area: f64,
    missing: String,
}

pub(in crate::app) struct GpmbForm {
    layers: Vec<(String, bool)>,
    tol: String,
    min_area: String,
    only_missing: bool,
    rows: Vec<GpmbRow>,
    edited: HashSet<u64>,
    info: String,
    has_result: bool,
}

pub(in crate::app) struct TlForm {
    params: Params,
    info: String,
}

pub(in crate::app) struct XuatForm {
    opts: XuatOptions,
    path: String,
    window: [f64; 4],
    has_crs: bool,
}

pub(in crate::app) struct CrsForm {
    current: String,
    province: Option<String>,
    old_province: Option<String>,
    meridian: String,
    zone6: bool,
    sat: bool,
    source: String,
    opacity: String,
}

// ── Widgets ─────────────────────────────────────────────────────────────────

fn label<'a>(s: &'a str, w: f32) -> Element<'a, Message> {
    text(s).size(12).width(w).into()
}

fn field<'a>(name: &'a str, key: &'static str, value: &str, w: f32) -> Element<'a, Message> {
    row![
        label(name, w),
        text_input("", value)
            .on_input(move |v| Message::CadMini(Msg::Set(key, v)))
            .padding([4, 6])
            .size(12)
            .width(Fill),
    ]
    .spacing(6)
    .align_y(iced::Center)
    .into()
}

fn picker<'a>(name: &'a str, key: &'static str, options: Vec<String>, selected: String, w: f32) -> Element<'a, Message> {
    row![
        label(name, w),
        pick_list(Some(selected), options, |v: &String| v.clone())
            .on_select(move |v| Message::CadMini(Msg::Set(key, v)))
            .padding([3, 6])
            .text_size(12)
            .width(Fill),
    ]
    .spacing(6)
    .align_y(iced::Center)
    .into()
}

fn check<'a>(name: &'a str, key: &'static str, on: bool) -> Element<'a, Message> {
    checkbox(on).label(name).text_size(12).on_toggle(move |v| Message::CadMini(Msg::Check(key, v))).into()
}

fn act<'a>(name: &'a str, key: &'static str, enabled: bool) -> Element<'a, Message> {
    button(text(name).size(12))
        .padding([5, 10])
        .on_press_maybe(enabled.then_some(Message::CadMini(Msg::Act(key))))
        .into()
}

fn group<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    column![text(title).size(13), container(body).padding([4, 8])].spacing(4).into()
}

fn ok_cancel<'a>(ok: &'a str, enabled: bool) -> Element<'a, Message> {
    row![
        Space::new().width(Fill),
        dialog_button("Huỷ", Message::CadMini(Msg::Act("cancel")), false),
        dialog_button_styled_opt(ok, enabled.then_some(Message::CadMini(Msg::Act("ok"))), button::primary),
    ]
    .spacing(8)
    .into()
}

const SCALES: &[&str] = &["Tự động", "100", "200", "250", "500", "1000", "2000", "2500", "5000", "10000"];
const XSCALES: &[&str] = &["Vừa khổ", "200", "500", "1000", "2000", "5000", "10000", "25000"];

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn sources() -> Vec<(String, String)> {
    crate::gis::tiles::sources().into_iter().map(|s| (s.key.to_string(), s.name.to_string())).collect()
}

fn source_name(key: &str) -> String {
    sources().into_iter().find(|(k, _)| k == key).map_or_else(|| key.to_string(), |(_, n)| n)
}

fn source_key(name: &str) -> String {
    sources().into_iter().find(|(_, n)| n == name).map_or_else(|| name.to_string(), |(k, _)| k)
}

// ── Views ───────────────────────────────────────────────────────────────────

pub(in crate::app) fn view<'a>(d: &'a Dialog, sizing: crate::ui::modal::ModalSizing) -> Element<'a, Message> {
    let body = match d {
        Dialog::Gpmb(f) => gpmb_view(f),
        Dialog::Trichluc(f) => tl_view(f),
        Dialog::Xuat(f) => xuat_view(f),
        Dialog::Crs(f) => crs_view(f),
    };
    container(body).padding(12).width(sizing.width).height(sizing.height).into()
}

fn gpmb_view(f: &GpmbForm) -> Element<'_, Message> {
    let mut layers = column![].spacing(2);
    for (k, (name, on)) in f.layers.iter().enumerate() {
        layers = layers.push(
            checkbox(*on)
                .label(name.as_str())
                .text_size(12)
                .on_toggle(move |v| Message::CadMini(Msg::Layer(k, v))),
        );
    }
    let settings = row![
        column![
            text("Layer ranh thửa (bỏ trống = mọi layer):").size(12),
            container(scrollable(layers).height(110)).padding(4).width(260),
        ]
        .spacing(4),
        column![
            field("Khe hở tối đa để đóng thửa (m):", "tol", &f.tol, 210.0),
            field("Bỏ mảnh vụn nhỏ hơn (m²):", "min_area", &f.min_area, 210.0),
        ]
        .spacing(6)
        .width(Fill),
    ]
    .spacing(16);
    let steps = row![
        act("B1. Làm sạch dữ liệu", "b1", true),
        act("B2. Bo thửa", "b2", true),
        act("B3. Tính DT GPMB…", "b3", true),
        act("B4. Trích lục theo đợt", "b4", f.has_result),
    ]
    .spacing(8);
    const W: [f32; 8] = [70.0, 50.0, 190.0, 70.0, 170.0, 70.0, 80.0, 120.0];
    let head = row![
        text("").width(34),
        text("Số thửa").size(12).width(W[0]),
        text("Tờ").size(12).width(W[1]),
        text("Chủ sử dụng").size(12).width(W[2]),
        text("Loại đất").size(12).width(W[3]),
        text("Địa chỉ").size(12).width(W[4]),
        text("DT ghi").size(12).width(W[5]),
        text("DT đo").size(12).width(W[6]),
        text("Thiếu").size(12).width(W[7]),
    ]
    .spacing(4);
    let cell = |h: u64, key: &'static str, v: &str, w: f32| -> Element<'_, Message> {
        text_input("", v)
            .on_input(move |s| Message::CadMini(Msg::Cell(h, key, s)))
            .padding([2, 4])
            .size(12)
            .width(w)
            .into()
    };
    let shown: Vec<&GpmbRow> = f.rows.iter().filter(|r| !f.only_missing || !r.missing.is_empty()).take(400).collect();
    let mut table = column![].spacing(2);
    for r in &shown {
        let h = r.handle;
        let mark = if f.edited.contains(&h) { "✎" } else { "" };
        table = table.push(
            row![
                button(text("⌕").size(12)).padding([1, 8]).on_press(Message::CadMini(Msg::Zoom(h))),
                cell(h, "so_thua", &r.data.so_thua, W[0]),
                cell(h, "to_ban_do", &r.data.to_ban_do, W[1]),
                cell(h, "chu", &r.data.chu, W[2]),
                cell(h, "loai_dat", &r.data.loai_dat, W[3]),
                cell(h, "dia_chi", &r.data.dia_chi, W[4]),
                text(r.data.dt_ghi.map(|v| format!("{v:.1}")).unwrap_or_default()).size(12).width(W[5]),
                text(format!("{:.1}", r.area)).size(12).width(W[6]),
                text(format!("{mark}{}", r.missing)).size(12).width(W[7]),
            ]
            .spacing(4)
            .align_y(iced::Center),
        );
    }
    let count = f.rows.iter().filter(|r| !f.only_missing || !r.missing.is_empty()).count();
    let more = if count > shown.len() { format!(" (hiện {} / {count})", shown.len()) } else { String::new() };
    let tools = row![
        check("Chỉ hiện thửa thiếu thông tin", "only_missing", f.only_missing),
        act("Lưu phần bổ sung", "save", !f.edited.is_empty()),
        act("Xuất Excel…", "xlsx", f.has_result),
        act("Ghi bảng tổng hợp vào bản vẽ…", "table", f.has_result),
        Space::new().width(Fill),
        dialog_button("Đóng", Message::CadMini(Msg::Act("cancel")), false),
    ]
    .spacing(8)
    .align_y(iced::Center);
    column![
        group("Dữ liệu", settings.into()),
        group("Các bước", steps.into()),
        text(format!(
            "Dữ liệu thửa{more} — sửa trực tiếp Số thửa / Tờ / Chủ / Loại đất / Địa chỉ rồi bấm Lưu; ⌕ để xem thửa trên bản vẽ:"
        ))
        .size(12),
        head,
        container(scrollable(table).height(Fill)).height(Fill),
        text(f.info.as_str()).size(12),
        tools,
    ]
    .spacing(8)
    .height(Fill)
    .into()
}

fn tl_view(f: &TlForm) -> Element<'_, Message> {
    let p = &f.params;
    const W: f32 = 130.0;
    let uses: Vec<String> = strings(T::LAND_USES);
    let info = column![
        field("Thửa đất số:", "so_thua", &p.so_thua, W),
        field("Tờ bản đồ số:", "to_ban_do", &p.to_ban_do, W),
        row![
            label("Mục đích sử dụng:", W),
            text_input("", &p.muc_dich)
                .on_input(|v| Message::CadMini(Msg::Set("muc_dich", v)))
                .padding([4, 6])
                .size(12)
                .width(Fill),
            pick_list(None::<String>, uses, |v: &String| v.clone())
                .placeholder("Chọn…")
                .on_select(|v| Message::CadMini(Msg::Set("muc_dich", v)))
                .padding([3, 6])
                .text_size(12)
                .width(110),
        ]
        .spacing(6)
        .align_y(iced::Center),
        field("Người sử dụng đất:", "chu", &p.chu, W),
        field("Địa chỉ thửa đất:", "dia_chi", &p.dia_chi, W),
        field("Ghi chú:", "ghi_chu", &p.ghi_chu, W),
    ]
    .spacing(6);
    let scale = if p.scale > 0 { p.scale.to_string() } else { "Tự động".into() };
    let layout = column![
        field("Tiêu đề:", "tieu_de", &p.tieu_de, W),
        row![
            column![
                picker("Khổ giấy:", "paper", strings(&["A4", "A3"]), p.paper.clone(), W),
                picker(
                    "Hướng giấy:",
                    "orient",
                    strings(&["Đứng", "Ngang"]),
                    if p.landscape { "Ngang" } else { "Đứng" }.into(),
                    W
                ),
                picker("Tỉ lệ 1:", "scale", strings(SCALES), scale, W),
            ]
            .spacing(6)
            .width(Fill),
            column![
                picker(
                    "Đơn vị bản vẽ:",
                    "unit",
                    strings(&["Mét", "Milimét"]),
                    if p.unit == "m" { "Mét" } else { "Milimét" }.into(),
                    100.0
                ),
                picker(
                    "Vị trí đặt:",
                    "placement",
                    strings(&["Bên cạnh bản vẽ", "Tại chỗ (quanh thửa)"]),
                    if p.side() { "Bên cạnh bản vẽ" } else { "Tại chỗ (quanh thửa)" }.into(),
                    100.0
                ),
            ]
            .spacing(6)
            .width(Fill),
        ]
        .spacing(16),
        row![
            column![
                check("Chép thửa liền kề", "neighbors", p.neighbors),
                check("Ghi chiều dài cạnh", "edge_len", p.edge_len),
            ]
            .spacing(4)
            .width(Fill),
            column![
                check("Đánh số đỉnh từ góc Tây Bắc", "renumber", p.renumber),
                check("Dấu phẩy thập phân", "comma", p.comma),
            ]
            .spacing(4)
            .width(Fill),
        ]
        .spacing(16),
    ]
    .spacing(6);
    let sign = row![
        column![
            field("Đơn vị:", "don_vi", &p.don_vi, 90.0),
            field("Địa danh:", "dia_danh", &p.dia_danh, 90.0),
            field("Ngày:", "ngay", &p.ngay, 90.0),
        ]
        .spacing(6)
        .width(Fill),
        column![
            field("Người trích lục:", "nguoi_lap", &p.nguoi_lap, 110.0),
            field("Thủ trưởng:", "thu_truong", &p.thu_truong, 110.0),
            field("Hệ tọa độ:", "he_toa_do", &p.he_toa_do, 110.0),
        ]
        .spacing(6)
        .width(Fill),
    ]
    .spacing(16);
    column![
        text(f.info.as_str()).size(12),
        group("Thông tin thửa", info.into()),
        group("Trình bày", layout.into()),
        group("Ký, xác nhận", sign.into()),
        ok_cancel("Vẽ trích lục", true),
    ]
    .spacing(10)
    .into()
}

fn xuat_view(f: &XuatForm) -> Element<'_, Message> {
    let o = &f.opts;
    const W: f32 = 120.0;
    let page = o.page(f.window);
    let landscape = page.paper[0] > page.paper[1];
    let info = format!(
        "Vùng chọn: {:.2} × {:.2} (đơn vị bản vẽ) → khổ {} {}, tỉ lệ 1:{:.0}",
        f.window[2] - f.window[0],
        f.window[3] - f.window[1],
        o.paper,
        if landscape { "ngang" } else { "đứng" },
        page.scale
    );
    let orient = match o.orient.as_str() {
        "portrait" => "Đứng",
        "landscape" => "Ngang",
        _ => "Tự động theo vùng chọn",
    };
    let scale = if o.scale > 0 { o.scale.to_string() } else { "Vừa khổ".into() };
    let paper = column![
        picker(
            "Định dạng:",
            "fmt",
            strings(&["PDF (vector, nét sắc)", "JPG (ảnh)", "PNG (ảnh, không nén mất)"]),
            match o.fmt.as_str() {
                "jpg" => "JPG (ảnh)",
                "png" => "PNG (ảnh, không nén mất)",
                _ => "PDF (vector, nét sắc)",
            }
            .into(),
            W
        ),
        picker("Khổ giấy:", "paper", PAPERS.iter().map(|p| p.0.to_string()).collect(), o.paper.clone(), W),
        picker("Hướng giấy:", "orient", strings(&["Tự động theo vùng chọn", "Đứng", "Ngang"]), orient.into(), W),
        row![
            label("Tỉ lệ 1:", W),
            text_input("Vừa khổ", &if o.scale > 0 { o.scale.to_string() } else { String::new() })
                .on_input(|v| Message::CadMini(Msg::Set("scale", v)))
                .padding([4, 6])
                .size(12)
                .width(Fill),
            pick_list(Some(scale), strings(XSCALES), |v: &String| v.clone())
                .on_select(|v| Message::CadMini(Msg::Set("scale", v)))
                .padding([3, 6])
                .text_size(12)
                .width(110),
        ]
        .spacing(6)
        .align_y(iced::Center),
        picker(
            "Đơn vị bản vẽ:",
            "unit",
            strings(&["Mét (VN-2000)", "Milimét"]),
            if o.unit == "m" { "Mét (VN-2000)" } else { "Milimét" }.into(),
            W
        ),
        field("Lề (mm):", "margin", &format!("{}", o.margin), W),
        picker(
            "Độ phân giải ảnh:",
            "dpi",
            strings(&["100", "150", "200", "300", "400", "600"]),
            o.dpi.to_string(),
            W
        ),
    ]
    .spacing(6);
    let pen = column![check("Nét đen trắng (monochrome)", "monochrome", o.monochrome)].spacing(6);
    let source_key = if o.sat_source.is_empty() { "esri_imagery".to_string() } else { o.sat_source.clone() };
    let mut bg = column![check(
        if f.has_crs { "Nền ảnh vệ tinh / bản đồ dưới bản vẽ" } else { "Nền ảnh vệ tinh (bản vẽ chưa có hệ toạ độ — GISCRS)" },
        "satellite",
        o.satellite && f.has_crs,
    )]
    .spacing(6);
    if f.has_crs && o.satellite {
        bg = bg
            .push(picker("Nguồn ảnh:", "sat_source", sources().into_iter().map(|(_, n)| n).collect(), source_name(&source_key), W))
            .push(picker(
                "Độ đậm:",
                "sat_opacity",
                strings(&["100%", "75%", "50%", "30%"]),
                format!("{:.0}%", o.sat_opacity * 100.0),
                W,
            ));
    }
    let file = column![
        row![
            label("File:", 40.0),
            text_input("", &f.path)
                .on_input(|v| Message::CadMini(Msg::Set("path", v)))
                .padding([4, 6])
                .size(12)
                .width(Fill),
            act("Chọn…", "browse", true),
        ]
        .spacing(6)
        .align_y(iced::Center),
        check("Mở file sau khi xuất", "open", o.open),
    ]
    .spacing(6);
    column![
        text(info).size(12),
        group("Giấy và tỉ lệ", paper.into()),
        group("Nét in", pen.into()),
        group("Ảnh nền", bg.into()),
        group("Lưu", file.into()),
        ok_cancel("Xuất", !f.path.trim().is_empty()),
    ]
    .spacing(10)
    .width(Length::Fixed(560.0))
    .into()
}

fn crs_view(f: &CrsForm) -> Element<'_, Message> {
    const W: f32 = 150.0;
    let new: Vec<String> = crate::gis::crs::PROVINCES_2025
        .iter()
        .map(|(n, d, m, from)| {
            if from.is_empty() { format!("{n} — {d}°{m:02}′") } else { format!("{n} — {d}°{m:02}′ ({from})") }
        })
        .collect();
    let old: Vec<String> =
        crate::gis::crs::PROVINCES_OLD.iter().map(|(n, d, m)| format!("{n} — {d}°{m:02}′")).collect();
    let system = column![
        text(format!("Hiện tại: {}", f.current)).size(12),
        row![
            label("Tỉnh (34 tỉnh, 2025):", W),
            pick_list(f.province.clone(), new, |v: &String| v.clone())
                .placeholder("Chọn tỉnh…")
                .on_select(|v| Message::CadMini(Msg::Set("province", v)))
                .padding([3, 6])
                .text_size(12)
                .width(Fill),
        ]
        .spacing(6)
        .align_y(iced::Center),
        row![
            label("Tỉnh cũ (63 tỉnh):", W),
            pick_list(f.old_province.clone(), old, |v: &String| v.clone())
                .placeholder("Chọn tỉnh…")
                .on_select(|v| Message::CadMini(Msg::Set("old_province", v)))
                .padding([3, 6])
                .text_size(12)
                .width(Fill),
        ]
        .spacing(6)
        .align_y(iced::Center),
        row![
            label("Hoặc kinh tuyến trục:", W),
            text_input("105.5 hoặc 105 30", &f.meridian)
                .on_input(|v| Message::CadMini(Msg::Set("meridian", v)))
                .padding([4, 6])
                .size(12)
                .width(Fill),
            check("Múi 6°", "zone6", f.zone6),
        ]
        .spacing(6)
        .align_y(iced::Center),
    ]
    .spacing(6);
    let bg = column![
        check("Hiện nền ảnh vệ tinh / bản đồ", "sat", f.sat),
        picker("Nguồn ảnh:", "source", sources().into_iter().map(|(_, n)| n).collect(), source_name(&f.source), W),
        picker("Độ đậm:", "opacity", strings(&["100%", "75%", "50%", "30%"]), f.opacity.clone(), W),
    ]
    .spacing(6);
    column![
        group("Hệ toạ độ VN-2000", system.into()),
        group("Nền bản đồ", bg.into()),
        ok_cancel("Áp dụng", true),
    ]
    .spacing(10)
    .width(Length::Fixed(560.0))
    .into()
}

// ── Behaviour ───────────────────────────────────────────────────────────────

fn missing_text(p: &G::Parcel) -> String {
    p.missing.join(", ")
}

impl OpenCADStudio {
    fn cm_open(&mut self, d: Dialog) {
        self.cadmini_dialog = Some(d);
        self.active_modal = Some(ModalKind::CadMini);
    }

    fn cm_close(&mut self) {
        self.cadmini_dialog = None;
        if self.active_modal == Some(ModalKind::CadMini) {
            self.close_active_modal();
        }
    }

    // GPMB

    fn gpmb_form(&mut self, i: usize) -> GpmbForm {
        self.gpmb_default_layers(i);
        let st = self.gpmb_state(i);
        let chosen: Vec<String> = st.layers.iter().map(|l| l.to_lowercase()).collect();
        let (tol, min_area) = (st.tol, st.min_area);
        // The layer table, and the layers objects name without it (some
        // DXF writers leave the table out).
        let mut names: Vec<String> = self.tabs[i].scene.document.layers.iter().map(|l| l.name.clone()).collect();
        for e in super::gpmb_cmd::model_entities(&self.tabs[i].scene) {
            let l = &e.common().layer;
            if !names.iter().any(|n| n.eq_ignore_ascii_case(l)) {
                names.push(l.clone());
            }
        }
        names.sort_by_key(|n| n.to_lowercase());
        let layers = names
            .into_iter()
            .filter(|n| !n.to_lowercase().starts_with("gpmb_") && !n.to_lowercase().starts_with("tl_"))
            .map(|n| {
                let on = chosen.contains(&n.to_lowercase());
                (n, on)
            })
            .collect();
        let mut f = GpmbForm {
            layers,
            tol: format!("{tol}"),
            min_area: format!("{min_area}"),
            only_missing: false,
            rows: Vec::new(),
            edited: HashSet::new(),
            info: String::new(),
            has_result: false,
        };
        self.gpmb_refresh(i, &mut f);
        f
    }

    fn gpmb_refresh(&mut self, i: usize, f: &mut GpmbForm) {
        let st = self.gpmb_state(i);
        f.rows = st
            .parcels
            .iter()
            .map(|p| GpmbRow { handle: p.handle, data: p.data.clone(), area: p.area, missing: missing_text(p) })
            .collect();
        f.edited.clear();
        f.has_result = !st.rows.is_empty();
        let missing = st.parcels.iter().filter(|p| !p.missing.is_empty()).count();
        f.info = if st.parcels.is_empty() {
            "Chưa đọc dữ liệu: chọn layer thửa rồi bấm B1.".into()
        } else {
            format!("{} thửa, {missing} thửa thiếu thông tin.", st.parcels.len())
        };
        if !st.rows.is_empty() {
            let s = G::summary(&st.rows, &st.out_parts, st.total);
            f.info.push_str(&format!(
                "  B3: {} thửa bị thu hồi, trong thửa {:.1} m², UBND quản lý {:.1} m², tổng ranh {:.1} m².",
                s.so_thua, s.thu_hoi_trong_thua, s.ngoai_thua, s.tong_ranh
            ));
        }
    }

    /// The dialog's layers and limits into GPMB's state.
    fn gpmb_apply_form(&mut self, i: usize) {
        let Some(Dialog::Gpmb(f)) = &self.cadmini_dialog else { return };
        let layers: Vec<String> = f.layers.iter().filter(|(_, on)| *on).map(|(n, _)| n.clone()).collect();
        let tol = f.tol.trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v >= 0.0);
        let min_area = f.min_area.trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v >= 0.0);
        let st = self.gpmb_state(i);
        if st.layers != layers {
            st.parcels.clear();
        }
        st.layers = layers;
        if let Some(t) = tol {
            st.tol = t;
        }
        if let Some(m) = min_area {
            st.min_area = m;
        }
    }

    pub(in crate::app) fn open_gpmb_dialog(&mut self, i: usize) {
        let f = self.gpmb_form(i);
        self.cm_open(Dialog::Gpmb(f));
    }

    fn gpmb_save_edits(&mut self, i: usize) {
        let Some(Dialog::Gpmb(f)) = &self.cadmini_dialog else { return };
        let edits: Vec<(u64, G::ParcelData)> =
            f.rows.iter().filter(|r| f.edited.contains(&r.handle)).map(|r| (r.handle, r.data.clone())).collect();
        if edits.is_empty() {
            return;
        }
        self.push_undo_snapshot(i, "GPMBEDIT");
        for (h, d) in &edits {
            if let Some(mut e) = self.tabs[i].scene.document.get_entity(codec::Handle::new(*h)).cloned() {
                super::gpmb_cmd::set_saved_data(&mut e, d);
                self.tabs[i].scene.update_entity(e);
            }
            if let Some(p) = self.gpmb_state(i).parcels.iter_mut().find(|p| p.handle == *h) {
                p.data = d.clone();
                p.check();
            }
        }
        self.tabs[i].dirty = true;
        self.command_line.push_output(&format!("GPMB: đã lưu thông tin {} thửa (trong bản vẽ).", edits.len()));
    }

    // TRICHLUC

    /// The sheet's form for the pending TRICHLUC.
    pub(in crate::app) fn open_trichluc_dialog(&mut self, i: usize) {
        let Some(pd) = &self.trichluc else { return };
        let ring = self.tabs[i]
            .scene
            .document
            .get_entity(codec::Handle::new(pd.src))
            .and_then(|e| match e {
                codec::EntityType::LwPolyline(pl) => {
                    Some(pl.vertices.iter().map(|v| [v.location.x, v.location.y, v.bulge]).collect::<Vec<_>>())
                }
                _ => None,
            })
            .unwrap_or_default();
        let area = T::ring_area(&ring).abs();
        let (mut xmin, mut ymin, mut xmax, mut ymax) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in &ring {
            xmin = xmin.min(q[0]);
            xmax = xmax.max(q[0]);
            ymin = ymin.min(q[1]);
            ymax = ymax.max(q[1]);
        }
        let info = format!(
            "{} — diện tích {} m², {} đỉnh, kích thước {:.1} × {:.1} m",
            if pd.editing { "Sửa bản trích lục" } else { "Bản trích lục mới" },
            T::fmt(area, 1, pd.params.comma),
            ring.len(),
            xmax - xmin,
            ymax - ymin
        );
        let f = TlForm { params: pd.params.clone(), info };
        self.cm_open(Dialog::Trichluc(f));
    }

    // XUAT

    pub(in crate::app) fn open_xuat_dialog(&mut self, i: usize) {
        let Some(pd) = &self.xuat else { return };
        let window = pd.window;
        let opts = self.xuat_options();
        let has_crs = super::gis_cmd::drawing_crs(&self.tabs[i].scene.document).is_some();
        let (dir, stem) = match &self.tabs[i].current_path {
            Some(p) => (
                std::fs::canonicalize(p)
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .or_else(|| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_default(),
                p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "BanVe".into()),
            ),
            None => (
                std::env::var_os("USERPROFILE")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(std::path::PathBuf::from)
                    .unwrap_or_default(),
                "BanVe".into(),
            ),
        };
        let path = dir.join(format!("{stem}_xuat.{}", opts.fmt)).to_string_lossy().into_owned();
        self.cm_open(Dialog::Xuat(XuatForm { opts, path, window, has_crs }));
    }

    // GISCRS

    pub(in crate::app) fn open_crs_dialog(&mut self, i: usize) {
        let current = super::gis_cmd::drawing_crs(&self.tabs[i].scene.document)
            .map(|c| c.label())
            .unwrap_or_else(|| "chưa có".into());
        let f = CrsForm {
            current,
            province: None,
            old_province: None,
            meridian: String::new(),
            zone6: false,
            sat: self.basemap.on,
            source: self.basemap.source_key.clone(),
            opacity: format!("{:.0}%", self.basemap.opacity * 100.0),
        };
        self.cm_open(Dialog::Crs(f));
    }

    // Messages

    pub(in crate::app) fn on_cadmini_msg(&mut self, msg: Msg) -> iced::Task<Message> {
        let i = self.active_tab;
        let Some(d) = self.cadmini_dialog.as_mut() else { return iced::Task::none() };
        match (d, msg) {
            (_, Msg::Act("cancel")) => {
                self.trichluc = None;
                self.xuat = None;
                self.cm_close();
            }
            // GPMB
            (Dialog::Gpmb(f), Msg::Layer(k, on)) => {
                if let Some(l) = f.layers.get_mut(k) {
                    l.1 = on;
                }
            }
            (Dialog::Gpmb(f), Msg::Set("tol", v)) => f.tol = v,
            (Dialog::Gpmb(f), Msg::Set("min_area", v)) => f.min_area = v,
            (Dialog::Gpmb(f), Msg::Check("only_missing", v)) => f.only_missing = v,
            (Dialog::Gpmb(f), Msg::Cell(h, key, v)) => {
                if let Some(r) = f.rows.iter_mut().find(|r| r.handle == h) {
                    match key {
                        "so_thua" => r.data.so_thua = v,
                        "to_ban_do" => r.data.to_ban_do = v,
                        "chu" => r.data.chu = v,
                        "loai_dat" => r.data.loai_dat = v,
                        _ => r.data.dia_chi = v,
                    }
                    f.edited.insert(h);
                }
            }
            (Dialog::Gpmb(_), Msg::Zoom(h)) => {
                let ring = self.tabs[i]
                    .scene
                    .document
                    .get_entity(codec::Handle::new(h))
                    .and_then(super::gpmb_cmd::closed_ring);
                if let Some(ring) = ring {
                    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                    for q in &ring {
                        x0 = x0.min(q[0]);
                        x1 = x1.max(q[0]);
                        y0 = y0.min(q[1]);
                        y1 = y1.max(q[1]);
                    }
                    let m = (x1 - x0).max(y1 - y0) * 0.8;
                    self.handle_zoom_to_window(
                        glam::DVec3::new(x0 - m, y0 - m, 0.0),
                        glam::DVec3::new(x1 + m, y1 + m, 0.0),
                    );
                    self.tabs[i].scene.deselect_all();
                    self.tabs[i].scene.select_entity(codec::Handle::new(h), false);
                }
            }
            (Dialog::Gpmb(_), Msg::Act(a)) => {
                self.gpmb_apply_form(i);
                match a {
                    "b1" | "b2" => {
                        self.gpmb_save_edits(i);
                        let t = if a == "b1" { self.gpmb_step1(i, true) } else { self.gpmb_step2(i) };
                        let mut f = self.gpmb_form(i);
                        if let Some(Dialog::Gpmb(old)) = &self.cadmini_dialog {
                            f.only_missing = old.only_missing;
                        }
                        self.cadmini_dialog = Some(Dialog::Gpmb(f));
                        return t;
                    }
                    "save" => {
                        self.gpmb_save_edits(i);
                        let mut f = self.gpmb_form(i);
                        if let Some(Dialog::Gpmb(old)) = &self.cadmini_dialog {
                            f.only_missing = old.only_missing;
                        }
                        self.cadmini_dialog = Some(Dialog::Gpmb(f));
                    }
                    "b3" | "b4" | "table" | "xlsx" => {
                        self.gpmb_save_edits(i);
                        self.cm_close();
                        if a == "b3" {
                            self.gpmb_state(i).reopen = true;
                        }
                        let cmd = match a {
                            "b3" => "GPMB3",
                            "b4" => "GPMB4",
                            "table" => "GPMBTABLE",
                            _ => "GPMBXLSX",
                        };
                        return self.dispatch_command(cmd);
                    }
                    _ => {}
                }
            }
            // TRICHLUC
            (Dialog::Trichluc(f), Msg::Set(key, v)) => {
                let p = &mut f.params;
                match key {
                    "orient" => p.landscape = v == "Ngang",
                    "scale" => p.scale = v.trim().parse().unwrap_or(0),
                    "unit" => p.unit = if v == "Mét" { "m".into() } else { "mm".into() },
                    "placement" => p.placement = if v.starts_with("Bên") { "side".into() } else { "here".into() },
                    _ => {
                        p.set(key, &v);
                    }
                }
                if matches!(key, "orient" | "paper" | "placement") {
                    p.base = None;
                }
            }
            (Dialog::Trichluc(f), Msg::Check(key, v)) => {
                f.params.set(key, if v { "true" } else { "false" });
            }
            (Dialog::Trichluc(f), Msg::Act("ok")) => {
                let params = f.params.clone();
                if let Some(pd) = self.trichluc.as_mut() {
                    pd.params = params;
                }
                self.cadmini_dialog = None;
                self.close_active_modal();
                return self.tl_draw(i);
            }
            // XUAT
            (Dialog::Xuat(f), Msg::Set(key, v)) => {
                let o = &mut f.opts;
                match key {
                    "fmt" => {
                        o.fmt = if v.starts_with("JPG") {
                            "jpg"
                        } else if v.starts_with("PNG") {
                            "png"
                        } else {
                            "pdf"
                        }
                        .into();
                        // The file's extension follows.
                        let p = std::path::Path::new(&f.path);
                        if p.extension().is_some() {
                            f.path = p.with_extension(&o.fmt).to_string_lossy().into_owned();
                        }
                    }
                    "paper" => o.paper = v,
                    "orient" => {
                        o.orient = match v.as_str() {
                            "Đứng" => "portrait",
                            "Ngang" => "landscape",
                            _ => "auto",
                        }
                        .into()
                    }
                    "scale" => o.scale = v.trim().trim_start_matches("1:").parse().unwrap_or(0),
                    "unit" => o.unit = if v.starts_with("Mét") { "m".into() } else { "mm".into() },
                    "margin" => {
                        if let Ok(m) = v.trim().replace(',', ".").parse::<f64>() {
                            o.margin = m.clamp(0.0, 50.0);
                        }
                    }
                    "dpi" => o.dpi = v.parse().unwrap_or(200),
                    "sat_source" => o.sat_source = source_key(&v),
                    "sat_opacity" => {
                        o.sat_opacity = v.trim_end_matches('%').parse::<f32>().map_or(1.0, |p| p / 100.0)
                    }
                    "path" => f.path = v,
                    _ => {}
                }
            }
            (Dialog::Xuat(f), Msg::Check(key, v)) => match key {
                "monochrome" => f.opts.monochrome = v,
                "satellite" => f.opts.satellite = v,
                "open" => f.opts.open = v,
                _ => {}
            },
            (Dialog::Xuat(f), Msg::Act("browse")) => {
                if !crate::sys::file_dia() {
                    return iced::Task::none();
                }
                let (ext, filter) = match f.opts.fmt.as_str() {
                    "jpg" => ("jpg", "JPG (*.jpg)"),
                    "png" => ("png", "PNG (*.png)"),
                    _ => ("pdf", "PDF (*.pdf)"),
                };
                let name = std::path::Path::new(&f.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("BanVe_xuat.{ext}"));
                return iced::Task::perform(
                    async move {
                        let handle = crate::sys::file_dialog()
                            .set_title("XUAT")
                            .add_filter(filter, &[ext])
                            .set_file_name(&name)
                            .save_file()
                            .await?;
                        Some(crate::sys::handle_path(&handle).to_string_lossy().into_owned())
                    },
                    |path| match path {
                        Some(p) => Message::CadMini(Msg::Set("path", p)),
                        None => Message::Noop,
                    },
                );
            }
            (Dialog::Xuat(f), Msg::Act("ok")) => {
                let (opts, path) = (f.opts.clone(), f.path.trim().to_string());
                if path.is_empty() {
                    return iced::Task::none();
                }
                self.xuat_store(&opts);
                self.cadmini_dialog = None;
                self.close_active_modal();
                return self.xuat_save(i, &path);
            }
            // GISCRS
            (Dialog::Crs(f), Msg::Set(key, v)) => match key {
                "province" => {
                    f.province = Some(v);
                    f.old_province = None;
                    f.meridian.clear();
                }
                "old_province" => {
                    f.old_province = Some(v);
                    f.province = None;
                    f.meridian.clear();
                }
                "meridian" => {
                    f.meridian = v;
                    f.province = None;
                    f.old_province = None;
                }
                "source" => f.source = source_key(&v),
                "opacity" => f.opacity = v,
                _ => {}
            },
            (Dialog::Crs(f), Msg::Check(key, v)) => match key {
                "zone6" => f.zone6 = v,
                "sat" => f.sat = v,
                _ => {}
            },
            (Dialog::Crs(f), Msg::Act("ok")) => {
                use crate::gis::crs::{DrawingCrs, PROVINCES_2025, PROVINCES_OLD};
                let name_of = |s: &str| s.split(" — ").next().unwrap_or(s).to_string();
                let (sat, source, opacity) = (f.sat, f.source.clone(), f.opacity.clone());
                let system = if let Some(p) = &f.province {
                    let n = name_of(p);
                    PROVINCES_2025.iter().find(|(pn, ..)| *pn == n).map(|(pn, d, m, _)| DrawingCrs {
                        lon0: f64::from(*d) + f64::from(*m) / 60.0,
                        zone: 3,
                        name: pn.to_string(),
                    })
                } else if let Some(p) = &f.old_province {
                    let n = name_of(p);
                    PROVINCES_OLD.iter().find(|(pn, ..)| *pn == n).map(|(pn, d, m)| DrawingCrs {
                        lon0: f64::from(*d) + f64::from(*m) / 60.0,
                        zone: 3,
                        name: format!("{pn} (cũ)"),
                    })
                } else if !f.meridian.trim().is_empty() {
                    let t = if f.zone6 { format!("{}/6", f.meridian.trim()) } else { f.meridian.trim().to_string() };
                    match DrawingCrs::parse_user(&t) {
                        Some(s) => Some(s),
                        None => {
                            self.command_line.push_error(&format!("GISCRS: \"{t}\" không phải kinh tuyến trục hợp lệ."));
                            return iced::Task::none();
                        }
                    }
                } else {
                    None
                };
                self.cadmini_dialog = None;
                self.close_active_modal();
                let mut tasks = Vec::new();
                if let Some(s) = system {
                    tasks.push(self.gis_store_crs(i, s));
                }
                if source != self.basemap.source_key {
                    tasks.push(self.dispatch_command(&format!("GISSOURCE {source}")));
                }
                let pct = opacity.trim_end_matches('%');
                if (self.basemap.opacity * 100.0 - pct.parse::<f32>().unwrap_or(100.0)).abs() > 0.5 {
                    tasks.push(self.dispatch_command(&format!("GISOPACITY {pct}")));
                }
                if sat != self.basemap.on {
                    tasks.push(self.dispatch_command(if sat { "GISSAT ON" } else { "GISSAT OFF" }));
                }
                return iced::Task::batch(tasks);
            }
            _ => {}
        }
        iced::Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::EntityType;

    fn add_square(app: &mut OpenCADStudio, x: f64, w: f64, layer: &str) -> codec::Handle {
        let i = app.active_tab;
        let mut pl = codec::entities::LwPolyline::from_points(
            [[x, 0.0], [x + w, 0.0], [x + w, w], [x, w]].iter().map(|p| codec::types::Vector2::new(p[0], p[1])).collect(),
        );
        pl.is_closed = true;
        pl.common.layer = layer.into();
        app.tabs[i].scene.add_entity(EntityType::LwPolyline(pl))
    }

    fn msg(app: &mut OpenCADStudio, m: Msg) {
        let _ = app.on_cadmini_msg(m);
    }

    #[test]
    fn gpmb_runs_from_its_dialog_and_keeps_edits() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        app.tabs[i].scene.ensure_layer("THUA");
        let h = add_square(&mut app, 0.0, 10.0, "THUA");
        add_square(&mut app, 10.0, 10.0, "THUA");
        let _ = app.dispatch_command("GPMB");
        assert_eq!(app.active_modal, Some(ModalKind::CadMini));
        let Some(Dialog::Gpmb(f)) = &app.cadmini_dialog else { panic!() };
        // The parcel layer is guessed and ticked.
        assert!(f.layers.iter().any(|(n, on)| n == "THUA" && *on));
        msg(&mut app, Msg::Act("b1"));
        let Some(Dialog::Gpmb(f)) = &app.cadmini_dialog else { panic!("closed after B1") };
        assert_eq!(f.rows.len(), 2);
        msg(&mut app, Msg::Cell(h.value(), "so_thua", "15".into()));
        msg(&mut app, Msg::Cell(h.value(), "chu", "Lê Văn C".into()));
        msg(&mut app, Msg::Act("save"));
        let e = app.tabs[i].scene.document.get_entity(h).unwrap().clone();
        let d = super::super::gpmb_cmd::saved_data(&e).unwrap();
        assert_eq!((d.so_thua.as_str(), d.chu.as_str()), ("15", "Lê Văn C"));
        let Some(Dialog::Gpmb(f)) = &app.cadmini_dialog else { panic!() };
        assert!(f.edited.is_empty() && f.rows.iter().any(|r| r.data.so_thua == "15"));
        msg(&mut app, Msg::Act("cancel"));
        assert!(app.cadmini_dialog.is_none() && app.active_modal.is_none());
    }

    #[test]
    fn trichluc_draws_from_its_form() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let h = add_square(&mut app, 0.0, 20.0, "0");
        let _ = app.dispatch_command(&format!("_TLRUN {:X}", h.value()));
        assert!(matches!(app.cadmini_dialog, Some(Dialog::Trichluc(_))));
        msg(&mut app, Msg::Set("so_thua", "8".into()));
        msg(&mut app, Msg::Set("orient", "Ngang".into()));
        msg(&mut app, Msg::Set("scale", "500".into()));
        msg(&mut app, Msg::Check("edge_len", false));
        msg(&mut app, Msg::Act("ok"));
        assert!(app.cadmini_dialog.is_none() && app.trichluc.is_none());
        let i = app.active_tab;
        let src = app.tabs[i].scene.document.get_entity(h).unwrap().clone();
        let rec = src.common().extended_data.get_record(T::APPID).unwrap();
        let strings: Vec<String> = rec
            .values
            .iter()
            .filter_map(|v| match v {
                codec::xdata::XDataValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        let Some(T::Tag::Source(_, Some(p))) = T::tag_from_xdata(&strings) else { panic!() };
        assert!(p.so_thua == "8" && p.landscape && p.scale == 500 && !p.edge_len);
    }

    #[test]
    fn crs_dialog_sets_a_province() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let i = app.active_tab;
        let _ = app.dispatch_command("GISCRS");
        assert!(matches!(app.cadmini_dialog, Some(Dialog::Crs(_))));
        msg(&mut app, Msg::Set("old_province", "Bắc Ninh — 105°30′".into()));
        msg(&mut app, Msg::Act("ok"));
        let c = super::super::gis_cmd::drawing_crs(&app.tabs[i].scene.document).unwrap();
        assert!((c.lon0 - 105.5).abs() < 1e-12, "{}", c.lon0);
        let _ = app.dispatch_command("GISCRS");
        msg(&mut app, Msg::Set("province", "Hà Tĩnh — 105°30′".into()));
        msg(&mut app, Msg::Act("ok"));
        let c = super::super::gis_cmd::drawing_crs(&app.tabs[i].scene.document).unwrap();
        assert_eq!(c.name, "Hà Tĩnh");
    }

    #[test]
    fn xuat_form_follows_the_format() {
        let mut app = OpenCADStudio::new_for_test();
        let _ = app.automation_op(r#"{"op":"new"}"#);
        let _ = app.dispatch_command("_XUATBOX 0,0,100,50");
        let Some(Dialog::Xuat(f)) = &app.cadmini_dialog else { panic!() };
        assert!(f.path.ends_with(".pdf") || f.path.ends_with(&format!(".{}", f.opts.fmt)));
        msg(&mut app, Msg::Set("fmt", "PNG (ảnh, không nén mất)".into()));
        msg(&mut app, Msg::Set("scale", "1:2000".into()));
        msg(&mut app, Msg::Set("paper", "A1".into()));
        let Some(Dialog::Xuat(f)) = &app.cadmini_dialog else { panic!() };
        assert!(f.path.ends_with(".png"));
        assert_eq!((f.opts.fmt.as_str(), f.opts.scale, f.opts.paper.as_str()), ("png", 2000, "A1"));
    }
}
