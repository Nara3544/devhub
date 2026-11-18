#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! devhub: ZENAIM キーボードと LAMZU Maya X をひとつの軽量アプリで設定する。
//!
//! 起動すると 127.0.0.1:7878 で UI と JSON API を提供し、Edge のアプリウィンドウで UI を開く。
//! - `devhub`        … サーバーが無ければ起動し、ウィンドウを開く
//! - `devhub serve`  … サーバーだけ起動（スタートアップ用）

mod api;
mod games;
mod hid;
mod keycodes;
mod lamzu;
mod zenaim;

use std::process::Command;

pub const ADDR: &str = "127.0.0.1:7878";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let serve_only = args.iter().any(|a| a == "serve");
    if args.first().map(|s| s.as_str()) == Some("hid-list") {
        // 解析用: 接続中の HID インターフェース一覧
        let api = hid::HidApi::new().expect("hid");
        for d in api.device_list() {
            let p = String::from_utf16_lossy(d.path());
            println!("{:04x}:{:04x} page={:04x} usage={:04x} {}", d.vendor_id(), d.product_id(), d.usage_page(), d.usage(), p.trim_end_matches('\0'));
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("mouse-probe") {
        let api = hid::HidApi::new().expect("hid");
        for d in api.device_list().filter(|d| d.vendor_id() == lamzu::VID && d.usage_page() >= 0xff00) {
            let p = String::from_utf16_lossy(d.path());
            println!("== {}", p.trim_end_matches('\0'));
            let dev = match api.open_path(d.path()) { Ok(x) => x, Err(e) => { println!("  open: {e}"); continue } };
            println!("  lens in/out/feat = {:?}", dev.lens());
            // (宛先, 長さ, クラス, コマンド)
            for (t, l, c, cmd) in [(1u8, 16u8, 0u8, 0x81u8), (2, 16, 0, 0x81), (1, 1, 0, 0x82), (2, 2, 0, 0x83), (2, 1, 0, 0x85), (1, 1, 0, 0x85)] {
                let mut q = [0u8; 65];
                q[3] = t; q[4] = l; q[5] = c; q[6] = cmd;
                let _ = dev.send_feature_report(&q);
                let mut last = [0u8; 65];
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                    let _ = dev.get_feature_report(&mut last);
                    if last[1] != 0xa0 { break; }
                }
                println!("  t={t} cmd={cmd:02x} -> {:02x?}", &last[..14]);
            }
        }
        return;
    }
    match tiny_http::Server::http(ADDR) {
        Ok(server) => {
            if !serve_only {
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    open_window();
                });
            }
            games::start_watcher();
            api::run(server);
        }
        Err(_) => {
            // 既に起動中 → ウィンドウだけ開く
            if !serve_only {
                open_window();
            }
        }
    }
}

pub fn open_window() {
    let url = format!("http://{ADDR}/");
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    for p in edge_paths {
        if std::path::Path::new(p).exists() {
            let _ = Command::new(p)
                .arg(format!("--app={url}"))
                .arg("--window-size=1100,760")
                .spawn();
            return;
        }
    }
    let _ = Command::new("cmd").args(["/C", "start", "", &url]).spawn();
}
