# devhub

ZENAIM キーボードと LAMZU Maya X の設定アプリ（Rust + React/Astryx）。

## Claude からデバイス設定を変える

アプリ起動中は `http://127.0.0.1:7878/api/` で操作できる。仕様は [docs/API.md](docs/API.md)。

1. まず `GET /api/keyboard` や `GET /api/mouse` で現状を読む
2. `PATCH` で変えたい項目だけ送る（`X-Devhub: 1` ヘッダー必須）
3. 返ってきた値で反映を確認する

書き込みはデバイス本体に保存される。大きく変える前に `POST /api/presets` で退避しておくと戻せる。

## 開発

- UI: `cd ui && npm install && npx vite build`（`ui/dist` を Rust 側が埋め込む）
- 本体: `cargo build --release` → `target/release/devhub.exe`
- GNU ツールチェーンでも通るよう、HID は `src/hid.rs` で hid.dll を直接呼んでいる（外部 HID クレート不使用）
- プロトコル: `src/zenaim.rs`（公式 ZENAIM SOFTWARE 由来）、`src/lamzu.rs`（公式 Web ドライバ由来）
- UI 文言は日本語・簡潔に
