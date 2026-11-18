//! キーコード表（QMK 基本キーコード = HID Usage）。表示名は JIS 配列寄り。

use serde::Serialize;

#[derive(Serialize)]
pub struct Key {
    pub code: u16,
    pub name: &'static str,
    pub label: &'static str,
}

macro_rules! keys {
    ($($code:expr, $name:expr, $label:expr;)*) => { &[$(Key { code: $code, name: $name, label: $label }),*] };
}

pub const KEYS: &[Key] = keys! {
    0x00, "KC_NO", "なし";
    0x01, "KC_TRNS", "▽";
    0x04, "KC_A", "A"; 0x05, "KC_B", "B"; 0x06, "KC_C", "C"; 0x07, "KC_D", "D";
    0x08, "KC_E", "E"; 0x09, "KC_F", "F"; 0x0A, "KC_G", "G"; 0x0B, "KC_H", "H";
    0x0C, "KC_I", "I"; 0x0D, "KC_J", "J"; 0x0E, "KC_K", "K"; 0x0F, "KC_L", "L";
    0x10, "KC_M", "M"; 0x11, "KC_N", "N"; 0x12, "KC_O", "O"; 0x13, "KC_P", "P";
    0x14, "KC_Q", "Q"; 0x15, "KC_R", "R"; 0x16, "KC_S", "S"; 0x17, "KC_T", "T";
    0x18, "KC_U", "U"; 0x19, "KC_V", "V"; 0x1A, "KC_W", "W"; 0x1B, "KC_X", "X";
    0x1C, "KC_Y", "Y"; 0x1D, "KC_Z", "Z";
    0x1E, "KC_1", "1"; 0x1F, "KC_2", "2"; 0x20, "KC_3", "3"; 0x21, "KC_4", "4";
    0x22, "KC_5", "5"; 0x23, "KC_6", "6"; 0x24, "KC_7", "7"; 0x25, "KC_8", "8";
    0x26, "KC_9", "9"; 0x27, "KC_0", "0";
    0x28, "KC_ENT", "Enter"; 0x29, "KC_ESC", "Esc"; 0x2A, "KC_BSPC", "BS"; 0x2B, "KC_TAB", "Tab";
    0x2C, "KC_SPC", "Space"; 0x2D, "KC_MINS", "-"; 0x2E, "KC_EQL", "^"; 0x2F, "KC_LBRC", "@";
    0x30, "KC_RBRC", "["; 0x31, "KC_BSLS", "]"; 0x32, "KC_NUHS", "]"; 0x33, "KC_SCLN", ";";
    0x34, "KC_QUOT", ":"; 0x35, "KC_GRV", "半/全"; 0x36, "KC_COMM", ","; 0x37, "KC_DOT", ".";
    0x38, "KC_SLSH", "/"; 0x39, "KC_CAPS", "Caps";
    0x3A, "KC_F1", "F1"; 0x3B, "KC_F2", "F2"; 0x3C, "KC_F3", "F3"; 0x3D, "KC_F4", "F4";
    0x3E, "KC_F5", "F5"; 0x3F, "KC_F6", "F6"; 0x40, "KC_F7", "F7"; 0x41, "KC_F8", "F8";
    0x42, "KC_F9", "F9"; 0x43, "KC_F10", "F10"; 0x44, "KC_F11", "F11"; 0x45, "KC_F12", "F12";
    0x46, "KC_PSCR", "PrtSc"; 0x47, "KC_SCRL", "ScrLk"; 0x48, "KC_PAUS", "Pause";
    0x49, "KC_INS", "Ins"; 0x4A, "KC_HOME", "Home"; 0x4B, "KC_PGUP", "PgUp";
    0x4C, "KC_DEL", "Del"; 0x4D, "KC_END", "End"; 0x4E, "KC_PGDN", "PgDn";
    0x4F, "KC_RGHT", "→"; 0x50, "KC_LEFT", "←"; 0x51, "KC_DOWN", "↓"; 0x52, "KC_UP", "↑";
    0x53, "KC_NUM", "NumLk"; 0x54, "KC_PSLS", "N/"; 0x55, "KC_PAST", "N*"; 0x56, "KC_PMNS", "N-";
    0x57, "KC_PPLS", "N+"; 0x58, "KC_PENT", "NEnt";
    0x59, "KC_P1", "N1"; 0x5A, "KC_P2", "N2"; 0x5B, "KC_P3", "N3"; 0x5C, "KC_P4", "N4";
    0x5D, "KC_P5", "N5"; 0x5E, "KC_P6", "N6"; 0x5F, "KC_P7", "N7"; 0x60, "KC_P8", "N8";
    0x61, "KC_P9", "N9"; 0x62, "KC_P0", "N0"; 0x63, "KC_PDOT", "N.";
    0x65, "KC_APP", "Menu";
    0x87, "KC_INT1", "\\ろ"; 0x88, "KC_INT2", "かな"; 0x89, "KC_INT3", "¥";
    0x8A, "KC_INT4", "変換"; 0x8B, "KC_INT5", "無変換";
    0xA8, "KC_MUTE", "ミュート"; 0xA9, "KC_VOLU", "音量+"; 0xAA, "KC_VOLD", "音量-";
    0xAB, "KC_MNXT", "次の曲"; 0xAC, "KC_MPRV", "前の曲"; 0xAD, "KC_MSTP", "停止"; 0xAE, "KC_MPLY", "再生/停止";
    0x5221, "MO(1)", "Fn1"; 0x5222, "MO(2)", "Fn";    0xE0, "KC_LCTL", "Ctrl"; 0xE1, "KC_LSFT", "Shift"; 0xE2, "KC_LALT", "Alt"; 0xE3, "KC_LGUI", "Win";
    0xE4, "KC_RCTL", "RCtrl"; 0xE5, "KC_RSFT", "RShift"; 0xE6, "KC_RALT", "RAlt"; 0xE7, "KC_RGUI", "RWin";
};

pub fn label(code: u16) -> String {
    KEYS.iter().find(|k| k.code == code).map(|k| k.label.to_string())
        .unwrap_or_else(|| format!("0x{code:04X}"))
}

/// "A", "KC_A", "0x04" のどれでも受け付ける
pub fn parse(s: &str) -> Option<u16> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u16::from_str_radix(h, 16).ok();
    }
    let up = t.to_uppercase();
    KEYS.iter().find(|k| k.name.eq_ignore_ascii_case(t) || k.name[3..].eq_ignore_ascii_case(t) || k.label.to_uppercase() == up)
        .map(|k| k.code)
}
