//! HTTP サーバー（UI 配信 + JSON API）。仕様は docs/API.md。

use crate::{games, keycodes, lamzu, zenaim};
use crate::hid::HidApi;
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use tiny_http::{Header, Method, Request, Response, Server};

static UI: Dir = include_dir!("$CARGO_MANIFEST_DIR/ui/dist");
static HID: Mutex<Option<HidApi>> = Mutex::new(None);

type R<T> = Result<T, String>;

pub fn run(server: Server) {
    for req in server.incoming_requests() {
        handle(req);
    }
}

fn handle(mut req: Request) {
    let url = req.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((&url, ""));
    let path = path.to_string();
    let query = query.to_string();
    let method = req.method().clone();

    // DNS リバインディング対策
    let host_ok = req.headers().iter().any(|h| {
        h.field.equiv("Host") && {
            let v = h.value.as_str();
            v.starts_with("127.0.0.1") || v.starts_with("localhost")
        }
    });
    if !host_ok {
        let _ = req.respond(Response::from_string("forbidden").with_status_code(403));
        return;
    }

    if !path.starts_with("/api/") {
        serve_static(req, &path);
        return;
    }

    // 書き込み系はカスタムヘッダー必須（ブラウザからの CSRF を防ぐ）
    if method != Method::Get {
        let has = req.headers().iter().any(|h| h.field.equiv("X-Devhub"));
        if !has {
            respond(req, 403, json!({"ok": false, "error": "X-Devhub ヘッダーが必要です"}));
            return;
        }
    }

    let mut body = String::new();
    let _ = req.as_reader().read_to_string(&mut body);
    let q = parse_query(&query);

    let result = route(&method, &path, &q, &body);
    match result {
        Ok(v) => respond(req, 200, json!({"ok": true, "data": v})),
        Err(e) => respond(req, 400, json!({"ok": false, "error": e})),
    }
}

fn route(m: &Method, path: &str, q: &[(String, String)], body: &str) -> R<Value> {
    let qp = |k: &str| q.iter().find(|x| x.0 == k).map(|x| x.1.clone());
    let profile = qp("profile").map(|s| s.parse::<u8>().map_err(|_| "profile は数値です".to_string())).transpose()?;
    match (m, path) {
        (Method::Get, "/api/status") => status(),
        (Method::Get, "/api/keycodes") => Ok(serde_json::to_value(keycodes::KEYS).unwrap()),
        (Method::Get, "/api/keyboard") => with_hid(|api| kb_read(api, profile)),
        (Method::Patch, "/api/keyboard") | (Method::Post, "/api/keyboard") => {
            let p: KbPatch = parse(body)?;
            let (v, profile) = with_hid(|api| kb_patch(api, &p))?;
            // ベースを変えたら、それを元にしたゲーム用プロファイルも追従（HID ロック外で）
            if p.keys.is_some() || p.all.is_some() || p.socd.is_some() { games::resync_from(profile)?; }
            Ok(v)
        }
        (Method::Get, "/api/keyboard/travel") => with_hid(|api| {
            let kb = zenaim::Keyboard::open(api)?;
            let st = kb.read_profile(kb.active_profile()?)?;
            let t = kb.travel()?;
            // ファームのスイッチ状態ビットは静止時も立つため使わず、ON 位置との比較で判定する
            Ok(Value::Array(t.into_iter().map(|(i, d, _)| {
                let k = st.keys.iter().find(|k| k.index == i);
                let label = k.map(|k| keycodes::label(k.keycodes[0])).unwrap_or_default();
                let on = k.map(|k| d >= k.point_on as i16).unwrap_or(false);
                json!({"index": i, "label": label, "depth": d, "on": on})
            }).collect()))
        }),
        (Method::Get, "/api/mouse") => with_hid(|api| {
            let ms = lamzu::Mouse::open(api)?;
            Ok(serde_json::to_value(ms.read(profile)?).unwrap())
        }),
        (Method::Patch, "/api/mouse") | (Method::Post, "/api/mouse") => {
            let p: lamzu::MousePatch = parse(body)?;
            with_hid(|api| {
                let ms = lamzu::Mouse::open(api)?;
                ms.apply(&p)?;
                Ok(serde_json::to_value(ms.read(p.profile)?).unwrap())
            })
        }
        (Method::Get, "/api/mouse/button-raw") => with_hid(|api| {
            let ms = lamzu::Mouse::open(api)?;
            let id: u8 = qp("id").and_then(|s| s.parse().ok()).ok_or("id が必要です")?;
            Ok(json!(ms.button_raw(profile.unwrap_or(0), id)?))
        }),
        (Method::Get, "/api/games") => Ok(json!({"rules": games::load(), "state": games::state()})),
        (Method::Put, "/api/games") => {
            let rules: Vec<games::GameRule> = parse(body)?;
            games::save(&rules)?;
            for r in rules.iter().filter(|r| r.enabled) { games::sync(r)?; }
            Ok(json!({"rules": games::load(), "state": games::state()}))
        }
        (Method::Post, "/api/games/sync") => {
            for r in games::load().iter().filter(|r| r.enabled) { games::sync(r)?; }
            Ok(json!(null))
        }
        (Method::Get, "/api/presets") => list_presets(),
        (Method::Post, "/api/presets") => {
            #[derive(Deserialize)]
            struct B { name: String }
            let b: B = parse(body)?;
            save_preset(&b.name)
        }
        (Method::Post, "/api/presets/apply") => {
            #[derive(Deserialize)]
            struct B { name: String }
            let b: B = parse(body)?;
            apply_preset(&b.name)
        }
        (Method::Delete, "/api/presets") => {
            let name = qp("name").ok_or("name が必要です")?;
            let p = preset_path(&name)?;
            std::fs::remove_file(p).map_err(|e| format!("削除できません: {e}"))?;
            Ok(json!(null))
        }
        (Method::Get, "/api/autostart") => Ok(json!(autostart_enabled())),
        (Method::Put, "/api/autostart") => {
            let on: bool = parse(body)?;
            set_autostart(on)?;
            Ok(json!(on))
        }
        _ => Err(format!("不明な API: {path}")),
    }
}

pub fn with_hid<T>(f: impl FnOnce(&HidApi) -> R<T>) -> R<T> {
    let mut g = HID.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(HidApi::new().map_err(|e| format!("HID 初期化失敗: {e}"))?);
    }
    let api = g.as_mut().unwrap();
    let _ = api.refresh_devices();
    f(api)
}

fn status() -> R<Value> {
    with_hid(|api| {
        let kb = match zenaim::Keyboard::open(api).and_then(|k| k.info()) {
            Ok(i) => json!({"connected": true, "info": i}),
            Err(e) => json!({"connected": false, "error": e}),
        };
        let ms = match lamzu::Mouse::open(api) {
            Ok(m) => {
                let (b, c) = m.battery();
                json!({"connected": true, "info": {"model": "LAMZU Maya X", "connection": m.connection, "battery": b, "charging": c, "firmware": m.firmware()}})
            }
            Err(e) => json!({"connected": false, "error": e}),
        };
        Ok(json!({"keyboard": kb, "mouse": ms, "version": env!("CARGO_PKG_VERSION")}))
    })
}

// ---------- キーボード ----------

fn kb_read(api: &HidApi, profile: Option<u8>) -> R<Value> {
    let kb = zenaim::Keyboard::open(api)?;
    let info = kb.info()?;
    let st = kb.read_profile(profile.unwrap_or(info.active_profile))?;
    let mut v = serde_json::to_value(&st).unwrap();
    // 表示用ラベル
    if let Some(keys) = v["keys"].as_array_mut() {
        for (k, s) in keys.iter_mut().zip(&st.keys) {
            k["label"] = json!(keycodes::label(s.keycodes[0]));
            k["fnLabel"] = json!(keycodes::label(s.keycodes[2]));
        }
    }
    Ok(json!({"info": info, "state": v}))
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RtPatch {
    range: Option<u16>,
    stroke_on: Option<u16>,
    stroke_off: Option<u16>,
    beyond: Option<bool>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyPatch {
    /// キー名（"A", "KC_A" など。現在の通常レイヤーの割り当てで探す）
    key: Option<String>,
    /// またはデバイス内のキー番号
    index: Option<usize>,
    point_on: Option<u16>,
    point_off: Option<u16>,
    rapid_trigger: Option<bool>,
    rt: Option<RtPatch>,
    /// 通常レイヤーの割り当て
    keycode: Option<String>,
    /// Fn レイヤーの割り当て
    fn_keycode: Option<String>,
    /// 無効化レイヤーの割り当て
    disable_keycode: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SocdPatch {
    mode: u8,
    key1: Option<String>,
    key2: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KbPatch {
    profile: Option<u8>,
    active_profile: Option<u8>,
    /// 全キーに適用（keys より先に当たる）
    all: Option<KeyPatch>,
    keys: Option<Vec<KeyPatch>>,
    socd: Option<Vec<SocdPatch>>,
    hold_time: Option<zenaim::HoldTime>,
}

fn find_key(st: &zenaim::ProfileState, name: &str) -> R<usize> {
    let code = keycodes::parse(name).ok_or(format!("キー名 \"{name}\" が分かりません"))?;
    st.keys.iter().find(|k| k.keycodes[0] == code).map(|k| k.index)
        .ok_or(format!("\"{name}\" が割り当てられたキーがありません"))
}

fn merge_key(k: &mut zenaim::KeySetting, p: &KeyPatch) -> R<()> {
    if let Some(v) = p.point_on { k.point_on = v; }
    if let Some(v) = p.point_off { k.point_off = v; }
    if let Some(v) = p.rapid_trigger { k.rapid_trigger = v; }
    if let Some(rt) = &p.rt {
        // スタビライザー付きキーは公式同様に範囲を 1.6/1.8 に補正
        if let Some(v) = rt.range { k.rt.range = if k.stabilized { (v as u32 * 16 / 18) as u16 } else { v }; }
        if let Some(v) = rt.stroke_on { k.rt.stroke_on = v; }
        if let Some(v) = rt.stroke_off { k.rt.stroke_off = v; }
        if let Some(v) = rt.beyond { k.rt.beyond = v; }
    }
    let kc = |s: &Option<String>| -> R<Option<u16>> {
        s.as_ref().map(|s| keycodes::parse(s).ok_or(format!("キー名 \"{s}\" が分かりません"))).transpose()
    };
    if let Some(c) = kc(&p.keycode)? { k.keycodes[0] = c; }
    if let Some(c) = kc(&p.disable_keycode)? { k.keycodes[1] = c; }
    if let Some(c) = kc(&p.fn_keycode)? { k.keycodes[2] = c; }
    Ok(())
}

fn kb_patch(api: &HidApi, p: &KbPatch) -> R<(Value, u8)> {
    let kb = zenaim::Keyboard::open(api)?;
    let profile = match p.profile { Some(x) => x, None => kb.active_profile()? };
    let mut st = kb.read_profile(profile)?;
    let before = st.keys.clone();
    if let Some(all) = &p.all {
        if all.keycode.is_some() || all.fn_keycode.is_some() || all.disable_keycode.is_some() {
            return Err("all でキー割り当ては変更できません".into());
        }
        for k in st.keys.iter_mut() { merge_key(k, all)?; }
    }
    if let Some(keys) = &p.keys {
        for kp in keys {
            let idx = match (&kp.key, kp.index) {
                (_, Some(i)) => i,
                (Some(n), None) => find_key(&zenaim::ProfileState { keys: before.clone(), ..st.clone() }, n)?,
                _ => return Err("keys の各要素には key か index が必要です".into()),
            };
            let k = st.keys.iter_mut().find(|k| k.index == idx).ok_or(format!("キー番号 {idx} はありません"))?;
            merge_key(k, kp)?;
        }
    }
    let changed: Vec<zenaim::KeySetting> = st.keys.iter().zip(&before)
        .filter(|(a, b)| serde_json::to_value(a).ok() != serde_json::to_value(b).ok())
        .map(|(a, _)| a.clone()).collect();
    let socd = match &p.socd {
        Some(list) => {
            let mut out = Vec::new();
            for s in list {
                let k1 = s.key1.as_deref().map(|n| find_key(&st, n)).transpose()?.unwrap_or(0) as u8;
                let k2 = s.key2.as_deref().map(|n| find_key(&st, n)).transpose()?.unwrap_or(0) as u8;
                out.push(zenaim::Socd { mode: s.mode, key1: k1, key2: k2 });
            }
            while out.len() < 2 { out.push(zenaim::Socd { mode: 0, key1: 0, key2: 0 }); }
            Some(out)
        }
        None => None,
    };
    kb.write_profile(profile, &changed, socd.as_deref(), p.hold_time)?;
    if let Some(ap) = p.active_profile { kb.set_active_profile(ap)?; }
    Ok((kb_read(api, Some(profile))?, profile))
}

// ---------- プリセット ----------

fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    base.join("devhub").join("presets")
}

fn preset_path(name: &str) -> R<PathBuf> {
    if name.is_empty() || name.len() > 64 || name.contains(['/', '\\', '.', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("プリセット名に使えない文字があります".into());
    }
    Ok(data_dir().join(format!("{name}.json")))
}

fn list_presets() -> R<Value> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir()).map(|rd| {
        rd.filter_map(|e| e.ok()).filter_map(|e| {
            let p = e.path();
            if p.extension()? != "json" { return None; }
            Some(p.file_stem()?.to_string_lossy().to_string())
        }).collect()
    }).unwrap_or_default();
    names.sort();
    Ok(json!(names))
}

fn save_preset(name: &str) -> R<Value> {
    let path = preset_path(name)?;
    let snap = with_hid(|api| {
        let kb = zenaim::Keyboard::open(api).ok().and_then(|k| {
            let p = k.active_profile().ok()?;
            k.read_profile(p).ok()
        });
        let ms = lamzu::Mouse::open(api).ok().and_then(|m| m.read(None).ok());
        Ok(json!({"keyboard": kb, "mouse": ms}))
    })?;
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(&snap).unwrap()).map_err(|e| format!("保存できません: {e}"))?;
    Ok(json!(name))
}

fn apply_preset(name: &str) -> R<Value> {
    let path = preset_path(name)?;
    let s = std::fs::read_to_string(path).map_err(|_| "プリセットがありません".to_string())?;
    let v: Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    with_hid(|api| {
        if let Some(kbv) = v.get("keyboard").filter(|x| !x.is_null()) {
            let keys: Vec<zenaim::KeySetting> = serde_json::from_value(kbv["keys"].clone()).map_err(|e| e.to_string())?;
            let socd: Vec<zenaim::Socd> = serde_json::from_value(kbv["socd"].clone()).map_err(|e| e.to_string())?;
            let hold: Option<zenaim::HoldTime> = serde_json::from_value(kbv["holdTime"].clone()).unwrap_or(None);
            let kb = zenaim::Keyboard::open(api)?;
            let p = kb.active_profile()?;
            kb.write_profile(p, &keys, Some(&socd), hold)?;
        }
        if let Some(m) = v.get("mouse").filter(|x| !x.is_null()) {
            let ms = lamzu::Mouse::open(api)?;
            let patch = lamzu::MousePatch {
                polling_rate: m["pollingRate"].as_u64().map(|x| x as u16),
                debounce_ms: m["debounceMs"].as_u64().map(|x| x as u8),
                sleep_sec: m["sleepSec"].as_u64().map(|x| x as u16),
                lod: m["lod"].as_f64().map(|x| x as f32),
                angle_snap: m["angleSnap"].as_bool(),
                motion_sync: m["motionSync"].as_bool(),
                ripple_control: m["rippleControl"].as_bool(),
                competition_mode: m["competitionMode"].as_bool(),
                dpi_xy: m["dpiXy"].as_bool(),
                angle_tune: m["angleTune"].as_i64().map(|x| x as i8),
                dpi_stages: serde_json::from_value(m["dpiStages"].clone()).ok(),
                active_stage: m["activeStage"].as_u64().map(|x| x as u8),
                ..Default::default()
            };
            ms.apply(&patch)?;
        }
        Ok(json!(name))
    })
}

// ---------- スタートアップ ----------

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn autostart_enabled() -> bool {
    std::process::Command::new("reg").args(["query", RUN_KEY, "/v", "devhub"])
        .output().map(|o| o.status.success()).unwrap_or(false)
}

fn set_autostart(on: bool) -> R<()> {
    let ok = if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let cmd = format!("\"{}\" serve", exe.display());
        std::process::Command::new("reg").args(["add", RUN_KEY, "/v", "devhub", "/t", "REG_SZ", "/d", &cmd, "/f"]).output()
    } else {
        std::process::Command::new("reg").args(["delete", RUN_KEY, "/v", "devhub", "/f"]).output()
    };
    match ok {
        Ok(o) if o.status.success() || !on => Ok(()),
        _ => Err("スタートアップの設定に失敗しました".into()),
    }
}

// ---------- 共通 ----------

fn parse<T: for<'a> Deserialize<'a>>(body: &str) -> R<T> {
    serde_json::from_str(body).map_err(|e| format!("JSON が不正です: {e}"))
}

fn parse_query(q: &str) -> Vec<(String, String)> {
    q.split('&').filter(|s| !s.is_empty()).map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k.to_string(), v.to_string())
    }).collect()
}

fn respond(req: Request, code: u16, v: Value) {
    let h = Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap();
    let _ = req.respond(Response::from_string(v.to_string()).with_status_code(code).with_header(h));
}

fn serve_static(req: Request, path: &str) {
    let p = path.trim_start_matches('/');
    let file = UI.get_file(if p.is_empty() { "index.html" } else { p }).or_else(|| UI.get_file("index.html"));
    match file {
        Some(f) => {
            let ct = match f.path().extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript",
                Some("css") => "text/css",
                Some("svg") => "image/svg+xml",
                Some("woff2") => "font/woff2",
                _ => "application/octet-stream",
            };
            let h = Header::from_bytes("Content-Type", ct).unwrap();
            let _ = req.respond(Response::from_data(f.contents()).with_header(h));
        }
        None => {
            let _ = req.respond(Response::from_string("UI がビルドされていません").with_status_code(404));
        }
    }
}
