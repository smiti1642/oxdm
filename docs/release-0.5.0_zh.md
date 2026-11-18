# OxDM 0.5.0

發行候選版。[English](release-0.5.0.md)

- **在 OxDM 內執行 oxvif 指令。** 指令台支援語法上色、指令及參數補全，
  並提供英文、繁中、俄文說明。可切換黑底終端或可折疊的結果卡片。
  選取攝影機後即可執行 `device info`、`media profiles` 等指令。
- **依位址查詢攝影機。** 執行 `inspect 192.168.1.100` 後，在表單輸入
  本次使用的帳密。用 `--read profiles` 取得設定檔 token，再用
  `--read stream-uri --profile TOKEN` 查即時 RTSP 網址。
- **分享診斷報告。** `diagnose --share` 匯出的 JSON 會省略帳密、
  裝置位址及本機路徑。
- **事件監看斷線後可恢復。** 裝置的事件頁會重連，並在相容裝置補讀歷史
  事件。只有即時狀態時，會請求目前的狀態快照並標記缺口。也可在指令台
  啟動或停止 `events watch`。
- **Media2 與 Quirks 改善。** 可編輯 Media2 裝置宣告的小數 FPS。
  Quirks 會保留重複鍵錄製項目的個別結果，並標示無法確定的基準比較。

Windows 本地驗收涵蓋 MSI 的內附 CLI 與可攜 ZIP 的執行檔。實機驗收涵蓋無歷史儲存時的事件恢復，
以及 GeoVision 攝影機的一次性查詢。歷史事件補讀仍待相容實機驗收。 [完整變更](../CHANGELOG.md)

OxDM 0.5.0 的目標依賴為 oxvif 0.18.0。HTTPS 快照會驗證憑證與主機名稱。
若攝影機使用私有 CA，請先讓作業系統信任該 CA。

發行目標包含 Windows x86-64 MSI／可攜 ZIP、Apple Silicon DMG 和
Ubuntu x86-64 DEB。使用 Windows 可攜版時，把兩個執行檔放在同一目錄。
原始碼安裝使用 `oxvif-device-manager` 套件。
