# OxDM — 施工藍圖（對照 ODM v2.2.250）

以 ONVIF Device Manager 為參考，整理其所有 ONVIF API 呼叫及對應 UI 功能，作為 OxDM 的開發藍圖。

> **oxvif 狀態**欄說明：
> - ✓ 已實作
> - ~ 部分實作
> - — 尚未實作

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
| `SetRelayOutputSettings` | 設定繼電器參數 | IO 控制頁 | — |
| `SetRelayOutputState` | 控制繼電器開關 | IO 控制頁 | — |

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
| `GetSystemBackup` | 取得系統備份檔 | 維護頁 | — |
| `RestoreSystem` | 從備份還原 | 維護頁 | — |
| `SetSystemFactoryDefault` | 恢復出廠設定 | 維護頁 | — |
| `UpgradeSystemFirmware` | 升級韌體 | 維護頁 | — |
| `StartFirmwareUpgrade` | 開始韌體升級流程 | 維護頁 | — |
| `GetSystemLog` | 取得系統日誌 | 維護 / 診斷頁 | — |
| `GetSystemSupportInformation` | 取得支援資訊 | 診斷頁 | — |

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
| `AddVideoSourceConfiguration` | 加入影像來源設定 | — |
| `RemoveVideoSourceConfiguration` | 移除影像來源設定 | — |
| `AddVideoEncoderConfiguration` | 加入影像編碼設定 | — |
| `RemoveVideoEncoderConfiguration` | 移除影像編碼設定 | — |
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
| `SetVideoSourceConfiguration` | 更新影像來源設定 | 影像設定頁 | — |
| `GetVideoSourceConfigurationOptions` | 取得影像來源選項 | 影像設定頁 | — |
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
| `GetMetadataConfigurationOptions` | 取得 Metadata 選項 | Metadata 設定頁 | — |
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
| `GetVideoAnalyticsConfigurations` | 取得 Analytics 設定 | ✓ |
| `GetVideoAnalyticsConfiguration` | 取得單一 Analytics 設定 | ✓ |
| `SetVideoAnalyticsConfiguration` | 更新 Analytics 設定 | ✓ |
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
| `SendAuxiliaryCommand` | 發送輔助指令 | PTZ 控制頁 | — |

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
| `SetSynchronizationPoint` | 設定事件同步點 | 事件處理 | — |
| `GetCurrentMessage` | 取得目前事件訊息 | 事件查詢 | — |

---

## 7. Analytics Service（analytics.wsdl）

### 7-1 Analytics 模組

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetSupportedAnalyticsModules` | 取得支援的模組類型 | Analytics 頁 | ✓ |
| `GetAnalyticsModules` | 取得目前模組 | Analytics 頁 | ✓ |
| `CreateAnalyticsModules` | 建立模組（移動偵測等） | Analytics 頁 | ✓ |
| `ModifyAnalyticsModules` | 更新模組設定 | Analytics 頁 | ✓ |
| `DeleteAnalyticsModules` | 刪除模組 | Analytics 頁 | ✓ |

### 7-2 規則引擎

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetSupportedRules` | 取得支援的規則類型 | Analytics 頁 | ✓ |
| `GetRules` | 取得目前規則 | Analytics 頁 | ✓ |
| `CreateRules` | 建立規則（事件觸發條件） | Analytics 頁 | ✓ |
| `ModifyRules` | 更新規則 | Analytics 頁 | ✓ |
| `DeleteRules` | 刪除規則 | Analytics 頁 | ✓ |

---

## 8. Recording Service（recording.wsdl）

### 8-1 錄影管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetRecordings` | 取得錄影清單 | 錄影管理頁 | ✓ |
| `CreateRecording` | 建立錄影 | 錄影管理頁 | ✓ |
| `GetRecordingConfiguration` | 取得錄影設定 | 錄影設定頁 | ✓ |
| `SetRecordingConfiguration` | 更新錄影設定 | 錄影設定頁 | ✓ |
| `DeleteRecording` | 刪除錄影 | 錄影管理頁 | ✓ |

### 8-2 Track 管理

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `CreateTrack` | 在錄影中建立 Track | 錄影設定頁 | ✓ |
| `GetTrackConfiguration` | 取得 Track 設定 | 錄影設定頁 | ✓ |
| `SetTrackConfiguration` | 更新 Track 設定 | 錄影設定頁 | ✓ |
| `DeleteTrack` | 刪除 Track | 錄影管理頁 | ✓ |

### 8-3 錄影 Job

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetRecordingJobs` | 取得 Job 清單 | 錄影 Job 管理頁 | ✓ |
| `CreateRecordingJob` | 建立 Job | 錄影 Job 管理頁 | ✓ |
| `GetRecordingJobConfiguration` | 取得 Job 設定 | 錄影 Job 設定頁 | ✓ |
| `SetRecordingJobConfiguration` | 更新 Job 設定 | 錄影 Job 設定頁 | ✓ |
| `SetRecordingJobMode` | 設定 Job 模式（開始 / 停止） | 錄影 Job 控制 | ✓ |
| `GetRecordingJobState` | 取得 Job 狀態 | 錄影 Job 監控 | ✓ |
| `DeleteRecordingJob` | 刪除 Job | 錄影 Job 管理頁 | ✓ |

---

## 9. Search Service（search.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetRecordingSummary` | 取得錄影摘要 | 錄影搜尋頁 | ✓ |
| `GetRecordingInformation` | 取得錄影詳細資訊 | 錄影搜尋頁 | ✓ |
| `GetMediaAttributes` | 取得媒體屬性 | 錄影搜尋頁 | ✓ |
| `FindRecordings` | 搜尋錄影 | 錄影搜尋頁 | ✓ |
| `GetRecordingSearchResults` | 取得搜尋結果 | 錄影搜尋頁 | ✓ |
| `FindEvents` | 搜尋事件 | 事件搜尋頁 | ✓ |
| `GetEventSearchResults` | 取得事件搜尋結果 | 事件搜尋頁 | ✓ |
| `FindMetadata` | 搜尋 Metadata | Metadata 搜尋頁 | ✓ |
| `GetMetadataSearchResults` | 取得 Metadata 搜尋結果 | Metadata 搜尋頁 | ✓ |
| `FindPTZPosition` | 搜尋 PTZ 位置 | PTZ 搜尋頁 | ✓ |
| `GetPTZPositionSearchResults` | 取得 PTZ 搜尋結果 | PTZ 搜尋頁 | ✓ |
| `GetSearchState` | 取得搜尋狀態 | 搜尋狀態顯示 | ✓ |
| `EndSearch` | 終止搜尋 | 搜尋管理 | ✓ |

---

## 10. Replay Service（replay.wsdl）

| 方法 | 說明 | UI 功能 | oxvif 狀態 |
|------|------|---------|-----------|
| `GetReplayUri` | 取得回放串流 URL | 影像回放頁 | ✓ |
| `GetReplayConfiguration` | 取得回放設定 | 回放設定頁 | ✓ |
| `SetReplayConfiguration` | 更新回放設定 | 回放設定頁 | ✓ |

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
| `GetReceivers` | 取得 RTP Receiver 清單 | Receiver 管理頁 | ✓ |
| `GetReceiver` | 取得單一 Receiver 設定 | Receiver 設定頁 | ✓ |
| `CreateReceiver` | 建立 Receiver | Receiver 管理頁 | ✓ |
| `ConfigureReceiver` | 更新 Receiver 設定 | Receiver 管理頁 | ✓ |
| `SetReceiverMode` | 設定 Receiver 模式 | Receiver 控制 | ✓ |
| `GetReceiverState` | 取得 Receiver 狀態 | Receiver 監控 | ✓ |
| `DeleteReceiver` | 刪除 Receiver | Receiver 管理頁 | ✓ |

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
