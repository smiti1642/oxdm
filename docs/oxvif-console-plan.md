# OxDM 指令台實作計畫

2026-11-12 · 已實作，平台驗收進行中

在 OxDM 頂列開啟 `oxvif` 指令台，對目前選定的攝影機執行 CLI。首批支援 `device info`、`media profiles` 和單機限時 `events watch`，包含游標位置感知的補全、指令說明、輸入與輸出的語法高亮，以及可確認結果的停止流程。計畫涵蓋本地驗收及安裝包驗證，實機與發布另行安排。

## 現有程式與接點

| 接點 | 現況與設計影響 |
| --- | --- |
| OxDM 視圖與裝置 | [頂列](../src/components/topbar.rs)已有全域入口，[主畫面](../src/views/main_content.rs)透過 `Ctx.selected` 取得裝置及有效憑證。[裝置狀態](../src/state.rs)保有完整 ONVIF service URL，憑證優先順序是單機覆寫再回退至全域。指令台放在主工作區，執行時固定裝置快照。 |
| OxDM 鍵盤與呈現 | [App](../src/main.rs)已處理全域方向鍵、F5 和 Ctrl／Cmd+F。[主題 CSS](../assets/main.css)和 [i18n](../src/i18n/mod.rs)可沿用。指令台需要自己的焦點範圍，避免補全時切換攝影機。 |
| OxDM 子程序 | [go2rtc](../src/video/go2rtc.rs)示範 Tokio 子程序及 Windows 隱藏視窗，但該程序關閉 stdio。指令台有自己的 runner，負責串流、停止與回收。OxDM 的 [Cargo.toml](../Cargo.toml)直接依賴 oxvif 函式庫。CLI 是額外的執行檔。 |
| CLI 說明與文法 | `crates/oxvif-cli/src/describe.rs:6` 提供 JSON 指令說明，`contract.rs:508` 的參數描述缺完整 argv 文法。 `main.rs:738` 已把 CLI 表面指令映射至 `CommandId`，`main.rs:2213` 從 `Cli::command()` 產生 shell completion。 |
| CLI 事件與停止 | `main.rs:552` 定義 `events watch`，`watch_output.rs:33` 目前以 Ctrl+C 觸發取消。`event_watch.rs:108` 接受取消 future，但在取得訂閱後才於 `observe` 使用。結束時會等待 managed subscription 清理並回報 `cleanup_failed`。 |
| 裝置補全資料 | [事件頁](../src/views/events.rs)已透過 `api::get_event_properties` 取得 Topic；[API](../src/api.rs)已有 profile 查詢及 session 快取。需要重用查詢與 Topic 正規化，CLI 仍獨立管理自己的訂閱和連線。 |

走訪基準為 OxDM `d67a07f`、oxvif `eaf217d` 加上工作目錄內尚未提交的 `events watch`。

## 1. CLI 匯出文法與說明

- 在 CLI 規劃 opt-in 的 `oxvif describe --grammar --output json --non-interactive`。保留現有 `describe` 的輸出契約。文法資料有獨立版本，並提供 CLI 版本、JSONL schema 及可用控制協定等能力，讓 OxDM 按能力啟用功能。
- 從 `Cli::command()` 匯出實際指令路徑與可見別名、位置參數、長短旗標、作用域、值數量、可重複性、預設值、枚舉及可取得的衝突資訊。用 CLI 端的明確映射對接 `CommandId` 和現有 `describe`，一個 ID 可以對應多種合法拼法。以測試檢查匯出的範例能被同一版 parser 接受。跨欄位規則及完整驗證由 CLI 執行。
- 對支援的指令另提供語意資料，包含是否接受單機直連、目標旗標應放在哪層、哪些值來自裝置、是否需要 TTY，以及輸出種類。只有語法允許且有指令台執行能力的項目才出現在首批可執行選項，說明可以標示其他指令的使用條件。
- OxDM 在首次開啟或執行檔更換時載入及快取規格。版本或能力不足時，在指令台顯示已偵測的版本與可採取的安裝動作，保留可取得的基本說明。各能力獨立判定，互動停止完整可用時才啟用事件監看入口。

## 2. 補全、指令說明與語法高亮

- 頂列加「指令台」入口。工作區放攝影機標籤、輸入欄、候選清單、可收合說明區及輸出區。候選清單依游標所在位置提出子指令、旗標或值，例如 `ev` → `events`、`events wa` → `watch`、`--output j` → `json`／`jsonl`。已填過的單次旗標、可判定互斥的選項和放錯層級的參數不列為候選。
- 共用一個 Rust tokenizer 產出 token、來源範圍及不完整引號狀態。補全只替換游標所在範圍，保留後面的參數。執行走相同的引號與轉義規則，可貼上帶 `oxvif` 前綴的單一指令，支援 `--flag=value` 與帶空白的引號值。執行時阻擋未閉合引號與多行命令。
- 輸入使用原生單行文字欄搭配等寬高亮層，共用字體、padding 和水平捲動。Dioxus 負責渲染文字 spans，薄的 DOM 橋接只處理游標、選取和 IME 狀態。區分指令、旗標、值、引號、未完成或錯誤片段，錯誤同時有文字提示。處理 WebView UTF-16 游標與 Rust UTF-8 byte 範圍，保留原生選取、貼上、復原、輔助工具及明暗主題。JSON／JSONL 顯示鍵、字串、數字和布林高亮，stderr 和缺口用文字標籤搭配顏色。外部輸出用純文字節點呈現，跳脫控制字元。 [HTML 選取與範圍替換](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#dom-textarea/input-setrangetext-dev) · [UI Events IME 狀態](https://w3c.github.io/uievents/#dom-keyboardevent-iscomposing)
- 候選開啟時 ↑／↓ 移動，Tab 或 Enter 接受候選。候選關閉時 Enter 執行，Esc 關閉候選，Tab 回到正常焦點導覽。輸入欄有焦點時攔下會觸發裝置列表或掃描的快捷鍵，IME 組字期間保留輸入法的 Enter 和方向鍵。提供可聚焦的說明、執行、停止與複製按鈕。
- 說明跟隨指令或旗標，顯示用法、必填／預設值、範例及輸出型態。範例可填入輸入欄，交由使用者執行。UI 文字走既有 i18n，CLI 說明以執行檔傳回內容為準。可預覽最後的目標與 argv，保留最近指令供重跑及複製。複製成一般終端可用的指令時，保留目標和使用者選項，移除 GUI 專用的控制參數。裝置值補全先接 `--topic` 的 ONVIF 完整路徑，日後有接受 `--profile` 的指令時用同一介面提供 profile token。目標或憑證變更時讓快取失效；按需取一次資料，離線或沒有資料時仍可手動輸入。

## 3. 選定攝影機與執行

- 開始執行時從 `Ctx.selected` 取出裝置名稱、完整 `DeviceEntry.addr` 及 `Ctx::credentials_for` 的結果。把快照及 run ID 綁在本次輸出上，避免切換或重新掃描裝置時改變執行目標。無選定裝置時仍可查說明。直接呼叫的命令必須有有效的目標來源。
- 首批用 CLI 各子指令既有的 `--target`，保留 HTTPS、port、自訂 service path 及 IPv6。若使用者明寫直連目標，以該值為準並在執行前顯示。只有目標與選定攝影機的規範化 URL 一致時才帶入其登入資料。CLI registry ID、位置式 ID、`--group` 或 `--view` 若超出首批單機範圍，顯示說明。驗證衝突時涵蓋位置式 ID 與根層選擇器，避免同時產生兩個目標。CLI 的 `application.rs:1057` 處理選擇器，`registry.rs:2019` 處理 URL 正規化。
- 以 `tokio::process::Command` 傳入固定路徑及獨立 argv，採用 `--non-interactive`。預設一般命令走 JSON、監看走 JSONL。輸入中已有輸出選項時，按指令台支援的模式解析，只產生一個有效輸出選擇。執行預覽顯示自動補上的參數。清掉子程序繼承的 `OXVIF_USERNAME`／`OXVIF_PASSWORD`，符合目標規則才用 `Credentials::as_options` 注入此次憑證。指令歷史、複製、畫面與診斷紀錄使用不含憑證的資料。 `application.rs:1129` 已支援直連的環境變數認證，但 `contract.rs:341` 拒絕空密碼，OxDM 可送出有帳號、空密碼的組合。CLI 階段須補齊直連認證的這個相容案例，保留現有儲存密碼的驗證，並分別驗收匿名、非空密碼及空密碼。
- 本地開發接受使用者指定的 `oxvif` 絕對路徑。產品預設隨 MSI、DMG、DEB 放入該平台的相容 CLI，以安裝目錄或資源目錄解析並顯示路徑和版本。每種安裝包驗證路徑、執行權限及版本能力，Windows 另驗證沒有跳出 console 視窗。OxDM 內部函式庫依賴的升級可獨立安排。 [現有安裝包工作流程](../.github/workflows/release.yml)

## 4. 事件輸出、匯出與停止

### 串流與顯示

- 以 `events watch --duration 30s --output jsonl` 為預設範例。CLI 在成功取得訂閱後起算觀察時間，`--topic` 接受精確 Topic。事件視圖入口可帶入目前裝置及使用者選定的一個 Topic，切換到指令台並等使用者執行。同一個規範化目標的事件頁與指令台共用訂閱名額。離開事件頁時，已送出的 Create 會等回覆，取得參照就先送 unsubscribe。等待期間可停止指令台，且 CLI 不會啟動。
- stdout、stderr 各自持續排空，採有上限的分行及解碼。依 `data.kind` 處理 `event_notification`、`event_gap`、`event_watch_summary`，並處理獨立的錯誤 envelope。缺口記錄可能帶有 `ok: false`，仍應等待後續資料與摘要。畫面顯示序號、時間、Topic、累計筆數、缺口及結束摘要。裝置端缺口的遺失數未知時顯示未知。收到不支援的 schema、損壞或超長的記錄時，回報解析問題，持續排空至退出。 `watch_output.rs:82` 定義資料列，`watch_output.rs:164` 定義摘要。
- UI 定時批次更新，只保留有筆數與位元組上限的最近輸出。暫停只停止畫面跟隨，stdout 繼續讀取。畫面淘汰的行數、裝置缺口、CLI 已知未送達數及匯出問題分開顯示，缺口與最終摘要保留在獨立狀態。開跑前可指定完整 JSONL 匯出，寫入有界佇列，畫面淘汰不影響匯出。磁碟失敗或寫入跟不上時標記匯出不完整、保留已寫完整行，監看與 pipe 讀取繼續。事後只另存畫面留存區時，在操作文字中標示範圍。

### 正常停止與強制回收

1. 為 CLI 新增只對 `events watch` 生效的 opt-in 控制模式，提案為 `--control-stdin`。父程序持有 stdin pipe，送一行 `stop\n` 或關閉 pipe（EOF）。CLI 有界讀取控制輸入，將 stop／EOF 與現有 Ctrl+C 合併至同一取消狀態。普通 CLI 使用維持現有 Ctrl+C。控制回報不得污染 JSONL。
2. OxDM 在「啟動中」也允許停止。CLI 啟動時先監聽取消，連線前後檢查。若 CreatePullPointSubscription 已送出，停止後仍在既有請求時限內等候回應。有訂閱參照就走正常 unsubscribe，若請求逾時且沒有參照就回報無法確認清理。取消狀態只能觸發一次，重複停止與限時到期相遇時只產生一次結束結果。 `event_watch.rs:126` 是目前啟動位置，`src/events/managed.rs:279` 是取得 PullPoint 的位置。
3. GUI 按下停止後轉為「停止中」，持續讀取輸出，等待 worker 排空、unsubscribe、最終摘要和子程序退出。合併摘要的 `reason`、`complete`、`cleanup_failed` 與 exit status 顯示結果。「使用者停止」可以伴隨缺口或清理未確認。若超過等待上限才強制終止並回收子程序。摘要缺失或程序被強制終止時，顯示清理未確認。上限由握手／請求時限、CLI 排空、清理及最後輸出時限推導，OxDM 管理模式須有有限的請求與重試預算。
4. Runner 由 app 層持有，每次只管理一個執行中的命令。離開指令台或切換攝影機可回到原執行結果。正常關閉 App 時先走同一個停止流程，完成或達上限後才退出。開發時驗證各平台的視窗關閉接點，保留子程序被意外斷開時的 EOF 與有限觀察時間兩道保護。

## 5. 實作順序與驗收

| 階段 | 交付與完成條件 |
| --- | --- |
| 1. oxvif-cli 文法和控制 | 新版規格測試涵蓋別名、根層與子層旗標、枚舉、位置參數及 schema 相容。以現有 `crates/oxvif-cli/tests/event_watch.rs` 的 MockServer 子程序測試為基礎，驗證 stdin stop、EOF、啟動中取消、限時自然退出、unsubscribe 失敗與結束摘要。用直連測試確認空密碼的行為與 OxDM 一致。原有 stdout 被塞滿和終端 Ctrl+C 路徑一併回歸。 |
| 2. OxDM 編輯器及一次性執行 | 完成入口、補全、說明、語法高亮與命令預覽。用測試執行檔記錄 argv，驗證引號、Unicode、IPv6、自訂 path、旗標位置、手動目標、無目標、憑證隔離與執行中換機。人工走過中英文 IME、焦點、鍵盤操作、貼上及明暗主題。 |
| 3. 監看及跨程序驗收 | 接上真實 CLI 與本地 MockServer，驗證 Topic 篩選、事件暴增、慢 UI／慢匯出、續租、斷線缺口、停止與 app 關閉。比較完整匯出的事件序號與摘要，確認畫面淘汰及匯出失敗各自可辨，並用 MockServer 記錄核對 unsubscribe。測試摘要遺失、舊執行檔及強制回收，確認 UI 能指出哪一段結果未確認。 |
| 4. 安裝包驗收 | Windows、macOS、Linux 各安裝相對應測試包，驗證相容執行檔、啟動路徑、基本指令與停止。真實攝影機的訂閱容量、廠商特有 Topics 與長時段監看另列實機驗收。 |

## 本機試跑與包裝

在 oxvif 工作目錄執行 `cargo build --locked -p oxvif-cli --bin oxvif --example console_mock`。在 OxDM 指令台選擇該執行檔，或在啟動前將 `OXDM_OXVIF` 指向它的絕對路徑。可用 `device info`、`media profiles` 或 `events watch --duration 30s` 試跑。

要建置安裝包，先用 `python scripts/stage-console-cli.py <oxvif 路徑>` 暫存 CLI，再執行該平台的 `dx bundle`。用 `python scripts/stage-console-cli.py --bundle <安裝包> --mock <console_mock 路徑>` 驗證內附執行檔。測試 fixture 跟 CLI 取自同一份 oxvif 原始碼，因為 OxDM 鎖定的 0.17.0 MockServer 回傳的事件租期為零。

GitHub Actions 手動執行時使用 `oxvif_sha` 指定相容的 oxvif commit，只產生驗收包。日後以 tag 產生安裝包時，`OXVIF_CONSOLE_SHA` 要指向同一份可取得的 commit。

### 原生桌面複驗

1. 在該平台安裝測試包並開啟 OxDM。從頂列開啟「指令台」，確認執行檔路徑指向包內的 `oxdm-oxvif`，且顯示 CLI 版本及指令說明。
2. 在 oxvif 工作目錄執行 `cargo run --locked -p oxvif-cli --example console_mock` 並保持 stdin 開啟。用 MockServer 印出的 loopback URL 執行 `device info --target <URL>`，再執行 `events watch --target <URL> --duration 30s`。收到事件後按「停止」，確認最終摘要顯示清理確認。再啟動一次監看並直接關閉視窗，確認 OxDM 與 sidecar 均退出。Windows 執行時觀察是否跳出額外的 console 視窗。
3. 用 `ev` 驗證 Tab 補全後能繼續打字。以中文輸入法組字，按方向鍵與 Enter 選字，確認候選清單和執行動作未被誤觸。切回英文輸入後確認可執行，並核對明暗主題的指令及 JSON 高亮、貼上與焦點導覽。

## 本地驗收紀錄

- Windows MSI 和 Ubuntu 24.04 DEB 均已建置及解包，並使用包內 CLI 對同版 MockServer 通過跨程序驗收。涵蓋單機查詢、JSONL 匯出、取消訂閱、事件暴增、畫面淘汰、匯出失敗與缺口回報。
- 追加的跨程序測試涵蓋所選攝影機憑證隔離、匯出逐行序號與 Topic 核對，以及摘要遺失和強制回收。CLI 測試涵蓋連線中停止、已授權但尚未回覆的 Create 停止後回收訂閱、unsubscribe 失敗與終端 Ctrl+C。
- 事件頁交接測試使用同一台 MockServer。事件頁訂閱仍有效時，指令台等待，且可以在子程序啟動前停止。事件頁離開後，測試確認原訂閱已失效，指令台能接手收取事件。停止指令台後，事件頁可重新取得訂閱名額。Ubuntu 安裝後的桌面也用「Watch this topic」確認已帶入攝影機與 `VideoSource/MotionAlarm`，監看期間切回事件頁會顯示等待，停止後事件頁恢復收取。
- 用兩台 MockServer 驗證執行中換機。輸入區顯示新攝影機，舊監看及停止摘要仍顯示原攝影機名稱和位址。下一次執行使用新目標與空密碼。另以測試執行檔確認帶中文、空格及引號的 IPv6 目標在子程序中仍是單一 argv。新版 MSI 共通過八組、DEB 共通過七組包內 CLI 驗收。
- 斷線重建訂閱期間的停止也已回歸。已送出的 Create 會等到既有請求時限，取得新參照就取消替代訂閱，回覆逾時則標示清理未確認。測試確認等待裝置回覆超過輸出排空時限時，CLI 仍能送出最終摘要。
- Windows 和 Ubuntu 均驗證了匯出寫到半行失敗時會截回最後一筆完整紀錄，以及匯出佇列塞滿時仍可讀取事件、缺口與最終摘要。重新建置的 MSI 和 DEB 各通過四組包內 CLI 的 MockServer 測試。
- Windows 使用 oxvif 0.16.0 驗證舊版相容路徑。文法探測失敗後仍可取得版本、`device info` 和 `media profiles` 的說明及範例。可用 `OXDM_OXVIF_LEGACY_TEST` 指定舊版執行檔的絕對路徑，執行 `cargo test --locked console::process::tests::legacy_cli_retains_version_and_command_help -- --ignored` 重現。
- Ubuntu 桌面確認 Tab 補全後可立即繼續輸入、選定裝置可帶入目標，並可補全裝置回報的 Topic。
- Ubuntu 24.04 容器用 `dpkg -i` 安裝 DEB，從 `/usr/bin/oxdm` 啟動桌面，並以 `/usr/bin/oxdm-oxvif` 通過 MockServer 測試。桌面也實際執行 `device info` 和 `events watch`，檢視明暗主題下的語法高亮，並確認停止摘要。複驗發現隱藏 WebView 可能暫停關閉任務，修正後在監看中關閉視窗，已確認 OxDM 與 CLI 程序均正常退出。
- Windows 以 MSI 行政解包後的 `oxdm.exe` 啟動，並從 WebView2 的原生無障礙樹確認新版「指令台」入口。另以 OxDM 的程序啟動設定執行 MSI 內附 CLI，確認監看中沒有可見的 console 視窗，且停止後仍回傳訂閱清理確認。桌面擷取逾時，WebView2 也未開啟本機測試用的除錯端點，原生畫面操作與 IME 尚待驗收。 [Windows GetConsoleWindow](https://learn.microsoft.com/en-us/windows/console/getconsolewindow)
- macOS DMG 原生驗收、實機及發布尚待後續。
