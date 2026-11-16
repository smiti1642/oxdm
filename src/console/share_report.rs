//! Closed v1 report fields. Keep this projection aligned with the oxvif CLI report schema.

use std::{
    io::{self, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::{i18n, state::Locale};

pub(super) const MAX_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Command {
    Diagnose,
    ConfigValidate,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Selection {
    SavedDevice,
    Direct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Passed,
    Failed,
    CompletedWithWarnings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum MediaService {
    Media1,
    Media2,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShareReport {
    kind: String,
    report_version: u8,
    cli_version: String,
    os: String,
    architecture: String,
    command: Command,
    selection: Option<Selection>,
    credentials_configured: Option<bool>,
    outcome: Outcome,
    complete: bool,
    exit_code: u64,
    primary_issue: Option<Check>,
    checks: Vec<Check>,
    clock_offset_secs: Option<i64>,
    services: Vec<Service>,
    media_service: Option<MediaService>,
    media_attempts: Vec<MediaAttempt>,
    session_attempts: Option<u64>,
    options: Option<Options>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    Input,
    Configuration,
    Execution,
    Registry,
    IndexedSnapshots,
    OrphanedSnapshots,
    Target,
    Credentials,
    CredentialStore,
    TransportSetup,
    OnvifSession,
    ClockRead,
    Capabilities,
    ServicesFallback,
    DeviceInfo,
    MediaProfiles,
    ProfileSelection,
    StreamUri,
    SnapshotUri,
    SnapshotDestination,
    SnapshotFetch,
    SnapshotSignature,
    RtspConnection,
    RtspOptions,
    RtspDescribe,
    RtspTransport,
    RtspPlay,
    RtspFirstPacket,
    RtspTeardown,
    VideoDecode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Passed,
    Failed,
    Warning,
    Unsupported,
    Blocked,
    NotEnabled,
    NotImplemented,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Reason {
    InvalidInput,
    MissingTarget,
    TargetNotFound,
    ConfigurationUnavailable,
    RegistryIo,
    RegistryCorrupt,
    RegistryVersion,
    SnapshotMissing,
    ScanLimit,
    UnindexedSnapshots,
    CredentialStoreUnavailable,
    CredentialMissing,
    CredentialIncomplete,
    InvalidCa,
    TransportSetupFailed,
    Timeout,
    TransportFailure,
    RemoteAuthorizationRejected,
    ResponseLimit,
    InvalidResponse,
    SoapFault,
    Unsupported,
    NoProfiles,
    ProfileNotFound,
    ProfileRequired,
    DestinationRejected,
    InvalidImage,
    SnapshotHttpRejected,
    AuthenticationChallenge,
    RtspTls,
    RtspRefused,
    FirstPacketMissing,
    CleanupFailed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    CheckUsage,
    SelectTarget,
    CheckConfigAccess,
    CheckRegistryBackup,
    CheckRegistryVersion,
    RestoreSnapshot,
    ReviewConfigSize,
    ReviewOrphanedSnapshots,
    CheckCredentialStore,
    SetCredential,
    CheckCredentialPair,
    CheckTlsTrust,
    CheckTransportOptions,
    CheckReachability,
    CheckRemoteAuth,
    CheckDeviceClock,
    CheckResponseLimits,
    CheckFirmware,
    CheckServiceSupport,
    SelectProfile,
    CheckDestination,
    CheckSnapshotImage,
    CheckSnapshotHttp,
    CheckAuthChallenge,
    CheckRtspService,
    CheckRtspVideo,
    CheckRtspCleanup,
    Retry,
    InspectLocally,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Check {
    stage: Stage,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<Reason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Action>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ClockSync {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum SoapAuth {
    Combined,
    Digest,
    WsSecurity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum MediaPolicy {
    Media1,
    Media2,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RtspProbe {
    Disabled,
    Describe,
    Packet,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Options {
    timeout_ms: u64,
    retries: u64,
    clock_sync: ClockSync,
    soap_auth: SoapAuth,
    media_service: MediaPolicy,
    rtsp_probe: RtspProbe,
    custom_ca: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ServiceKind {
    Device,
    Media,
    Media2,
    Ptz,
    Imaging,
    Events,
    Recording,
    Search,
    Replay,
    DeviceIo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum EndpointSource {
    Configured,
    Capabilities,
    ServicesFallback,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Service {
    service: ServiceKind,
    source: EndpointSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum MediaAttemptOutcome {
    NotAdvertised,
    Unsupported,
    Succeeded,
    Failed(MediaFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum MediaFailure {
    Authentication,
    Timeout,
    Transport,
    ResponseLimit,
    XmlLimit,
    XmlSyntax,
    DeviceFault,
    ResponseData,
    InvalidArgument,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MediaAttempt {
    service: MediaService,
    endpoint_source: EndpointSource,
    outcome: MediaAttemptOutcome,
}

impl ShareReport {
    pub(super) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_BYTES {
            return None;
        }
        let report: Self = serde_json::from_slice(bytes).ok()?;
        let version = report.cli_version.split(['+', '-']).next()?;
        if report.kind != "shareable_diagnostic"
            || report.report_version != 1
            || report.cli_version.len() > 64
            || report.cli_version != version
            || version.split('.').count() != 3
            || !version.split('.').all(|part| part.parse::<u32>().is_ok())
            || !matches!(
                report.os.as_str(),
                "windows"
                    | "linux"
                    | "macos"
                    | "freebsd"
                    | "openbsd"
                    | "netbsd"
                    | "dragonfly"
                    | "android"
            )
            || !matches!(
                report.architecture.as_str(),
                "x86_64"
                    | "aarch64"
                    | "x86"
                    | "arm"
                    | "riscv64"
                    | "powerpc64"
                    | "s390x"
                    | "loongarch64"
            )
            || report.exit_code > 255
            || report.checks.len() > 64
            || report.services.len() > 16
            || report.media_attempts.len() > 4
            || report.primary_issue.as_ref()
                != report
                    .checks
                    .iter()
                    .find(|check| check.status == Status::Failed)
                    .or_else(|| {
                        report
                            .checks
                            .iter()
                            .find(|check| check.status == Status::Cancelled)
                    })
            || !report.checks.iter().all(Check::valid)
        {
            return None;
        }
        Some(report)
    }

    pub(super) fn issue(
        &self,
        locale: Locale,
    ) -> Option<(&'static str, &'static str, &'static str)> {
        let issue = self.primary_issue.as_ref()?;
        Some((
            issue.stage.label(locale),
            issue.reason?.label(locale),
            issue.action.as_ref()?.localized(locale),
        ))
    }

    pub(super) fn skipped(
        &self,
        locale: Locale,
    ) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.checks.iter().filter_map(move |check| {
            let key = match check.status {
                Status::Blocked => "console_share_blocked",
                Status::NotEnabled => "console_share_not_enabled",
                Status::NotImplemented => "console_share_not_implemented",
                Status::Unsupported => "console_share_unsupported",
                _ => return None,
            };
            Some((check.stage.label(locale), i18n::t(locale, key)))
        })
    }

    pub(super) fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|mut json| {
            json.push('\n');
            json
        })
    }

    pub(super) fn save(&self, destination: PathBuf) -> io::Result<()> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Report destination must have a directory",
                )
            })?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(self.json()?.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(destination)?;
        Ok(())
    }
}

impl Reason {
    fn label(self, locale: Locale) -> &'static str {
        let (en, zh, ru) = match self {
            Self::InvalidInput => ("Invalid parameters", "參數無效", "Недопустимые параметры"),
            Self::MissingTarget => ("Camera target missing", "未指定攝影機", "Не указана камера"),
            Self::TargetNotFound => (
                "Saved camera was not found",
                "找不到已儲存的攝影機",
                "Сохранённая камера не найдена",
            ),
            Self::ConfigurationUnavailable => (
                "Configuration unavailable",
                "無法存取設定",
                "Конфигурация недоступна",
            ),
            Self::RegistryIo => (
                "Registry read or write failed",
                "登錄檔讀寫失敗",
                "Не удалось прочитать или записать реестр",
            ),
            Self::RegistryCorrupt => (
                "Registry or snapshot is unreadable",
                "無法解析登錄檔或快照",
                "Не удалось разобрать реестр или снимок",
            ),
            Self::RegistryVersion => (
                "Unsupported registry version",
                "不支援此登錄檔版本",
                "Версия реестра не поддерживается",
            ),
            Self::SnapshotMissing => (
                "Indexed snapshot file is missing",
                "找不到已登錄的探索快照",
                "Файл снимка из реестра отсутствует",
            ),
            Self::ScanLimit => (
                "File scan reached its limit",
                "已達檔案掃描上限",
                "Достигнут предел проверки файлов",
            ),
            Self::UnindexedSnapshots => (
                "Unindexed snapshots were found",
                "發現未登錄的探索快照",
                "Найдены снимки вне реестра",
            ),
            Self::CredentialStoreUnavailable => (
                "Credential store is unavailable",
                "無法存取系統憑證庫",
                "Хранилище учётных данных недоступно",
            ),
            Self::CredentialMissing => (
                "Camera credentials are missing",
                "缺少攝影機憑證",
                "Отсутствуют учётные данные камеры",
            ),
            Self::CredentialIncomplete => (
                "Camera credentials are incomplete",
                "攝影機憑證不完整",
                "Учётные данные камеры неполные",
            ),
            Self::InvalidCa => (
                "Invalid CA certificate",
                "CA 憑證無效",
                "Недопустимый сертификат CA",
            ),
            Self::TransportSetupFailed => (
                "Transport setup failed",
                "傳輸設定失敗",
                "Не удалось настроить транспорт",
            ),
            Self::Timeout => (
                "Request timed out",
                "請求逾時",
                "Истекло время ожидания запроса",
            ),
            Self::TransportFailure => (
                "Transport request failed",
                "傳輸請求失敗",
                "Ошибка транспорта при запросе",
            ),
            Self::RemoteAuthorizationRejected => (
                "Camera rejected authorization",
                "攝影機拒絕授權",
                "Камера отклонила авторизацию",
            ),
            Self::ResponseLimit => (
                "Response exceeded a limit",
                "回應超出上限",
                "Ответ превысил допустимый предел",
            ),
            Self::InvalidResponse => (
                "Invalid camera response",
                "攝影機回應無效",
                "Недопустимый ответ камеры",
            ),
            Self::SoapFault => (
                "Camera returned a SOAP fault",
                "攝影機回傳 SOAP Fault",
                "Камера вернула ошибку SOAP",
            ),
            Self::Unsupported => (
                "Operation is unsupported",
                "裝置不支援此操作",
                "Операция не поддерживается",
            ),
            Self::NoProfiles => (
                "Camera reported no media profiles",
                "攝影機未回報媒體設定檔",
                "Камера не вернула медиапрофили",
            ),
            Self::ProfileNotFound => (
                "Media profile was not found",
                "找不到指定媒體設定檔",
                "Медиапрофиль не найден",
            ),
            Self::ProfileRequired => (
                "A media profile is required",
                "需要指定媒體設定檔",
                "Требуется медиапрофиль",
            ),
            Self::DestinationRejected => (
                "Service destination violates policy",
                "服務端點未通過目的地檢查",
                "Адрес службы не прошёл проверку",
            ),
            Self::InvalidImage => (
                "Invalid snapshot image",
                "快照影像無效",
                "Недопустимое изображение снимка",
            ),
            Self::SnapshotHttpRejected => (
                "Snapshot HTTP request was rejected",
                "快照 HTTP 請求遭拒",
                "HTTP-запрос снимка отклонён",
            ),
            Self::AuthenticationChallenge => (
                "Authentication challenge was unsupported",
                "不支援服務要求的認證方式",
                "Запрошенный способ аутентификации не поддерживается",
            ),
            Self::RtspTls => (
                "RTSP TLS negotiation failed",
                "RTSP TLS 協商失敗",
                "Не удалось согласовать TLS для RTSP",
            ),
            Self::RtspRefused => (
                "RTSP service refused the request",
                "RTSP 服務拒絕請求",
                "Служба RTSP отклонила запрос",
            ),
            Self::FirstPacketMissing => (
                "No first video packet arrived",
                "未收到首個影像封包",
                "Первый видеопакет не получен",
            ),
            Self::CleanupFailed => (
                "Session cleanup failed",
                "工作階段清理失敗",
                "Не удалось завершить сеанс",
            ),
            Self::Cancelled => ("Check was cancelled", "檢查已取消", "Проверка отменена"),
            Self::Unknown => ("Reason is unknown", "原因未知", "Причина неизвестна"),
        };
        match locale {
            Locale::En => en,
            Locale::ZhTw => zh,
            Locale::Ru => ru,
        }
    }
}

impl Check {
    fn valid(&self) -> bool {
        let has_explanation = match (&self.reason, &self.action, &self.next_step) {
            (None, None, None) => true,
            (Some(_), Some(action), Some(next)) => {
                next == action.next_step()
                    || (*action == Action::CheckCredentialStore
                        && matches!(next.as_str(),
                        "Check that Windows Credential Manager is available to this process."
                        | "Unlock the login Keychain and check this process's access."
                        | "Check that this D-Bus session has an unlocked Secret Service provider."
                        | "Check whether a supported native credential store is available."))
            }
            _ => false,
        };
        has_explanation
            && self
                .http_status
                .is_none_or(|status| (100..=999).contains(&status))
    }
}

impl Stage {
    fn label(self, locale: Locale) -> &'static str {
        let (en, zh, ru) = match self {
            Self::Input => ("Input", "輸入", "Ввод"),
            Self::Configuration => ("Configuration", "設定", "Конфигурация"),
            Self::Execution => ("Execution", "執行", "Выполнение"),
            Self::Registry => ("Device registry", "裝置登錄", "Реестр устройств"),
            Self::IndexedSnapshots => (
                "Indexed discovery snapshots",
                "已登錄的探索快照",
                "Снимки обнаружения в реестре",
            ),
            Self::OrphanedSnapshots => (
                "Unindexed discovery snapshots",
                "未登錄的探索快照",
                "Снимки обнаружения вне реестра",
            ),
            Self::Target => ("Camera target", "攝影機目標", "Целевая камера"),
            Self::Credentials => ("Camera credentials", "攝影機憑證", "Учётные данные камеры"),
            Self::CredentialStore => (
                "System credential store",
                "系統憑證庫",
                "Системное хранилище учётных данных",
            ),
            Self::TransportSetup => ("Transport setup", "傳輸設定", "Настройка транспорта"),
            Self::OnvifSession => ("ONVIF session", "ONVIF 工作階段", "Сеанс ONVIF"),
            Self::ClockRead => ("Camera clock", "讀取攝影機時鐘", "Чтение времени камеры"),
            Self::Capabilities => ("Device capabilities", "裝置能力", "Возможности устройства"),
            Self::ServicesFallback => {
                ("Service fallback", "備用服務查詢", "Резервный запрос служб")
            }
            Self::DeviceInfo => ("Device information", "裝置資訊", "Сведения об устройстве"),
            Self::MediaProfiles => ("Media profiles", "媒體設定檔", "Медиапрофили"),
            Self::ProfileSelection => ("Profile selection", "選擇設定檔", "Выбор профиля"),
            Self::StreamUri => ("Stream URI", "串流 URI", "URI потока"),
            Self::SnapshotUri => ("Snapshot URI", "快照 URI", "URI снимка"),
            Self::SnapshotDestination => {
                ("Snapshot destination", "快照端點", "Адрес сервиса снимков")
            }
            Self::SnapshotFetch => ("Snapshot download", "下載快照", "Загрузка снимка"),
            Self::SnapshotSignature => {
                ("Snapshot image signature", "快照影像格式", "Формат снимка")
            }
            Self::RtspConnection => ("RTSP connection", "RTSP 連線", "Подключение RTSP"),
            Self::RtspOptions => ("RTSP OPTIONS", "RTSP OPTIONS", "RTSP OPTIONS"),
            Self::RtspDescribe => ("RTSP DESCRIBE", "RTSP DESCRIBE", "RTSP DESCRIBE"),
            Self::RtspTransport => ("RTSP transport", "RTSP 傳輸", "Транспорт RTSP"),
            Self::RtspPlay => ("RTSP PLAY", "RTSP PLAY", "RTSP PLAY"),
            Self::RtspFirstPacket => (
                "First RTSP video packet",
                "RTSP 首個影像封包",
                "Первый видеопакет RTSP",
            ),
            Self::RtspTeardown => ("RTSP TEARDOWN", "RTSP TEARDOWN", "RTSP TEARDOWN"),
            Self::VideoDecode => ("Video decoding", "影像解碼", "Декодирование видео"),
        };
        match locale {
            Locale::En => en,
            Locale::ZhTw => zh,
            Locale::Ru => ru,
        }
    }
}

impl Action {
    fn localized(&self, locale: Locale) -> &'static str {
        if locale == Locale::En {
            return self.next_step();
        }
        let (zh, ru) = match self {
            Self::CheckUsage => ("用 --help 檢查語法與可分享模式參數。", "Проверьте синтаксис и параметры отчёта через --help."),
            Self::SelectTarget => ("從 oxvif devices 選取已儲存攝影機，或用 --target 指定目標。", "Выберите сохранённую камеру в oxvif devices или укажите --target."),
            Self::CheckConfigAccess => ("在本機執行 oxvif config path，檢查設定目錄與權限。", "Выполните oxvif config path локально и проверьте каталог и права доступа."),
            Self::CheckRegistryBackup => ("將本機裝置登錄或探索快照與可用的備份比對。", "Сверьте локальный реестр или снимок обнаружения с рабочей резервной копией."),
            Self::CheckRegistryVersion => ("遷移前確認建立登錄檔的 CLI 版本。", "Перед переносом уточните, какая версия CLI создала реестр."),
            Self::RestoreSnapshot => ("在本機檢查並從可用的備份還原該探索快照。", "Проверьте снимок обнаружения локально и восстановите его из рабочей резервной копии."),
            Self::ReviewConfigSize => ("檢查本機裝置登錄與探索快照的檔案大小和數量。", "Проверьте размер и количество файлов реестра и снимков обнаружения."),
            Self::ReviewOrphanedSnapshots => ("在本機執行 oxvif config validate，檢查未登錄的探索快照。", "Выполните oxvif config validate локально, чтобы проверить файлы вне реестра."),
            Self::CheckCredentialStore => ("確認本程序可以存取已解鎖的系統憑證庫。", "Убедитесь, что системное хранилище разблокировано и доступно процессу."),
            Self::SetCredential => ("在本機使用 oxvif auth 設定攝影機帳密。", "Задайте учётные данные камеры локально через oxvif auth."),
            Self::CheckCredentialPair => ("在本機確認帳號與密碼都已提供。", "Убедитесь локально, что заданы и имя пользователя, и пароль."),
            Self::CheckTlsTrust => ("在本機檢查伺服器憑證、信任鏈、主機名稱和提供的 CA 憑證。", "Проверьте сертификат сервера, цепочку доверия, имя хоста и предоставленные сертификаты CA."),
            Self::CheckTransportOptions => ("在本機檢查逾時與 TLS 設定。", "Проверьте тайм-аут и параметры TLS локально."),
            Self::CheckReachability => ("檢查攝影機是否可連線、服務連接埠及 TLS 信任，再重試該檢查。", "Проверьте доступность камеры, порт службы и доверие TLS, затем повторите проверку."),
            Self::CheckRemoteAuth => ("在本機檢查攝影機帳號權限與認證方式。", "Проверьте права учётной записи камеры и настроенный способ аутентификации."),
            Self::CheckDeviceClock => ("確認 GetSystemDateAndTime 可用，並檢查攝影機是否回傳有效的 UTC 時間。", "Проверьте, отвечает ли GetSystemDateAndTime и возвращает ли камера корректное время UTC."),
            Self::CheckResponseLimits => ("確認攝影機回應是否超出文件記載的上限。", "Проверьте, не превышает ли ответ камеры указанный в документации лимит."),
            Self::CheckFirmware => ("比對受影響服務和攝影機韌體在正常情況下的行為。", "Сравните поведение затронутой службы и прошивки камеры с рабочим вариантом."),
            Self::CheckServiceSupport => ("檢查裝置宣告的服務與支援的媒體設定檔。", "Проверьте объявленную службу и поддерживаемые медиапрофили."),
            Self::SelectProfile => ("在本機執行 oxvif profiles，再用 --profile 傳入可用的 token。", "Выполните oxvif profiles локально и укажите доступный идентификатор через --profile."),
            Self::CheckDestination => ("檢查裝置宣告的端點是否符合主機與 TLS 政策。", "Проверьте, соответствует ли объявленный адрес правилам хоста и TLS."),
            Self::CheckSnapshotImage => ("確認快照端點回傳支援的影像格式。", "Убедитесь, что служба снимков возвращает изображение поддерживаемого формата."),
            Self::CheckSnapshotHttp => ("在本機檢查快照端點與 HTTP 狀態。", "Проверьте адрес службы снимков и код HTTP локально."),
            Self::CheckAuthChallenge => ("檢查攝影機服務提供的認證方式。", "Проверьте, какие способы аутентификации предлагает служба камеры."),
            Self::CheckRtspService => ("在本機檢查串流設定檔及 RTSP 服務設定。", "Проверьте профиль потока и настройки службы RTSP локально."),
            Self::CheckRtspVideo => ("確認選定的串流可透過 RTSP TCP 傳送影像。", "Проверьте, передаёт ли выбранный поток видео по RTSP через TCP."),
            Self::CheckRtspCleanup => ("重新測試前，先檢查攝影機如何結束 RTSP 工作階段。", "Перед повторной проверкой выясните, как камера завершает сеанс RTSP."),
            Self::Retry => ("等檢查可以跑完時再執行一次。", "Повторите проверку, когда она сможет завершиться."),
            Self::InspectLocally => ("在本機用一般診斷命令檢查失敗階段。", "Проверьте этап сбоя локально обычной командой диагностики."),
        };
        match locale {
            Locale::En => self.next_step(),
            Locale::ZhTw => zh,
            Locale::Ru => ru,
        }
    }

    fn next_step(&self) -> &'static str {
        match self {
            Self::CheckUsage => "Check the command syntax and share-mode options with --help.",
            Self::SelectTarget => {
                "Select a saved camera from oxvif devices or specify an explicit --target."
            }
            Self::CheckConfigAccess => {
                "Use oxvif config path locally and check the configuration directory and permissions."
            }
            Self::CheckRegistryBackup => {
                "Check the local registry or indexed snapshot against a known-good backup."
            }
            Self::CheckRegistryVersion => {
                "Check which CLI version created the registry before migrating it."
            }
            Self::RestoreSnapshot => {
                "Check the indexed snapshot locally and restore its file from a known-good backup."
            }
            Self::ReviewConfigSize => {
                "Review the size and number of registry and snapshot files locally."
            }
            Self::ReviewOrphanedSnapshots => {
                "Use oxvif config validate locally to review unindexed files."
            }
            Self::CheckCredentialStore => {
                #[cfg(windows)]
                return "Check that Windows Credential Manager is available to this process.";
                #[cfg(target_os = "macos")]
                return "Unlock the login Keychain and check this process's access.";
                #[cfg(target_os = "linux")]
                return "Check that this D-Bus session has an unlocked Secret Service provider.";
                #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
                return "Check whether a supported native credential store is available.";
            }
            Self::SetCredential => "Set the camera credential locally with oxvif auth.",
            Self::CheckCredentialPair => {
                "Check locally that both a username and password were supplied."
            }
            Self::CheckTlsTrust => {
                "Check the server certificate, trust chain, hostname and supplied CA bundle locally."
            }
            Self::CheckTransportOptions => "Check the timeout and TLS settings locally.",
            Self::CheckReachability => {
                "Check camera reachability, service port and TLS trust, then retry the affected check."
            }
            Self::CheckRemoteAuth => {
                "Check the camera account permissions and configured authentication mode locally."
            }
            Self::CheckDeviceClock => {
                "Check whether GetSystemDateAndTime works and whether the camera returns valid UTC time."
            }
            Self::CheckResponseLimits => {
                "Check whether the camera response exceeds the documented limit."
            }
            Self::CheckFirmware => {
                "Check the affected service and camera firmware against known-good behavior."
            }
            Self::CheckServiceSupport => {
                "Check the advertised service and supported media profiles."
            }
            Self::SelectProfile => {
                "Run oxvif profiles locally and pass an available token with --profile."
            }
            Self::CheckDestination => {
                "Check whether the advertised endpoint violates the host or TLS destination policy."
            }
            Self::CheckSnapshotImage => {
                "Check whether the snapshot endpoint returns a supported image."
            }
            Self::CheckSnapshotHttp => "Check the snapshot endpoint and its HTTP status locally.",
            Self::CheckAuthChallenge => {
                "Check the authentication methods offered by the camera service."
            }
            Self::CheckRtspService => {
                "Check the stream profile and RTSP service configuration locally."
            }
            Self::CheckRtspVideo => "Check whether the selected stream sends video over RTSP TCP.",
            Self::CheckRtspCleanup => {
                "Check the camera's RTSP session handling before repeating the probe."
            }
            Self::Retry => "Run the check again when it can finish.",
            Self::InspectLocally => {
                "Review the failed stage locally with the ordinary diagnostic command."
            }
        }
    }
}
