# OxDM：升級 oxvif 0.17 計畫

日期：2026-09-22

狀態：升級、相容性修正與 Windows 本機驗證已完成；GUI／實機與其他平台另列待驗收。

評估基準：oxdm `aa91b11`（0.4.1）、crates.io `oxvif 0.17.0`。

## 目標與範圍

將 oxvif 從 0.16.0 升級至 0.17.0，取得健康檢查快照修正、錄製資料
保留與 replay 行為改善，同時修正 OxDM 對 Quirks 列識別與比較結果的
相容性假設。完成編譯及資料語意遷移後，才具備發布條件。

本計畫不決定下一個 OxDM 版本號，也不包含發布操作。以下另列後續工作：

- 主畫面／MJPEG 的自訂快照下載器整合至 oxvif 共用下載器。現有 TLS
  偏好、URL 相容性及驗證行為須另行設計，不能直接替換。
- 完整 Media2 讀取、設定與小數 FPS 編輯。這次整數轉浮點修正僅維持
  現有 Media1 → Media2 的 H.265 寫入路徑，不宣稱 GUI 已支援小數 FPS。
- 新的 Media synchronization、push peer API 與 CLI 維護功能介面。
- Dioxus、keyring 等不相關的框架／重大依賴升級。

## 已完成的評估證據

以下結果來自獨立暫存副本，不是正式升級完成紀錄：

- [x] 確認一般及開發依賴均指定 `oxvif = "0.16"`，lockfile 為 0.16.0。
- [x] 使用 crates.io 0.17.0 執行 `cargo check --all-targets`，確認
  `src/api.rs::to_video_encoder_configuration2` 有 `u32` → `f32` 型別錯誤。
- [x] 僅修正該轉型後，`cargo test --all-targets` 原有 **174 個測試全過**；
  `cargo clippy --all-targets -- -D warnings` 通過。
- [x] 額外加入並單獨執行 Quirks 碰撞案例：兩筆相同 canonical key 的
  回應，只有一筆有結構差異，`group_results` 卻回報兩筆問題。
  該案例在一般測試批次暫時標記 ignored，單獨啟用後確實失敗；
  正式修正必須移除 ignored，讓它成為正常回歸測試。
- [x] 確認主畫面快照走 `fetch_snapshot_data_uri` 自訂實作；只有健康檢查
  會直接使用 0.17 共用下載器。
- [ ] 實機驗收及 Linux／macOS 的建置與執行驗證尚未完成。

## 1. 更新依賴與編譯相容性

涉及：`Cargo.toml`、`Cargo.lock`、`src/api.rs`、
`src/components/about_dialog.rs`。

- [x] 將一般及開發依賴一起改為 `version = "0.17"`，保留原有 features。
  使用 crates.io，不能以鄰近 oxvif 工作目錄的 path patch 作為驗收結果。
- [x] 定向更新 lockfile，確認實際解析至 0.17.0。若執行時要採用其他
  0.17.x，先檢查該版本的額外變更並更新本計畫的驗證基準。
- [x] 在 `to_video_encoder_configuration2` 明確轉換既有整數 FPS 為 `f32`。
  新增 H.265 轉換測試，驗證正常相機 FPS、bitrate、GovLength、profile
  與 rate control 缺省狀態均保留；不得引入浮點 → 整數截斷流程。
- [x] 將 `OXVIF_VERSION` 更新為實際解析版本，使 About 與 baseline stamp
  一致。oxvif 的 Rust 最低版本仍為 1.88；完整應用的最低版本須依
  實測另記，不能直接沿用函式庫的宣告。
- [x] 記錄升級前後的 `cargo audit` 結果；只處理本次變更相關或必要的
  安全修正，列明既有／新增警告，不以忽略 advisory 代替說明。

驗收：全目標可編譯，lockfile 與版本戳一致，H.265 轉換測試通過。

## 2. Quirks 改以每筆錄製資料建立畫面列（發布前必修）

涉及：`src/mock_servers.rs`、`src/views/settings/quirks.rs`，必要時調整
`src/api.rs` 的分析結果交接。

0.17 的 `(action, key_canon)` 只代表一組資料，不是唯一識別碼。
目前 `HashMap<key_canon, …>`、Dioxus key、selected／expanded 集合及匯出
篩選都可能混用不同錄製資料；只把 action 加進 key 仍不足以修正。

- [x] 在 OxDM 分析層建立同一份不可變 `FixtureStore` snapshot 的完整列
  結果，使用「本次分析識別 + fixture ordinal」識別每一列。
  ordinal 僅在該 snapshot 有效，不作跨次掃描或永久 baseline 的 identity。
- [x] 完整 `diff_details` 與 `ParseReport.verdicts` 只可在同一 snapshot
  依 insertion ordinal 配對。檢查筆數及 action／key 一致性；若不一致，
  顯示分析失敗或未完成，不以 zip 截短或退回 key map 靜默配對。
- [x] 結構差異數量與匯出的 `OperationQuirk` 必須對應同一 fixture。
  必要時逐筆計算結構報告。`QuirkReport.quirks` 是過濾後的集合，
  不得直接按完整列索引取值，也不得以非唯一 key 回填各列。
- [x] 統一讓 badge、問題數、左右 XML、選取、展開與 JSON 匯出使用相同
  列資料及 row ID。保留既有匯出的必要欄位；同 key 的多筆 quirk 不去重，
  勾選其中一筆只能匯出該筆，既有只匯出結構差異的範圍保持一致。
- [x] 重新測試、切換裝置或替換分析 snapshot 時，清除／隔離舊的勾選及
  展開狀態；舊的 async parse 結果不可配上新 snapshot 的結構資料。
- [x] 加入正常執行的回歸測試：同 action／key 一筆有差異、一筆正常；
  同 key 不同 action；同 key 不同解析結果；兩筆不同 quirk；
  個別選取／匯出；分析更新後不混入舊結果。
  至少一個案例須經真實 `FixtureStore` 產生報告，避免只有手工 row 測試。

驗收：重現案例從「2 筆問題」變成「1 筆問題」，各列 XML、badge、計數
與匯出一致；沒有因 collision 被覆蓋或錯配的列。

## 3. Baseline 比較與舊資料相容性

涉及：`src/persist.rs`、`src/views/settings/quirks.rs`、
`src/i18n/{en,zh_tw,ru}.rs`。

- [x] 保留目前帶版本的 baseline envelope 與舊版裸 `QuirkReport` 載入。
  加入 0.16 → 0.17 mismatch、未知版本及同版本的回歸案例。
- [x] 保留跨版本警告；跨版本結果標明參考基準已變，不能作為相機發生
  變更的證據。不自動覆寫使用者的 baseline，重新儲存後才更新版本戳。
- [x] 比較前檢查目前／舊 baseline 中重複的 `(action, key_canon)` 群組。
  任一側不唯一時，該群組顯示為「無法唯一配對的觀測資料」，保留兩側
  資料，不能沿用紅色「新出現」／綠色「已消失」表示故障新增或修復。
  唯一群組仍保留既有 appeared／resolved／changed 比較。
- [x] 測試相同且含碰撞的報告與自身比較：即使上游 diff 不為空，畫面也
  不應宣稱異常新增／消失。測試唯一群組仍能正確顯示實際差異。
- [x] 新增或修正文案時，同步英文、繁體中文、俄文及 i18n parity 測試。
- [x] 使用受控的舊版格式 clone fixture 驗證讀取、解析、啟動 replay 與儲存。
  載入本身不得改寫來源檔；0.17 儲存／再載入須保留碰撞筆數與回應。
  必要 metadata 缺漏若導致解析失敗，應呈現實際診斷，不填入猜測值。

驗收：舊資料可依既有格式讀取；不錯認基準漂移或無法配對為相機變更；
新版碰撞錄製資料經儲存後不遺失。

## 4. 分身與健康檢查行為驗證

涉及：`src/mock_servers.rs`、既有 integration smoke tests、健康檢查測試。

- [x] 保留 record → serve → drive → diff → stop 與 live quirk sweep 測試，
  驗證完整報告仍可配對，分身的成功寫入會刷新相關 replay 讀取。
- [x] 加入被拒絕寫入不改狀態／錄製資料的案例。重啟等未模擬效果應接受
  0.17 的明確拒絕；不為了維持舊成功回應而全面開啟 acknowledgment-only。
  若某個測試只驗證 receipt，僅針對該 operation 明確 opt in。
- [x] 健康檢查測試須涵蓋 OxDM 實際啟用的 liveness probes；既有
  `healthtab_smoke` 預設 `HealthCheck::run` 並未涵蓋此路徑。
  使用可控 HTTP fixture 驗證快照成功，以及錯誤驗證／非圖片回應能正確
  反映在報告；區別快照驗證與 RTSP／解碼未驗證項目。
- [x] 記錄新下載器的同 host、禁止 redirect／HTTPS downgrade、TLS 驗證
  與圖片大小限制的健康報告行為邊界，實機相容性另列待驗收。主畫面與健康檢查的結果可能
  不同，不把它描述成所有快照路徑已統一。
- [x] 檢查 health、PTZ、imaging、IO、recordings 與 baseline 既有測試；
  測試通過不能代替真實分身效果或實機相容性證據。

驗收：成功與拒絕都符合 0.17 契約，不以放寬 mock 規則掩蓋失敗。

## 5. 整體驗證與發布準備

執行以下檢查並記錄 toolchain、平台、測試數與未驗證項目：

```powershell
rtk cargo fmt --check
rtk cargo clippy --locked --all-targets -- -D warnings
rtk cargo build --locked
rtk cargo test --locked --all-targets
rtk cargo audit
rtk git diff --check
```

- [x] Windows 本機品質檢查通過，結果見下方。
- [ ] Linux 現有 CI 及發布所支援的其他平台建置／封裝驗證。
  本次未推送或觸發 hosted CI，不宣稱跨平台已驗收。
- [ ] GUI 驗收：兩筆碰撞資料分別展開、勾選／匯出、切換裝置、重新測試、
  舊 baseline 警告、無法配對提示及重新儲存 baseline。
- [ ] 實機驗收：至少一台 H.264／H.265 相機的 profiles、整數 FPS 設定
  寫入與讀回、縮圖／直播及健康檢查；快照 Digest 與 HTTPS 情境依
  可取得的設備執行。變更設定後還原原值。沒有設備的情境明列待驗收。
- [x] 更新 CHANGELOG 的 Unreleased 區段、相關開發說明及本計畫結果。
  說明分身拒絕行為、baseline 版本差異、碰撞比較語意與快照收益範圍。
- [x] 最終 diff 僅包含必要升級、相容性修正、測試及文件。
  本計畫完成不代表已建立 release、推送 tag 或發布套件。

## 發布條件與退版

以下任一項未完成，不能將版本號更新視為可直接發布的升級：

- 正常執行的 collision regression 尚未通過，或 UI／匯出仍混用列。
- 無法配對的 baseline 群組仍呈現為已證實的異常新增／消失。
- 版本戳／lockfile 不一致，或必要品質檢查失敗。

實機與平台驗收結果須逐項記錄；任何缺口均須在發布決策時明確列出，
不能用 174 個既有測試或 oxvif 自己的 CI 代替 OxDM 的驗證。

退版前保留升級前的 clone／baseline 備份，並另存 0.17 產生的資料。
新版 `fixtures.json` 雖仍可被舊 reader 讀取，舊 reader 可能折疊碰撞
資料，重新儲存後造成遺失；不可用 0.16 開啟後覆寫唯一的 0.17 副本。
程式碼退版須一起還原 manifest、lockfile、型別調整及版本戳。

## 實作與驗證結果（2026-09-22）

實際使用 crates.io `oxvif 0.17.0`，保留原有 features。Windows x86-64
MSVC、`rustc 1.97.0 (2d8144b78 2026-07-07)`；應用版本仍為 0.4.1，
變更記於 Unreleased，沒有建立 release 或 tag。

### Quirks 與資料格式

- `src/mock_servers/analysis.rs` 從同一份不可變 snapshot 建立列資料。
  完整 XML details 與 parse verdict 依 insertion ordinal 配對。
  oxvif 0.17 的 structural report 依相同順序產生，但略過 `differs=false`
  的列；實作以這個明確 predicate 還原 ordinal，檢查筆數及 action/key，
  不直接拿過濾後報告索引當完整列索引。真實 collision fixture 覆蓋
  正常列位於異常列前／後、多筆異常、不同 action、個別匯出與檔案 replay。
- row ID 由分析 generation 與 ordinal 組成；重測／切換裝置會隔離
  舊結果與選取狀態，匯出及儲存基準也檢查目前 generation。
- baseline envelope 新增可省略的 `ambiguous_keys`，記錄完整資料中的
  `(action, key_canon)` 碰撞，包括被結構報告略過的正常列。舊格式仍可
  載入，但無法還原未曾保存的碰撞資訊。退版後重新儲存 baseline 會丟掉
  此欄位，因此 clone 與 baseline 都應保留備份。
- 無法唯一配對的觀測資料以中性提示與可展開的兩側資料顯示，唯一群組
  才沿用新增／消失／變化分類。異常與正常列不再共用 badge 或勾選狀態。
- 補強測試發現 0.17 的 metamorph verifier 尚未 dispatch metadata parser。
  應用保留 `Unverified`，不當成已解析；另以專用 Media2 metadata API
  驗證缺少 Multicast 的舊格式回應會失敗，且不會補入猜測值或改寫錄製資料。

### 本機品質檢查

- `cargo fmt --check`：通過。
- `cargo clippy --locked --all-targets -- -D warnings`：通過。
- `cargo build --locked`：通過。
- `cargo test --locked --all-targets`：**199 passed、0 failed、0 ignored**。
  141 個 binary unit tests；healthtab 12、imaging_focus 13、io_control 10、
  ptz_absolute 13、recordings 10。部分 integration suites 透過既有
  source inclusion 重複執行 session tests；199 是執行總數，不是唯一案例數。
- `git diff --check`：通過。
- 建置使用暫存評估目錄的 `--target-dir` 重用快取；manifest／source
  來自正式 oxdm 工作目錄，依賴來自 crates.io，沒有 path patch。

健康測試透過 OxDM 的 `run_health_check` 與實際 loopback HTTP 驗證
圖片簽章、非圖片拒絕、Digest 標準 qop 及驗證失敗不降級 Basic。
分身測試以不同於 synthetic 預設值的 encoder 名稱，確認拒絕寫入
保留錄製回應、成功寫入使後續讀取反映已提交值、重啟不假裝成功。

### 依賴稽核

- [升級前](oxvif-0.17-audit-before.json)：1 項 vulnerability、5 項 warnings。
- [升級後](oxvif-0.17-audit-after.json)：**0 項 vulnerability、5 項 warnings**。
- advisory DB：`57ad4063bb49c1deb04b6fcee30cfbac6b508474`
  （2026-09-21 更新）。沒有加入 advisory ignore。
- rustls 0.23.38 → 0.23.45，修正 `RUSTSEC-2026-0285`；其相容依賴
  aws-lc-rs 1.18.1、aws-lc-sys 0.45.0、rustls-webpki 0.103.15 一併更新。
  oxvif 帶入 quick-xml 0.42.0；wayland-scanner 仍使用 0.41.0。

保留的五項警告與前次升級相同：fxhash 0.2.1（RUSTSEC-2025-0057）、
paste 1.0.15（RUSTSEC-2024-0436）、proc-macro-error 1.0.4
（RUSTSEC-2024-0370）為 unmaintained；glib 0.18.5
（RUSTSEC-2024-0429）、rand 0.7.3（RUSTSEC-2026-0097）為 unsound。
這些來自現有 desktop／build 依賴，不以 Windows 通過推論 Linux 風險消失，
也不把 audit 無 vulnerability 宣稱為完全無警告。

### 尚未驗收

未操作桌面 GUI、實體攝影機或既有使用者資料；未執行 Linux/macOS
CI／封裝與實際畫面驗收。這些維持上方未勾選項目。主畫面快照下載器、
完整小數 FPS 編輯及 metadata verifier 的上游覆蓋缺口沒有在本次擴充。

## 參考

- [oxvif 0.17.0 release](https://github.com/smiti1642/oxvif/releases/tag/v0.17.0)
- [0.17.0 migration](https://github.com/smiti1642/oxvif/blob/v0.17.0/docs/releases/0.17.0-changelog.md)
- [Recording storage and report identity](https://github.com/smiti1642/oxvif/blob/v0.17.0/docs/replay-storage.md)
- [前次 0.16 升級紀錄](oxvif-0.16-upgrade-plan.md)
