# CadMini Studio

Phần mềm CAD 2D có GIS và dựng hình 3D. Mã nguồn theo GPL-3.0 (xem `LICENSE`);
một phần mã nguồn và dữ liệu được lấy từ Open CAD Studio.

## Tính năng

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

Giao diện mặc định: **chế độ sáng**, vùng vẽ **nền tối** (đổi ở Options).

### 2. Hai kiểu giao diện: Cổ điển (menu + toolbar) và Ribbon

Chọn ở **Tools › Workspaces › Classic Menus and Toolbars / Ribbon**, hoặc gõ
`UISTYLE CLASSIC`, `UISTYLE RIBBON` (`UISTYLE` không tham số: đổi qua lại). Lựa chọn được nhớ.

- **Cổ điển** (mặc định, như CadMini / AutoCAD 2007): thanh menu kéo xuống, ribbon đóng. Các toolbar
  Standard, Properties, Dimension, Inquiry, Fade, GIS (2D) nằm cùng hàng với Quick Access; **Draw**
  dọc mép trái bản vẽ (**Modeling** bên cạnh khi ở 3D), **Modify** dọc mép phải.
  - **Kéo thả**: nắm vào vạch `⋮`/`⋯` ở đầu toolbar để kéo ra nổi trên bản vẽ hoặc dời chỗ; thả sát
    mép trên vùng vẽ để gắn lại vào hàng trên.
  - **Co giãn**: kéo góc `◢` của toolbar nổi sang ngang để đổi số cột (1 cột ↔ nhiều cột ↔ 1 hàng).
  - Vị trí được nhớ; **Tools › Workspaces › Reset Toolbars** (`TOOLBARRESET`) trả về như ban đầu.
  - Bảng Properties mặc định thu về dải biểu tượng ở mép trái (rê chuột vào để mở, Ctrl+1 để bật/tắt),
    thanh tab tên bản vẽ thấp — dành chỗ cho bản vẽ.
- **Ribbon**: ribbon; thanh menu vẫn giữ phía trên để luôn đổi lại được.

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

#### Nền bản đồ vệ tinh (như CadMini)

Bản vẽ đã có hệ toạ độ thì **nền vệ tinh tự hiện** dưới bản vẽ (không gian 2D, tab Model). Ảnh được
tải theo mức zoom của màn hình, lưu đệm trên đĩa (thư mục cấu hình `tiles/`), mỗi ô ảnh được nắn
sang VN-2000 (lưới 8×8) nên khớp toạ độ bản vẽ; trong khi ô nét đang tải, ảnh mức thấp hơn hiện tạm.

| Lệnh | Menu GIS | Việc làm |
|---|---|---|
| `GISSAT` (`SAT`) `[ON/OFF]` | Satellite Background On/Off | Bật/tắt nền (không tham số: đổi qua lại); lựa chọn được nhớ |
| `GISSOURCE [số/khoá]` | Background Source › … | Esri vệ tinh, Esri đường phố, OpenStreetMap, Google Hybrid, Google vệ tinh, Google địa hình, OpenTopoMap |
| `GISURL <url>` | Background Source › Custom Tile URL… | Nguồn tuỳ chỉnh dạng `https://…/{z}/{x}/{y}.png` |
| `GISOPACITY <10–100>` | Background Strength › 100/75/50/30 % | Độ đậm ảnh nền |
| `GMAP` | Open Google Maps at Point | Chọn điểm → mở Google Maps tại đó |

Thanh trạng thái hiện kinh độ, vĩ độ tại con trỏ bên cạnh toạ độ X, Y.

Đối tượng xuất được: Point, Line, LWPolyline (đóng → vùng, cung được chia nhỏ), Circle,
Text/MText (điểm, thuộc tính `name`), Block (điểm chèn, thuộc tính `block`).

Độ chính xác: so với PROJ (pyproj) cùng tham số, sai khác dưới 1 mm.

### 4. Hiển thị mờ và hatch (từ CadMini)

| Lệnh | Menu | Việc làm |
|---|---|---|
| `HATCHFADE [0–90]` | View › Fade › Hatch Fade … | Vẽ hatch mờ đi (đặc, mẫu, gradient) để nổi nét vẽ. Gõ không kèm số: hỏi %, Enter đổi tắt ↔ 50 % |
| `LINEFADE [0–90]` | View › Fade › Line Fade … | Vẽ nét, cung, chữ mờ đi để nổi hatch/ảnh nền |
| `TPY` (`TRANSPARENCYDISPLAY`) | View › Fade › Transparency Display | Bật/tắt hiển thị độ trong suốt của layer/đối tượng |
| `HATCHB` (`HATCHGENERATEBOUNDARY`) | Modify › Object › Hatch Boundary to Polyline | Tạo polyline kín theo từng vòng biên của các hatch đã chọn, trên layer hiện hành; đoạn thẳng giữ đỉnh, **cung tròn giữ là cung** (bulge), chỉ elip/spline được chia nhỏ; hatch giữ nguyên; hoàn tác một bước |

Mức mờ được nhớ cho lần mở sau; chỉ là cách hiển thị trên màn hình, không đổi dữ liệu bản vẽ.
Lưu ý: lựa chọn `ALL` chưa lấy được hatch tô đặc — hãy chọn hatch bằng cách bấm
hoặc khung chọn.

### 5. GPMB — giải phóng mặt bằng 4 bước (như CadMini)

Menu **GIS › GPMB (Land Clearance)**, hoặc gõ `GPMB` để hiện các bước thành nút trên dòng lệnh.
Mỗi bước là một bước hoàn tác (Undo). Dữ liệu thửa được lưu trong XDATA `CADMINI_GPMB` của polyline
thửa — giống CadMini, nên hai phần mềm đọc được kết quả của nhau.

| Lệnh | Việc làm |
|---|---|
| `GPMBLAYERS` | Chọn một đối tượng trên mỗi layer thửa (Enter không chọn gì: dùng mọi layer) |
| `GPMB1` (B1 làm sạch) | Mỗi thửa kín (polyline kín, đường tròn) gom chữ / MTEXT / thuộc tính block nằm trong nó: **số thửa** (số gần tâm, chữ to), **chủ sử dụng**, **loại đất** (mã ONT, LUC, CLN…), **diện tích ghi**; thuộc tính block theo tag (`SO_THUA`, `CHU`, `LOAI_DAT`, `DIEN_TICH`, `TO`…). Chữ nằm trong thửa lồng nhau thuộc thửa nhỏ nhất. Thửa thiếu thông tin có dấu **?** (layer `GPMB_THIEU_TT`) |
| `GPMB2` (B2 bo thửa) | Đóng polyline hở có khe ≤ 0,05 m; dựng thửa từ đường rời ở chỗ chưa có thửa kín (layer `GPMB_THUA_BO`); dấu **X** ở đầu mút treo cần xem lại (`GPMB_LOI`); rồi chạy lại B1 và ghi diện tích lên từng thửa (`GPMB_DIEN_TICH`) |
| `GPMB3` (B3 tính) | Chọn ranh GPMB (polyline kín): giao từng thửa với ranh → diện tích **thu hồi / còn lại**, ghi chú *thu hồi toàn bộ / một phần (n phần)*; phần trong ranh không thuộc thửa nào là **đất UBND quản lý**. Vẽ hatch đỏ gạch chéo phần thu hồi (`GPMB_THU_HOI`), hatch xanh phần UBND (`GPMB_NGOAI_THUA`), nhãn `TH số: diện tích` (`GPMB_NHAN`); hatch nằm dưới nét thửa |
| `GPMBEDIT` | Chọn một thửa, nhập lần lượt số thửa, tờ bản đồ, chủ sử dụng, loại đất, địa chỉ, ghi chú (Enter giữ giá trị cũ); lưu trong bản vẽ |
| `GPMBTABLE` | Bảng tổng hợp vẽ vào bản vẽ tại góc chọn (layer `GPMB_BANG`): STT, số thửa, tờ, chủ, loại đất, DT thửa, DT thu hồi, DT còn lại, ghi chú; dòng tổng cộng, đất UBND quản lý, tổng diện tích trong ranh |
| `GPMBXLSX` | Bảng tổng hợp ra file Excel `.xlsx` |
| `GPMB4` (B4 trích lục) | Một bản trích lục (TRICHLUC) cho mỗi thửa bị thu hồi, xếp thành lưới 5 tờ một hàng bên phải bản vẽ; phần thu hồi gạch chéo đỏ kèm nhãn diện tích, ghi chú *diện tích thu hồi / còn lại*; chỉ thửa đang xét (không chép thửa liền kề). Chạy lại: vẽ lại đúng chỗ cũ. `TLPDF` xuất tất cả ra một file PDF |

Mảnh giao nhỏ hơn 0,1 m² hoặc dưới 0,2 % diện tích thửa coi là sai số số hoá (thửa chỉ chạm ranh).

### 6. TRICHLUC — trích lục thửa đất (như CadMini)

Menu **GIS › Parcel Extract Sheet (TRICHLUC)**, hoặc gõ `TRICHLUC` (`TL`). Chọn một polyline khép kín
(đoạn cung giữ là cung), Enter. Dòng lệnh hiện tình trạng tờ trích lục và các nút:

| Nút | Việc làm |
|---|---|
| **Draw** (Enter) | Vẽ bản trích lục |
| **Parcel Info** | Nhập lần lượt số thửa, tờ bản đồ, mục đích sử dụng (gõ mã `ONT`, `LUC`, `CLN`… thành tên đầy đủ), người sử dụng, địa chỉ, ghi chú. Enter giữ giá trị cũ, `-` xoá |
| **Signatures** | Tiêu đề, đơn vị, địa danh, ngày (dd/mm/yyyy), người trích lục, thủ trưởng, hệ toạ độ |
| **A4/A3**, **Portrait/Landscape** | Khổ giấy, hướng giấy |
| **Scale** | Tỉ lệ 1:n; `0` = tỉ lệ chuẩn nhỏ nhất vừa khung (100, 200, 250, 500, 1000, 2000, 2500, 5000, 10000) |
| **Beside/In Place** | Đặt tờ bên phải bản vẽ (chép thửa sang) hoặc tại chỗ, quanh thửa |
| **Neighbors** | Chép các đối tượng nằm trọn trong khung bản đồ (thửa liền kề, số thửa…) sang tờ |
| **Edge Lengths** | Ghi chiều dài cạnh |

Bản trích lục gồm: khung tờ giấy (lề trái 20 mm đóng gáy), quốc hiệu, tiêu đề; thông tin thửa (số thửa,
tờ, diện tích, mục đích, người sử dụng, địa chỉ, hệ toạ độ, ngày trích xuất, ghi chú); khung bản đồ
với thửa, **số hiệu đỉnh đánh theo chiều kim đồng hồ từ góc Tây Bắc**, chiều dài cạnh, nhãn thửa
(số thửa / diện tích / mã loại đất), mũi tên Bắc, tỉ lệ; **bảng toạ độ VN-2000** (X = Bắc, Y = Đông,
cạnh i: từ điểm i đến i+1; nhiều đỉnh thì thu nhỏ dòng, chia nhiều cột); dòng địa danh, ngày tháng
và phần ký. Mỗi tờ là một bước hoàn tác; layer `TL_KHUNG`, `TL_CHU`, `TL_SODINH`, `TL_CANH`,
`TL_THUA`, `TL_THUHOI`.

- **Sửa**: chạy lại `TRICHLUC` và chọn polyline thửa **hoặc bất kỳ nét nào của tờ trích lục** — các
  thông số cũ hiện lại, Draw vẽ lại tờ đó tại chỗ.
- Thửa đã có dữ liệu GPMB (B1 hoặc GPMBEDIT) thì số thửa, tờ, chủ, loại đất, địa chỉ tự điền; thửa bị
  thu hồi (sau B3) có phần thu hồi gạch đỏ.
- Đơn vị, địa danh, người ký, khổ/hướng giấy, tiêu đề… được nhớ cho lần trích lục sau.
- Thông số lưu trong XDATA `CADMINI_TL` của polyline — giống CadMini, hai phần mềm sửa được tờ của nhau.
- `TLPDF` (**GIS › Extract Sheets to PDF**): mọi tờ trích lục trong bản vẽ ra **một file PDF**, mỗi tờ
  một trang đúng khổ giấy và tỉ lệ, in đen trắng (monochrome), nét theo lineweight.
