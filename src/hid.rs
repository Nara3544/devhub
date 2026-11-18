//! 依存ゼロの最小 Windows HID レイヤー（hidapi 互換の一部 API）。
//! hid.dll / setupapi.dll は実行時に読み込むので、インポートライブラリ不要。

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::sync::OnceLock;

type HANDLE = *mut c_void;
type BOOL = i32;
const INVALID: HANDLE = -1isize as HANDLE;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const SHARE_RW: u32 = 3;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const DIGCF_PRESENT: u32 = 2;
const DIGCF_DEVICEINTERFACE: u32 = 0x10;
const WAIT_OBJECT_0: u32 = 0;
const ERROR_IO_PENDING: u32 = 997;

#[repr(C)]
#[derive(Clone, Copy)]
struct GUID { a: u32, b: u16, c: u16, d: [u8; 8] }

#[repr(C)]
struct SpDevIfData { cb: u32, guid: GUID, flags: u32, reserved: usize }

#[repr(C)]
struct HiddAttributes { size: u32, vid: u16, pid: u16, ver: u16 }

#[repr(C)]
struct HidpCaps {
    usage: u16, usage_page: u16, input_len: u16, output_len: u16, feature_len: u16,
    reserved: [u16; 17], rest: [u16; 10],
}

#[repr(C)]
struct Overlapped { internal: usize, internal_high: usize, offset: u32, offset_high: u32, event: HANDLE }

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> HANDLE;
    fn GetProcAddress(m: HANDLE, name: *const u8) -> *const c_void;
    fn CreateFileW(n: *const u16, acc: u32, share: u32, sa: *const c_void, disp: u32, flags: u32, t: HANDLE) -> HANDLE;
    fn CloseHandle(h: HANDLE) -> BOOL;
    fn ReadFile(h: HANDLE, b: *mut u8, n: u32, read: *mut u32, ov: *mut Overlapped) -> BOOL;
    fn WriteFile(h: HANDLE, b: *const u8, n: u32, w: *mut u32, ov: *mut Overlapped) -> BOOL;
    fn GetOverlappedResult(h: HANDLE, ov: *mut Overlapped, n: *mut u32, wait: BOOL) -> BOOL;
    fn CreateEventW(sa: *const c_void, manual: BOOL, init: BOOL, name: *const u16) -> HANDLE;
    fn ResetEvent(h: HANDLE) -> BOOL;
    fn WaitForSingleObject(h: HANDLE, ms: u32) -> u32;
    fn CancelIo(h: HANDLE) -> BOOL;
    fn GetLastError() -> u32;
}

struct Fns {
    get_guid: unsafe extern "system" fn(*mut GUID),
    get_attr: unsafe extern "system" fn(HANDLE, *mut HiddAttributes) -> u8,
    get_pre: unsafe extern "system" fn(HANDLE, *mut isize) -> u8,
    free_pre: unsafe extern "system" fn(isize) -> u8,
    get_caps: unsafe extern "system" fn(isize, *mut HidpCaps) -> i32,
    set_feature: unsafe extern "system" fn(HANDLE, *const u8, u32) -> u8,
    get_feature: unsafe extern "system" fn(HANDLE, *mut u8, u32) -> u8,
    class_devs: unsafe extern "system" fn(*const GUID, *const u16, HANDLE, u32) -> HANDLE,
    enum_if: unsafe extern "system" fn(HANDLE, *const c_void, *const GUID, u32, *mut SpDevIfData) -> BOOL,
    if_detail: unsafe extern "system" fn(HANDLE, *const SpDevIfData, *mut u8, u32, *mut u32, *mut c_void) -> BOOL,
    destroy: unsafe extern "system" fn(HANDLE) -> BOOL,
}
unsafe impl Send for Fns {}
unsafe impl Sync for Fns {}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }

fn fns() -> Result<&'static Fns, String> {
    static F: OnceLock<Option<Fns>> = OnceLock::new();
    F.get_or_init(|| unsafe {
        let hid = LoadLibraryW(wide("hid.dll").as_ptr());
        let sa = LoadLibraryW(wide("setupapi.dll").as_ptr());
        if hid.is_null() || sa.is_null() { return None; }
        macro_rules! p { ($m:expr, $n:literal) => {{
            let f = GetProcAddress($m, concat!($n, "\0").as_ptr());
            if f.is_null() { return None; }
            std::mem::transmute(f)
        }}}
        Some(Fns {
            get_guid: p!(hid, "HidD_GetHidGuid"),
            get_attr: p!(hid, "HidD_GetAttributes"),
            get_pre: p!(hid, "HidD_GetPreparsedData"),
            free_pre: p!(hid, "HidD_FreePreparsedData"),
            get_caps: p!(hid, "HidP_GetCaps"),
            set_feature: p!(hid, "HidD_SetFeature"),
            get_feature: p!(hid, "HidD_GetFeature"),
            class_devs: p!(sa, "SetupDiGetClassDevsW"),
            enum_if: p!(sa, "SetupDiEnumDeviceInterfaces"),
            if_detail: p!(sa, "SetupDiGetDeviceInterfaceDetailW"),
            destroy: p!(sa, "SetupDiDestroyDeviceInfoList"),
        })
    }).as_ref().ok_or_else(|| "hid.dll を読み込めません".to_string())
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    path: Vec<u16>,
    vid: u16,
    pid: u16,
    usage_page: u16,
    usage: u16,
}

impl DeviceInfo {
    pub fn vendor_id(&self) -> u16 { self.vid }
    pub fn product_id(&self) -> u16 { self.pid }
    pub fn usage_page(&self) -> u16 { self.usage_page }
    pub fn usage(&self) -> u16 { self.usage }
    pub fn path(&self) -> &[u16] { &self.path }
}

pub struct HidApi { list: Vec<DeviceInfo> }

impl HidApi {
    pub fn new() -> Result<Self, String> {
        let mut a = HidApi { list: Vec::new() };
        a.refresh_devices()?;
        Ok(a)
    }

    pub fn device_list(&self) -> impl Iterator<Item = &DeviceInfo> { self.list.iter() }

    pub fn refresh_devices(&mut self) -> Result<(), String> {
        let f = fns()?;
        let mut out = Vec::new();
        unsafe {
            let mut guid = GUID { a: 0, b: 0, c: 0, d: [0; 8] };
            (f.get_guid)(&mut guid);
            let set = (f.class_devs)(&guid, std::ptr::null(), std::ptr::null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
            if set == INVALID { return Err("デバイス一覧を取得できません".into()); }
            let mut i = 0;
            loop {
                let mut d = SpDevIfData { cb: std::mem::size_of::<SpDevIfData>() as u32, guid, flags: 0, reserved: 0 };
                if (f.enum_if)(set, std::ptr::null(), &guid, i, &mut d) == 0 { break; }
                i += 1;
                let mut need = 0u32;
                (f.if_detail)(set, &d, std::ptr::null_mut(), 0, &mut need, std::ptr::null_mut());
                if need < 8 { continue; }
                let mut buf = vec![0u8; need as usize];
                // SP_DEVICE_INTERFACE_DETAIL_DATA_W.cbSize = 8 (x64)
                buf[..4].copy_from_slice(&8u32.to_le_bytes());
                if (f.if_detail)(set, &d, buf.as_mut_ptr(), need, std::ptr::null_mut(), std::ptr::null_mut()) == 0 { continue; }
                let wide: Vec<u16> = buf[4..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
                let mut path = wide[..end].to_vec();
                path.push(0);
                if let Some(info) = probe(f, path) { out.push(info); }
            }
            (f.destroy)(set);
        }
        self.list = out;
        Ok(())
    }

    pub fn open_path(&self, path: &[u16]) -> Result<HidDevice, String> {
        let f = fns()?;
        unsafe {
            let h = CreateFileW(path.as_ptr(), GENERIC_READ | GENERIC_WRITE, SHARE_RW, std::ptr::null(), OPEN_EXISTING, FILE_FLAG_OVERLAPPED, std::ptr::null_mut());
            if h == INVALID { return Err(format!("開けません (エラー {})", GetLastError())); }
            let caps = caps_of(f, h);
            let ev = CreateEventW(std::ptr::null(), 1, 0, std::ptr::null());
            let (inl, outl, featl) = caps.map(|c| (c.input_len, c.output_len, c.feature_len)).unwrap_or((0, 0, 0));
            Ok(HidDevice { h, ev, input_len: inl as usize, output_len: outl as usize, feature_len: featl as usize })
        }
    }
}

unsafe fn caps_of(f: &Fns, h: HANDLE) -> Option<HidpCaps> {
    let mut pre = 0isize;
    if (f.get_pre)(h, &mut pre) == 0 { return None; }
    let mut caps: HidpCaps = std::mem::zeroed();
    let ok = (f.get_caps)(pre, &mut caps) == 0x0011_0000; // HIDP_STATUS_SUCCESS
    (f.free_pre)(pre);
    ok.then_some(caps)
}

unsafe fn probe(f: &Fns, path: Vec<u16>) -> Option<DeviceInfo> {
    // アクセス権 0 で開けば、キーボード/マウス本体のコレクションでも属性は読める
    let h = CreateFileW(path.as_ptr(), 0, SHARE_RW, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
    if h == INVALID { return None; }
    let mut a = HiddAttributes { size: std::mem::size_of::<HiddAttributes>() as u32, vid: 0, pid: 0, ver: 0 };
    let ok = (f.get_attr)(h, &mut a) != 0;
    let caps = caps_of(f, h);
    CloseHandle(h);
    let c = caps?;
    ok.then(|| DeviceInfo { path, vid: a.vid, pid: a.pid, usage_page: c.usage_page, usage: c.usage })
}

pub struct HidDevice { h: HANDLE, ev: HANDLE, input_len: usize, output_len: usize, feature_len: usize }
unsafe impl Send for HidDevice {}

impl HidDevice {
    pub fn lens(&self) -> (usize, usize, usize) { (self.input_len, self.output_len, self.feature_len) }

    /// data[0] は Report ID。出力レポート長に合わせて 0 埋めする
    pub fn write(&self, data: &[u8]) -> Result<usize, String> {
        let mut buf = data.to_vec();
        if self.output_len > buf.len() { buf.resize(self.output_len, 0); }
        unsafe {
            ResetEvent(self.ev);
            let mut ov: Overlapped = std::mem::zeroed();
            ov.event = self.ev;
            let mut n = 0u32;
            if WriteFile(self.h, buf.as_ptr(), buf.len() as u32, &mut n, &mut ov) == 0 {
                if GetLastError() != ERROR_IO_PENDING { return Err(format!("書き込み失敗 ({})", GetLastError())); }
                if WaitForSingleObject(self.ev, 1000) != WAIT_OBJECT_0 { CancelIo(self.h); return Err("書き込みタイムアウト".into()); }
                if GetOverlappedResult(self.h, &mut ov, &mut n, 0) == 0 { return Err("書き込み失敗".into()); }
            }
            Ok(n as usize)
        }
    }

    /// Report ID (0) を除いたデータを返す。タイムアウト時は 0
    pub fn read_timeout(&self, out: &mut [u8], ms: i32) -> Result<usize, String> {
        let len = self.input_len.max(out.len() + 1);
        let mut buf = vec![0u8; len];
        unsafe {
            ResetEvent(self.ev);
            let mut ov: Overlapped = std::mem::zeroed();
            ov.event = self.ev;
            let mut n = 0u32;
            if ReadFile(self.h, buf.as_mut_ptr(), len as u32, &mut n, &mut ov) == 0 {
                if GetLastError() != ERROR_IO_PENDING { return Err(format!("読み込み失敗 ({})", GetLastError())); }
                if WaitForSingleObject(self.ev, ms as u32) != WAIT_OBJECT_0 {
                    CancelIo(self.h);
                    GetOverlappedResult(self.h, &mut ov, &mut n, 1);
                    return Ok(0);
                }
                if GetOverlappedResult(self.h, &mut ov, &mut n, 0) == 0 { return Err("読み込み失敗".into()); }
            }
            let n = n as usize;
            if n == 0 { return Ok(0); }
            let data = &buf[1..n];
            let c = data.len().min(out.len());
            out[..c].copy_from_slice(&data[..c]);
            Ok(c)
        }
    }

    pub fn send_feature_report(&self, data: &[u8]) -> Result<(), String> {
        let f = fns()?;
        let mut buf = data.to_vec();
        if self.feature_len > buf.len() { buf.resize(self.feature_len, 0); }
        unsafe {
            if (f.set_feature)(self.h, buf.as_ptr(), buf.len() as u32) == 0 {
                return Err(format!("Feature 送信失敗 ({})", GetLastError()));
            }
        }
        Ok(())
    }

    /// buf[0] に Report ID を入れて呼ぶ。返り値は buf 全体の長さ
    pub fn get_feature_report(&self, buf: &mut [u8]) -> Result<usize, String> {
        let f = fns()?;
        let len = self.feature_len.max(buf.len());
        let mut tmp = vec![0u8; len];
        tmp[0] = buf[0];
        unsafe {
            if (f.get_feature)(self.h, tmp.as_mut_ptr(), len as u32) == 0 {
                return Err(format!("Feature 受信失敗 ({})", GetLastError()));
            }
        }
        let c = buf.len().min(len);
        buf[..c].copy_from_slice(&tmp[..c]);
        Ok(c)
    }
}

impl Drop for HidDevice {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.h); CloseHandle(self.ev); }
    }
}
