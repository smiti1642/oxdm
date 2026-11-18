# OxDM 指令台命令覆蓋

清單依 `oxvif describe --grammar --output json` 的公開命令路徑建立。`discover` 與 `discover scan` 共用命令 ID。CLI 工作區包含 CLI 的裝置登錄、群組、動態檢視、探索快照及憑證設定檔。直接地址或 OxDM 選取的攝影機才會依位址比對帶入 OxDM 憑證。

目前 70 條公開命令路徑中，69 條可在指令台執行，1 條走外部終端。指令台寫入與刪除會先顯示 CLI 工作區和完整參數供確認。憑證表單經由 stdin 將密碼送入 CLI。儲存帳密時，結果卡片只顯示執行狀態。一次性查詢會顯示裝置回傳的資料。

| 命令 | 路徑 | 範圍或限制 |
| --- | --- | --- |
| `inspect` | 指令台 | 明確輸入位址，使用一次性帳密查詢裝置資訊、媒體設定檔、能力、服務或指定設定檔的 RTSP 網址。 |
| `manage` | 外部終端 | Windows 以固定執行檔和 argv 開啟真正的終端。其他平台提供可複製用法。 |
| `setup` | 指令台 | 需明確目標和 `--id`。用獨立表單確認 CLI 登錄及憑證寫入。 |
| `auth` | 指令台 | 用獨立表單設定 CLI 裝置的系統憑證。 |
| `info` | 指令台 | 以 CLI 本機工作區為準。 |
| `test` | 指令台 | 以 CLI 本機工作區為準。 |
| `profiles` | 指令台 | 以 CLI 本機工作區為準。 |
| `stream` | 指令台 | CLI 儲存裝置快捷命令，需指定 `--profile`。 |
| `snapshot` | 指令台 | CLI 儲存裝置快捷命令，需指定 `--profile`。下載使用 `media snapshot-save`。 |
| `diagnose` | 指令台 | 支援單機 `--share`。一般報告可使用群組或檢視。 |
| `list` | 指令台 | 以 CLI 本機工作區為準。 |
| `devices` | 指令台 | 以 CLI 本機工作區為準。 |
| `groups` | 指令台 | 以 CLI 本機工作區為準。 |
| `views` | 指令台 | 以 CLI 本機工作區為準。 |
| `agent guide` | 指令台 | 以 CLI 本機工作區為準。 |
| `agent prompt` | 指令台 | 以 CLI 本機工作區為準。 |
| `describe` | 指令台 | 可看指令契約。`--grammar` 由 OxDM 內部讀取。 |
| `device add` | 指令台 | 新增 CLI 裝置登錄，需確認。 |
| `device list` | 指令台 | 以 CLI 本機工作區為準。 |
| `device show` | 指令台 | 以 CLI 本機工作區為準。 |
| `device update` | 指令台 | 更新 CLI 裝置登錄，需確認。 |
| `device rename` | 指令台 | 更新 CLI 裝置顯示名稱，需確認。 |
| `device remove` | 指令台 | 移除 CLI 裝置與原生憑證，需確認。 |
| `device import` | 指令台 | 用 `--plan` 預覽，從結果帶入 fingerprint，確認後以 `--apply` 套用。覆寫設定可從 JSON 檔讀取。 |
| `device credential set` | 指令台 | 用獨立表單設定 CLI 裝置的系統憑證。 |
| `device credential delete` | 指令台 | 刪除指定裝置的原生憑證，需確認。 |
| `device credential use-profile` | 指令台 | 變更指定裝置的原生憑證引用，需確認。 |
| `device test` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `device info` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `device capabilities` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `device services` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `device refresh` | 指令台 | 將查詢結果寫回 CLI 登錄，需確認。 |
| `media snapshot-save` | 指令台 | 單機，需 `--profile` 和 `--save`，僅寫入新檔。 |
| `media profiles` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `media stream-uri` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `media snapshot-uri` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `ptz status` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `ptz presets` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `events watch` | 指令台 | 限 OxDM 所選攝影機或直接 `--target`，以同一 ONVIF URL 協調訂閱與停止。 |
| `health` | 指令台 | 以 CLI 的裝置 ID、群組或檢視操作。用 `health check` 可帶入 OxDM 選取的攝影機。 |
| `health check` | 指令台 | 支援 OxDM 選取的攝影機及直接地址。 |
| `group create` | 指令台 | 建立 CLI 群組，需確認。 |
| `group list` | 指令台 | 以 CLI 本機工作區為準。 |
| `group show` | 指令台 | 以 CLI 本機工作區為準。 |
| `group delete` | 指令台 | 刪除 CLI 群組，需確認。 |
| `group member add` | 指令台 | 加入 CLI 群組成員，需確認。 |
| `group member remove` | 指令台 | 移除 CLI 群組成員，需確認。 |
| `view create` | 指令台 | 新增 CLI 動態檢視，需確認。 |
| `view list` | 指令台 | 以 CLI 本機工作區為準。 |
| `view show` | 指令台 | 以 CLI 本機工作區為準。 |
| `view evaluate` | 指令台 | 以 CLI 本機工作區為準。 |
| `view delete` | 指令台 | 刪除 CLI 動態檢視，需確認。 |
| `credential profile set` | 指令台 | 用獨立表單設定 CLI 憑證設定檔。 |
| `credential profile list` | 指令台 | 以 CLI 本機工作區為準。 |
| `credential profile show` | 指令台 | 以 CLI 本機工作區為準。 |
| `credential profile delete` | 指令台 | 刪除原生憑證設定檔，需確認。 |
| `discover` | 指令台 | 與 `discover scan` 同一命令 ID，採暫時性探索。 |
| `discover scan` | 指令台 | 可暫時探索，也可用 `--save` 經確認儲存具名快照。 |
| `discover refresh` | 指令台 | 覆寫 CLI 探索快照，需確認。 |
| `discover enrich` | 指令台 | 使用原生憑證設定檔連線並更新 CLI 探索快照，需確認。 |
| `discover list` | 指令台 | 以 CLI 本機工作區為準。 |
| `discover snapshots` | 指令台 | 以 CLI 本機工作區為準。 |
| `discover remove` | 指令台 | 移除 CLI 探索快照，需確認。 |
| `config export` | 指令台 | 單機，`--save` 寫入新檔。 |
| `config diff` | 指令台 | 單機，`--against` 讀取同一台裝置先前匯出的設定盤點。 |
| `config path` | 指令台 | 以 CLI 本機工作區為準。 |
| `config validate` | 指令台 | 檢查 CLI 本機工作區，支援 `--share`。 |
| `completion` | 指令台 | 以純文字產生指定 Shell 的補全腳本，結果卡片可複製完整腳本。 |
| `use` | 指令台 | 修改 CLI 目前裝置，需確認。 |
| `current` | 指令台 | 以 CLI 本機工作區為準。 |

## Windows 本地驗收

- 套件內 CLI 以本機 MockServer 驗證目標與憑證隔離、報告格式、事件監看和停止、逾時與事件暴增，並比對公開命令覆蓋及三語說明。以測試子程序驗證憑證表單的環境變數、stdin 與輸出隔離，另用唯一 ID 和隔離的 CLI 工作區在 Windows Credential Manager 寫入、讀回並清除一筆合成憑證。Shell 補全測試逐位元組比對可複製的內容與 CLI 原始輸出。
- oxvif 的受控時鐘與腳本測試覆蓋拉取尚未完成、消費端佇列滿載時的續租，以及缺口、恢復、容量限制和訂閱清理。OxDM 測試確認終端模式展開輸出時保留一般模式的收合選擇，舊執行淘汰後也會清理其收合狀態。
- 2026-11-13 的獨立盲測從新版 MSI 解包，在隔離的英文首頁開始。測試者用鍵盤進入指令台，首次執行 `device info` 觸發選機，使用 Tab 和 Enter 選取模擬攝影機後，用保留的草稿查得 `MockCam-1080p` 與結束碼 0。以鍵盤切換終端與卡片、離開並返回指令台後，草稿、所選攝影機與同一次查詢結果都保留。
- 同一輪盲測的 `diagnose --share` 顯示缺少媒體 Profile 及下一步，`media profiles` 查得 `Profile_1`，補上後顯示對應方塊。測試也發現執行按鈕更換時會失去焦點。修正後用新版 MSI 重測，鍵盤執行 `device info` 後能接著從輸入欄開始 `events watch --duration 60s`，再以底部按鈕停止，結果顯示「使用者要求停止」及結束碼 0。
- 新版 MSI 的 `diagnose --share --profile Profile_1` 成功，測試者透過 Windows 原生另存對話框匯出 v1 JSON。檔案顯示 19 項檢查，未包含測試攝影機的位址、名稱、序號或 Profile token。合成帳密遮蔽另由套件測試驗證。
- 複驗中用鍵盤收合 `device info` 卡片並展開，結束碼和原始結果一致。另以 5 分鐘監看驗證在卡片模式按 Esc 離開再進入後，仍能接續原本的監看並正常停止。終端模式按 Esc 可回到卡片模式，原本的執行紀錄仍在。
- 俄文介面實測顯示命令說明、用法和參數描述，輸入 `duration` 可找到俄文的時間單位和範圍。新版 MSI 的獨立盲測在英文介面從 `events watch` 搜尋 `duration`，在參數表單輸入 `45s` 並套用，接著開啟方塊改成 `30s`。無障礙樹顯示 `duration: 30s` 與同步更新的草稿。切成繁中後，確認 `--duration` 的時間單位和 30 秒範例已翻譯，草稿仍保留。
- 另在 Windows WebView2 以隔離的開發版驗證完整表單流程。測試重現快速修改後套用會寫回舊值，修正後確認從 `events watch` 搜尋並加入 `duration: 45s`，立即改成 `duration: 30s` 時，草稿同步變成 `events watch --duration=30s`，焦點回到輸入欄。切換模式後方塊與草稿一致，繁中說明也有顯示時間單位。輸入和執行共用監看最多 60 分鐘的限制。
- Windows WebView2 的俄文測試從未選機的 `diagnose --share` 開啟攝影機選擇，選定本機模擬攝影機後保留草稿。說明側欄可從 390 拉伸至 406 像素，折疊再展開保留寬度，說明區無水平溢出。以 `длительность` 找到 `duration` 並套用值後，可執行及停止監看。另驗證 `diagnose --share` 失敗結束碼 20 與 Profile 指引可收合再展開。同一筆 `events watch` 串流可在終端和卡片模式切換，再停止並顯示「Отмена подписки подтверждена」。
- Windows 自動化回報 `window capture timed out` 和 `coordinate input geometry is unavailable`，盲測使用輔助功能樹及鍵盤。Windows WebView2 實測終端背景為 `#0b0e13`，命令前景為 `#86b9ff`。Chromium 也驗證命令、旗標和值的樣式各有不同顏色。舊版相容測試已用 oxvif 0.16.0 驗證，確認語法探測失敗後仍會顯示版本與命令說明。
- 指令台選機搜尋欄與卡片支援方向鍵移動焦點，Enter 選機後回到草稿。JavaScript 測試覆蓋卡片焦點順序、空清單、Esc 關閉補全後可用於返回，以及組字期間避免送出或離開。
- 打包時先驗證 CLI 的分享報告版本、憑證 stdin 契約、事件停止及外部終端能力。若使用只有舊版指令台能力的執行檔，打包檢查會指出缺少的能力。
- Windows 外部終端測試從輸出重導的父程序啟動子程序，確認 stdin、stdout 和 stderr 均為 TTY，並驗證含空格、引號、反斜線及 Unicode 的 argv 和 oxvif 憑證環境隔離。實際 `manage` 選單操作仍屬於原生驗收。

### 重跑 Windows 獨立盲測

從 OxDM checkout 啟動 MSI 內的程式及本機模擬攝影機。保留啟動腳本直到測試者關閉 OxDM。

```powershell
python scripts/stage-console-cli.py --blind-test target/local-test/Oxdm_0.4.1_x64-console-expansion.msi --mock ../oxvif/target/debug/examples/console_mock.exe
```

啟動器會解包 MSI、驗證內附 CLI、建立暫存的 OxDM 設定及 CLI 工作區，並放入一台名為 `Local test camera` 的攝影機。兩個工作區使用獨立目錄，OxDM 憑證庫也依設定目錄隔離。測試者從無選取的首頁開始。

請把下面的任務交給未看過操作步驟的測試者，記錄每項操作是否完成及遇到的阻礙。

1. 在未選取攝影機時執行 `device info`，使用本機測試攝影機完成查詢並確認草稿仍在。
2. 瀏覽英文、繁中、俄文說明。加入並編輯參數方塊，展開或收合結果。切換黑底終端後回到卡片模式，再離開並重新進入指令台。檢查按鈕和 Esc 路徑，以及草稿與紀錄是否保留。
3. 執行 `diagnose --share`，另存到自行選擇的測試檔案，確認檔案可讀且沒有攝影機位址。
4. 在本機攝影機執行 `events watch`，確認事件計數更新。切換模式、離開並返回指令台，確認同一監看仍在進行，再停止並確認訂閱清理狀態。
