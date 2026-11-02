# CadMini Studio

CadMini Studio là bản phát triển từ [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio)
(giấy phép GPL-3.0), điều chỉnh cho người dùng Việt Nam theo hướng của CadMini. Mã nguồn vẫn
theo GPL-3.0; xem `LICENSE`.

## Khác với Open CAD Studio

### 1. Hai không gian làm việc tách biệt: 2D và 3D

| | 2D (mặc định) | 3D |
|---|---|---|
| Menu | File, Edit, View, …, Modify, **GIS**, Parametric | File, Edit, View, …, Modify, **Modeling**, Parametric |
| Ribbon (nếu bật) | không có tab Model | không có tab GIS |
| Khung nhìn | luôn nhìn bằng (Top), kiểu hiển thị 2D Wireframe | nhìn trục đo (SW Isometric) khi chuyển sang |
| ViewCube | ẩn | hiện (nếu bật trong Options) |
| Chuột giữa + Shift | kéo màn hình (không xoay) | xoay (orbit) |
| Lệnh 3D (BOX, EXTRUDE, UNION, 3DORBIT…) | bị chặn, báo chuyển sang 3D | dùng được |

Chuyển không gian: **Tools › Workspaces › 2D Drafting / 3D Modeling**, hoặc gõ
`WS2D`, `WS3D`, `WSCURRENT 2D|3D` (`WSCURRENT` không tham số: đổi qua lại).
Lựa chọn được nhớ cho lần mở sau.

### 2. Menu kiểu truyền thống

Mặc định hiện thanh menu kéo xuống (MENUBAR 1) và đóng ribbon (RIBBONCLOSE) để dành diện
tích cho bản vẽ. Gõ `RIBBON` để mở lại ribbon, `RIBBONCLOSE` để đóng.

### 3. GIS (trong không gian 2D)

Hệ toạ độ VN-2000 (lưới chiếu UTM múi 3°/6°, 7 tham số chuyển sang WGS84 theo QĐ 05/2007/QĐ-BTNMT,
kinh tuyến trục 34 tỉnh sau sáp nhập theo TT 24/2025/TT-BNNMT và 63 tỉnh cũ). Hệ toạ độ được lưu
trong thuộc tính tuỳ chỉnh `CADMINI_CRS` của bản vẽ — giống CadMini, nên hai phần mềm đọc được
bản vẽ của nhau.

| Lệnh | Menu GIS | Việc làm |
|---|---|---|
| `GISCRS` | Coordinate System… | Đặt hệ toạ độ: tên tỉnh (`Hà Tĩnh`, `ha tinh`, `Đà Nẵng`), kinh tuyến trục (`105.5`, `105 30`), múi 6° (`105/6`); `?` liệt kê tỉnh |
| `GISLL` | Point Coordinates (Lat/Long) | Chọn điểm → X, Y VN-2000 và vĩ độ, kinh độ WGS84 (thập phân và độ-phút-giây) |
| `GISGOTO` | Go to Lat/Long | Nhập `vĩ độ, kinh độ` → phóng tới vị trí đó |
| `GISIMPORT` | Import GIS Data… | Nhập GeoJSON, KML/KMZ, Shapefile. Toạ độ WGS84 được đổi sang VN-2000; Shapefile có `.prj` dạng chiếu (hoặc số lớn) giữ nguyên toạ độ. Mỗi lớp/thư mục KML → một layer; thuộc tính `name` → chữ ghi chú |
| `GISEXPORT` | Export GIS Data… | Xuất đối tượng đang chọn (hoặc cả Model) ra GeoJSON/KML/KMZ (WGS84) hoặc Shapefile VN-2000 (`_point`, `_line`, `_polygon` kèm `.prj`, `.cpg` UTF-8) |

Đối tượng xuất được: Point, Line, LWPolyline (đóng → vùng, cung được chia nhỏ), Circle,
Text/MText (điểm, thuộc tính `name`), Block (điểm chèn, thuộc tính `block`).

Độ chính xác: so với PROJ (pyproj) cùng tham số, sai khác dưới 1 mm.

### 4. Hiển thị mờ và hatch (từ CadMini)

| Lệnh | Menu | Việc làm |
|---|---|---|
| `HATCHFADE [0–90]` | View › Fade › Hatch Fade … | Vẽ hatch mờ đi (đặc, mẫu, gradient) để nổi nét vẽ. Gõ không kèm số: hỏi %, Enter đổi tắt ↔ 50 % |
| `LINEFADE [0–90]` | View › Fade › Line Fade … | Vẽ nét, cung, chữ mờ đi để nổi hatch/ảnh nền |
| `TPY` (`TRANSPARENCYDISPLAY`) | View › Fade › Transparency Display | Bật/tắt hiển thị độ trong suốt của layer/đối tượng |
| `HATCHB` (`HATCHGENERATEBOUNDARY`) | Modify › Object › Hatch Boundary to Polyline | Tạo polyline kín theo từng vòng biên của các hatch đã chọn, trên layer hiện hành; hatch giữ nguyên; hoàn tác một bước |

Mức mờ được nhớ cho lần mở sau; chỉ là cách hiển thị trên màn hình, không đổi dữ liệu bản vẽ.
Lưu ý: lựa chọn `ALL` của Open CAD Studio chưa lấy được hatch tô đặc — hãy chọn hatch bằng cách bấm
hoặc khung chọn.
