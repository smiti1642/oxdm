# OxDM 原生功能實作計畫：隱私遮罩與錄影

2026-11-16。走訪基準為 OxDM `develop` 的 `c36c560` 與 oxvif `master` 的
`026da13`。目前僅規劃。兩條路的實機互通性均待驗收。

## 開發順序

**先做裝置端隱私遮罩。** 第一批要做到「選定來源 → 在畫面畫出遮罩 → 寫入攝影機 →
讀回裝置設定 → 檢查新畫面的遮蔽效果」。oxvif 已有這條路需要的 Media2 選項與寫入
API。OxDM 需要補的主要是預覽座標、來源與權限辨識、寫入狀態和畫面驗證。

| 候選 | 使用者能完成的任務 | 主要新增成本 | 首批相容條件 |
| --- | --- | --- | --- |
| **隱私遮罩** | 從 OxDM 調整攝影機輸出的遮蔽區域 | 對齊預覽與裝置座標，辨識共用來源的影響，分開呈現設定讀回與畫面證據 | Media2 遮罩操作、可確認的來源組態與畫面對應、具寫入權限 |
| **錄影時間軸與匯出** | 找到時段或事件，匯出裝置上的錄影 | Search 結果與 Replay 播放分開，另外管理裝置端匯出工作和儲存目的地 | Search、Replay、匯出格式和可寫入目的地各自探測 |

第一批遮罩寫入以可驗證的矩形為範圍。若手邊設備的唯讀盤點找不到能驗證遮罩畫面的
設備，開發可以先完成本地 fixture 與唯讀列表。下一個可獨立交付的選項是錄影頁的
唯讀時間範圍與事件索引，精準跳播和匯出各走自己的驗收。

## 程式碼走訪

| 現有入口 | 已有行為與缺口 |
| --- | --- |
| [OxDM 導覽](../src/components/device_panel.rs#L14)、[畫面路由](../src/views/main_content.rs#L19)、[DeviceGate](../src/api.rs#L91) | 錄影入口由 Search URL 控制。OSD 以 Media1 為前提，Media2-only 會被排除。遮罩要有自己的入口與能力判斷。 |
| [OxDM OSD](../src/views/osd.rs#L42)、[共用快照](snapshot-media2-followup.md)、[Media2 編碼](../src/api/encoder.rs#L160) | 有裝置編輯表單、刪除確認、帶認證的快照下載和 Media2 profile 查詢可重用。現有 OSD profile/組態查詢走 Media1。遮罩預覽要從自己的 Media2 profile 解出 URI，再沿用共用下載。 |
| [oxvif Media2Overlays](https://github.com/smiti1642/oxvif/blob/026da13/src/overlays.rs#L35)、[遮罩 API](https://github.com/smiti1642/oxvif/blob/026da13/src/overlays/mask.rs#L116)、[來源組態型別](https://github.com/smiti1642/oxvif/blob/026da13/src/types/video.rs#L139) | `media2_overlays(vsc_token)` 綁定可信服務端點與來源組態，已有 options、CRUD 與獨立讀回。來源型別保留裁切 Bounds，尚未保留 Rotate 等幾何資訊。 |
| [OxDM 錄影頁](../src/views/recordings.rs#L19)、[API](../src/api.rs#L1680)、[既有 smoke](../tests/recordings_smoke.rs#L26) | 已能列出 `RecordingInformation`，取得選中錄影的 RTSP URI，再交給 go2rtc 播放整筆。現在以錯誤文字中的 `service URL` 區分不支援。現有 smoke 證明 Search 與取 URI，播放與跳播需獨立驗證。 |
| [oxvif Search owner](https://github.com/smiti1642/oxvif/blob/026da13/src/recording/search.rs#L351)、[時間軸/事件](https://github.com/smiti1642/oxvif/blob/026da13/src/recording/timeline.rs#L226)、[匯出](https://github.com/smiti1642/oxvif/blob/026da13/src/recording/export.rs#L100)、[下載](https://github.com/smiti1642/oxvif/blob/026da13/src/recording/download.rs#L64) | 已有有界搜尋、`EndSearch`、錄影摘要與事件搜尋、裝置端匯出工作、明確信任 URL 的檔案下載。摘要和單筆最早／最晚時間只能描述報告的範圍。OxDM 的播放器沒有接上 ONVIF Replay 的 `Require: onvif-replay` 與 `Range: clock=`；匯出回應提供工作 token 和檔名。 |
| [OxDM Cargo.toml](../Cargo.toml#L29)、[oxvif Cargo.toml](https://github.com/smiti1642/oxvif/blob/026da13/Cargo.toml#L6) | OxDM 原生依賴目前使用 crates.io 的 `0.17`。上述 API 位於 oxvif 開發 checkout，雖然 workspace 版本仍標為 `0.17.1`。本地原型可用 Cargo 的命令列 patch 或未納入 Git 的本機設定，驗收記錄鎖定實際來源。可重現的整合分支若先於 registry 發行，使用明確 Git revision，待正式 crate 可用後再切回。 |

## 第一批：Media2 隱私遮罩

### 入口與相容性

1. 在裝置側欄加入「隱私遮罩」，沿用現有 `View`、`MainContent`、i18n 和確認
   對話框。頁面載入時才探測。保留明確的未知、檢查中、支援、不支援、權限不足、
   回應異常和暫時離線狀態。未宣告 Media2 端點、明確宣告 `Mask=false` 或 SOAP
   明確回報遮罩操作不支援才判為不支援。端點信任檢查失敗另報錯誤。
   `Mask` 屬性缺漏時可做有界唯讀驗證，只有該組態的 options 與列表都讀得回來
   才開放寫入。空列表表示這個可讀組態目前沒有遮罩。
2. 新增窄範圍的 `src/api/privacy_masks.rs` 配接層，重用 `sessions::get` 和
   oxvif 原生 crate。在進入 i18n UI 前保留結構化錯誤，區分 SOAP 拒絕、傳輸、
   回覆異常和寫入結果不明。每個讀取結果對應裝置、工作階段、Media2 profile 和
   VideoSourceConfiguration；遮罩的寫入身分由裝置、組態與 mask token 決定。
   使用 Media2 profile 內的 `video_source_config_token`，並辨識底層
   `video_source_token`。切換裝置、憑證或來源時忽略舊回覆，未儲存草稿可先提示。
3. 根據 `MaxMasks`、`MaxPoints`、`RectangleOnly`、效果和顏色選項啟用控制項。
   首批可畫矩形、移動和縮放、為現有矩形啟用或停用及明確刪除。既有多邊形和未知
   效果可列出，保留原樣。可寫入的色值依裝置回傳的清單或範圍。oxvif 目前要求
   options 有可辨識的顏色選項及非空效果清單。若實機提供不同形狀，保留原始失敗
   分類，先用脫敏 fixture 核對 WSDL，再在 oxvif 調整解析。
4. 列出可能共用此組態的 profiles。設備回報 `SourceMask=true` 時，整個物理
   VideoSource 的相關組態可能一起變更；`SingleColorOnly=true` 時，改色可能
   影響該來源的其他遮罩。首批若無法盤點並讀回受影響組態，相關操作維持唯讀。
   若可驗證影響範圍，確認視窗顯示它，寫入後重新讀取該來源的相關列表。
   [Media2 §5.10](https://www.onvif.org/specs/srv/media/ONVIF-Media2-Service-Spec.pdf?189db0=) · [Media2 WSDL](https://www.onvif.org/ver20/media/wsdl/media.wsdl)

### 預覽與寫入

1. 首批使用選定 Media2 profile 的即時擷取快照，讓畫布有可量測的影像尺寸。
   採用實際可見影像矩形，滑鼠、觸控及鍵盤調整共用同一套純座標轉換。
   在已確認預覽方向和來源組態一致的情況下，影像左到右的比例 `u` 與上到下的
   `v` 轉成 `x = 2u - 1`、`y = 1 - 2v`。留黑區和縮放後尺寸不屬於
   影像座標。拖曳期間維持草稿，重新取圖或視窗縮放後重算投影。
2. 用 Media2 的來源 Bounds、預覽畫面與可讀到的方向資訊判斷幾何。裁切座標的
   參考系為該來源組態窗，不以整顆 sensor 的原點計算。只知道畫面比例不足以
   判斷旋轉、鏡射或去畸變。不能可靠建立映射時只顯示列表。若試點需要 Rotate
   或其他必要資訊，在 oxvif 增加小範圍的 Media2 讀取與 fixture，避免在 OxDM
   重寫 SOAP 或把推測值接進寫入流程。
3. 寫入前重新確認選中的組態、options 和該遮罩原始值。外部工具已改動時提示
   重新載入；雙擊、同來源的並行編輯由 app 擋下。協議沒有 compare-and-swap，
   遠端仍可能在讀與寫之間變更。採用 oxvif 的 preflight 和讀回結果。
   對 `OutcomeUnknown` 先重新載入範圍內列表，只有能安全比對時才標記成功；
   寫入失敗不自動重送。成功回覆後若讀回被裝置調整，畫面顯示實際值。
4. 分別呈現「草稿」、「裝置設定已讀回」及「新畫面已確認」。
   重取裝置畫面時去掉本地遮罩圖層，讓使用者檢查真正的遮蔽。由使用者確認後
   才標記畫面已確認。若只有串流會套用
   遮罩，則用實機同 profile 的獨立串流畫面驗證，UI 保留未確認狀態。支援寫入
   設定的機型與驗證過哪些輸出須明列。刪除和停用會露出畫面，操作前顯示對象
   和影響範圍。測試用非敏感場景，預設不把前後影像寫進一般記錄。
   [Media2 §5.10 與 Figure 4](https://www.onvif.org/specs/srv/media/ONVIF-Media2-Service-Spec.pdf?189db0=)

## 後續：錄影頁分三段交付

1. **時間範圍與事件索引。** 沿用 `RecordingsView` 和整筆播放入口。用
   `recording_search`、`RecordingTimeline::summary/information` 產生時間尺度，
   將錄影時間邊界畫為「裝置回報範圍」。沒有區段證據的位置保留未知，避免暗示
   整段都有畫面。時間以 `RecordingTimestamp::instant()` 比較，在 UI 標出
   顯示時區。事件標記以選中的錄影、日期時段和確認過的服務為範圍，再加
   `event_search`。查詢 owner 放在可取消的工作內；換頁、換裝置、重新搜尋
   和關閉時用 cancellation，等待有界 `EndSearch`。頁面分辨部分結果、已完成空
   結果、搜尋失敗及清理未確認。摘要或事件操作不支援時保留清單和可用的其他能力。
   [ONVIF Recording Search](https://www.onvif.org/specs/srv/rsrch/ONVIF-RecordingSearch-Service-Spec.pdf)
2. **精準回放。** 另外設計支援 `Require: onvif-replay` 與 `Range: clock=` 的
   replay adapter，先用實機確認 seek、第一個可解碼畫面與時間，再接播放器。
   用實際結果區分純 RTSP 整筆播放和相容的時間定位。責任集中在 replay backend，
   負責 URI 信任、防止憑證外送、切換錄影時釋放資源、逾時及播放中止。現有
   oxvif RTSP probe 能檢查 RTP 到達，播放解碼與時間定位需另有證據。
   [OxDM ReplayStage](../src/views/recordings.rs#L126) · [OxDM go2rtc](../src/video/go2rtc.rs#L253) ·
   [ONVIF Streaming §6.3–6.5](https://www.onvif.org/specs/stream/ONVIF-Streaming-Spec.pdf)
3. **匯出工作與檔案取得。** 用 `recording_exports().capabilities()` 與
   `get_storage_configurations()` 分別取得格式和已配置的裝置端儲存。
   選清楚錄影、UTC 時段、格式、具名目標及相對目錄後才送出。首批匯出可先
   明確標示「匯出到裝置儲存」，記錄進度和裝置所報檔名。app 層持有工作 owner，
   可切換頁面而不中斷查詢。UI 分別呈現進行中、觀測逾時、裝置確認完成、
   取消已確認或未確認、結果不明。暫時斷線可重查已知工作。啟動回覆遺失且
   未取得 token 時，標示結果不明，讓使用者到裝置或儲存端核對後再決定是否
   重試。關閉應用程式時，有未完成工作就明示遠端可能繼續執行。若將來要
   支援重啟後繼續追蹤，先規劃以裝置和
   operation token 查回既有工作的契約；現有 API 由 `start` 回傳 owner。
   只有另有經驗證的檔案來源才提供儲存到本機。用 `recording-download` 的
   deadline、容量限制、取消與不覆寫，禁止用回傳檔名猜 URL。NAS、SD 卡和
   可下載端點各自驗收。
   [ONVIF Recording Control §5.21–5.23](https://www.onvif.org/specs/srv/rec/ONVIF-RecordingControl-Service-Spec.pdf)

## 驗收

| 層級 | 遮罩先做 | 錄影後續 |
| --- | --- | --- |
| oxvif 與 app 整合 | 沿用雙來源 mock。加測 Media2-only、有 Media1/2 但只有一個支援、不同組態共用物理來源、矩形限制、滿額、未知效果、權限拒絕、寫入回覆遺失與讀回差異。測到受影響的組態和無關來源各自保持正確。 | 延伸既有 Search/Replay smoke。用 scripted SOAP 測部分頁、取消和清理、時區邊界、缺失服務與 export start/progress/stop。用有界 RTSP peer 驗證 Replay headers 和實際回放時刻。下載覆蓋檔名混淆、重導、容量限制、中斷及同名檔競爭。 |
| OxDM 本地操作 | 用有方向標記的靜態畫面測四角、非等比例視窗留黑、縮放與鍵盤移動。驗證換機時舊結果不回填、未儲存草稿、寫入結果不明、讀回與視覺狀態。Windows 原生 UI 走完整流程，繁中、英文、俄文都檢查。 | 用 UTC 與本地時間差異、跨午夜和 DST 重疊案例驗證顯示；缺少 Replay 或儲存配置時仍能看清單。檢查選時段、取消、切頁、重連及錯誤訊息。 |
| 實機 | 先做唯讀盤點。有支援樣機才做非敏感場景寫入。至少一台測出設定讀回和新畫面效果，跨廠相容性需第二種廠牌或韌體以及一台不支援的對照。記錄 profile／物理來源、畫面方向、輸出差異和改動前後狀態，驗收後恢復測試遮罩。 | 分別找有既存錄影的攝影機或 NVR、要求 ONVIF Replay 控制的裝置，以及可匯出到已配置儲存的裝置。以已知事件或時間標記核對查詢、跳播和匯出檔案，並記錄輸出能否播放。未覆蓋組合標待驗。 |

已有證據可用來起跑。oxvif 的雙來源測試覆蓋作用範圍、無效幾何及回覆遺失，
錄影則有獨立 SOAP fixture、Search owner 與 HTTP 下載測試。Mock 的寫入讀回
能證明狀態改變，畫面遮蔽須另外驗證。每次實機記錄廠牌／型號、韌體、服務、
帳號權限級別、運作結果與脫敏證據。既有 I08／I09 驗收表仍標為待實機。
[遮罩測試](https://github.com/smiti1642/oxvif/blob/026da13/tests/media2_overlays.rs#L103) ·
[錄影測試](https://github.com/smiti1642/oxvif/blob/026da13/tests/extended_onvif.rs#L506) ·
[互通性證據](https://github.com/smiti1642/oxvif/blob/026da13/docs/active/onvif-interoperability-plan_zh.md#i07i10-交付證據)
