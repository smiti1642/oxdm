# OxDM — 施工藍圖（對照 ODM v2.2.250）

以 ONVIF Device Manager 為參考，整理其所有 ONVIF API 呼叫及對應 UI 功能，作為 OxDM 的開發藍圖。

> **oxvif 狀態**欄說明：
> - ✓ 已實作
> - ~ 部分實作
> - — 尚未實作
>
> oxvif 欄已於 **2026-05-29** 對照實際 API（`OnvifSession` + `src/client/`）重新校正。
> 重點：Analytics / Receiver / Action Engine / 憑證**整個服務未實作**；
> Recording / Search / Replay 只做了 CRUD 與 `FindRecordings` / `GetReplayUri`，
> 各種 `*Configuration` get/set 與進階搜尋（Events / Metadata / PTZ position）尚未實作。

---

## 優先順序建議

| 階段 | 功能 | 說明 |
|------|------|------|
| P1 | 裝置探索 + 基本資訊 | 最基礎，也是 oxvif 最完整的部分 |
| P1 | 即時影像（RTSP URI） | 核心功能，用戶最先需要 |
| P2 | PTZ 控制 | 有 PTZ 的設備必備 |
| P2 | 影像設定（亮度/對比） | 常用設定 |
| P3 | 網路設定 / 使用者管理 | 進階管理功能 |
| P3 | 事件 / 告警 | push-subscribe 已實作 |
| P4 | 錄影 / 搜尋 / 回放 | NVR 功能 |
| P4 | 分析 / Analytics | 複雜，少數設備支援 |

---

## 1. WS-Discovery

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `probe()` | UDP multicast 掃描裝置 | 主畫面：掃描網路 | ✓ |
| `listen()` | 被動監聽 Hello/Bye | 裝置上下線通知 | ✓ |

---

## 2. Device Service（devicemgmt.wsdl）

### 2-1 裝置資訊（讀取）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetDeviceInformation` | 取得廠商、型號、韌體、序號 | 裝置識別頁 | ✓ |
| `GetCapabilities` | 取得所有服務能力 | 判斷支援哪些服務 | ✓ |
| `GetServices` | 取得服務清單 | 服務探索 | ✓ |
| `GetScopes` | 取得 discovery scopes | 名稱 / 位置顯示 | ✓ |
| `GetWsdlUrl` | 取得 WSDL URL | 服務資訊 | — |

### 2-2 網路設定

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetNetworkInterfaces` | 取得網路介面 / IP | 網路設定頁 | ✓ |
| `SetNetworkInterfaces` | 設定 IP / 遮罩 | 網路設定頁 | ✓ |
| `GetHostname` | 取得主機名稱 | 網路設定頁 | ✓ |
| `SetHostname` | 設定主機名稱 | 網路設定頁 | ✓ |
| `SetHostnameFromDHCP` | 從 DHCP 取得主機名稱 | 網路設定頁 | — |
| `GetDNS` | 取得 DNS 設定 | 網路設定頁 | ✓ |
| `SetDNS` | 設定 DNS | 網路設定頁 | ✓ |
| `GetNTP` | 取得 NTP 設定 | 網路設定頁 | ✓ |
| `SetNTP` | 設定 NTP | 網路設定頁 | ✓ |
| `GetNetworkDefaultGateway` | 取得預設閘道 | 網路設定頁 | ✓ |
| `SetNetworkDefaultGateway` | 設定預設閘道 | 網路設定頁 | ✓ |
| `GetNetworkProtocols` | 取得協定 / 埠號（HTTP/RTSP） | 網路設定頁 | ✓ |
| `SetNetworkProtocols` | 設定協定 / 埠號 | 網路設定頁 | ✓ |
| `GetZeroConfiguration` | 取得 Zero-conf 設定 | 網路設定頁 | — |
| `SetZeroConfiguration` | 設定 Zero-conf | 網路設定頁 | — |
| `GetDynamicDNS` | 取得 DDNS 設定 | 網路設定頁 | — |
| `SetDynamicDNS` | 設定 DDNS | 網路設定頁 | — |
| `GetIPAddressFilter` | 取得 IP 過濾設定 | 安全設定頁 | — |
| `SetIPAddressFilter` | 設定 IP 過濾 | 安全設定頁 | — |
| `GetDiscoveryMode` | 取得 Discovery 模式 | 網路設定頁 | ✓ |
| `SetDiscoveryMode` | 設定 Discovery 模式 | 網路設定頁 | ✓ |
| `GetRemoteDiscoveryMode` | 取得遠端 Discovery 模式 | 網路設定頁 | — |
| `SetRemoteDiscoveryMode` | 設定遠端 Discovery 模式 | 網路設定頁 | — |
| `GetDPAddresses` | 取得 Discovery Proxy 地址 | 網路設定頁 | — |
| `SetDPAddresses` | 設定 Discovery Proxy 地址 | 網路設定頁 | — |

### 2-3 系統時間

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetSystemDateAndTime` | 取得系統時間 / 時區 | 時間設定頁 | ✓ |
| `SetSystemDateAndTime` | 設定系統時間 | 時間設定頁 | ✓ |

### 2-4 使用者管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetUsers` | 取得使用者清單 | 使用者管理頁 | ✓ |
| `CreateUsers` | 新增使用者 | 使用者管理頁 | ✓ |
| `SetUser` | 更新使用者 | 使用者管理頁 | ✓ |
| `DeleteUsers` | 刪除使用者 | 使用者管理頁 | ✓ |

### 2-5 Scopes（識別名稱）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `SetScopes` | 設定 scopes（名稱 / 位置） | 識別頁 | ✓ |
| `AddScopes` | 新增 scopes | 識別頁 | — |
| `RemoveScopes` | 移除 scopes | 識別頁 | — |

### 2-6 IO / 繼電器

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetRelayOutputs` | 取得繼電器輸出清單 | IO 控制頁 | ✓ |
| `SetRelayOutputSettings` | 設定繼電器參數 | IO 控制頁 | ✓ |
| `SetRelayOutputState` | 控制繼電器開關 | IO 控制頁 | ✓ |

### 2-7 憑證管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetCertificates` | 取得憑證清單 | 安全設定頁 | — |
| `GetCertificatesStatus` | 取得憑證狀態 | 安全設定頁 | — |
| `CreateCertificate` | 建立自簽憑證 | 安全設定頁 | — |
| `DeleteCertificates` | 刪除憑證 | 安全設定頁 | — |
| `SetCertificatesStatus` | 啟用 / 停用憑證 | 安全設定頁 | — |
| `LoadCertificates` | 載入憑證 | 安全設定頁 | — |
| `GetPkcs10Request` | 產生 CSR | 安全設定頁 | — |
| `GetAccessPolicy` | 取得存取政策 | 安全設定頁 | — |
| `SetAccessPolicy` | 設定存取政策 | 安全設定頁 | — |
| `SetClientCertificateMode` | 啟用用戶端憑證驗證 | 安全設定頁 | — |

### 2-8 系統維護

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `SystemReboot` | 重新開機 | 維護頁 | ✓ |
| `SetSystemFactoryDefault` | 恢復出廠設定 | 維護頁 | ✓ |
| `GetSystemUris` | 取得備份/日誌/支援資訊下載 URI | 維護頁 | ✓ |
| `GetSystemLog` | 取得系統日誌 | 維護 / 診斷頁 | ✓ |
| `StartFirmwareUpgrade` | 開始韌體升級流程（上傳 URI）| 維護頁 | ✓ |
| `StartSystemRestore` | 開始還原流程（上傳 URI）| 維護頁 | ✓ |
| `GetSystemBackup` | 取得系統備份檔（MTOM 附件）| 維護頁 | — |
| `RestoreSystem` | 從備份還原（MTOM 附件）| 維護頁 | — |
| `UpgradeSystemFirmware` | 升級韌體（已棄用，MTOM 附件）| 維護頁 | — |
| `GetSystemSupportInformation` | 取得支援資訊 | 診斷頁 | — |

> 註：備份**下載**走 `GetSystemUris` → `SystemBackupUri`（HTTP GET）；韌體升級與還原走
> `StartFirmwareUpgrade` / `StartSystemRestore` 的上傳 URI（HTTP POST）。三者皆避開
> oxvif SOAP-only transport 無法產生的 MTOM 附件，故對應的 `GetSystemBackup` /
> `RestoreSystem` / `UpgradeSystemFirmware`（附件版）維持未實作。

---

## 3. Media Service（media.wsdl）

### 3-1 Profile 管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetProfiles` | 取得所有 profile | 即時影像、Profile 管理頁 | ✓ |
| `GetProfile` | 取得單一 profile | 即時影像、PTZ、影像設定 | ✓ |
| `CreateProfile` | 建立 profile | Profile 管理頁 | ✓ |
| `DeleteProfile` | 刪除 profile | Profile 管理頁 | ✓ |
| `GetVideoSources` | 取得影像輸入清單 | Profile 管理頁 | ✓ |
| `GetAudioSources` | 取得音訊輸入清單 | Profile 管理頁 | ✓ |

### 3-2 Profile 設定（Add / Remove 各設定）

| 方法 | 說明 | oxvif 狀態 |
|------|------|-----------|
| `AddVideoSourceConfiguration` | 加入影像來源設定 | ✓ |
| `RemoveVideoSourceConfiguration` | 移除影像來源設定 | ✓ |
| `AddVideoEncoderConfiguration` | 加入影像編碼設定 | ✓ |
| `RemoveVideoEncoderConfiguration` | 移除影像編碼設定 | ✓ |
| `AddAudioSourceConfiguration` | 加入音訊來源設定 | — |
| `RemoveAudioSourceConfiguration` | 移除音訊來源設定 | — |
| `AddAudioEncoderConfiguration` | 加入音訊編碼設定 | — |
| `RemoveAudioEncoderConfiguration` | 移除音訊編碼設定 | — |
| `AddPTZConfiguration` | 加入 PTZ 設定 | — |
| `RemovePTZConfiguration` | 移除 PTZ 設定 | — |
| `AddMetadataConfiguration` | 加入 Metadata 設定 | — |
| `RemoveMetadataConfiguration` | 移除 Metadata 設定 | — |
| `AddVideoAnalyticsConfiguration` | 加入 Analytics 設定 | — |
| `RemoveVideoAnalyticsConfiguration` | 移除 Analytics 設定 | — |

> 註：上表 Add/Remove 為 Media1，oxvif 僅實作 Video Source / Video Encoder 兩種。
> 其餘型別（PTZ / Metadata / Audio）改由 Media2 統一的 `AddConfiguration` /
> `RemoveConfiguration` 處理（oxvif 已實作），不再走 Media1 的個別 Add/Remove。

### 3-3 影像編碼設定

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetVideoEncoderConfigurations` | 取得所有影像編碼設定 | 影像設定頁 | ✓ |
| `GetVideoEncoderConfiguration` | 取得單一影像編碼設定 | 影像設定頁 | ✓ |
| `SetVideoEncoderConfiguration` | 更新解析度 / 碼率 / FPS / 編碼 | 影像設定頁 | ✓ |
| `GetVideoEncoderConfigurationOptions` | 取得可用選項範圍 | 影像設定頁 | ✓ |
| `GetCompatibleVideoEncoderConfigurations` | 取得 Profile 相容編碼設定 | Profile 設定頁 | — |
| `GetGuaranteedNumberOfVideoEncoderInstances` | 取得編碼器實例數上限 | 影像設定頁 | — |

### 3-4 影像來源設定

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetVideoSourceConfigurations` | 取得所有影像來源設定 | 設定頁 | ✓ |
| `GetVideoSourceConfiguration` | 取得單一影像來源設定 | 設定頁 | ✓ |
| `SetVideoSourceConfiguration` | 更新影像來源設定 | 影像設定頁 | ✓ |
| `GetVideoSourceConfigurationOptions` | 取得影像來源選項 | 影像設定頁 | ✓ |
| `GetCompatibleVideoSourceConfigurations` | 取得 Profile 相容來源 | Profile 設定頁 | — |

### 3-5 音訊設定

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetAudioEncoderConfigurations` | 取得音訊編碼設定 | 音訊設定頁 | ✓ |
| `GetAudioEncoderConfiguration` | 取得單一音訊編碼設定 | 音訊設定頁 | ✓ |
| `SetAudioEncoderConfiguration` | 更新音訊編碼 | 音訊設定頁 | ✓ |
| `GetAudioEncoderConfigurationOptions` | 取得音訊編碼選項 | 音訊設定頁 | ✓ |
| `GetAudioSourceConfigurations` | 取得音訊來源設定 | 設定頁 | ✓ |
| `GetAudioSourceConfiguration` | 取得單一音訊來源設定 | 設定頁 | ✓ |
| `SetAudioSourceConfiguration` | 更新音訊來源設定 | 音訊設定頁 | — |
| `GetAudioSourceConfigurationOptions` | 取得音訊來源選項 | 音訊設定頁 | — |
| `GetCompatibleAudioEncoderConfigurations` | 取得 Profile 相容音訊編碼 | Profile 設定頁 | — |
| `GetCompatibleAudioSourceConfigurations` | 取得 Profile 相容音訊來源 | Profile 設定頁 | — |

### 3-6 Metadata 設定

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetMetadataConfigurations` | 取得 Metadata 設定 | Metadata 設定頁 | ✓ |
| `GetMetadataConfiguration` | 取得單一 Metadata 設定 | Metadata 設定頁 | ✓ |
| `SetMetadataConfiguration` | 更新 Metadata 設定 | Metadata 設定頁 | ✓ |
| `GetMetadataConfigurationOptions` | 取得 Metadata 選項 | Metadata 設定頁 | ✓ |
| `GetCompatibleMetadataConfigurations` | 取得 Profile 相容 Metadata | Profile 設定頁 | — |

### 3-7 串流 / 快照

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetStreamUri` | 取得 RTSP 串流 URL | 即時影像頁 | ✓ |
| `GetSnapshotUri` | 取得快照 JPEG URL | 即時影像頁 | ✓ |
| `SetSynchronizationPoint` | 設定影像同步點 | 即時影像頁 | — |
| `StartMulticastStreaming` | 開始 Multicast 串流 | 影像播放頁 | — |
| `StopMulticastStreaming` | 停止 Multicast 串流 | 影像播放頁 | — |

### 3-8 Video Analytics（Media 內）

| 方法 | 說明 | oxvif 狀態 |
|------|------|-----------|
| `GetVideoAnalyticsConfigurations` | 取得 Analytics 設定 | — |
| `GetVideoAnalyticsConfiguration` | 取得單一 Analytics 設定 | — |
| `SetVideoAnalyticsConfiguration` | 更新 Analytics 設定 | — |
| `GetCompatibleVideoAnalyticsConfigurations` | 取得相容 Analytics | — |

---

## 4. PTZ Service（ptz.wsdl）

### 4-1 設定

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetNodes` | 取得 PTZ 節點清單 | PTZ 控制頁 | ✓ |
| `GetNode` | 取得單一 PTZ 節點 | PTZ 控制頁 | ✓ |
| `GetConfigurations` | 取得 PTZ 設定清單 | Profile 設定頁 | ✓ |
| `GetConfiguration` | 取得單一 PTZ 設定 | PTZ 控制頁 | ✓ |
| `SetConfiguration` | 更新 PTZ 設定 | PTZ 設定頁 | ✓ |
| `GetConfigurationOptions` | 取得 PTZ 選項 | PTZ 設定頁 | ✓ |

### 4-2 Preset（預設位置）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetPresets` | 取得 Preset 清單 | PTZ 控制頁 | ✓ |
| `SetPreset` | 建立 / 更新 Preset | PTZ 控制頁 | ✓ |
| `RemovePreset` | 刪除 Preset | PTZ 控制頁 | ✓ |
| `GotoPreset` | 移動至 Preset | PTZ 控制頁 | ✓ |

### 4-3 移動控制

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `AbsoluteMove` | 移動至絕對座標 | PTZ 控制頁 | ✓ |
| `RelativeMove` | 相對位移 | PTZ 控制頁 | ✓ |
| `ContinuousMove` | 持續移動（搖桿模式） | PTZ 控制頁 | ✓ |
| `Stop` | 停止移動 | PTZ 控制頁 | ✓ |

### 4-4 Home 位置

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GotoHomePosition` | 回到 Home 位置 | PTZ 控制頁 | ✓ |
| `SetHomePosition` | 設定目前位置為 Home | PTZ 控制頁 | ✓ |

### 4-5 狀態

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetStatus` | 取得目前 PTZ 座標 / 狀態 | PTZ 控制頁 | ✓ |
| `SendAuxiliaryCommand` | 發送輔助指令 | PTZ 控制頁 | ✓ |

---

## 5. Imaging Service（imaging.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetImagingSettings` | 取得亮度 / 對比 / 飽和度等 | 影像調整頁 | ✓ |
| `SetImagingSettings` | 更新影像設定 | 影像調整頁 | ✓ |
| `GetOptions` | 取得各參數可調範圍 | 影像調整頁 | ✓ |
| `GetStatus` | 取得目前 Imaging 狀態 | 影像調整頁 | ✓ |
| `GetMoveOptions` | 取得對焦移動選項 | 影像調整頁 | ✓ |
| `Move` | 控制對焦馬達（自動 / 手動） | 影像調整頁 | ✓ |
| `Stop` | 停止對焦移動 | 影像調整頁 | ✓ |

---

## 6. Event / Notification Service（events.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetEventProperties` | 取得可訂閱事件主題 | Metadata 設定 / Action 管理 | ✓ |
| `CreatePullPointSubscription` | 建立 Pull 訂閱（輪詢模式） | 事件訂閱 | ✓ |
| `PullMessages` | 拉取待收事件 | 事件輪詢 | ✓ |
| `Subscribe` | 建立 Push 訂閱（裝置主動 POST） | 事件通知設定 | ✓ |
| `Renew` | 更新訂閱有效期 | 訂閱管理 | ✓ |
| `Unsubscribe` | 取消訂閱 | 訂閱管理 | ✓ |
| `SetSynchronizationPoint` | 設定事件同步點 | 事件處理 | ✓ |
| `GetCurrentMessage` | 取得目前事件訊息 | 事件查詢 | — |

---

## 7. Analytics Service（analytics.wsdl）

### 7-1 Analytics 模組

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetSupportedAnalyticsModules` | 取得支援的模組類型 | Analytics 頁 | — |
| `GetAnalyticsModules` | 取得目前模組 | Analytics 頁 | — |
| `CreateAnalyticsModules` | 建立模組（移動偵測等） | Analytics 頁 | — |
| `ModifyAnalyticsModules` | 更新模組設定 | Analytics 頁 | — |
| `DeleteAnalyticsModules` | 刪除模組 | Analytics 頁 | — |

### 7-2 規則引擎

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetSupportedRules` | 取得支援的規則類型 | Analytics 頁 | — |
| `GetRules` | 取得目前規則 | Analytics 頁 | — |
| `CreateRules` | 建立規則（事件觸發條件） | Analytics 頁 | — |
| `ModifyRules` | 更新規則 | Analytics 頁 | — |
| `DeleteRules` | 刪除規則 | Analytics 頁 | — |

---

## 8. Recording Service（recording.wsdl）

### 8-1 錄影管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetRecordings` | 取得錄影清單 | 錄影管理頁 | ✓ |
| `CreateRecording` | 建立錄影 | 錄影管理頁 | ✓ |
| `GetRecordingConfiguration` | 取得錄影設定 | 錄影設定頁 | — |
| `SetRecordingConfiguration` | 更新錄影設定 | 錄影設定頁 | — |
| `DeleteRecording` | 刪除錄影 | 錄影管理頁 | ✓ |

### 8-2 Track 管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `CreateTrack` | 在錄影中建立 Track | 錄影設定頁 | ✓ |
| `GetTrackConfiguration` | 取得 Track 設定 | 錄影設定頁 | — |
| `SetTrackConfiguration` | 更新 Track 設定 | 錄影設定頁 | — |
| `DeleteTrack` | 刪除 Track | 錄影管理頁 | ✓ |

### 8-3 錄影 Job

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetRecordingJobs` | 取得 Job 清單 | 錄影 Job 管理頁 | ✓ |
| `CreateRecordingJob` | 建立 Job | 錄影 Job 管理頁 | ✓ |
| `GetRecordingJobConfiguration` | 取得 Job 設定 | 錄影 Job 設定頁 | — |
| `SetRecordingJobConfiguration` | 更新 Job 設定 | 錄影 Job 設定頁 | — |
| `SetRecordingJobMode` | 設定 Job 模式（開始 / 停止） | 錄影 Job 控制 | ✓ |
| `GetRecordingJobState` | 取得 Job 狀態 | 錄影 Job 監控 | ✓ |
| `DeleteRecordingJob` | 刪除 Job | 錄影 Job 管理頁 | ✓ |

---

## 9. Search Service（search.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `FindRecordings` | 搜尋錄影 | 錄影搜尋頁 | ✓ |
| `GetRecordingSearchResults` | 取得搜尋結果 | 錄影搜尋頁 | ✓ |
| `EndSearch` | 終止搜尋 | 搜尋管理 | ✓ |
| `GetRecordingSummary` | 取得錄影摘要 | 錄影搜尋頁 | — |
| `GetRecordingInformation` | 取得錄影詳細資訊 | 錄影搜尋頁 | — |
| `GetMediaAttributes` | 取得媒體屬性 | 錄影搜尋頁 | — |
| `FindEvents` | 搜尋事件 | 事件搜尋頁 | — |
| `GetEventSearchResults` | 取得事件搜尋結果 | 事件搜尋頁 | — |
| `FindMetadata` | 搜尋 Metadata | Metadata 搜尋頁 | — |
| `GetMetadataSearchResults` | 取得 Metadata 搜尋結果 | Metadata 搜尋頁 | — |
| `FindPTZPosition` | 搜尋 PTZ 位置 | PTZ 搜尋頁 | — |
| `GetPTZPositionSearchResults` | 取得 PTZ 搜尋結果 | PTZ 搜尋頁 | — |
| `GetSearchState` | 取得搜尋狀態 | 搜尋狀態顯示 | — |

---

## 10. Replay Service（replay.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetReplayUri` | 取得回放串流 URL | 影像回放頁 | ✓ |
| `GetReplayConfiguration` | 取得回放設定 | 回放設定頁 | — |
| `SetReplayConfiguration` | 更新回放設定 | 回放設定頁 | — |

---

## 11. Display Service（display.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetLayout` | 取得顯示排版 | 顯示設定頁 | — |
| `SetLayout` | 設定顯示排版 | 顯示設定頁 | — |
| `GetDisplayOptions` | 取得顯示選項 | 顯示設定頁 | — |
| `GetPaneConfigurations` | 取得面板設定清單 | 顯示排版頁 | — |
| `GetPaneConfiguration` | 取得單一面板設定 | 顯示設定頁 | — |
| `SetPaneConfigurations` | 更新面板設定 | 顯示排版頁 | — |
| `SetPaneConfiguration` | 更新單一面板 | 顯示設定頁 | — |
| `CreatePaneConfiguration` | 新增面板 | 顯示排版頁 | — |
| `DeletePaneConfiguration` | 刪除面板 | 顯示管理頁 | — |

---

## 12. Receiver Service（receiver.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetReceivers` | 取得 RTP Receiver 清單 | Receiver 管理頁 | — |
| `GetReceiver` | 取得單一 Receiver 設定 | Receiver 設定頁 | — |
| `CreateReceiver` | 建立 Receiver | Receiver 管理頁 | — |
| `ConfigureReceiver` | 更新 Receiver 設定 | Receiver 管理頁 | — |
| `SetReceiverMode` | 設定 Receiver 模式 | Receiver 控制 | — |
| `GetReceiverState` | 取得 Receiver 狀態 | Receiver 監控 | — |
| `DeleteReceiver` | 刪除 Receiver | Receiver 管理頁 | — |

---

## 13. Action Engine Service（actionengine.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetSupportedActions` | 取得支援的 Action 類型 | Action 管理頁 | — |
| `GetActions` | 取得目前 Action 清單 | Action 管理頁 | — |
| `CreateActions` | 建立 Action | Action 管理頁 | — |
| `ModifyActions` | 更新 Action 設定 | Action 管理頁 | — |
| `DeleteActions` | 刪除 Action | Action 管理頁 | — |
| `GetActionTriggers` | 取得 Trigger 清單 | Trigger 管理頁 | — |
| `CreateActionTriggers` | 建立 Trigger | Trigger 管理頁 | — |
| `ModifyActionTriggers` | 更新 Trigger | Trigger 管理頁 | — |
| `DeleteActionTriggers` | 刪除 Trigger | Trigger 管理頁 | — |

---

## UI 功能組合對照表

| 功能頁面 | 需要呼叫的 API 組合 |
|---------|-------------------|
| **即時影像** | GetProfiles → GetStreamUri → (GetSnapshotUri) |
| **裝置識別** | GetDeviceInformation + GetCapabilities + GetScopes + GetNetworkInterfaces |
| **Profile 管理** | GetProfiles + GetVideoSources + GetAudioSources + CreateProfile / DeleteProfile |
| **Profile 設定** | GetCompatibleXxxConfigurations → AddXxxConfiguration / RemoveXxxConfiguration |
| **PTZ 控制面板** | GetProfile → GetConfiguration → GetNode → GetPresets → GetStatus → ContinuousMove / Stop / GotoPreset |
| **影像設定** | GetProfile → GetVideoEncoderConfiguration → GetVideoEncoderConfigurationOptions → SetVideoEncoderConfiguration |
| **影像調整（Imaging）** | GetImagingSettings → GetOptions → SetImagingSettings → (Move / Stop) |
| **網路設定** | GetNetworkInterfaces + GetDNS + GetNTP + GetNetworkDefaultGateway + GetNetworkProtocols + Set 對應方法 |
| **時間設定** | GetSystemDateAndTime → SetSystemDateAndTime |
| **使用者管理** | GetUsers → CreateUsers / SetUser / DeleteUsers |
| **事件監控** | GetEventProperties → CreatePullPointSubscription → PullMessages (輪詢) |
| **推播訂閱** | Subscribe → (裝置 POST 到本地 listener) → Unsubscribe |
| **Analytics** | GetCapabilities → GetAnalyticsModules → GetSupportedAnalyticsModules → CreateAnalyticsModules → GetRules → CreateRules |
| **錄影管理** | GetRecordings → CreateRecording → CreateTrack → CreateRecordingJob → SetRecordingJobMode |
| **影像搜尋** | FindRecordings → GetRecordingSearchResults → GetReplayUri |
| **維護** | GetSystemBackup / SystemReboot / UpgradeSystemFirmware / SetSystemFactoryDefault |

---

*最後更新：2026-04-06，對照 ODM v2.2.250 原始碼分析*

---

## 附錄：OxDM UI 設計決策（對照 ODM）

> 目標：讓 ODM 使用者無痛轉移，同時改善可維護性與使用體驗。

### 保留現有三欄架構，不搬 TreeView

ODM 用單一樹狀元件混合「裝置清單」與「功能導覽」，裝置多時展開節點會撐長清單、難以切換。
OxDM 拆成 DeviceList（左側裝置清單）+ DevicePanel（選中裝置的功能導覽）+ MainContent（右側內容區），
資訊完全對應但更清晰，且 Dioxus 不需自建遞迴 TreeView 元件。

### 內容區加入 Tab Bar（裝置設定分組）

ODM 在裝置資訊頁用 tab 切換 Identification / Network / DateTime / Maintenance。
OxDM 採用相同模式：將 DevicePanel 的 General 區五項（Identification / Network / Time / Users / Maintenance）
合併為一個「Settings」視圖，內部用 tab 切換。Events 獨立保留（即時日誌，非設定）。

### 不加獨立工具列

ODM 的 Discover / Add / Remove 獨占一整行，功能過少。
OxDM 的 Scan / Add 按鈕放在 sidebar footer，Topbar 保留搜尋與全域功能（設定、主題、說明），空間利用更好。

### 屬性表格加 zebra striping + 值可複製

ODM 的兩欄 key-value 表格直接採用，額外加上：
- 交替行底色提高可讀性
- 值欄可選取複製（ODM 做不好的地方）
- 分組標題將屬性分群

### 不做可拖曳分隔條

Dioxus 無原生 splitter，自製拖曳邏輯維護成本高。固定寬度 + 合理預設值即可。

### 新增底部狀態列

ODM 底部顯示「Connected to 3 devices」「Scanning...」，成本低但回饋感強。
OxDM 新增 StatusBar 元件顯示裝置數量、掃描狀態、錯誤訊息。

### 決策摘要

| ODM 模式 | 決定 | 理由 |
|---------|------|------|
| TreeView 導覽 | 不搬 | 兩欄設計更清晰，免建遞迴元件 |
| 內容區 Tab Bar | 採用 | 將相關設定分組，減少導覽項目數 |
| 獨立工具列 | 不搬 | Topbar + sidebar footer 已足夠 |
| 屬性表格 | 採用 | 加 zebra striping + 可複製值 |
| 可拖曳分隔條 | 不搬 | 維護成本高，固定寬度即可 |
| 底部狀態列 | 新增 | 成本低、回饋感強 |

*更新：2026-04-14*

---

## 附錄：商業級桌面程式基礎建設

> 以下列出 OxDM 作為成熟商業產品所需的基礎建設，依優先順序排列。

### P0 — 沒有就不能用 ✓ 已完成

#### 憑證管理 ✓

與 ODM 相同，掃描發現的裝置統一使用全域帳密。
手動新增（Manual Add）的裝置可以選填獨立帳密：

- **全域帳密** — 設定頁設定一組，所有掃描到的裝置共用
- **手動新增裝置** — Add 對話框提供選填帳密欄位，沒填則 fallback 到全域帳密
- **優先順序**：手動新增時的帳密（若有）> 全域帳密
- **儲存方式**：目前存在記憶體（TODO: 整合系統 keychain）

實作：`GlobalCredentialsDialog`（齒輪按鈕）+ `AddDeviceDialog`（＋ Add 按鈕）
`Ctx::credentials_for(device)` 處理優先順序邏輯

#### Toast / 通知系統 ✓

- 四種等級：Success（綠）、Info（藍）、Warning（黃）、Error（紅）
- 4 秒自動消失 + 可手動關閉
- 右下角疊加顯示，入場動畫
- `ctx.push_toast(level, message)` 便捷方法

實作：`ToastContainer` + `ToastItem`（`src/components/toast.rs`）

#### 確認對話框 ✓

- 半透明遮罩，點外面關閉
- `dangerous: true` 時確認按鈕變紅
- `ctx.dialog.set(Some(ConfirmDialog { ... }))` 觸發

實作：`ConfirmDialogModal`（`src/components/dialog.rs`）

危險操作必須二次確認，否則視為 bug：

- 恢復出廠設定
- 刪除使用者
- 系統重開機
- 韌體升級
- 移除裝置

### P1 — 沒有就難用

#### 設定持久化

- 主題、語言、視窗大小/位置 → 存到 `~/.oxdm/config.toml`
- 啟動時還原上次狀態

#### 裝置持久化

- 已知裝置清單存到本地（`~/.oxdm/devices.toml`）
- 啟動時載入上次已知裝置，背景重新掃描更新狀態
- 手動新增的裝置不會因為掃描不到而消失

#### 連線狀態心跳

- 定期偵測每台裝置的連線狀態（每 30 秒一次）
- 即時更新上線/離線狀態
- 離線裝置顯示最後已知資訊，不是空白

#### 右鍵選單

ODM 使用者的肌肉記憶：

- 裝置卡片右鍵 → 複製 IP、移除裝置、重新連線
- 屬性表格右鍵 → 複製值、複製整行
- Tab 標題右鍵 → 重新整理

### P2 — 沒有就不專業

#### 鍵盤快捷鍵

- `Ctrl/Cmd + F` → 搜尋裝置
- `F5` → 掃描網路
- `Esc` → 關閉對話框 / 返回 Welcome
- `↑ ↓` → 切換裝置

#### 應用日誌

- 記錄到檔案（`~/.oxdm/logs/`）
- 日誌等級可調（Debug / Info / Warn / Error）
- ONVIF 原始 SOAP 封包可選開啟記錄（ODM 有此功能，進階排錯用）

#### 關於視窗

- OxDM 版本號
- oxvif 版本
- 系統資訊（OS、架構）
- 授權條款
- GitHub 連結

### P3 — 成熟產品標配

#### 自動更新

- 啟動時檢查 GitHub Releases 有無新版
- 顯示更新通知 + 下載連結
- 可選整合 Tauri updater 做靜默更新

#### 安裝包與簽名

- macOS `.dmg` + notarization
- Windows `.msi` / `.exe` + code signing
- Linux `.AppImage` / `.deb`
- 自訂應用圖標
- CI/CD 自動建置所有平台（GitHub Actions）

#### 批次操作與分組

- 多選裝置，同時設定時間 / 重開機 / 升級韌體
- 裝置分組 / 標籤（例如「一樓」「戶外」）
- 進階篩選（按廠牌、型號、狀態）

#### 效能

- 虛擬捲動（裝置清單 100+ 台時）
- 背景任務不阻塞 UI
- 惰性載入（切到 tab 時才拉資料）

*更新：2026-04-14*

---

## 附錄：ODM 對標缺口（2026-05-29 快照）

> 自 2026-04-21 快照以來，Tier 1 全數完成，Tier 2 的功能項也大致清空。
> 目前距離「正式發布」的真正阻擋已不在功能面，而在**發行 / 封裝基礎建設**
> （見最下方「發布前阻擋項」）。

### Tier 1 — 管理基礎 ✅ 完成

- ✅ Identification 讀寫（Set Scopes：裝置名稱 + 位置）
- ✅ Time 讀寫（SetSystemDateAndTime + 從 PC 同步 + 時區下拉 / 自訂 + DST + 活動秒針）
- ✅ PTZ Preset CRUD（Set / Goto / Remove）
- ✅ **Users CRUD**（Create / SetUser / Delete 對話框，`views/settings/users.rs`）
- ✅ **Network 寫入**（Hostname / Interfaces / DNS / NTP / DefaultGateway /
  NetworkProtocols，`views/settings/network.rs`）
- ✅ **Maintenance**（重開機 + 出廠重設，皆 confirm-gated）
- ✅ **帳密進系統 keychain**（原 P0 的「TODO: 整合 keychain」已解決，單一 JSON blob）
- ✅ **OSD CRUD**（讀 / 建立 / 更新 / 刪除，依相機 OSDOptions 配額，`views/osd.rs`）
- ✅ **Profile 建立 / 刪除**（`device_panel.rs`）

### Tier 2 — 操作體驗 ✅ 大致完成

- ✅ **真正 RTSP H.264 / H.265 影像** — go2rtc bridge 已落地（`video/go2rtc.rs`），
  可在 session 內以 Snapshot / RTSP 分頁切換；H.265 強制轉 H.264 + MSE fallback。
  MJPEG snapshot loop（`video/mjpeg.rs`）仍是 always-on 預設 backend。
- ✅ **Events 即時監看** — PullPoint 訂閱 + 滾動事件日誌 + 搜尋過濾（`views/events.rs`）。
- ✅ **About 對話框** — 版本 / oxvif 版本 / GitHub 連結 + log-to-file / TLS-strict 開關
  （`components/about_dialog.rs`，Topbar help 按鈕入口）。
- ✅ **檔案日誌** — `~/.oxdm/logs/` daily-rolling file appender（`main.rs`，About 內可開關）。
- ✅ **鍵盤快捷鍵** — `Ctrl/Cmd+F` 搜尋、`F5` 掃描、`↑↓` 切 device、`Esc` 關 modal
  （`GlobalKey` bus，原列在 Tier 4）。
- ✅ **Snapshot 存檔** — 縮圖右上下載鈕（`device_panel.rs`）+ Live Video 標題列下載鈕
  （`views/live_video.rs`，按下時即抓一張新鮮 GetSnapshotUri，Snapshot/RTSP 皆可）。
  共用 helper `util::sanitize_filename` / `util::decode_jpeg_data_uri`。
- ✅ **Logs GUI viewer** — Topbar file-text 鈕開 modal，tail 最新 `~/.oxdm/logs/oxdm.log.*`
  末 800 行 + 子字串過濾 + 重新整理（`components/log_viewer.rs`）。

### Tier 3 — NVR / 進階（oxvif 端齊全，缺 oxdm UI）

- ⬜ **Recording / Search / Replay** — oxvif 端完整（`recording.rs` / `search.rs` /
  `replay.rs`）。需要整個新的 View，UI 設計類似 ODM Replay 視圖。
- ⬜ **Analytics** — 動偵 / 越線等規則引擎（oxvif `GetAnalyticsModules` 等齊全）。
  UI 需 rule builder，相當於一個子應用。
- ⬜ **Firmware upgrade** — `StartFirmwareUpgrade` / `UpgradeSystemFirmware`。
  寫 UI 簡單，但失敗會變磚，要配二次確認 + 進度條 + 失敗回報。
- ⬜ **GetSystemLog 下載** — 維護頁補一個「下載日誌」按鈕。
- ⬜ **憑證管理** — HTTPS client cert 管理（ONVIF 14 系列 API）。少數部署會用。
- ✅ **IO / Relay 控制** — `GetRelayOutputs` / `SetRelayOutputState` /
  `SetRelayOutputSettings` + `GetDigitalInputs`(oxvif 0.9.9）。UI 在
  `views/io_control.rs`(IoControl view):Bistable 給 Activate/Deactivate
  按鈕、Monostable 給 Pulse、可編輯 mode/idle_state/delay_time。
  Digital Input 顯示 token+idle_state(配置面);即時狀態走 PullPoint
  訂閱(Events 分頁,topic `tns1:Device/Trigger/DigitalInput`)。

### Tier 4 — Polish

- ⬜ 多 camera 網格 / 監控牆視圖（2×2、3×3 等）
- ⬜ 裝置匯出 / 匯入（備份 `~/.oxdm/devices.toml` 到其他機器）
- ✅ 死碼清理：`views/main_content.rs` 的 `PlaceholderView` 已移除（七個 View 全有實作）。

### 發布前阻擋項（release blocker — 功能面之外的真正門檻）

這些一個都還沒做，是「能交到使用者手上」的關卡：

- ⬜ **安裝包與簽章** — macOS `.dmg` + notarize、Windows `.msi` / `.exe` + code sign、
  Linux AppImage / `.deb`。
- ⬜ **應用程式圖示**（目前無自訂 icon）。
- ⬜ **Release CI** — `.github/workflows/ci.yml` 目前只在 Linux 跑 fmt / clippy /
  build / test，沒有跨平台 release build matrix、沒有產物上傳。
- ⬜ **自動更新 / 版本檢查** — 啟動時檢查 GitHub Releases 新版。
- ⬜ **oxdm 自身的版本與 tag 發行流程**。

*更新：2026-05-31*
