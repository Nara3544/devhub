//! LAMZU Maya X の設定プロトコル。
//!
//! 公式 Web ドライバ (lamzu.net) の JS を読んで再実装したもの。
//! - 64 バイトの Feature Report (Report ID 0)
//! - [2]=対象(2=マウス) [3]=データ長 [4]=クラス [5]=コマンド(取得は |0x80) [6]=プロファイル [7..]=値
//! - 応答の状態バイト 0xA1 = 成功, それ未満 = 処理中, それより大きい = 再送

use crate::hid::{HidApi, HidDevice};
use serde::{Deserialize, Serialize};
use std::thread::sleep;
use std::time::Duration;

pub const VID: u16 = 0x373E;
/// 有線 / 1K ドングル / 8K ドングル
pub const PIDS: &[(u16, &str)] = &[(0x001C, "有線"), (0x001D, "1K ドングル"), (0x001E, "8K ドングル")];
const LEN: usize = 64;
const OK: u8 = 0xA1;
const DELAY: Duration = Duration::from_millis(12);

pub const POLLING: &[(u16, u8)] = &[(125, 8), (250, 4), (500, 2), (1000, 1), (2000, 16), (4000, 32), (8000, 64)];
pub const LODS: &[f32] = &[0.7, 1.0, 2.0];
pub const DPI_MAX: u16 = 30000;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct DpiStage { pub x: u16, pub y: u16 }

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MouseState {
    pub model: String,
    pub connection: String,
    pub firmware: String,
    pub battery: Option<u8>,
    pub charging: bool,
    pub active_profile: u8,
    pub profile: u8,
    pub polling_rate: u16,
    pub debounce_ms: u8,
    pub sleep_sec: u16,
    pub lod: f32,
    pub angle_snap: bool,
    pub motion_sync: bool,
    pub ripple_control: bool,
    /// 競技モード（HyperMode / 高速モード）
    pub competition_mode: bool,
    pub dpi_xy: bool,
    pub angle_tune: i8,
    pub dpi_stages: Vec<DpiStage>,
    /// 1 始まり
    pub active_stage: u8,
}

/// 変更したい項目だけ指定する
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MousePatch {
    pub profile: Option<u8>,
    pub active_profile: Option<u8>,
    pub polling_rate: Option<u16>,
    pub debounce_ms: Option<u8>,
    pub sleep_sec: Option<u16>,
    pub lod: Option<f32>,
    pub angle_snap: Option<bool>,
    pub motion_sync: Option<bool>,
    pub ripple_control: Option<bool>,
    pub competition_mode: Option<bool>,
    pub dpi_xy: Option<bool>,
    pub angle_tune: Option<i8>,
    pub dpi_stages: Option<Vec<DpiStage>>,
    pub active_stage: Option<u8>,
}

pub struct Mouse {
    dev: HidDevice,
    pub connection: &'static str,
    /// 応答バッファ内のずれ（Report ID を含むかどうか）
    shift: usize,
}

type R<T> = Result<T, String>;

impl Mouse {
    pub fn open(api: &HidApi) -> R<Self> {
        let mut last = "LAMZU Maya X が見つかりません".to_string();
        for d in api.device_list().filter(|d| d.vendor_id() == VID && d.usage_page() >= 0xFF00) {
            let Some(&(_, conn)) = PIDS.iter().find(|p| p.0 == d.product_id()) else { continue };
            let Ok(dev) = api.open_path(d.path()) else { continue };
            if dev.lens().2 < LEN { continue; } // 64 バイト Feature を持つ IF だけ
            let mut m = Mouse { dev, connection: conn, shift: 0 };
            match m.probe() {
                Ok(()) => return Ok(m),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Feature Report が通るインターフェースか確認する
    fn probe(&mut self) -> R<()> {
        let mut req = [0u8; LEN];
        req[2] = 2;
        req[3] = 1;
        req[5] = 0x85; // 現在のプロファイル取得
        self.send(&req)?;
        let mut pending = false;
        for _ in 0..2 {
            sleep(DELAY);
            for _ in 0..30 {
                let r = self.recv()?;
                if r[1] == OK && r[6] == 0x85 { self.shift = 0; return Ok(()); }
                if r[0] == OK && r[5] == 0x85 { self.shift = 1; return Ok(()); }
                if r[1] == 0xA0 || r[0] == 0xA0 { pending = true; }
                if r[1] > OK { break; } // エラー → 再送
                sleep(DELAY);
            }
            self.send(&req)?;
        }
        if pending {
            Err("マウスがスリープ中です。マウスを動かしてから再読み込みしてください".into())
        } else {
            Err("マウスが応答しません（ドングル接続を確認）".into())
        }
    }

    fn send(&self, data: &[u8; LEN]) -> R<()> {
        let mut buf = [0u8; LEN + 1];
        buf[1..].copy_from_slice(data);
        self.dev.send_feature_report(&buf).map_err(|e| format!("送信失敗: {e}"))
    }

    /// 戻り値は「先頭が Report ID」の並びに正規化する
    fn recv(&self) -> R<[u8; LEN + 1]> {
        let mut buf = [0u8; LEN + 1];
        let n = self.dev.get_feature_report(&mut buf).map_err(|e| format!("受信失敗: {e}"))?;
        if n == 0 { return Err("応答なし".into()); }
        Ok(buf)
    }

    fn norm(&self, r: [u8; LEN + 1]) -> [u8; LEN + 1] {
        if self.shift == 0 { return r; }
        let mut o = [0u8; LEN + 1];
        o[1..].copy_from_slice(&r[..LEN]);
        o
    }

    /// 送信して成功応答を待つ。返り値のインデックスは「idx0」基準
    /// (状態=[1], コマンド=[6], プロファイル付き値=[8])
    fn xfer(&self, req: &[u8; LEN]) -> R<[u8; LEN + 1]> {
        let mut pending = false;
        for _attempt in 0..3 {
            self.send(req)?;
            sleep(DELAY);
            for _ in 0..30 {
                let r = self.norm(self.recv()?);
                // 状態 OK かつ同じコマンドへの応答だけ採用（古い応答の取り違え防止）
                if r[1] == OK && r[6] == req[5] { return Ok(r); }
                if r[1] == OK { sleep(DELAY); continue; }
                pending = r[1] == 0xA0;
                if r[1] > OK || r[1] == 2 { break; } // エラー → 再送
                sleep(DELAY);
            }
            sleep(DELAY);
        }
        if pending {
            Err("マウスがスリープ中です。マウスを動かしてから再読み込みしてください".into())
        } else {
            Err(format!("マウスが応答しません (cmd 0x{:02x})", req[5]))
        }
    }

    fn get(&self, len: u8, class: u8, c: u8, profile: Option<u8>) -> R<[u8; LEN + 1]> {
        let mut q = [0u8; LEN];
        q[2] = 2; q[3] = len; q[4] = class; q[5] = c;
        if let Some(p) = profile { q[6] = p; }
        self.xfer(&q)
    }

    fn set(&self, len: u8, class: u8, c: u8, values: &[u8]) -> R<()> {
        let mut q = [0u8; LEN];
        q[2] = 2; q[3] = len; q[4] = class; q[5] = c;
        q[6..6 + values.len()].copy_from_slice(values);
        self.xfer(&q).map(|_| ())
    }

    fn get_flag(&self, c: u8, p: u8) -> R<u8> { Ok(self.get(2, 1, c, Some(p))?[8]) }
    fn set_flag(&self, c: u8, p: u8, v: u8) -> R<()> { self.set(2, 1, c, &[p, v]) }

    pub fn firmware(&self) -> String {
        match self.get(16, 0, 0x81, None) {
            Ok(r) => format!("{}.{}.{}.{}", r[7], r[8], r[9], r[10]),
            Err(_) => "不明".into(),
        }
    }

    pub fn battery(&self) -> (Option<u8>, bool) {
        match self.get(2, 0, 0x83, None) {
            Ok(r) if r[6] == 0x83 => (Some(r[8].min(100)), r[7] == 1),
            _ => (None, false),
        }
    }

    pub fn active_profile(&self) -> R<u8> { Ok(self.get(1, 0, 0x85, None)?[7]) }
    pub fn set_active_profile(&self, p: u8) -> R<()> {
        if !(1..=3).contains(&p) { return Err("マウスのプロファイルは 1〜3 です".into()); }
        self.set(1, 0, 0x05, &[p])
    }

    pub fn read(&self, profile: Option<u8>) -> R<MouseState> {
        let active = self.active_profile()?;
        let p = profile.unwrap_or(active);
        if !(1..=3).contains(&p) { return Err("マウスのプロファイルは 1〜3 です".into()); }
        let (battery, charging) = self.battery();
        let pr_code = self.get_flag(0x80, p)?;
        let polling_rate = POLLING.iter().find(|x| x.1 == pr_code).map(|x| x.0).unwrap_or(0);
        let debounce_ms = self.get(2, 0, 0x88, Some(p))?[8];
        let s = self.get(3, 0, 0x87, Some(p))?;
        let sleep_sec = ((s[8] as u16) << 8) | s[9] as u16;
        let lod = decode_lod(self.get_flag(0x88, p)?);
        let dpi_stages = self.read_stages(p)?;
        Ok(MouseState {
            model: "LAMZU Maya X".into(),
            connection: self.connection.into(),
            firmware: self.firmware(),
            battery,
            charging,
            active_profile: active,
            profile: p,
            polling_rate,
            debounce_ms,
            sleep_sec,
            lod,
            angle_snap: self.get_flag(0x84, p)? == 1,
            motion_sync: self.get_flag(0x89, p)? == 1,
            ripple_control: self.get_flag(0x8A, p)? == 1,
            competition_mode: self.get_flag(0x8B, p)? == 1,
            dpi_xy: self.get_flag(0x8D, p)? == 1,
            angle_tune: self.get_flag(0x94, p)? as i8,
            dpi_stages,
            active_stage: self.get_flag(0x82, p)?,
        })
    }

    fn read_stages(&self, p: u8) -> R<Vec<DpiStage>> {
        let mut q = [0u8; LEN];
        q[2] = 2; q[3] = 10; q[4] = 1; q[5] = 0x81; q[6] = p; q[7] = 6;
        let r = self.xfer(&q)?;
        if r[6] != 0x81 { return Err("DPI 段階を読めません".into()); }
        let n = (r[8] as usize).min(6);
        Ok((0..n).map(|i| {
            let o = 9 + i * 4;
            DpiStage { x: be(&r, o), y: be(&r, o + 2) }
        }).collect())
    }

    pub fn apply(&self, patch: &MousePatch) -> R<()> {
        let active = self.active_profile()?;
        let p = patch.profile.unwrap_or(active);
        if !(1..=3).contains(&p) { return Err("マウスのプロファイルは 1〜3 です".into()); }
        if let Some(hz) = patch.polling_rate {
            let code = POLLING.iter().find(|x| x.0 == hz).ok_or("ポーリングレートは 125/250/500/1000/2000/4000/8000")?.1;
            if self.connection == "有線" && hz > 1000 { return Err("有線は 1000Hz までです".into()); }
            if self.connection == "8K ドングル" && hz < 500 { return Err("8K ドングルは 500Hz 以上です".into()); }
            self.set_flag(0x00, p, code)?;
        }
        if let Some(ms) = patch.debounce_ms {
            if ms > 15 { return Err("デバウンスは 0〜15ms です".into()); }
            self.set(2, 0, 0x08, &[p, ms])?;
        }
        if let Some(sec) = patch.sleep_sec {
            if !(10..=1800).contains(&sec) { return Err("スリープは 10〜1800 秒です".into()); }
            self.set(3, 0, 0x07, &[p, (sec >> 8) as u8, sec as u8])?;
        }
        if let Some(lod) = patch.lod {
            let code = encode_lod(lod).ok_or("LOD は 0.7 / 1 / 2 mm です")?;
            self.set_flag(0x08, p, code)?;
        }
        let flags = [
            (patch.angle_snap, 0x04), (patch.motion_sync, 0x09), (patch.ripple_control, 0x0A),
            (patch.competition_mode, 0x0B), (patch.dpi_xy, 0x0D),
        ];
        for (v, c) in flags {
            if let Some(b) = v { self.set_flag(c, p, b as u8)?; }
        }
        if let Some(a) = patch.angle_tune {
            if !(-30..=30).contains(&a) { return Err("角度補正は -30〜30 です".into()); }
            self.set_flag(0x14, p, a as u8)?;
        }
        if let Some(st) = &patch.dpi_stages {
            if st.is_empty() || st.len() > 5 { return Err("DPI 段階は 1〜5 個です".into()); }
            let mut v = vec![p, st.len() as u8];
            for s in st {
                for d in [s.x, s.y] {
                    if !(50..=DPI_MAX).contains(&d) || d % 50 != 0 {
                        return Err("DPI は 50〜30000 の 50 刻みです".into());
                    }
                    v.extend_from_slice(&d.to_be_bytes());
                }
            }
            self.set(26, 1, 0x01, &v)?;
        }
        if let Some(i) = patch.active_stage {
            self.set_flag(0x02, p, i)?;
        }
        if let Some(ap) = patch.active_profile {
            self.set_active_profile(ap)?;
        }
        Ok(())
    }

    /// 解析用: ボタン設定の生データ
    pub fn button_raw(&self, p: u8, id: u8) -> R<String> {
        let mut q = [0u8; LEN];
        q[2] = 2; q[3] = 15; q[4] = 3; q[5] = 0x80; q[6] = p; q[7] = id; q[9] = 255; q[10] = 10;
        let r = self.xfer(&q)?;
        Ok(r[..32].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "))
    }
}

fn be(b: &[u8], i: usize) -> u16 { ((b[i] as u16) << 8) | b[i + 1] as u16 }

fn decode_lod(v: u8) -> f32 {
    if v >= 0x80 { (v - 0x80) as f32 / 10.0 }
    else if v >= 0x10 { (v >> 4) as f32 + (v & 0xF) as f32 / 10.0 }
    else { v as f32 }
}

fn encode_lod(l: f32) -> Option<u8> {
    LODS.iter().find(|&&x| (x - l).abs() < 0.01)?;
    Some(if l < 1.0 { ((l * 10.0).round() as u8) | 0x80 } else if (l - l.round()).abs() < 0.01 { l.round() as u8 }
         else { ((l.floor() as u8) << 4) | ((l * 10.0).round() as u8 % 10) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lod_roundtrip() {
        for &l in LODS { assert!((decode_lod(encode_lod(l).unwrap()) - l).abs() < 0.01); }
        assert_eq!(encode_lod(0.7), Some(0x87));
    }
}
