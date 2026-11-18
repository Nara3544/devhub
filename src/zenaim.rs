//! ZENAIM KEYBOARD (ZK1 シリーズ) の設定プロトコル。
//!
//! 公式 ZENAIM SOFTWARE に同梱の @zenaim/device-native を読んで再実装したもの。
//! - VIA と同じ Raw HID インターフェース (usage page 0xFF60 / usage 0x61)
//! - 32 バイト固定長。最終バイトは先頭 31 バイトの CRC-8 (poly 0x07, init 0)
//! - 設定は「Set → Apply(反映) → Save(EEPROM 保存)」の順
//! - 距離の単位は 0.001mm (1500 = 1.5mm)

use crate::hid::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};

pub const VID: u16 = 0x3507;
const USAGE_PAGE: u16 = 0xFF60;
const USAGE: u16 = 0x61;
const RAW: usize = 32;
const GET_PAYLOAD: usize = 26; // 32 - header4 - reserved1 - crc1
const SET_PAYLOAD: usize = 27; // 32 - header4 - crc1

pub const PROFILE_COUNT: u8 = 5;
pub const LAYER_COUNT: u8 = 3;

mod cmd {
    pub const PROTOCOL_VERSION: u8 = 0x30;
    pub const FIRMWARE_VERSION: u8 = 0x31;
    pub const KEYBOARD_INFO_GET: u8 = 0x40;
    pub const POINT_ALL_GET: u8 = 0x4a;
    pub const POINT_ALL_SET: u8 = 0x4b;
    pub const POINT_APPLY: u8 = 0x4d;
    pub const POINT_SAVE: u8 = 0x4f;
    pub const DISTANCE_ALL_GET: u8 = 0x6b;
    pub const KEYMAP_ALL_GET: u8 = 0x70;
    pub const KEYMAP_ALL_SET: u8 = 0x71;
    pub const KEYMAP_APPLY: u8 = 0x73;
    pub const KEYMAP_SAVE: u8 = 0x75;
    pub const PROFILE_SELECT_GET: u8 = 0xa0;
    pub const PROFILE_SELECT_SET: u8 = 0xa1;
    pub const PROFILE_SELECT_APPLY: u8 = 0xa3;
    pub const PROFILE_SELECT_SAVE: u8 = 0xa5;
    pub const TYPE_ALL_GET: u8 = 0xb8;
    pub const TYPE_ALL_SET: u8 = 0xb9;
    pub const TYPE_APPLY: u8 = 0xbb;
    pub const TYPE_SAVE: u8 = 0xbd;
    pub const RT_ALL_GET: u8 = 0xc2;
    pub const RT_ALL_SET: u8 = 0xc3;
    pub const RT_APPLY: u8 = 0xc5;
    pub const RT_SAVE: u8 = 0xc7;
    pub const USER_CONFIG_GET: u8 = 0xd4;
    pub const USER_CONFIG_SET: u8 = 0xd5;
    pub const USER_CONFIG_APPLY: u8 = 0xd7;
    pub const USER_CONFIG_SAVE: u8 = 0xd9;
    pub const SOCD_ALL_GET: u8 = 0xde;
    pub const SOCD_ALL_SET: u8 = 0xdf;
    pub const SOCD_APPLY: u8 = 0xe1;
    pub const SOCD_SAVE: u8 = 0xe3;
    pub const ERROR: u8 = 0xfe;
    pub const UNHANDLED: u8 = 0xff;
}

/// 機種ごとの定義（ZENAIM SOFTWARE の deviceProperties より）
pub struct Model {
    pub name: &'static str,
    pub pid: u16,
    pub rows: usize,
    pub cols: usize,
    /// 実在するキーのデバイス内インデックス範囲
    pub key_ranges: &'static [(usize, usize)],
    /// スタビライザー付きキー（ストロークが短いので RT 範囲を補正する）
    pub stabilized: &'static [usize],
}

pub const MODELS: &[Model] = &[
    Model { name: "ZK1 60JIS", pid: 0x0005, rows: 5, cols: 15,
        key_ranges: &[(0, 15), (17, 30), (32, 43), (45, 45), (47, 58), (60, 63), (66, 66), (69, 73)],
        stabilized: &[29, 45, 66] },
    Model { name: "ZK1 60US", pid: 0x0006, rows: 5, cols: 15,
        key_ranges: &[(0, 12), (14, 15), (17, 30), (32, 42), (44, 45), (47, 56), (58, 58), (60, 62), (66, 66), (70, 72), (74, 74)],
        stabilized: &[14, 44, 45, 58, 66] },
    Model { name: "ZK1 80JIS", pid: 0x0002, rows: 6, cols: 18,
        key_ranges: &[(0, 36), (38, 54), (56, 67), (72, 72), (74, 85), (88, 88), (90, 93), (96, 96), (99, 103), (105, 107)],
        stabilized: &[50, 72, 96] },
    Model { name: "ZK1 80US", pid: 0x0003, rows: 6, cols: 18,
        key_ranges: &[(0, 30), (32, 36), (38, 54), (56, 66), (68, 68), (72, 72), (74, 83), (85, 85), (88, 88), (90, 92), (96, 96), (100, 102), (104, 107)],
        stabilized: &[32, 68, 72, 85, 96] },
];

impl Model {
    pub fn count(&self) -> usize { self.rows * self.cols }
    pub fn keys(&self) -> Vec<usize> {
        self.key_ranges.iter().flat_map(|&(a, b)| a..=b).collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RapidTrigger {
    /// RT が働く範囲（AP からの深さ, 0.001mm）
    pub range: u16,
    /// 押し込み判定ストローク（0.001mm）
    pub stroke_on: u16,
    /// 戻り判定ストローク（0.001mm）
    pub stroke_off: u16,
    /// ビヨンドモード（範囲外でも RT を継続）
    pub beyond: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeySetting {
    pub index: usize,
    pub row: usize,
    pub col: usize,
    pub stabilized: bool,
    /// レイヤー0(通常) / 1(無効化レイヤー) / 2(Fn) のキーコード
    pub keycodes: [u16; 3],
    /// ON になる深さ（0.001mm）
    pub point_on: u16,
    /// OFF に戻る深さ（0.001mm）
    pub point_off: u16,
    /// true = ラピッドトリガー
    pub rapid_trigger: bool,
    pub rt: RapidTrigger,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Socd {
    /// 0=OFF 1=ニュートラル 2=先入力優先 3=後入力優先 4=キー1優先 5=キー2優先
    pub mode: u8,
    pub key1: u8,
    pub key2: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HoldTime {
    /// ON 判定を確定するまでの保持時間（ファーム値そのまま）
    pub on: u16,
    /// OFF 判定を確定するまでの保持時間
    pub off: u16,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardInfo {
    pub model: String,
    pub device_name: String,
    pub serial: String,
    pub firmware: String,
    pub protocol: u16,
    pub rows: usize,
    pub cols: usize,
    pub active_profile: u8,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProfileState {
    pub profile: u8,
    pub keys: Vec<KeySetting>,
    pub socd: Vec<Socd>,
    pub hold_time: Option<HoldTime>,
}

pub struct Keyboard {
    dev: HidDevice,
    pub model: &'static Model,
    pub protocol: u16,
}

pub fn find(api: &HidApi) -> Option<(Vec<u16>, &'static Model)> {
    api.device_list()
        .filter(|d| d.vendor_id() == VID && d.usage_page() == USAGE_PAGE && d.usage() == USAGE)
        .find_map(|d| {
            MODELS.iter().find(|m| m.pid == d.product_id()).map(|m| (d.path().to_owned(), m))
        })
}

pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
    }
    crc
}

type R<T> = Result<T, String>;

impl Keyboard {
    pub fn open(api: &HidApi) -> R<Self> {
        let (path, model) = find(api).ok_or("ZENAIM キーボードが見つかりません")?;
        let dev = api.open_path(&path).map_err(|e| format!("キーボードを開けません: {e}"))?;
        let mut kb = Keyboard { dev, model, protocol: 0 };
        let r = kb.xfer(&[cmd::PROTOCOL_VERSION])?;
        kb.protocol = u16::from_le_bytes([r[1], r[2]]);
        Ok(kb)
    }

    fn xfer(&self, req: &[u8]) -> R<[u8; RAW]> {
        let mut out = [0u8; RAW + 1];
        out[1..1 + req.len()].copy_from_slice(req);
        out[RAW] = crc8(&out[1..RAW]);
        self.dev.write(&out).map_err(|e| format!("送信失敗: {e}"))?;
        for _ in 0..8 {
            let mut buf = [0u8; RAW];
            let n = self.dev.read_timeout(&mut buf, 1000).map_err(|e| format!("受信失敗: {e}"))?;
            if n == 0 {
                return Err("キーボードが応答しません".into());
            }
            match buf[0] {
                cmd::ERROR => return Err(format!("キーボードがエラーを返しました (0x{:02x}, cmd 0x{:02x})", buf[1], req[0])),
                cmd::UNHANDLED => return Err(format!("未対応のコマンドです (0x{:02x})", req[0])),
                c if c == req[0] => {
                    if crc8(&buf[..RAW - 1]) != buf[RAW - 1] {
                        return Err("CRC 不一致".into());
                    }
                    return Ok(buf);
                }
                _ => continue, // 他の応答は読み捨て
            }
        }
        Err("応答が一致しません".into())
    }

    fn get_buffer(&self, c: u8, offset: usize, size: usize) -> R<Vec<u8>> {
        let mut out = Vec::with_capacity(size);
        while out.len() < size {
            let off = offset + out.len();
            let n = (size - out.len()).min(GET_PAYLOAD);
            let r = self.xfer(&[c, off as u8, (off >> 8) as u8, n as u8])?;
            out.extend_from_slice(&r[4..4 + n]);
        }
        Ok(out)
    }

    fn set_buffer(&self, c: u8, offset: usize, data: &[u8]) -> R<()> {
        for (i, chunk) in data.chunks(SET_PAYLOAD).enumerate() {
            let off = offset + i * SET_PAYLOAD;
            let mut req = vec![c, off as u8, (off >> 8) as u8, chunk.len() as u8];
            req.extend_from_slice(chunk);
            self.xfer(&req)?;
        }
        Ok(())
    }

    fn simple(&self, c: u8) -> R<()> {
        self.xfer(&[c]).map(|_| ())
    }

    pub fn info(&self) -> R<KeyboardInfo> {
        let fw = self.xfer(&[cmd::FIRMWARE_VERSION])?;
        let end = fw[1..].iter().position(|&b| b == 0).unwrap_or(RAW - 2) + 1;
        let firmware = String::from_utf8_lossy(&fw[1..end]).to_string();
        let b = self.get_buffer(cmd::KEYBOARD_INFO_GET, 0, 96)?;
        let s = |r: &[u8]| {
            let e = r.iter().position(|&x| x == 0).unwrap_or(r.len());
            String::from_utf8_lossy(&r[..e]).to_string()
        };
        Ok(KeyboardInfo {
            model: self.model.name.to_string(),
            device_name: s(&b[0..32]),
            serial: s(&b[64..96]),
            firmware,
            protocol: self.protocol,
            rows: self.model.rows,
            cols: self.model.cols,
            active_profile: self.active_profile()?,
        })
    }

    pub fn active_profile(&self) -> R<u8> {
        Ok(self.get_buffer(cmd::PROFILE_SELECT_GET, 0, 1)?[0])
    }

    pub fn set_active_profile(&self, p: u8) -> R<()> {
        check_profile(p)?;
        self.set_buffer(cmd::PROFILE_SELECT_SET, 0, &[p])?;
        self.simple(cmd::PROFILE_SELECT_APPLY)?;
        self.simple(cmd::PROFILE_SELECT_SAVE)
    }

    /// 保存せずに切り替える（ゲームモード用。EEPROM を消耗させない・再起動で元に戻る）
    pub fn switch_profile_temp(&self, p: u8) -> R<()> {
        check_profile(p)?;
        self.set_buffer(cmd::PROFILE_SELECT_SET, 0, &[p])?;
        self.simple(cmd::PROFILE_SELECT_APPLY)
    }

    /// src の設定を dst に丸ごとコピーし、disable に含まれるキーコードを「なし」にする
    pub fn copy_profile(&self, src: u8, dst: u8, disable: &[u16]) -> R<()> {
        let st = self.read_profile(src)?;
        let keys: Vec<KeySetting> = st.keys.into_iter().map(|mut k| {
            for l in [0usize, 2] {
                if disable.contains(&k.keycodes[l]) { k.keycodes[l] = 0; }
            }
            k
        }).collect();
        self.write_profile(dst, &keys, Some(&st.socd), None)
    }

    pub fn read_profile(&self, p: u8) -> R<ProfileState> {
        check_profile(p)?;
        let n = self.model.count();
        let pi = p as usize;
        let points = self.get_buffer(cmd::POINT_ALL_GET, pi * n * 4, n * 4)?;
        let types = self.get_buffer(cmd::TYPE_ALL_GET, pi * n, n)?;
        let rts = self.get_buffer(cmd::RT_ALL_GET, pi * n * 7, n * 7)?;
        let mut layers = Vec::new();
        for l in 0..LAYER_COUNT as usize {
            layers.push(self.get_buffer(cmd::KEYMAP_ALL_GET, (pi * LAYER_COUNT as usize + l) * n * 2, n * 2)?);
        }
        let socd_raw = self.get_buffer(cmd::SOCD_ALL_GET, pi * 6, 6)?;
        let hold_time = if self.protocol >= 2 {
            let b = self.get_buffer(cmd::USER_CONFIG_GET, 0, 4)?;
            Some(HoldTime { on: le(&b, 0), off: le(&b, 2) })
        } else {
            None
        };
        let keys = self.model.keys().into_iter().map(|i| KeySetting {
            index: i,
            row: i / self.model.cols,
            col: i % self.model.cols,
            stabilized: self.model.stabilized.contains(&i),
            keycodes: [le(&layers[0], i * 2), le(&layers[1], i * 2), le(&layers[2], i * 2)],
            point_on: le(&points, i * 4),
            point_off: le(&points, i * 4 + 2),
            rapid_trigger: types[i] == 1,
            rt: RapidTrigger {
                range: le(&rts, i * 7),
                stroke_on: le(&rts, i * 7 + 2),
                stroke_off: le(&rts, i * 7 + 4),
                beyond: rts[i * 7 + 6] != 0,
            },
        }).collect();
        let socd = socd_raw.chunks(3).map(|c| Socd { mode: c[0], key1: c[1], key2: c[2] }).collect();
        Ok(ProfileState { profile: p, keys, socd, hold_time })
    }

    /// プロファイル全体を書き込み、反映・保存する。
    /// keys に含まれないキーは現状維持。
    pub fn write_profile(&self, p: u8, keys: &[KeySetting], socd: Option<&[Socd]>, hold: Option<HoldTime>) -> R<()> {
        check_profile(p)?;
        let n = self.model.count();
        let pi = p as usize;
        // 現在値をベースに差分を当てる（存在しないマトリクス位置も壊さない）
        let mut points = self.get_buffer(cmd::POINT_ALL_GET, pi * n * 4, n * 4)?;
        let mut types = self.get_buffer(cmd::TYPE_ALL_GET, pi * n, n)?;
        let mut rts = self.get_buffer(cmd::RT_ALL_GET, pi * n * 7, n * 7)?;
        let mut layers = Vec::new();
        for l in 0..LAYER_COUNT as usize {
            layers.push(self.get_buffer(cmd::KEYMAP_ALL_GET, (pi * LAYER_COUNT as usize + l) * n * 2, n * 2)?);
        }
        let valid = self.model.keys();
        for k in keys {
            if !valid.contains(&k.index) {
                return Err(format!("キー番号 {} は存在しません", k.index));
            }
            validate_key(k)?;
            let i = k.index;
            put(&mut points, i * 4, k.point_on);
            put(&mut points, i * 4 + 2, k.point_off);
            types[i] = k.rapid_trigger as u8;
            put(&mut rts, i * 7, k.rt.range);
            put(&mut rts, i * 7 + 2, k.rt.stroke_on);
            put(&mut rts, i * 7 + 4, k.rt.stroke_off);
            rts[i * 7 + 6] = k.rt.beyond as u8;
            for l in 0..3 {
                put(&mut layers[l], i * 2, k.keycodes[l]);
            }
        }
        if !keys.is_empty() {
            self.set_buffer(cmd::POINT_ALL_SET, pi * n * 4, &points)?;
            self.set_buffer(cmd::TYPE_ALL_SET, pi * n, &types)?;
            self.set_buffer(cmd::RT_ALL_SET, pi * n * 7, &rts)?;
            for l in 0..LAYER_COUNT as usize {
                self.set_buffer(cmd::KEYMAP_ALL_SET, (pi * LAYER_COUNT as usize + l) * n * 2, &layers[l])?;
            }
            self.simple(cmd::POINT_APPLY)?;
            self.simple(cmd::TYPE_APPLY)?;
            self.simple(cmd::RT_APPLY)?;
            self.simple(cmd::KEYMAP_APPLY)?;
            self.simple(cmd::TYPE_SAVE)?;
            self.simple(cmd::POINT_SAVE)?;
            self.simple(cmd::RT_SAVE)?;
            self.simple(cmd::KEYMAP_SAVE)?;
        }
        if let Some(s) = socd {
            if s.len() != 2 {
                return Err("SOCD は 2 組で指定してください".into());
            }
            for x in s {
                if x.mode > 5 {
                    return Err("SOCD モードは 0〜5 です".into());
                }
                if x.mode != 0 && (!valid.contains(&(x.key1 as usize)) || !valid.contains(&(x.key2 as usize))) {
                    return Err("SOCD のキー番号が不正です".into());
                }
            }
            let raw: Vec<u8> = s.iter().flat_map(|x| [x.mode, x.key1, x.key2]).collect();
            self.set_buffer(cmd::SOCD_ALL_SET, pi * 6, &raw)?;
            self.simple(cmd::SOCD_APPLY)?;
            self.simple(cmd::SOCD_SAVE)?;
        }
        if let Some(h) = hold {
            if self.protocol < 2 {
                return Err("このファームウェアは保持時間の設定に未対応です".into());
            }
            if h.on > 1000 || h.off > 1000 {
                return Err("保持時間が大きすぎます (最大 1000)".into());
            }
            let mut b = [0u8; 4];
            b[..2].copy_from_slice(&h.on.to_le_bytes());
            b[2..].copy_from_slice(&h.off.to_le_bytes());
            self.set_buffer(cmd::USER_CONFIG_SET, 0, &b)?;
            self.simple(cmd::USER_CONFIG_APPLY)?;
            self.simple(cmd::USER_CONFIG_SAVE)?;
        }
        Ok(())
    }

    /// 全キーの現在の押し込み量（0.001mm）。チャタリング診断用。
    pub fn travel(&self) -> R<Vec<(usize, i16, bool)>> {
        let n = self.model.count();
        let b = self.get_buffer(cmd::DISTANCE_ALL_GET, 0, n * 2)?;
        Ok(self.model.keys().into_iter().map(|i| {
            let lo = b[i * 2];
            let hi = b[i * 2 + 1];
            // bit14 = スイッチ状態, bit15 = 符号
            let on = hi & 0x40 != 0;
            let sign = hi & 0x80 != 0;
            let hi2 = if sign { hi | 0x40 } else { hi & !0x40 };
            (i, i16::from_le_bytes([lo, hi2]), on)
        }).collect())
    }
}

fn check_profile(p: u8) -> R<()> {
    if p >= PROFILE_COUNT { Err(format!("プロファイルは 0〜{} です", PROFILE_COUNT - 1)) } else { Ok(()) }
}

fn validate_key(k: &KeySetting) -> R<()> {
    let ok = |v: u16, lo: u16, hi: u16| v >= lo && v <= hi;
    if !ok(k.point_on, 50, 1800) { return Err(format!("キー{}: ON 位置は 0.05〜1.8mm", k.index)); }
    if !ok(k.point_off, 0, 1800) || k.point_off >= k.point_on {
        return Err(format!("キー{}: OFF 位置は ON 位置より浅くしてください", k.index));
    }
    if !ok(k.rt.stroke_on, 1, 1800) || !ok(k.rt.stroke_off, 1, 1800) {
        return Err(format!("キー{}: RT 感度は 0.001〜1.8mm", k.index));
    }
    if k.rt.range > 1800 { return Err(format!("キー{}: RT 範囲は最大 1.8mm", k.index)); }
    Ok(())
}

fn le(b: &[u8], i: usize) -> u16 { u16::from_le_bytes([b[i], b[i + 1]]) }
fn put(b: &mut [u8], i: usize, v: u16) { b[i..i + 2].copy_from_slice(&v.to_le_bytes()); }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_matches_table() {
        // 公式 crc8 テーブル: crc8([0x01]) = 0x07, crc8([0x80]) = 0x89
        assert_eq!(crc8(&[0x01]), 0x07);
        assert_eq!(crc8(&[0x80]), 0x89);
        assert_eq!(crc8(&[0x00, 0x01]), 0x07);
    }
    #[test]
    fn model_key_counts() {
        assert_eq!(MODELS[0].keys().len(), 65);
    }
}
