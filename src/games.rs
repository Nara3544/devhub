//! ゲームモード: 指定したゲームが前面にある間だけ、キーボード/マウスのプロファイルを自動で切り替える。
//!
//! キーボードは「ベースをコピーし、指定キーを無効化したプロファイル」を使う。
//! 無効化はキーボード本体で行うので、ゲーム中に PC 側の処理（フック等）は挟まらない。

use crate::{api::with_hid, keycodes, lamzu, zenaim};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameRule {
    pub name: String,
    pub enabled: bool,
    /// 実行ファイル名（小文字比較）
    pub exe: Vec<String>,
    /// ゲーム中に使うキーボードのプロファイル（2〜4 = MEM1〜3 推奨）
    pub keyboard_profile: u8,
    /// コピー元（通常 0 = ベース）
    #[serde(default)]
    pub base_profile: u8,
    /// 無効化するキー（"Win" "KC_LGUI" など）
    pub disable_keys: Vec<String>,
    /// Fn と同時押しなら無効化したキーも効く
    #[serde(default = "yes")]
    pub allow_with_fn: bool,
    /// ゲーム中だけ Fn キーとして使うキー（例: ["RCtrl"]）
    #[serde(default)]
    pub fn_keys: Vec<String>,
    /// ゲーム中に使うマウスのプロファイル（1〜3、省略で切り替えない）
    #[serde(default)]
    pub mouse_profile: Option<u8>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModeState {
    /// 今有効なルール名（なければ null）
    pub active: Option<String>,
    pub foreground: String,
    pub last_error: Option<String>,
}

struct Inner {
    state: ModeState,
    /// ゲーム開始前のプロファイル（戻す先）
    restore_kb: Option<u8>,
    restore_mouse: Option<u8>,
}

static INNER: Mutex<Inner> = Mutex::new(Inner {
    state: ModeState { active: None, foreground: String::new(), last_error: None },
    restore_kb: None,
    restore_mouse: None,
});

fn yes() -> bool { true }

fn path() -> PathBuf {
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    base.join("devhub").join("games.json")
}

pub fn load() -> Vec<GameRule> {
    std::fs::read_to_string(path()).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(rules: &[GameRule]) -> Result<(), String> {
    for r in rules {
        if r.name.trim().is_empty() { return Err("ゲーム名が空です".into()); }
        if r.exe.is_empty() { return Err(format!("{}: 実行ファイル名がありません", r.name)); }
        if r.keyboard_profile == r.base_profile || r.keyboard_profile >= zenaim::PROFILE_COUNT {
            return Err(format!("{}: ゲーム用プロファイルはベース以外の 1〜4 にしてください", r.name));
        }
        if let Some(m) = r.mouse_profile { if !(1..=3).contains(&m) { return Err("マウスのプロファイルは 1〜3 です".into()); } }
        disable_codes(r)?;
    }
    std::fs::create_dir_all(path().parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path(), serde_json::to_string_pretty(rules).unwrap()).map_err(|e| e.to_string())
}

pub fn disable_codes(r: &GameRule) -> Result<Vec<u16>, String> {
    let mut out = Vec::new();
    for k in &r.disable_keys {
        let c = keycodes::parse(k).ok_or(format!("キー名 \"{k}\" が分かりません"))?;
        out.push(c);
        // Win は左右とも
        if c == 0xE3 { out.push(0xE7); }
        if c == 0xE7 { out.push(0xE3); }
    }
    Ok(out)
}

/// ベースの内容をゲーム用プロファイルへ反映する
pub fn sync(r: &GameRule) -> Result<(), String> {
    let codes = disable_codes(r)?;
    with_hid(|api| {
        let kb = zenaim::Keyboard::open(api)?;
        let fn_codes = r.fn_keys.iter()
            .map(|k| keycodes::parse(k).ok_or(format!("キー名 \"{k}\" が分かりません")))
            .collect::<Result<Vec<u16>, String>>()?;
        if fn_codes.iter().any(|c| codes.contains(c)) { return Err("無効にするキーを Fn にはできません".into()); }
        kb.copy_profile(r.base_profile, r.keyboard_profile, &codes, r.allow_with_fn, &fn_codes)
    })
}

/// ベースが書き換わったら、それを元にしているゲーム用プロファイルも作り直す
pub fn resync_from(base: u8) -> Result<(), String> {
    for r in load().iter().filter(|r| r.base_profile == base) {
        sync(r)?;
    }
    Ok(())
}

pub fn state() -> ModeState {
    INNER.lock().unwrap_or_else(|e| e.into_inner()).state.clone()
}

/// ゲーム用プロファイルのまま取り残されていたら、そのベースに戻す（前回の終了時にゲーム中だった場合など）
fn recover_stuck(rules: &[GameRule], fg: &str) {
    if rules.iter().any(|r| r.enabled && r.exe.iter().any(|e| e.eq_ignore_ascii_case(fg))) { return; }
    let _ = with_hid(|api| {
        let kb = zenaim::Keyboard::open(api)?;
        let cur = kb.active_profile()?;
        if let Some(r) = rules.iter().find(|r| r.keyboard_profile == cur) {
            kb.switch_profile_temp(r.base_profile)?;
        }
        Ok(())
    });
}

pub fn start_watcher() {
    std::thread::spawn(|| {
        recover_stuck(&load(), &foreground_exe().unwrap_or_default());
        watch_loop()
    });
}

fn watch_loop() -> ! {
    loop {
        let rules = load();
        if rules.iter().any(|r| r.enabled) || INNER.lock().map(|g| g.state.active.is_some()).unwrap_or(false) {
            let fg = foreground_exe().unwrap_or_default();
            tick(&rules, &fg);
            std::thread::sleep(Duration::from_millis(1000));
        } else {
            std::thread::sleep(Duration::from_millis(3000));
        }
    }
}

fn tick(rules: &[GameRule], fg: &str) {
    let want = rules.iter().find(|r| r.enabled && r.exe.iter().any(|e| e.eq_ignore_ascii_case(fg)));
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    g.state.foreground = fg.to_string();
    let current = g.state.active.clone();
    match (want, current) {
        (Some(r), cur) if cur.as_deref() != Some(&r.name) => {
            // ゲーム開始（または別ゲームへ）
            let res = with_hid(|api| {
                if let Ok(kb) = zenaim::Keyboard::open(api) {
                    if g.restore_kb.is_none() {
                        // すでにゲーム用プロファイルなら、戻す先はそのベース
                        let cur = kb.active_profile()?;
                        let back = rules.iter().find(|x| x.keyboard_profile == cur).map(|x| x.base_profile).unwrap_or(cur);
                        g.restore_kb = Some(back);
                    }
                    kb.switch_profile_temp(r.keyboard_profile)?;
                }
                if let Some(mp) = r.mouse_profile {
                    if let Ok(ms) = lamzu::Mouse::open(api) {
                        if g.restore_mouse.is_none() { g.restore_mouse = Some(ms.active_profile()?); }
                        ms.set_active_profile(mp)?;
                    }
                }
                Ok(())
            });
            g.state.last_error = res.err();
            g.state.active = Some(r.name.clone());
        }
        (None, Some(_)) => {
            // ゲーム終了 → 元に戻す
            let (rk, rm) = (g.restore_kb.take(), g.restore_mouse.take());
            let res = with_hid(|api| {
                if let (Some(p), Ok(kb)) = (rk, zenaim::Keyboard::open(api)) { kb.switch_profile_temp(p)?; }
                if let (Some(p), Ok(ms)) = (rm, lamzu::Mouse::open(api)) { ms.set_active_profile(p)?; }
                Ok(())
            });
            g.state.last_error = res.err();
            g.state.active = None;
        }
        _ => {}
    }
}

// ---------- 前面ウィンドウの実行ファイル名 ----------

type HANDLE = *mut std::ffi::c_void;

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> HANDLE;
    fn GetWindowThreadProcessId(h: HANDLE, pid: *mut u32) -> u32;
}
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> HANDLE;
    fn QueryFullProcessImageNameW(h: HANDLE, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    fn CloseHandle(h: HANDLE) -> i32;
}

pub fn foreground_exe() -> Option<String> {
    unsafe {
        let w = GetForegroundWindow();
        if w.is_null() { return None; }
        let mut pid = 0u32;
        GetWindowThreadProcessId(w, &mut pid);
        if pid == 0 { return None; }
        let h = OpenProcess(0x1000, 0, pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if h.is_null() { return None; }
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 { return None; }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit('\\').next().map(|s| s.to_lowercase())
    }
}
