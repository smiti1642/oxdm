# oxvif 0.18.0 與 OxDM 0.5.0 發行檢查

## 本地驗證 2026-11-18

- oxvif 與 oxvif-cli 的 0.18.0 隔離候選使用 `cargo package --workspace
  --locked` 打包並重建通過，Rust 1.88 全功能工作區檢查通過。
- 兩個 repo 的 Windows 測試已跑過。工作區的 oxvif 與 oxvif-cli 以
  全功能回歸驗證，OxDM 另跑了指令編輯器測試。
- OxDM 0.5.0 的 Windows MSI 和可攜 ZIP 均已解包驗證。兩包內附的
  oxvif CLI 0.18.0 與本次建置的 CLI 有相同 SHA-256，各通過 20 項
  指令台整合測試，涵蓋 Windows 憑證管理員、事件監看與停止、診斷報告、
  說明與補全。兩包也都用 `inspect --password` 對模擬攝影機讀出裝置
  資訊、媒體設定檔及 RTSP URI。
- 兩份目前的 lockfile 通過 `cargo audit`。oxvif／CLI 有 3 項 Linux
  憑證依賴的既有維護警告。OxDM 有 5 項既有警告，其中兩項標為
  unsound，分別位於 Linux GTK 的 `glib` 和舊版建置相依的 `rand`。
- 遠端最近一次 CI 涵蓋 oxvif `026da13` 與 OxDM `c36c560`，
  發行候選仍需跑各自的遠端工作流程。檢查時 OxDM 的
  `OXVIF_CONSOLE_SHA` 尚無設定。
- crates.io 尚未提供這次的三個候選版本。OxDM 單獨 checkout 的
  registry 套件驗證接在 SDK 發布和 lockfile 更新之後。
- 相容依賴更新後，兩份 lockfile 重新通過 `cargo audit`，兩個
  `.crate`、MSI 和可攜 ZIP 也已用更新後的 lockfile 重建。
  另核對了兩個 `.crate` 內嵌 lockfile 的 registry 版本及 checksum。

## 發行順序

1. oxvif 工作區及 CLI 的 SDK 依賴已切到 `0.18.0`，兩個本機 crate 的
   lockfile 版號也已同步。正式發布時填入實際日期並更新雙語發行說明的狀態。
   GitHub 發行作業以 `v0.18.0` 核對版本與文件。
2. crates.io 先發布 `oxvif`，待 registry 可解析 0.18.0 後，再發布
   `oxvif-cli`。兩者同步使用同一個已審查的版本與來源。用 oxvif 的
   **Prepare release artifacts** 工作流程先驗證發行封裝，再依審查結果發布
   GitHub 下載檔。下載表使用產出的檔名和 SHA-256。
3. OxDM 版號已切到 `0.5.0`，About 和 Quirks 基準檔的 SDK 版本也已
   同步。待 crates.io 可解析 oxvif 0.18.0 後，將一般與開發依賴切換到
   registry，移除相鄰 checkout 的 `path`，重新產生並審查
   `Cargo.lock`。以只有 OxDM checkout 的方式跑
   Windows 和 Linux CI，再跑 `cargo package --locked` 驗證 crates.io
   來源套件。
4. OxDM 打包內附 CLI 時，將 GitHub repository variable
   `OXVIF_CONSOLE_SHA` 設為已確認的 oxvif 0.18.0 完整 commit SHA。
   手動驗證可在 **Release** workflow 的 `oxvif_sha` 欄位傳入同一 SHA。
   檢查 Windows MSI／可攜 ZIP、Apple Silicon DMG 與 Ubuntu DEB 的
   內附執行檔和相容性，再以雙語說明完成 `v0.5.0` 的 GitHub draft。
5. 保留錄影首張畫面、同一 session 跳播與事件歷史補讀的實機驗收缺口。
   0.18.0 發行說明只列出已驗證的範圍。

## Registry 切換 2026-11-19

- crates.io 已提供 oxvif 0.18.0（09:09 UTC）與 oxvif-cli 0.18.0
  （09:10 UTC），兩者均未 yank。oxvif 的 `v0.18.0` tag 指向
  `9cdc631dc2377a438bed163e06a47c386d59980c`。
- OxDM 的一般與開發依賴已移除相鄰 checkout 的 `path`。`Cargo.lock`
  只在 oxvif 條目加入 registry 來源與 checksum，checksum 與 crates.io
  公布的值相同。其餘已審查的套件版本與相依關係沒有變動。
- Windows 本機以 `--locked` 通過 fmt、Clippy、build 與測試，共 222 項
  通過、23 項依環境變數略過。`cargo package --locked` 以 registry 的
  oxvif 打包 108 個檔案並重建通過。
- 發行順序第 3 步的遠端 CI 與第 4 步之後的項目尚未執行。

相關檔案

- [OxDM 發行工作流程](../.github/workflows/release.yml)
- [OxDM 0.5.0 發行說明](release-0.5.0_zh.md)
