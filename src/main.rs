#![windows_subsystem = "windows"]

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use windows::core::{implement, w, IUnknown, Interface, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, BOOL, COLORREF, ERROR_ALREADY_EXISTS, ERROR_IO_PENDING, HANDLE,
    HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F,
    D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush, D2D1_ELLIPSE,
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP,
};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplaySettingsW,
    SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DEVMODEW, DIB_RGB_COLORS, ENUM_CURRENT_SETTINGS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::Media::Audio::{
    eConsole, eRender, ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX,
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, WriteFile, FILE_FLAG_OVERLAPPED, MOVEFILE_COPY_ALLOWED,
    MOVEFILE_REPLACE_EXISTING, PIPE_ACCESS_OUTBOUND,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IAgileObject, IAgileObject_Impl, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, GetCurrentProcessId, GetCurrentThread, ReleaseMutex, SetEvent,
    SetPriorityClass, SetThreadPriority, WaitForMultipleObjects, WaitForSingleObject,
    HIGH_PRIORITY_CLASS, THREAD_PRIORITY_HIGHEST,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, RegisterHotKey, ReleaseCapture, SetCapture, SetFocus,
    TrackMouseEvent, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_CONTROL, MOD_SHIFT,
    TME_LEAVE, TRACKMOUSEEVENT, VK_CONTROL, VK_ESCAPE, VK_F7, VK_F8, VK_F9, VK_SHIFT,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumChildWindows,
    EnumWindows, FindWindowW, GetMessageW, GetShellWindow, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, KillTimer, LoadCursorW, LoadIconW, LoadImageW,
    MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW, SendMessageW, SetCursor,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
    UpdateLayeredWindow, GWLP_USERDATA, GWL_EXSTYLE, HCURSOR, HICON, HTCAPTION, HWND_TOPMOST,
    ICON_BIG, ICON_SMALL, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_SIZEALL, IMAGE_ICON, LR_SHARED,
    MB_ICONERROR, MB_OK, MSG, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, ULW_ALPHA,
    WM_APP, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCLBUTTONDOWN, WM_SETICON, WM_TIMER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const WM_APP_RECORDER_EVENT: u32 = WM_APP + 101;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WAVE_FORMAT_PCM: u16 = 1;
const WM_MOUSELEAVE: u32 = 0x02A3;
const APP_ICON_RES_ID: PCWSTR = PCWSTR(1 as *const u16);

const SAMPLE_RATE: u32 = 48000;
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const BLOCK_ALIGN: u16 = CHANNELS * (BITS_PER_SAMPLE / 8);
const AVG_BYTES_PER_SEC: u32 = SAMPLE_RATE * (BLOCK_ALIGN as u32);
const AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK: i32 = 1;
const PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE: i32 = 0;
const PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE: i32 = 1;
const VT_BLOB: u16 = 65;

const HOTKEY_F9: i32 = 9001;
const HOTKEY_F8: i32 = 9002;
const HOTKEY_F7: i32 = 9003;
const HOTKEY_CTRL_SHIFT_R: i32 = 9004;
const TIMER_REC_ID: usize = 1;
const TIMER_TOAST_ID: usize = 2;

static SILENCE_BUFFER: [u8; (AVG_BYTES_PER_SEC as usize) / 20] =
    [0u8; (AVG_BYTES_PER_SEC as usize) / 20];
static LOG_LOCK: Mutex<()> = Mutex::new(());
static EXE_DIR: OnceLock<PathBuf> = OnceLock::new();
static JOB_HANDLE: OnceLock<isize> = OnceLock::new();
static FFMPEG_PATH: OnceLock<String> = OnceLock::new();
static PROBE_STATE: OnceLock<(Mutex<Option<ProbeInfo>>, Condvar)> = OnceLock::new();
static PIPE_SEQ: AtomicU32 = AtomicU32::new(1);

fn exe_dir() -> &'static PathBuf {
    EXE_DIR.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

fn log_error(message: &str) {
    if let Ok(_guard) = LOG_LOCK.lock() {
        let path = exe_dir().join("quicksnip_debug.log");
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let st = unsafe { GetLocalTime() };
            let _ = writeln!(
                f,
                "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}] [ERROR] {}",
                st.wYear,
                st.wMonth,
                st.wDay,
                st.wHour,
                st.wMinute,
                st.wSecond,
                st.wMilliseconds,
                message
            );
        }
    }
}

fn local_filename_timestamp() -> String {
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

#[inline]
fn to_wide_null(s: &str) -> Vec<u16> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend(s.encode_utf16());
    v.push(0);
    v
}

#[inline]
fn wide_slice_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[inline(always)]
fn lparam_logical(lparam: LPARAM, dpi: f32) -> (f32, f32) {
    let px = (lparam.0 & 0xFFFF) as i16 as f32;
    let py = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
    (px / dpi, py / dpi)
}

#[inline]
fn register_class_once(once: &Once, wc: &WNDCLASSW) {
    once.call_once(|| unsafe {
        RegisterClassW(wc);
    });
}

#[inline]
fn load_app_icon(hinst: HINSTANCE) -> HICON {
    unsafe { LoadIconW(hinst, APP_ICON_RES_ID).unwrap_or_default() }
}

#[inline]
fn apply_window_icons(hwnd: HWND, hinst: HINSTANCE, big_icon: HICON) {
    unsafe {
        if !big_icon.0.is_null() {
            SendMessageW(
                hwnd,
                WM_SETICON,
                WPARAM(ICON_BIG as usize),
                LPARAM(big_icon.0 as isize),
            );
        }
        if let Ok(sm) = LoadImageW(hinst, APP_ICON_RES_ID, IMAGE_ICON, 16, 16, LR_SHARED) {
            SendMessageW(
                hwnd,
                WM_SETICON,
                WPARAM(ICON_SMALL as usize),
                LPARAM(sm.0 as isize),
            );
        }
    }
}

fn assign_process_to_job(raw_process_handle: *mut std::ffi::c_void) {
    let job_raw = *JOB_HANDLE.get_or_init(|| unsafe {
        match CreateJobObjectW(None, None) {
            Ok(job) => {
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let _ = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                job.0 as isize
            }
            Err(_) => 0,
        }
    });
    if job_raw != 0 && !raw_process_handle.is_null() {
        unsafe {
            let _ = AssignProcessToJobObject(
                HANDLE(job_raw as *mut _),
                HANDLE(raw_process_handle),
            );
        }
    }
}

fn wait_child_exit(child: &mut Child, timeout_ms: u32) -> Option<bool> {
    let res = unsafe { WaitForSingleObject(HANDLE(child.as_raw_handle()), timeout_ms) };
    if res == WAIT_OBJECT_0 {
        child.wait().ok().map(|s| s.success())
    } else {
        let _ = child.kill();
        let _ = child.wait();
        None
    }
}

#[derive(Clone, Debug)]
struct CapturableWindowInfo {
    handle: isize,
    audio_root_pid: u32,
    process_name: String,
    title: String,
    physical_bounds: RECT,
}

impl CapturableWindowInfo {
    #[inline(always)]
    fn width(&self) -> i32 {
        (self.physical_bounds.right - self.physical_bounds.left).max(0)
    }

    #[inline(always)]
    fn height(&self) -> i32 {
        (self.physical_bounds.bottom - self.physical_bounds.top).max(0)
    }

    #[inline(always)]
    fn short_badge(&self) -> &str {
        if self.process_name.is_empty() {
            "App"
        } else {
            &self.process_name
        }
    }
}

#[derive(Clone, Copy)]
struct FpsSteps {
    vals: [i32; 3],
    len: usize,
}

impl FpsSteps {
    fn slice(&self) -> &[i32] {
        &self.vals[..self.len]
    }
}

fn get_dpi_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi > 0 {
        dpi as f32 / 96.0
    } else {
        1.0
    }
}

fn get_dynamic_fps_steps() -> FpsSteps {
    let mut max_hz = 60i32;
    unsafe {
        let mut dm = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        if EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut dm).as_bool()
            && dm.dmDisplayFrequency > 60
        {
            max_hz = (dm.dmDisplayFrequency as i32).min(240);
        }
    }
    if max_hz > 60 {
        FpsSteps { vals: [30, 60, max_hz], len: 3 }
    } else {
        FpsSteps { vals: [30, 60, 0], len: 2 }
    }
}

fn get_primary_screen_physical_rect() -> RECT {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN).max(1280);
        let h = GetSystemMetrics(SM_CYSCREEN).max(720);
        RECT {
            left: 0,
            top: 0,
            right: w & !1,
            bottom: h & !1,
        }
    }
}

fn get_virtual_screen_physical_rect() -> RECT {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if w <= 0 || h <= 0 {
            return get_primary_screen_physical_rect();
        }
        RECT {
            left,
            top,
            right: left + w,
            bottom: top + h,
        }
    }
}

fn try_get_window_physical_bounds(hwnd: HWND) -> Option<RECT> {
    unsafe {
        if hwnd.0.is_null() || !IsWindow(hwnd).as_bool() {
            return None;
        }
        let mut rect = RECT::default();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            size_of::<RECT>() as u32,
        )
        .is_ok()
            && (rect.right - rect.left) >= 32
            && (rect.bottom - rect.top) >= 32
        {
            return Some(rect);
        }
        if GetWindowRect(hwnd, &mut rect).is_ok()
            && (rect.right - rect.left) >= 32
            && (rect.bottom - rect.top) >= 32
        {
            return Some(rect);
        }
        None
    }
}

#[inline]
fn is_window_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked: i32 = 0;
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            size_of::<i32>() as u32,
        )
        .is_ok()
            && cloaked != 0
    }
}

type ProcMap = HashMap<u32, (u32, String)>;

fn capture_process_tree_snapshot() -> ProcMap {
    let mut dict = HashMap::with_capacity(256);
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return dict;
        };
        let mut pe = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                dict.insert(
                    pe.th32ProcessID,
                    (pe.th32ParentProcessID, wide_slice_to_string(&pe.szExeFile)),
                );
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    dict
}

struct ChildPidSearch {
    parent_pid: u32,
    found_pid: u32,
}

unsafe extern "system" fn enum_child_pid_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut ChildPidSearch);
    let mut c_pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut c_pid));
    if c_pid != 0 && c_pid != state.parent_pid {
        state.found_pid = c_pid;
        return BOOL(0);
    }
    BOOL(1)
}

fn resolve_app_process_info(hwnd: HWND, shared_map: Option<&ProcMap>) -> (u32, String) {
    let mut win_pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
    }
    if win_pid == 0 {
        return (0, "Unknown".to_string());
    }

    let owned_map;
    let proc_map = match shared_map {
        Some(m) => m,
        None => {
            owned_map = capture_process_tree_snapshot();
            &owned_map
        }
    };

    let mut exe_name: &str = proc_map.get(&win_pid).map(|e| e.1.as_str()).unwrap_or("");

    if exe_name.len() >= 20 && exe_name[..20].eq_ignore_ascii_case("applicationframehost") {
        let mut search = ChildPidSearch {
            parent_pid: win_pid,
            found_pid: win_pid,
        };
        unsafe {
            let _ = EnumChildWindows(
                hwnd,
                Some(enum_child_pid_proc),
                LPARAM(&mut search as *mut _ as isize),
            );
        }
        if search.found_pid != win_pid {
            win_pid = search.found_pid;
            exe_name = proc_map.get(&win_pid).map(|e| e.1.as_str()).unwrap_or("");
        }
    }

    let mut root_pid = win_pid;
    if !exe_name.is_empty() && !proc_map.is_empty() {
        let mut curr = win_pid;
        for _ in 0..16 {
            let Some(&(parent_pid, _)) = proc_map.get(&curr) else {
                break;
            };
            if parent_pid == 0 || parent_pid == curr {
                break;
            }
            if let Some((_, p_exe)) = proc_map.get(&parent_pid) {
                if p_exe.eq_ignore_ascii_case(exe_name) {
                    root_pid = parent_pid;
                    curr = parent_pid;
                    continue;
                }
            }
            break;
        }
    }

    let clean_name = if exe_name.len() > 4 && exe_name[exe_name.len() - 4..].eq_ignore_ascii_case(".exe") {
        exe_name[..exe_name.len() - 4].trim()
    } else {
        exe_name.trim()
    };

    let final_name = if clean_name.is_empty() {
        format!("PID {}", root_pid)
    } else {
        clean_name.to_string()
    };
    (root_pid, final_name)
}

struct EnumWinContext<'a> {
    shell_hwnd: isize,
    exclude1: isize,
    exclude2: isize,
    self_pid: u32,
    proc_map: &'a ProcMap,
    scratch_buf: Vec<u16>,
    results: Vec<CapturableWindowInfo>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumWinContext);
    let raw = hwnd.0 as isize;
    if raw == ctx.shell_hwnd || raw == ctx.exclude1 || raw == ctx.exclude2 {
        return BOOL(1);
    }
    if !IsWindowVisible(hwnd).as_bool() || is_window_cloaked(hwnd) {
        return BOOL(1);
    }
    if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0 {
        return BOOL(1);
    }
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return BOOL(1);
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 || pid == ctx.self_pid {
        return BOOL(1);
    }

    let mut bounds = RECT::default();
    if !IsIconic(hwnd).as_bool() {
        let Some(b) = try_get_window_physical_bounds(hwnd) else {
            return BOOL(1);
        };
        if (b.right - b.left) < 64 || (b.bottom - b.top) < 64 {
            return BOOL(1);
        }
        bounds = b;
    }

    let (audio_root_pid, proc_name) = resolve_app_process_info(hwnd, Some(ctx.proc_map));
    if proc_name.eq_ignore_ascii_case("TextInputHost")
        || proc_name.eq_ignore_ascii_case("SearchHost")
        || proc_name.eq_ignore_ascii_case("ShellExperienceHost")
    {
        return BOOL(1);
    }

    let needed = (len + 1) as usize;
    if ctx.scratch_buf.len() < needed {
        ctx.scratch_buf.resize(needed, 0);
    }
    let copied = GetWindowTextW(hwnd, &mut ctx.scratch_buf[..needed]);
    if copied <= 0 {
        return BOOL(1);
    }
    let raw_slice = &ctx.scratch_buf[..copied as usize];
    let start = raw_slice.iter().position(|&c| c > 0x20).unwrap_or(raw_slice.len());
    let end = raw_slice[start..]
        .iter()
        .rposition(|&c| c > 0x20)
        .map_or(start, |p| start + p + 1);
    let slice = &raw_slice[start..end];
    if slice.is_empty() {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(slice);

    ctx.results.push(CapturableWindowInfo {
        handle: raw,
        audio_root_pid,
        process_name: proc_name,
        title,
        physical_bounds: bounds,
    });
    BOOL(1)
}

fn enumerate_capturable_windows(exclude1: HWND, exclude2: HWND) -> Vec<CapturableWindowInfo> {
    let proc_map = capture_process_tree_snapshot();
    let mut ctx = EnumWinContext {
        shell_hwnd: unsafe { GetShellWindow().0 as isize },
        exclude1: exclude1.0 as isize,
        exclude2: exclude2.0 as isize,
        self_pid: unsafe { GetCurrentProcessId() },
        proc_map: &proc_map,
        scratch_buf: Vec::with_capacity(256),
        results: Vec::with_capacity(24),
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut ctx as *mut _ as isize),
        );
    }
    ctx.results
}

fn shell_open(target: &str) {
    let w_target = to_wide_null(target);
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(w_target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

fn explorer_select_file(file_path: &str) {
    let args = format!("/select,\"{}\"", file_path);
    let w_args = to_wide_null(&args);
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("explorer.exe"),
            PCWSTR(w_args.as_ptr()),
            None,
            SW_SHOWNORMAL,
        );
    }
}

#[repr(C)]
struct AudioClientProcessLoopbackParams {
    target_process_id: u32,
    process_loopback_mode: i32,
}

#[repr(C)]
struct AudioClientActivationParams {
    activation_type: i32,
    process_loopback_params: AudioClientProcessLoopbackParams,
}

#[repr(C)]
struct RawPropVariantBlob {
    vt: u16,
    w_reserved1: u16,
    w_reserved2: u16,
    w_reserved3: u16,
    cb_size: u32,
    _pad: u32,
    p_blob_data: *mut u8,
}

#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
struct AudioActivationHandler {
    state: Arc<(Mutex<Option<IAudioClient>>, Condvar)>,
}

impl IAgileObject_Impl for AudioActivationHandler_Impl {}

impl IActivateAudioInterfaceCompletionHandler_Impl for AudioActivationHandler_Impl {
    fn ActivateCompleted(
        &self,
        activateoperation: Option<&IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let mut client_opt = None;
        if let Some(op) = activateoperation {
            let mut act_hr = HRESULT(0);
            let mut unk: Option<IUnknown> = None;
            unsafe {
                if op.GetActivateResult(&mut act_hr, &mut unk).is_ok() && act_hr.is_ok() {
                    if let Some(u) = unk {
                        if let Ok(c) = u.cast::<IAudioClient>() {
                            client_opt = Some(c);
                        }
                    }
                }
            }
        }
        let (lock, cvar) = &*self.state;
        if let Ok(mut guard) = lock.lock() {
            *guard = client_opt;
            cvar.notify_all();
        }
        Ok(())
    }
}

struct WasapiAudioPipeStreamer {
    pipe_path: String,
    stop_flag: Arc<AtomicBool>,
    stop_event: isize,
    pump_thread: Option<thread::JoinHandle<()>>,
}

impl WasapiAudioPipeStreamer {
    fn start(
        pipe_path: String,
        target_process_id: u32,
        isolate_target_process: bool,
    ) -> Result<Self, String> {
        let w_pipe = to_wide_null(&pipe_path);

        let pipe_handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(w_pipe.as_ptr()),
                PIPE_ACCESS_OUTBOUND | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                AVG_BYTES_PER_SEC / 2,
                0,
                0,
                None,
            )
        };
        if pipe_handle.is_invalid() || pipe_handle.0.is_null() {
            return Err("Không thể tạo Named Pipe cho luồng âm thanh.".to_string());
        }

        let stop_event = unsafe { CreateEventW(None, true, false, None) }.map_err(|_| {
            unsafe {
                let _ = CloseHandle(pipe_handle);
            }
            "Không thể tạo sự kiện đồng bộ âm thanh.".to_string()
        })?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let init_pair = Arc::new((Mutex::new(None::<bool>), Condvar::new()));

        let thread_stop_flag = Arc::clone(&stop_flag);
        let thread_init_pair = Arc::clone(&init_pair);
        let pipe_raw = pipe_handle.0 as isize;
        let stop_ev_raw = stop_event.0 as isize;
        let isolate = isolate_target_process && target_process_id != 0;

        let handle = thread::Builder::new()
            .name("QuickSnip_WASAPI_Pump".to_string())
            .spawn(move || {
                run_audio_pump(
                    HANDLE(pipe_raw as *mut _),
                    HANDLE(stop_ev_raw as *mut _),
                    target_process_id,
                    isolate,
                    thread_stop_flag,
                    thread_init_pair,
                );
            })
            .map_err(|e| {
                unsafe {
                    let _ = CloseHandle(pipe_handle);
                    let _ = CloseHandle(stop_event);
                }
                e.to_string()
            })?;

        let (lock, cvar) = &*init_pair;
        let guard = lock.lock().unwrap();
        let (guard, _) = cvar
            .wait_timeout_while(guard, Duration::from_millis(2000), |v| v.is_none())
            .unwrap();

        if *guard != Some(true) {
            stop_flag.store(true, Ordering::SeqCst);
            unsafe {
                let _ = SetEvent(stop_event);
            }
            let _ = handle.join();
            unsafe {
                let _ = CloseHandle(stop_event);
            }
            return Err("Không thể khởi tạo thiết bị thu âm WASAPI.".to_string());
        }

        Ok(Self {
            pipe_path,
            stop_flag,
            stop_event: stop_ev_raw,
            pump_thread: Some(handle),
        })
    }

    fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if self.stop_event != 0 {
            unsafe {
                let _ = SetEvent(HANDLE(self.stop_event as *mut _));
            }
        }
        if let Some(t) = self.pump_thread.take() {
            let _ = t.join();
        }
        if self.stop_event != 0 {
            unsafe {
                let _ = CloseHandle(HANDLE(self.stop_event as *mut _));
            }
            self.stop_event = 0;
        }
    }
}

impl Drop for WasapiAudioPipeStreamer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn signal_init(init_pair: &Arc<(Mutex<Option<bool>>, Condvar)>, ok: bool) {
    let (lock, cvar) = &**init_pair;
    if let Ok(mut g) = lock.lock() {
        *g = Some(ok);
        cvar.notify_all();
    }
}

fn run_audio_pump(
    pipe_handle: HANDLE,
    stop_event: HANDLE,
    target_process_id: u32,
    isolate_target_process: bool,
    stop_flag: Arc<AtomicBool>,
    init_pair: Arc<(Mutex<Option<bool>>, Condvar)>,
) {
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
        let hr_com = CoInitializeEx(None, COINIT_MULTITHREADED);

        let result = run_audio_pump_inner(
            pipe_handle,
            stop_event,
            target_process_id,
            isolate_target_process,
            &stop_flag,
            &init_pair,
        );
        if !result {
            signal_init(&init_pair, false);
        }

        let _ = CloseHandle(pipe_handle);
        if hr_com.is_ok() {
            CoUninitialize();
        }
    }
}

unsafe fn cancel_pending_io(pipe_handle: HANDLE, ov: &mut OVERLAPPED) {
    let _ = CancelIoEx(pipe_handle, Some(ov));
    let mut dummy = 0u32;
    let _ = GetOverlappedResult(pipe_handle, ov, &mut dummy, true);
}

unsafe fn run_audio_pump_inner(
    pipe_handle: HANDLE,
    stop_event: HANDLE,
    target_process_id: u32,
    isolate_target_process: bool,
    stop_flag: &AtomicBool,
    init_pair: &Arc<(Mutex<Option<bool>>, Condvar)>,
) -> bool {
    let Some(audio_client) = create_configured_audio_client(target_process_id, isolate_target_process)
    else {
        return false;
    };

    let wfx = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM,
        nChannels: CHANNELS,
        nSamplesPerSec: SAMPLE_RATE,
        nAvgBytesPerSec: AVG_BYTES_PER_SEC,
        nBlockAlign: BLOCK_ALIGN,
        wBitsPerSample: BITS_PER_SAMPLE,
        cbSize: 0,
    };

    let base_flags = AUDCLNT_STREAMFLAGS_LOOPBACK
        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    let buffer_duration_hns = 250_000i64;

    let mut use_event = audio_client
        .Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            base_flags | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            buffer_duration_hns,
            0,
            &wfx,
            None,
        )
        .is_ok();

    if !use_event
        && audio_client
            .Initialize(AUDCLNT_SHAREMODE_SHARED, base_flags, buffer_duration_hns, 0, &wfx, None)
            .is_err()
    {
        return false;
    }

    let mut sample_event = HANDLE::default();
    if use_event {
        if let Ok(ev) = CreateEventW(None, false, false, None) {
            if audio_client.SetEventHandle(ev).is_err() {
                let _ = CloseHandle(ev);
                use_event = false;
            } else {
                sample_event = ev;
            }
        } else {
            use_event = false;
        }
    }

    let capture_client: IAudioCaptureClient = match audio_client.GetService() {
        Ok(c) => c,
        Err(_) => {
            if !sample_event.0.is_null() {
                let _ = CloseHandle(sample_event);
            }
            return false;
        }
    };

    let Ok(io_event) = CreateEventW(None, true, false, None) else {
        if !sample_event.0.is_null() {
            let _ = CloseHandle(sample_event);
        }
        return false;
    };

    signal_init(init_pair, true);

    let mut ov = OVERLAPPED {
        hEvent: io_event,
        ..Default::default()
    };
    let conn_res = ConnectNamedPipe(pipe_handle, Some(&mut ov));
    if conn_res.is_err() && GetLastError() == ERROR_IO_PENDING {
        let handles = [io_event, stop_event];
        let wait_res = WaitForMultipleObjects(&handles, false, 10_000);
        if wait_res != WAIT_OBJECT_0 || stop_flag.load(Ordering::Relaxed) {
            cancel_pending_io(pipe_handle, &mut ov);
            let _ = CloseHandle(io_event);
            if !sample_event.0.is_null() {
                let _ = CloseHandle(sample_event);
            }
            return true;
        }
        let mut dummy = 0u32;
        if GetOverlappedResult(pipe_handle, &ov, &mut dummy, false).is_err() {
            let _ = CloseHandle(io_event);
            if !sample_event.0.is_null() {
                let _ = CloseHandle(sample_event);
            }
            return true;
        }
    }

    if audio_client.Start().is_err() {
        let _ = CloseHandle(io_event);
        if !sample_event.0.is_null() {
            let _ = CloseHandle(sample_event);
        }
        return true;
    }

    let mut freq = 0i64;
    let _ = QueryPerformanceFrequency(&mut freq);
    let freq = freq.max(1);
    let silence_threshold_ticks = (freq * 22) / 1000;
    let mut last_audio_tick = 0i64;
    let _ = QueryPerformanceCounter(&mut last_audio_tick);
    let wait_handles = [sample_event, stop_event];

    while !stop_flag.load(Ordering::Relaxed) {
        if use_event && !sample_event.0.is_null() {
            let res = WaitForMultipleObjects(&wait_handles, false, 22);
            if res.0 == WAIT_OBJECT_0.0 + 1 {
                break;
            }
        } else if WaitForSingleObject(stop_event, 10) == WAIT_OBJECT_0 {
            break;
        }

        let mut received_packets = false;
        while let Ok(packet_frames) = capture_client.GetNextPacketSize() {
            if packet_frames == 0 {
                break;
            }
            let mut data_ptr = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;
            if capture_client
                .GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)
                .is_err()
            {
                break;
            }

            let byte_count = (num_frames as usize) * (BLOCK_ALIGN as usize);
            let mut write_ok = true;
            if byte_count > 0 {
                if (flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)) != 0 || data_ptr.is_null() {
                    write_ok = write_silence_bytes(pipe_handle, byte_count, &mut ov, stop_event);
                } else {
                    let slice = std::slice::from_raw_parts(data_ptr, byte_count);
                    write_ok = write_pipe_all(pipe_handle, slice, &mut ov, stop_event);
                }
                received_packets = true;
            }

            let _ = capture_client.ReleaseBuffer(num_frames);
            if !write_ok {
                stop_flag.store(true, Ordering::Relaxed);
                break;
            }
        }

        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        let mut now_tick = 0i64;
        let _ = QueryPerformanceCounter(&mut now_tick);
        if received_packets {
            last_audio_tick = now_tick;
        } else {
            let silent_ticks = now_tick - last_audio_tick;
            if silent_ticks >= silence_threshold_ticks {
                let silent_frames = ((silent_ticks * (SAMPLE_RATE as i64)) / freq) as i32;
                if silent_frames > 0 {
                    let clamped = silent_frames.min((SAMPLE_RATE as i32) / 20);
                    if !write_silence_bytes(
                        pipe_handle,
                        (clamped as usize) * (BLOCK_ALIGN as usize),
                        &mut ov,
                        stop_event,
                    ) {
                        break;
                    }
                    last_audio_tick += ((clamped as i64) * freq) / (SAMPLE_RATE as i64);
                }
            }
        }
    }

    let _ = audio_client.Stop();
    let _ = CloseHandle(io_event);
    if !sample_event.0.is_null() {
        let _ = CloseHandle(sample_event);
    }
    true
}

#[inline(always)]
unsafe fn write_silence_bytes(
    pipe_handle: HANDLE,
    mut byte_count: usize,
    ov: &mut OVERLAPPED,
    stop_event: HANDLE,
) -> bool {
    while byte_count > 0 {
        let chunk = byte_count.min(SILENCE_BUFFER.len());
        if !write_pipe_all(pipe_handle, &SILENCE_BUFFER[..chunk], ov, stop_event) {
            return false;
        }
        byte_count -= chunk;
    }
    true
}

#[inline(always)]
unsafe fn write_pipe_all(
    pipe_handle: HANDLE,
    mut data: &[u8],
    ov: &mut OVERLAPPED,
    stop_event: HANDLE,
) -> bool {
    while !data.is_empty() {
        ov.Anonymous.Anonymous.Offset = 0;
        ov.Anonymous.Anonymous.OffsetHigh = 0;
        let mut written = 0u32;
        if WriteFile(pipe_handle, Some(data), Some(&mut written), Some(ov)).is_err() {
            if GetLastError() != ERROR_IO_PENDING {
                return false;
            }
            let handles = [ov.hEvent, stop_event];
            if WaitForMultipleObjects(&handles, false, 2000) != WAIT_OBJECT_0 {
                cancel_pending_io(pipe_handle, ov);
                return false;
            }
            if GetOverlappedResult(pipe_handle, ov, &mut written, false).is_err() {
                return false;
            }
        }
        if written == 0 {
            return false;
        }
        data = &data[written as usize..];
    }
    true
}

unsafe fn create_configured_audio_client(
    target_process_id: u32,
    isolate_target_process: bool,
) -> Option<IAudioClient> {
    let pid = if isolate_target_process {
        target_process_id
    } else {
        GetCurrentProcessId()
    };
    let mode = if isolate_target_process {
        PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE
    } else {
        PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE
    };

    if let Some(client) = try_activate_process_loopback_client(pid, mode) {
        return Some(client);
    }
    if !isolate_target_process {
        return try_activate_default_endpoint_client();
    }
    None
}

unsafe fn try_activate_process_loopback_client(
    process_id: u32,
    loopback_mode: i32,
) -> Option<IAudioClient> {
    let state = Arc::new((Mutex::new(None::<IAudioClient>), Condvar::new()));
    let handler: IActivateAudioInterfaceCompletionHandler = AudioActivationHandler {
        state: Arc::clone(&state),
    }
    .into();

    let mut act_params = AudioClientActivationParams {
        activation_type: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        process_loopback_params: AudioClientProcessLoopbackParams {
            target_process_id: process_id,
            process_loopback_mode: loopback_mode,
        },
    };

    let raw_pv = RawPropVariantBlob {
        vt: VT_BLOB,
        w_reserved1: 0,
        w_reserved2: 0,
        w_reserved3: 0,
        cb_size: size_of::<AudioClientActivationParams>() as u32,
        _pad: 0,
        p_blob_data: &mut act_params as *mut _ as *mut u8,
    };

    if let Err(e) = ActivateAudioInterfaceAsync(
        w!("VAD\\Process_Loopback"),
        &IAudioClient::IID as *const GUID,
        Some(&raw_pv as *const RawPropVariantBlob as *const _),
        &handler,
    ) {
        log_error(&format!("ActivateAudioInterfaceAsync error: {}", e));
        return None;
    }

    let (lock, cvar) = &*state;
    let guard = lock.lock().unwrap();
    let (mut guard, _) = cvar
        .wait_timeout_while(guard, Duration::from_millis(1500), |v| v.is_none())
        .unwrap();
    guard.take()
}

unsafe fn try_activate_default_endpoint_client() -> Option<IAudioClient> {
    let enumerator: IMMDeviceEnumerator =
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
    device.Activate::<IAudioClient>(CLSCTX_ALL, None).ok()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureTargetMode {
    FullScreen,
    CustomRegion,
    AppWindow,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecorderState {
    Idle,
    Starting,
    Recording,
    Pausing,
    Paused,
    Resuming,
    Stopping,
}

#[derive(Clone)]
enum RecorderEvent {
    Started,
    Paused,
    Resumed,
    Completed(String),
    Failed(String),
    Cancelled,
}

#[derive(Clone)]
struct RecordingConfig {
    mode: CaptureTargetMode,
    record_audio: bool,
    auto_minimize_on_record: bool,
    selected_physical_region: Option<(i32, i32, i32, i32)>,
    target_app_window: Option<CapturableWindowInfo>,
    fps: i32,
    draw_mouse: bool,
    output_directory: PathBuf,
}

impl RecordingConfig {
    fn new() -> Self {
        let output_directory = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .map(|p| p.join("Videos"))
            .filter(|v| v.exists())
            .map(|v| v.join("Captures"))
            .unwrap_or_else(|| exe_dir().join("Recordings"));

        Self {
            mode: CaptureTargetMode::FullScreen,
            record_audio: true,
            auto_minimize_on_record: true,
            selected_physical_region: None,
            target_app_window: None,
            fps: 60,
            draw_mouse: true,
            output_directory,
        }
    }

    fn get_new_file_path(&self) -> PathBuf {
        let _ = fs::create_dir_all(&self.output_directory);
        let mut path_name = String::with_capacity(48);
        match (&self.mode, &self.target_app_window) {
            (CaptureTargetMode::AppWindow, Some(app)) => {
                path_name.push_str("App_");
                for c in app.process_name.chars() {
                    if !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                        path_name.push(c);
                    }
                }
            }
            _ => path_name.push_str("Recording"),
        }
        let _ = write!(path_name, "_{}.mp4", local_filename_timestamp());
        self.output_directory.join(path_name)
    }
}

struct ProbeInfo {
    video_encoder_args: &'static [&'static str],
    encoder_badge: &'static str,
    supports_ddagrab: bool,
    supports_dda_nv12: bool,
}

const FALLBACK_X264_ARGS: &[&str] = &[
    "-c:v",
    "libx264",
    "-preset",
    "ultrafast",
    "-tune",
    "zerolatency",
    "-crf",
    "21",
    "-threads",
    "0",
    "-pix_fmt",
    "yuv420p",
];

fn get_ffmpeg_path() -> &'static str {
    FFMPEG_PATH.get_or_init(resolve_ffmpeg_path)
}

fn warm_up_probe() {
    let pair = PROBE_STATE.get_or_init(|| (Mutex::new(None), Condvar::new()));
    thread::spawn(move || {
        let ffmpeg = get_ffmpeg_path();
        let mut info = ProbeInfo {
            video_encoder_args: FALLBACK_X264_ARGS,
            encoder_badge: "x264",
            supports_ddagrab: false,
            supports_dda_nv12: false,
        };

        if !ffmpeg.is_empty() {
            const CANDIDATES: [(&str, &[&str]); 4] = [
                (
                    "NVENC",
                    &[
                        "-c:v", "h264_nvenc", "-preset", "p1", "-tune", "ull",
                        "-rc", "constqp", "-qp", "20", "-pix_fmt", "yuv420p",
                    ],
                ),
                (
                    "QSV",
                    &[
                        "-c:v", "h264_qsv", "-preset", "veryfast",
                        "-global_quality", "20", "-pix_fmt", "yuv420p",
                    ],
                ),
                (
                    "AMF",
                    &[
                        "-c:v", "h264_amf", "-usage", "ultralowlatency", "-quality", "speed",
                        "-rc", "cqp", "-qp_i", "20", "-qp_p", "20", "-pix_fmt", "yuv420p",
                    ],
                ),
                (
                    "MF-HW",
                    &[
                        "-c:v", "h264_mf", "-hw_encoding", "true",
                        "-rate_control", "u_vbr", "-b:v", "16M", "-maxrate", "28M",
                        "-scenario", "live_streaming", "-pix_fmt", "nv12",
                    ],
                ),
            ];

            let (dda_bgra, dda_nv12, enc_ok) = thread::scope(|s| {
                let h_dda = s.spawn(|| {
                    run_ffmpeg_probe(
                        ffmpeg,
                        &[
                            "-f",
                            "lavfi",
                            "-i",
                            "ddagrab=output_idx=0:framerate=60:video_size=64x64,hwdownload,format=bgra",
                            "-frames:v",
                            "1",
                            "-f",
                            "null",
                            "-",
                        ],
                        &[],
                    )
                });
                let h_dda_nv12 = s.spawn(|| {
                    run_ffmpeg_probe(
                        ffmpeg,
                        &[
                            "-f",
                            "lavfi",
                            "-i",
                            "ddagrab=output_idx=0:framerate=60:video_size=64x64,scale_d3d11=format=nv12,hwdownload,format=nv12",
                            "-frames:v",
                            "1",
                            "-f",
                            "null",
                            "-",
                        ],
                        &[],
                    )
                });
                let h0 = s.spawn(|| {
                    run_ffmpeg_probe(
                        ffmpeg,
                        &["-f", "lavfi", "-i", "color=c=black:s=256x256:r=60", "-frames:v", "1"],
                        CANDIDATES[0].1,
                    )
                });
                let h1 = s.spawn(|| {
                    run_ffmpeg_probe(
                        ffmpeg,
                        &["-f", "lavfi", "-i", "color=c=black:s=256x256:r=60", "-frames:v", "1"],
                        CANDIDATES[1].1,
                    )
                });
                let h2 = s.spawn(|| {
                    run_ffmpeg_probe(
                        ffmpeg,
                        &["-f", "lavfi", "-i", "color=c=black:s=256x256:r=60", "-frames:v", "1"],
                        CANDIDATES[2].1,
                    )
                });
                let h3 = s.spawn(|| {
                    run_ffmpeg_probe(
                        ffmpeg,
                        &["-f", "lavfi", "-i", "color=c=black:s=256x256:r=60", "-frames:v", "1"],
                        CANDIDATES[3].1,
                    )
                });

                let dda = h_dda.join().unwrap_or(false);
                let dda_nv12 = h_dda_nv12.join().unwrap_or(false);
                let wins = [
                    h0.join().unwrap_or(false),
                    h1.join().unwrap_or(false),
                    h2.join().unwrap_or(false),
                    h3.join().unwrap_or(false),
                ];
                (dda, dda_nv12, wins)
            });

            info.supports_ddagrab = dda_bgra || dda_nv12;
            info.supports_dda_nv12 = dda_nv12;
            for (i, &ok) in enc_ok.iter().enumerate() {
                if ok {
                    info.encoder_badge = CANDIDATES[i].0;
                    info.video_encoder_args = CANDIDATES[i].1;
                    break;
                }
            }
        }

        let (lock, cvar) = pair;
        if let Ok(mut g) = lock.lock() {
            *g = Some(info);
            cvar.notify_all();
        }
    });
}

fn get_probe_info(wait_ms: u64) -> (&'static [&'static str], &'static str, bool, bool) {
    let pair = PROBE_STATE.get_or_init(|| {
        warm_up_probe();
        (Mutex::new(None), Condvar::new())
    });
    let (lock, cvar) = pair;
    let guard = lock.lock().unwrap();
    let guard = if guard.is_none() && wait_ms > 0 {
        cvar.wait_timeout_while(guard, Duration::from_millis(wait_ms), |v| v.is_none())
            .unwrap()
            .0
    } else {
        guard
    };
    if let Some(info) = &*guard {
        (
            info.video_encoder_args,
            info.encoder_badge,
            info.supports_ddagrab,
            info.supports_dda_nv12,
        )
    } else {
        (FALLBACK_X264_ARGS, "x264", false, false)
    }
}

fn run_ffmpeg_probe(ffmpeg: &str, prefix_args: &[&str], enc_args: &[&str]) -> bool {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error"])
        .args(prefix_args);
    if !enc_args.is_empty() {
        cmd.args(enc_args).args(["-f", "null", "-"]);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);

    let Ok(mut child) = cmd.spawn() else {
        return false;
    };
    wait_child_exit(&mut child, 3500).unwrap_or(false)
}

fn is_valid_binary(path: &Path, min_bytes: u64) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() >= min_bytes)
        .unwrap_or(false)
}

fn resolve_ffmpeg_path() -> String {
    const MIN_VALID_SIZE: u64 = 5_000_000;

    let local_path = exe_dir().join("ffmpeg.exe");
    if is_valid_binary(&local_path, MIN_VALID_SIZE) {
        return local_path.to_string_lossy().into_owned();
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let app_data_path = PathBuf::from(local_app_data)
            .join("QuickSnipRecorder")
            .join("ffmpeg.exe");

        #[cfg(has_embedded_ffmpeg)]
        {
            const EMBEDDED_FFMPEG: &[u8] = include_bytes!("../ffmpeg.exe");
            if EMBEDDED_FFMPEG.len() as u64 > MIN_VALID_SIZE {
                if let Some(parent) = app_data_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let needs_write = fs::metadata(&app_data_path)
                    .map(|m| m.len() != EMBEDDED_FFMPEG.len() as u64)
                    .unwrap_or(true);
                if needs_write {
                    let tmp_path = app_data_path.with_extension("exe.tmp");
                    if fs::write(&tmp_path, EMBEDDED_FFMPEG).is_ok() {
                        let _ = fs::rename(&tmp_path, &app_data_path);
                    }
                    let _ = fs::remove_file(&tmp_path);
                }
            }
        }

        if is_valid_binary(&app_data_path, MIN_VALID_SIZE) {
            return app_data_path.to_string_lossy().into_owned();
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("ffmpeg.exe");
            if is_valid_binary(&candidate, MIN_VALID_SIZE) {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    "ffmpeg.exe".to_string()
}

struct RecordingSession {
    session_id: u32,
    segment_paths: Vec<PathBuf>,
    current_segment_path: Option<PathBuf>,
    current_process: Option<Child>,
    current_audio_streamer: Option<WasapiAudioPipeStreamer>,
    final_output_path: PathBuf,
    is_discarding: bool,
    last_error: Arc<Mutex<String>>,
}

struct RecorderInner {
    state: RecorderState,
    config: RecordingConfig,
    session_counter: u32,
    current_session: Option<RecordingSession>,
}

enum RecorderCmd {
    Start(u32, RecordingConfig),
    Pause(u32),
    Resume(u32, RecordingConfig),
    Stop(u32),
}

struct FFmpegRecorder {
    inner: Mutex<RecorderInner>,
    cmd_queue: Mutex<VecDeque<RecorderCmd>>,
    cmd_cv: Condvar,
    self_weak: OnceLock<std::sync::Weak<FFmpegRecorder>>,
    shutdown: AtomicBool,
    timer_active: AtomicBool,
    notify_hwnd: AtomicIsize,
    events: Mutex<VecDeque<RecorderEvent>>,
}

impl FFmpegRecorder {
    fn new() -> Arc<Self> {
        let rec = Arc::new(Self {
            inner: Mutex::new(RecorderInner {
                state: RecorderState::Idle,
                config: RecordingConfig::new(),
                session_counter: 0,
                current_session: None,
            }),
            cmd_queue: Mutex::new(VecDeque::with_capacity(8)),
            cmd_cv: Condvar::new(),
            self_weak: OnceLock::new(),
            shutdown: AtomicBool::new(false),
            timer_active: AtomicBool::new(false),
            notify_hwnd: AtomicIsize::new(0),
            events: Mutex::new(VecDeque::with_capacity(8)),
        });
        let _ = rec.self_weak.set(Arc::downgrade(&rec));

        let weak = Arc::downgrade(&rec);
        let _ = thread::Builder::new()
            .name("QuickSnip_Worker".to_string())
            .spawn(move || {
                while let Some(this) = weak.upgrade() {
                    let cmd = {
                        let mut q = this.cmd_queue.lock().unwrap();
                        while q.is_empty() && !this.shutdown.load(Ordering::Relaxed) {
                            q = this.cmd_cv.wait(q).unwrap();
                        }
                        if this.shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                        q.pop_front()
                    };
                    if let Some(cmd) = cmd {
                        this.process_command(cmd);
                    }
                }
            });

        rec
    }

    fn process_command(&self, cmd: RecorderCmd) {
        match cmd {
            RecorderCmd::Start(session_id, config) => {
                let _ = get_probe_info(2500);
                {
                    let g = self.inner.lock().unwrap();
                    let still_starting = g.state == RecorderState::Starting
                        && g.current_session.as_ref().map(|s| s.session_id) == Some(session_id);
                    if !still_starting {
                        return;
                    }
                }
                self.acquire_high_res_timer();
                match self.start_new_segment_locked(session_id, &config) {
                    Ok(()) => {
                        let mut emit = false;
                        {
                            let mut g = self.inner.lock().unwrap();
                            if g.state == RecorderState::Starting
                                && g.current_session.as_ref().map(|s| s.session_id)
                                    == Some(session_id)
                            {
                                g.state = RecorderState::Recording;
                                emit = true;
                            }
                        }
                        if emit {
                            self.emit_event(RecorderEvent::Started);
                        }
                    }
                    Err(e) => {
                        self.release_high_res_timer();
                        let mut reset = false;
                        {
                            let mut g = self.inner.lock().unwrap();
                            if g.current_session.as_ref().map(|s| s.session_id) == Some(session_id)
                            {
                                g.state = RecorderState::Idle;
                                g.current_session = None;
                                reset = true;
                            }
                        }
                        if reset {
                            self.emit_event(RecorderEvent::Failed(e));
                        }
                    }
                }
            }
            RecorderCmd::Pause(session_id) => {
                self.stop_current_segment_sync();
                let mut emit = false;
                {
                    let mut g = self.inner.lock().unwrap();
                    if g.state == RecorderState::Pausing
                        && g.current_session.as_ref().map(|s| s.session_id) == Some(session_id)
                    {
                        g.state = RecorderState::Paused;
                        emit = true;
                    }
                }
                if emit {
                    self.emit_event(RecorderEvent::Paused);
                }
            }
            RecorderCmd::Resume(session_id, config) => {
                {
                    let g = self.inner.lock().unwrap();
                    let still_resuming = g.state == RecorderState::Resuming
                        && g.current_session.as_ref().map(|s| s.session_id) == Some(session_id);
                    if !still_resuming {
                        return;
                    }
                }
                match self.start_new_segment_locked(session_id, &config) {
                    Ok(()) => {
                        let mut emit = false;
                        {
                            let mut g = self.inner.lock().unwrap();
                            if g.state == RecorderState::Resuming
                                && g.current_session.as_ref().map(|s| s.session_id)
                                    == Some(session_id)
                            {
                                g.state = RecorderState::Recording;
                                emit = true;
                            }
                        }
                        if emit {
                            self.emit_event(RecorderEvent::Resumed);
                        }
                    }
                    Err(err) => {
                        self.release_high_res_timer();
                        let sess_data = {
                            let mut g = self.inner.lock().unwrap();
                            if g.current_session.as_ref().map(|s| s.session_id) == Some(session_id)
                            {
                                g.state = RecorderState::Idle;
                                g.current_session.take()
                            } else {
                                None
                            }
                        };
                        if let Some(sess) = sess_data {
                            if sess.segment_paths.is_empty() {
                                self.emit_event(RecorderEvent::Failed(err));
                            } else {
                                match finalize_video(
                                    session_id,
                                    &sess.segment_paths,
                                    &sess.final_output_path,
                                    &err,
                                ) {
                                    Ok(out) => self.emit_event(RecorderEvent::Completed(out)),
                                    Err(fe) => self.emit_event(RecorderEvent::Failed(fe)),
                                }
                            }
                        }
                    }
                }
            }
            RecorderCmd::Stop(session_id) => {
                self.stop_current_segment_sync();
                self.release_high_res_timer();

                let sess_opt = {
                    let mut g = self.inner.lock().unwrap();
                    if g.current_session.as_ref().map(|s| s.session_id) == Some(session_id) {
                        g.state = RecorderState::Idle;
                        g.current_session.take()
                    } else {
                        None
                    }
                };

                let Some(sess) = sess_opt else {
                    return;
                };

                if sess.is_discarding {
                    for seg in &sess.segment_paths {
                        let _ = fs::remove_file(seg);
                    }
                    self.emit_event(RecorderEvent::Cancelled);
                } else {
                    let last_error = sess.last_error.lock().map(|e| e.clone()).unwrap_or_default();
                    match finalize_video(
                        session_id,
                        &sess.segment_paths,
                        &sess.final_output_path,
                        &last_error,
                    ) {
                        Ok(out) => self.emit_event(RecorderEvent::Completed(out)),
                        Err(err) => self.emit_event(RecorderEvent::Failed(err)),
                    }
                }
            }
        }
    }

    fn enqueue_cmd(&self, cmd: RecorderCmd) {
        if let Ok(mut q) = self.cmd_queue.lock() {
            q.push_back(cmd);
            self.cmd_cv.notify_one();
        }
    }

    fn set_notify_hwnd(&self, hwnd: HWND) {
        self.notify_hwnd.store(hwnd.0 as isize, Ordering::SeqCst);
    }

    fn pop_event(&self) -> Option<RecorderEvent> {
        self.events.lock().ok()?.pop_front()
    }

    fn emit_event(&self, ev: RecorderEvent) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(mut q) = self.events.lock() {
            q.push_back(ev);
        }
        let raw = self.notify_hwnd.load(Ordering::SeqCst);
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(
                    HWND(raw as *mut _),
                    WM_APP_RECORDER_EVENT,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }

    fn state(&self) -> RecorderState {
        self.inner.lock().unwrap().state
    }

    fn with_config<R, F: FnOnce(&RecordingConfig) -> R>(&self, f: F) -> R {
        let g = self.inner.lock().unwrap();
        f(&g.config)
    }

    fn update_config<F: FnOnce(&mut RecordingConfig)>(&self, f: F) {
        let mut g = self.inner.lock().unwrap();
        f(&mut g.config);
    }

    fn active_encoder_badge(&self) -> (&'static str, bool) {
        let (_, badge, ddagrab, _) = get_probe_info(0);
        (badge, ddagrab)
    }

    fn acquire_high_res_timer(&self) {
        if !self.timer_active.swap(true, Ordering::SeqCst) {
            unsafe {
                timeBeginPeriod(1);
            }
        }
    }

    fn release_high_res_timer(&self) {
        if self.timer_active.swap(false, Ordering::SeqCst) {
            unsafe {
                timeEndPeriod(1);
            }
        }
    }

    fn start_recording(&self) -> Result<(), &'static str> {
        let (session_id, config_snap) = {
            let mut g = self.inner.lock().unwrap();
            if g.state != RecorderState::Idle {
                let msg = match g.state {
                    RecorderState::Starting => "Đang khởi động tiến trình quay...",
                    RecorderState::Recording => "Đang trong quá trình quay.",
                    RecorderState::Pausing => "Đang tạm dừng quay...",
                    RecorderState::Paused => "Đang tạm dừng quay.",
                    RecorderState::Resuming => "Đang tiếp tục quay...",
                    RecorderState::Stopping => "Đang lưu video trước đó, vui lòng chờ...",
                    RecorderState::Idle => "",
                };
                return Err(msg);
            }
            g.state = RecorderState::Starting;
            g.session_counter += 1;
            let id = g.session_counter;
            let cfg = g.config.clone();
            let final_output = cfg.get_new_file_path();
            g.current_session = Some(RecordingSession {
                session_id: id,
                segment_paths: Vec::with_capacity(4),
                current_segment_path: None,
                current_process: None,
                current_audio_streamer: None,
                final_output_path: final_output,
                is_discarding: false,
                last_error: Arc::new(Mutex::new(String::new())),
            });
            (id, cfg)
        };

        self.enqueue_cmd(RecorderCmd::Start(session_id, config_snap));
        Ok(())
    }

    fn start_new_segment_locked(
        &self,
        expected_session_id: u32,
        config: &RecordingConfig,
    ) -> Result<(), String> {
        let (x, y, w, h, fits_primary, audio_pid, isolate_audio) =
            resolve_capture_geometry(config)?;

        let seg_idx = {
            let g = self.inner.lock().unwrap();
            let Some(sess) = &g.current_session else {
                return Err("Phiên quay không tồn tại.".to_string());
            };
            if sess.session_id != expected_session_id {
                return Err("Phiên quay đã thay đổi.".to_string());
            }
            sess.segment_paths.len()
        };

        let seg_path = std::env::temp_dir().join(format!(
            "quicksnip_s{}_seg{}.mp4",
            expected_session_id, seg_idx
        ));

        let mut audio_streamer: Option<WasapiAudioPipeStreamer> = None;
        let mut capture_audio = config.record_audio;
        if capture_audio {
            let seq = PIPE_SEQ.fetch_add(1, Ordering::Relaxed);
            let pid = unsafe { GetCurrentProcessId() };
            let pipe_path = format!(r"\\.\pipe\qs_aud_{}_{}", pid, seq);
            match WasapiAudioPipeStreamer::start(pipe_path, audio_pid, isolate_audio) {
                Ok(s) => audio_streamer = Some(s),
                Err(e) => {
                    log_error(&format!("Audio capture fallback to video-only: {}", e));
                    capture_audio = false;
                }
            }
        }

        let (encoder_args, _, supports_ddagrab, supports_dda_nv12) = get_probe_info(0);
        let mouse_arg = if config.draw_mouse { "1" } else { "0" };
        let fps_str = config.fps.to_string();
        let use_dda = supports_ddagrab && fits_primary;

        let mut cmd = Command::new(get_ffmpeg_path());
        cmd.args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-filter_threads",
            "2",
            "-thread_queue_size",
            "1024",
        ]);

        if use_dda {
            let filter_tail = if supports_dda_nv12 {
                "scale_d3d11=format=nv12,hwdownload,format=nv12"
            } else {
                "hwdownload,format=bgra"
            };
            cmd.args(["-f", "lavfi", "-i"]).arg(format!(
                "ddagrab=output_idx=0:framerate={}:draw_mouse={}:offset_x={}:offset_y={}:video_size={}x{},{}",
                fps_str, mouse_arg, x, y, w, h, filter_tail
            ));
        } else {
            cmd.args(["-rtbufsize", "256M", "-f", "gdigrab", "-framerate"])
                .arg(&fps_str)
                .arg("-offset_x")
                .arg(x.to_string())
                .arg("-offset_y")
                .arg(y.to_string())
                .arg("-video_size")
                .arg(format!("{}x{}", w, h))
                .args(["-draw_mouse", mouse_arg, "-i", "desktop"]);
        }

        if capture_audio {
            if let Some(ref streamer) = audio_streamer {
                cmd.args([
                    "-probesize",
                    "32",
                    "-analyzeduration",
                    "0",
                    "-thread_queue_size",
                    "1024",
                    "-f",
                    "s16le",
                    "-ar",
                    "48000",
                    "-ac",
                    "2",
                    "-i",
                ])
                .arg(&streamer.pipe_path);
            }
        }

        if !use_dda || !supports_dda_nv12 {
            cmd.args(["-sws_flags", "fast_bilinear"]);
        }

        cmd.args(encoder_args)
            .args(["-g"])
            .arg(&fps_str)
            .args(["-fps_mode", "cfr", "-r"])
            .arg(&fps_str);

        if capture_audio && audio_streamer.is_some() {
            cmd.args(["-c:a", "aac", "-b:a", "192k"]);
        } else {
            cmd.arg("-an");
        }

        cmd.args(["-movflags", "+frag_keyframe+empty_moov+default_base_moof"])
            .arg(&seg_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                drop(audio_streamer);
                log_error(&format!("StartNewSegment spawn error: {}", e));
                if e.kind() == ErrorKind::NotFound {
                    return Err(
                        "Không tìm thấy ffmpeg.exe. Hãy đặt ffmpeg.exe cùng thư mục với QuickSnipRecorder.exe."
                            .to_string(),
                    );
                }
                return Err(e.to_string());
            }
        };

        let raw_handle = child.as_raw_handle();
        assign_process_to_job(raw_handle);
        unsafe {
            let _ = SetPriorityClass(HANDLE(raw_handle), HIGH_PRIORITY_CLASS);
        }

        let mut g = self.inner.lock().unwrap();
        let Some(sess) = g.current_session.as_mut() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Phiên quay đã bị hủy.".to_string());
        };
        if sess.session_id != expected_session_id {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Phiên quay đã bị hủy.".to_string());
        }
        sess.current_segment_path = Some(seg_path);
        sess.current_audio_streamer = audio_streamer;
        let err_arc = Arc::clone(&sess.last_error);
        let rec_weak = self.self_weak.get().cloned();
        if let Some(stderr) = child.stderr.take() {
            let _ = thread::Builder::new()
                .name("QuickSnip_Stderr".to_string())
                .stack_size(64 * 1024)
                .spawn(move || {
                    let mut reader = BufReader::new(stderr);
                    let mut line = String::with_capacity(256);
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                let trimmed = line.trim();
                                if !trimmed.is_empty() {
                                    if let Ok(mut e) = err_arc.lock() {
                                        e.clear();
                                        e.push_str(trimmed);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(rec) = rec_weak.and_then(|w| w.upgrade()) {
                        let should_stop = {
                            let g = rec.inner.lock().unwrap();
                            g.state == RecorderState::Recording
                                && g.current_session.as_ref().map(|s| s.session_id)
                                    == Some(expected_session_id)
                        };
                        if should_stop {
                            rec.stop_recording(false);
                        }
                    }
                });
        }
        sess.current_process = Some(child);
        Ok(())
    }

    fn pause_recording(&self) -> bool {
        let session_id = {
            let mut g = self.inner.lock().unwrap();
            if g.state != RecorderState::Recording {
                return false;
            }
            let Some(id) = g.current_session.as_ref().map(|s| s.session_id) else {
                return false;
            };
            g.state = RecorderState::Pausing;
            id
        };
        self.enqueue_cmd(RecorderCmd::Pause(session_id));
        true
    }

    fn resume_recording(&self) -> bool {
        let (session_id, config_snap) = {
            let mut g = self.inner.lock().unwrap();
            if g.state != RecorderState::Paused {
                return false;
            }
            let Some(id) = g.current_session.as_ref().map(|s| s.session_id) else {
                return false;
            };
            g.state = RecorderState::Resuming;
            (id, g.config.clone())
        };
        self.enqueue_cmd(RecorderCmd::Resume(session_id, config_snap));
        true
    }

    fn stop_current_segment_sync(&self) {
        let (mut proc_opt, mut audio_opt, seg_path_opt) = {
            let mut g = self.inner.lock().unwrap();
            let Some(sess) = g.current_session.as_mut() else {
                return;
            };
            (
                sess.current_process.take(),
                sess.current_audio_streamer.take(),
                sess.current_segment_path.take(),
            )
        };

        if let Some(ref mut child) = proc_opt {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(b"q\n");
                let _ = stdin.flush();
            }
            let _ = wait_child_exit(child, 1500);
        }

        if let Some(ref mut audio) = audio_opt {
            audio.stop();
        }
        drop(audio_opt);

        if let Some(seg_path) = seg_path_opt {
            let valid = fs::metadata(&seg_path).map(|m| m.len() > 512).unwrap_or(false);
            if valid {
                let mut g = self.inner.lock().unwrap();
                if let Some(sess) = g.current_session.as_mut() {
                    sess.segment_paths.push(seg_path);
                }
            } else {
                let _ = fs::remove_file(&seg_path);
            }
        }
    }

    fn stop_recording(&self, discard: bool) -> bool {
        let session_id = {
            let mut g = self.inner.lock().unwrap();
            if !matches!(
                g.state,
                RecorderState::Recording
                    | RecorderState::Pausing
                    | RecorderState::Paused
                    | RecorderState::Resuming
            ) {
                return false;
            }
            let Some(id) = g.current_session.as_mut().map(|sess| {
                sess.is_discarding = discard;
                sess.session_id
            }) else {
                return false;
            };
            g.state = RecorderState::Stopping;
            id
        };

        self.enqueue_cmd(RecorderCmd::Stop(session_id));
        true
    }

    fn force_kill(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Ok(mut q) = self.cmd_queue.lock() {
            q.clear();
            self.cmd_cv.notify_all();
        }
        self.release_high_res_timer();
        let sess_opt = {
            let mut g = self.inner.lock().unwrap();
            g.state = RecorderState::Idle;
            g.current_session.take()
        };
        if let Some(mut sess) = sess_opt {
            if let Some(ref mut a) = sess.current_audio_streamer {
                a.stop();
            }
            if let Some(ref mut p) = sess.current_process {
                let _ = p.kill();
                let _ = p.wait();
            }
            if let Some(ref seg) = sess.current_segment_path {
                let _ = fs::remove_file(seg);
            }
            for seg in &sess.segment_paths {
                let _ = fs::remove_file(seg);
            }
        }
    }
}

fn resolve_capture_geometry(
    config: &RecordingConfig,
) -> Result<(i32, i32, i32, i32, bool, u32, bool), String> {
    let primary = get_primary_screen_physical_rect();
    let virtual_screen = get_virtual_screen_physical_rect();

    if config.mode == CaptureTargetMode::AppWindow {
        if let Some(ref app) = config.target_app_window {
            let hwnd = HWND(app.handle as *mut _);
            unsafe {
                if !IsWindow(hwnd).as_bool() {
                    return Err(format!("Cửa sổ ứng dụng [{}] đã đóng.", app.process_name));
                }
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    thread::sleep(Duration::from_millis(80));
                }
                let _ = SetForegroundWindow(hwnd);
            }

            let Some(win_rect) = try_get_window_physical_bounds(hwnd) else {
                return Err(format!(
                    "Không thể lấy kích thước cửa sổ [{}].",
                    app.process_name
                ));
            };

            let mut left = virtual_screen.left.max(win_rect.left);
            let mut top = virtual_screen.top.max(win_rect.top);
            let right = virtual_screen.right.min(win_rect.right);
            let bottom = virtual_screen.bottom.min(win_rect.bottom);

            let max_vw = ((virtual_screen.right - virtual_screen.left) & !1).max(32);
            let max_vh = ((virtual_screen.bottom - virtual_screen.top) & !1).max(32);
            let width = ((right - left) & !1).max(32).min(max_vw);
            let height = ((bottom - top) & !1).max(32).min(max_vh);
            if left + width > virtual_screen.right {
                left = (virtual_screen.right - width).max(virtual_screen.left);
            }
            if top + height > virtual_screen.bottom {
                top = (virtual_screen.bottom - height).max(virtual_screen.top);
            }
            let fits_primary = left >= 0
                && top >= 0
                && (left + width) <= primary.right
                && (top + height) <= primary.bottom;

            let (root_pid, _) = resolve_app_process_info(hwnd, None);
            let audio_pid = if root_pid != 0 {
                root_pid
            } else {
                app.audio_root_pid
            };
            return Ok((left, top, width, height, fits_primary, audio_pid, true));
        }
    }

    if config.mode == CaptureTargetMode::CustomRegion {
        if let Some((rx, ry, rw, rh)) = config.selected_physical_region {
            let mut left = virtual_screen.left.max(rx);
            let mut top = virtual_screen.top.max(ry);
            let max_vw = ((virtual_screen.right - virtual_screen.left) & !1).max(32);
            let max_vh = ((virtual_screen.bottom - virtual_screen.top) & !1).max(32);
            let mut w = (rw & !1).max(32).min(max_vw);
            let mut h = (rh & !1).max(32).min(max_vh);
            if left + w > virtual_screen.right {
                w = ((virtual_screen.right - left) & !1).max(32).min(max_vw);
                if left + w > virtual_screen.right {
                    left = (virtual_screen.right - w).max(virtual_screen.left);
                }
            }
            if top + h > virtual_screen.bottom {
                h = ((virtual_screen.bottom - top) & !1).max(32).min(max_vh);
                if top + h > virtual_screen.bottom {
                    top = (virtual_screen.bottom - h).max(virtual_screen.top);
                }
            }
            let fits_primary = left >= 0
                && top >= 0
                && (left + w) <= primary.right
                && (top + h) <= primary.bottom;
            return Ok((left, top, w, h, fits_primary, 0, false));
        }
    }

    let w = (primary.right & !1).max(32);
    let h = (primary.bottom & !1).max(32);
    Ok((0, 0, w, h, true, 0, false))
}

fn finalize_video(
    session_id: u32,
    segments: &[PathBuf],
    final_output: &Path,
    last_error: &str,
) -> Result<String, String> {
    if segments.is_empty() {
        let msg = if last_error.trim().is_empty() {
            "Không có dữ liệu video.".to_string()
        } else {
            last_error.to_string()
        };
        log_error(&format!("FinalizeVideo empty segments: {}", msg));
        return Err(msg);
    }

    if segments.len() == 1 {
        let src = to_wide_null(&segments[0].to_string_lossy());
        let dst = to_wide_null(&final_output.to_string_lossy());
        let ok = unsafe {
            MoveFileExW(
                PCWSTR(src.as_ptr()),
                PCWSTR(dst.as_ptr()),
                MOVEFILE_COPY_ALLOWED | MOVEFILE_REPLACE_EXISTING,
            )
            .is_ok()
        };
        if ok {
            return Ok(final_output.to_string_lossy().into_owned());
        }
        return Err("Không thể lưu file video.".to_string());
    }

    let list_file = std::env::temp_dir().join(format!("concat_s{}.txt", session_id));
    let mut content = String::with_capacity(segments.len() * 64);
    for seg in segments {
        content.push_str("file '");
        for ch in seg.to_string_lossy().chars() {
            match ch {
                '\\' => content.push('/'),
                '\'' => content.push_str("'\\''"),
                _ => content.push(ch),
            }
        }
        content.push_str("'\n");
    }
    if let Err(e) = fs::write(&list_file, content) {
        return Err(e.to_string());
    }

    let mut cmd = Command::new(get_ffmpeg_path());
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
    ])
    .arg(&list_file)
    .args(["-c", "copy", "-movflags", "+faststart"])
    .arg(final_output)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .creation_flags(CREATE_NO_WINDOW);

    if let Ok(mut child) = cmd.spawn() {
        let _ = wait_child_exit(&mut child, 6000);
    }

    for seg in segments {
        let _ = fs::remove_file(seg);
    }
    let _ = fs::remove_file(&list_file);

    let ok = fs::metadata(final_output).map(|m| m.len() > 512).unwrap_or(false);
    if ok {
        Ok(final_output.to_string_lossy().into_owned())
    } else {
        log_error("Concat output file missing or empty");
        Err("Lỗi ghép nối các đoạn video.".to_string())
    }
}

#[inline]
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    }
}

const C_BG_DARK: D2D1_COLOR_F = rgba(9, 9, 11, 255);
const C_BG_CARD: D2D1_COLOR_F = rgba(18, 18, 22, 255);
const C_BG_HOVER: D2D1_COLOR_F = rgba(28, 28, 34, 255);
const C_BG_ACTIVE: D2D1_COLOR_F = rgba(38, 38, 48, 255);
const C_BORDER_DARK: D2D1_COLOR_F = rgba(34, 34, 40, 255);
const C_BORDER_HOVER: D2D1_COLOR_F = rgba(60, 60, 70, 255);
const C_BORDER_ACTIVE: D2D1_COLOR_F = rgba(90, 90, 110, 255);
const C_TEXT_SECONDARY: D2D1_COLOR_F = rgba(185, 185, 195, 255);
const C_TEXT_MUTED: D2D1_COLOR_F = rgba(115, 115, 125, 255);
const C_ACCENT_RED: D2D1_COLOR_F = rgba(239, 68, 68, 255);
const C_ACCENT_AMBER: D2D1_COLOR_F = rgba(245, 158, 11, 255);
const C_ACCENT_CYAN: D2D1_COLOR_F = rgba(56, 189, 248, 255);
const C_WHITE: D2D1_COLOR_F = rgba(255, 255, 255, 255);
const C_BLACK: D2D1_COLOR_F = rgba(0, 0, 0, 255);
const C_OVERLAY_BG: D2D1_COLOR_F = rgba(0, 0, 0, 130);
const C_SEL_FILL: D2D1_COLOR_F = rgba(255, 255, 255, 25);
const C_SNAP_FILL: D2D1_COLOR_F = rgba(56, 189, 248, 30);
const C_BADGE_BG: D2D1_COLOR_F = rgba(12, 12, 16, 245);

struct D2DContext {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
    fmt_bold_12: IDWriteTextFormat,
    fmt_bold_11: IDWriteTextFormat,
    fmt_semi_11: IDWriteTextFormat,
    fmt_semi_10: IDWriteTextFormat,
    fmt_mono_12: IDWriteTextFormat,
    fmt_bold_13_left: IDWriteTextFormat,
    fmt_semi_12_left: IDWriteTextFormat,
    fmt_wrap_11: IDWriteTextFormat,
    utf16_buf: RefCell<Vec<u16>>,
}

impl D2DContext {
    fn new() -> Self {
        unsafe {
            let factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).unwrap();
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).unwrap();

            let make_fmt = |font: PCWSTR,
                            weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
                            size: f32,
                            center: bool,
                            wrap: bool|
             -> IDWriteTextFormat {
                let fmt = dwrite
                    .CreateTextFormat(
                        font,
                        None,
                        weight,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        size,
                        w!("en-us"),
                    )
                    .unwrap();
                let _ = fmt.SetTextAlignment(if center {
                    DWRITE_TEXT_ALIGNMENT_CENTER
                } else {
                    DWRITE_TEXT_ALIGNMENT_LEADING
                });
                let _ = fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                let _ = fmt.SetWordWrapping(if wrap {
                    DWRITE_WORD_WRAPPING_WRAP
                } else {
                    DWRITE_WORD_WRAPPING_NO_WRAP
                });
                fmt
            };

            Self {
                factory,
                fmt_bold_12: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_BOLD, 12.0, true, false),
                fmt_bold_11: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_BOLD, 11.2, true, false),
                fmt_semi_11: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_SEMI_BOLD, 11.0, true, false),
                fmt_semi_10: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_SEMI_BOLD, 10.2, true, false),
                fmt_mono_12: make_fmt(w!("Consolas"), DWRITE_FONT_WEIGHT_BOLD, 12.0, true, false),
                fmt_bold_13_left: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_BOLD, 13.5, false, false),
                fmt_semi_12_left: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_SEMI_BOLD, 12.0, false, false),
                fmt_wrap_11: make_fmt(w!("Segoe UI"), DWRITE_FONT_WEIGHT_SEMI_BOLD, 11.0, false, true),
                dwrite,
                utf16_buf: RefCell::new(Vec::with_capacity(256)),
            }
        }
    }

    fn measure_text(&self, text: &str, fmt: &IDWriteTextFormat) -> f32 {
        let mut buf = self.utf16_buf.borrow_mut();
        buf.clear();
        buf.extend(text.encode_utf16());
        unsafe {
            if let Ok(layout) = self.dwrite.CreateTextLayout(&buf, fmt, 2000.0, 100.0) {
                let mut metrics = DWRITE_TEXT_METRICS::default();
                if layout.GetMetrics(&mut metrics).is_ok() {
                    return metrics.widthIncludingTrailingWhitespace.ceil();
                }
            }
        }
        (text.len() as f32) * 7.0
    }

    fn draw_text(
        &self,
        rt: &ID2D1DCRenderTarget,
        text: &str,
        fmt: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        let mut buf = self.utf16_buf.borrow_mut();
        buf.clear();
        buf.extend(text.encode_utf16());
        unsafe {
            rt.DrawText(
                &buf,
                fmt,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

struct BrushPool {
    bg_dark: ID2D1SolidColorBrush,
    bg_card: ID2D1SolidColorBrush,
    bg_hover: ID2D1SolidColorBrush,
    bg_active: ID2D1SolidColorBrush,
    border_dark: ID2D1SolidColorBrush,
    border_hover: ID2D1SolidColorBrush,
    border_active: ID2D1SolidColorBrush,
    text_secondary: ID2D1SolidColorBrush,
    text_muted: ID2D1SolidColorBrush,
    accent_red: ID2D1SolidColorBrush,
    accent_amber: ID2D1SolidColorBrush,
    accent_cyan: ID2D1SolidColorBrush,
    white: ID2D1SolidColorBrush,
    black: ID2D1SolidColorBrush,
    overlay_bg: ID2D1SolidColorBrush,
    sel_fill: ID2D1SolidColorBrush,
    snap_fill: ID2D1SolidColorBrush,
    badge_bg: ID2D1SolidColorBrush,
    scratch: ID2D1SolidColorBrush,
}

impl BrushPool {
    fn new(rt: &ID2D1DCRenderTarget) -> Self {
        unsafe {
            let b = |c: D2D1_COLOR_F| rt.CreateSolidColorBrush(&c, None).unwrap();
            Self {
                bg_dark: b(C_BG_DARK),
                bg_card: b(C_BG_CARD),
                bg_hover: b(C_BG_HOVER),
                bg_active: b(C_BG_ACTIVE),
                border_dark: b(C_BORDER_DARK),
                border_hover: b(C_BORDER_HOVER),
                border_active: b(C_BORDER_ACTIVE),
                text_secondary: b(C_TEXT_SECONDARY),
                text_muted: b(C_TEXT_MUTED),
                accent_red: b(C_ACCENT_RED),
                accent_amber: b(C_ACCENT_AMBER),
                accent_cyan: b(C_ACCENT_CYAN),
                white: b(C_WHITE),
                black: b(C_BLACK),
                overlay_bg: b(C_OVERLAY_BG),
                sel_fill: b(C_SEL_FILL),
                snap_fill: b(C_SNAP_FILL),
                badge_bg: b(C_BADGE_BG),
                scratch: b(C_WHITE),
            }
        }
    }
}

struct LayeredSurface {
    mem_dc: HDC,
    hbm: HBITMAP,
    old_bm: HGDIOBJ,
    rt: Option<ID2D1DCRenderTarget>,
    brushes: Option<BrushPool>,
    phys_w: i32,
    phys_h: i32,
    dpi: f32,
}

impl LayeredSurface {
    fn new() -> Self {
        let mem_dc = unsafe { CreateCompatibleDC(None) };
        Self {
            mem_dc,
            hbm: HBITMAP::default(),
            old_bm: HGDIOBJ::default(),
            rt: None,
            brushes: None,
            phys_w: 0,
            phys_h: 0,
            dpi: 0.0,
        }
    }

    fn render<F: FnOnce(&ID2D1DCRenderTarget, &BrushPool)>(
        &mut self,
        d2d: &D2DContext,
        hwnd: HWND,
        x: i32,
        y: i32,
        logical_w: f32,
        logical_h: f32,
        dpi: f32,
        draw_fn: F,
    ) {
        let phys_w = (logical_w * dpi).ceil().max(1.0) as i32;
        let phys_h = (logical_h * dpi).ceil().max(1.0) as i32;

        unsafe {
            if phys_w != self.phys_w || phys_h != self.phys_h || self.hbm.0.is_null() {
                if !self.hbm.0.is_null() {
                    SelectObject(self.mem_dc, self.old_bm);
                    let _ = DeleteObject(self.hbm);
                    self.hbm = HBITMAP::default();
                }
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: phys_w,
                        biHeight: -phys_h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits = std::ptr::null_mut();
                if let Ok(hbm) = CreateDIBSection(self.mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                    self.old_bm = SelectObject(self.mem_dc, hbm);
                    self.hbm = hbm;
                    self.phys_w = phys_w;
                    self.phys_h = phys_h;
                }
            }

            if self.rt.is_none() || (self.dpi - dpi).abs() > 0.001 {
                let props = D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0 * dpi,
                    dpiY: 96.0 * dpi,
                    usage: D2D1_RENDER_TARGET_USAGE_NONE,
                    minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                };
                if let Ok(rt) = d2d.factory.CreateDCRenderTarget(&props) {
                    self.brushes = Some(BrushPool::new(&rt));
                    self.rt = Some(rt);
                    self.dpi = dpi;
                }
            }

            if let (Some(rt), Some(brushes)) = (&self.rt, &self.brushes) {
                let bind_rect = RECT {
                    left: 0,
                    top: 0,
                    right: phys_w,
                    bottom: phys_h,
                };
                if rt.BindDC(self.mem_dc, &bind_rect).is_ok() {
                    rt.BeginDraw();
                    rt.Clear(Some(&D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }));
                    draw_fn(rt, brushes);
                    let _ = rt.EndDraw(None, None);
                }
            }

            let pt_dst = POINT { x, y };
            let sz = SIZE {
                cx: phys_w,
                cy: phys_h,
            };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd,
                HDC::default(),
                Some(&pt_dst),
                Some(&sz),
                self.mem_dc,
                Some(&pt_src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
        }
    }
}

impl Drop for LayeredSurface {
    fn drop(&mut self) {
        unsafe {
            if !self.hbm.0.is_null() {
                SelectObject(self.mem_dc, self.old_bm);
                let _ = DeleteObject(self.hbm);
            }
            if !self.mem_dc.0.is_null() {
                let _ = DeleteDC(self.mem_dc);
            }
        }
    }
}

#[inline(always)]
fn fill_round_rect(
    rt: &ID2D1DCRenderTarget,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let rr = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left,
            top,
            right,
            bottom,
        },
        radiusX: radius,
        radiusY: radius,
    };
    unsafe {
        rt.FillRoundedRectangle(&rr, brush);
    }
}

#[inline(always)]
fn stroke_round_rect(
    rt: &ID2D1DCRenderTarget,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let rr = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left,
            top,
            right,
            bottom,
        },
        radiusX: radius,
        radiusY: radius,
    };
    unsafe {
        rt.DrawRoundedRectangle(&rr, brush, stroke, None);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnId {
    DragBrand,
    StartRecord,
    Area,
    AppPicker,
    Audio,
    Cursor,
    Fps,
    AutoMin,
    Folder,
    Pause,
    Resume,
    Stop,
    Cancel,
    Minimize,
    Close,
    ToastPlay,
    ToastFolder,
    ToastClose,
}

#[derive(Clone, Copy)]
struct HitRect {
    id: BtnId,
    rect: D2D_RECT_F,
}

#[inline(always)]
fn pt_in_rect(x: f32, y: f32, r: D2D_RECT_F) -> bool {
    x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}

#[derive(Clone, Copy)]
enum IconKind {
    Area,
    AppWin,
    SpeakerOn,
    SpeakerMute,
    Cursor,
    Fps,
    EyeHide,
    Folder,
    Pause,
    Play,
    Stop,
}

fn draw_icon(
    rt: &ID2D1DCRenderTarget,
    kind: IconKind,
    ox: f32,
    oy: f32,
    b: &ID2D1SolidColorBrush,
) {
    unsafe {
        match kind {
            IconKind::Area => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(1.0, 4.0), p(1.0, 1.0), b, 1.3, None);
                rt.DrawLine(p(1.0, 1.0), p(4.0, 1.0), b, 1.3, None);
                rt.DrawLine(p(8.0, 1.0), p(11.0, 1.0), b, 1.3, None);
                rt.DrawLine(p(11.0, 1.0), p(11.0, 4.0), b, 1.3, None);
                rt.DrawLine(p(11.0, 8.0), p(11.0, 11.0), b, 1.3, None);
                rt.DrawLine(p(11.0, 11.0), p(8.0, 11.0), b, 1.3, None);
                rt.DrawLine(p(4.0, 11.0), p(1.0, 11.0), b, 1.3, None);
                rt.DrawLine(p(1.0, 11.0), p(1.0, 8.0), b, 1.3, None);
            }
            IconKind::AppWin => {
                stroke_round_rect(rt, ox + 1.0, oy + 1.0, ox + 11.0, oy + 11.0, 1.5, 1.2, b);
                rt.DrawLine(
                    D2D_POINT_2F { x: ox + 1.0, y: oy + 4.0 },
                    D2D_POINT_2F { x: ox + 11.0, y: oy + 4.0 },
                    b,
                    1.2,
                    None,
                );
            }
            IconKind::SpeakerOn => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(1.5, 4.5), p(3.5, 4.5), b, 1.3, None);
                rt.DrawLine(p(3.5, 4.5), p(6.5, 2.0), b, 1.3, None);
                rt.DrawLine(p(6.5, 2.0), p(6.5, 10.0), b, 1.3, None);
                rt.DrawLine(p(6.5, 10.0), p(3.5, 7.5), b, 1.3, None);
                rt.DrawLine(p(3.5, 7.5), p(1.5, 7.5), b, 1.3, None);
                rt.DrawLine(p(1.5, 7.5), p(1.5, 4.5), b, 1.3, None);
                rt.DrawLine(p(8.5, 4.0), p(9.5, 6.0), b, 1.2, None);
                rt.DrawLine(p(9.5, 6.0), p(8.5, 8.0), b, 1.2, None);
                rt.DrawLine(p(10.2, 2.5), p(11.5, 6.0), b, 1.2, None);
                rt.DrawLine(p(11.5, 6.0), p(10.2, 9.5), b, 1.2, None);
            }
            IconKind::SpeakerMute => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(1.5, 4.5), p(3.5, 4.5), b, 1.3, None);
                rt.DrawLine(p(3.5, 4.5), p(6.5, 2.0), b, 1.3, None);
                rt.DrawLine(p(6.5, 2.0), p(6.5, 10.0), b, 1.3, None);
                rt.DrawLine(p(6.5, 10.0), p(3.5, 7.5), b, 1.3, None);
                rt.DrawLine(p(3.5, 7.5), p(1.5, 7.5), b, 1.3, None);
                rt.DrawLine(p(1.5, 7.5), p(1.5, 4.5), b, 1.3, None);
                rt.DrawLine(p(8.5, 4.0), p(11.5, 8.0), b, 1.3, None);
                rt.DrawLine(p(11.5, 4.0), p(8.5, 8.0), b, 1.3, None);
            }
            IconKind::Cursor => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(2.0, 1.0), p(2.0, 10.5), b, 1.5, None);
                rt.DrawLine(p(2.0, 1.0), p(9.0, 7.5), b, 1.5, None);
                rt.DrawLine(p(2.0, 10.5), p(4.8, 8.0), b, 1.5, None);
                rt.DrawLine(p(4.8, 8.0), p(9.0, 7.5), b, 1.5, None);
                rt.DrawLine(p(5.0, 8.0), p(7.0, 11.5), b, 1.5, None);
            }
            IconKind::Fps => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(6.5, 1.0), p(2.5, 6.2), b, 1.4, None);
                rt.DrawLine(p(2.5, 6.2), p(6.0, 6.2), b, 1.4, None);
                rt.DrawLine(p(6.0, 6.2), p(4.5, 11.0), b, 1.4, None);
                rt.DrawLine(p(4.5, 11.0), p(9.5, 5.0), b, 1.4, None);
                rt.DrawLine(p(9.5, 5.0), p(6.0, 5.0), b, 1.4, None);
                rt.DrawLine(p(6.0, 5.0), p(6.5, 1.0), b, 1.4, None);
            }
            IconKind::EyeHide => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(1.0, 6.0), p(6.0, 2.5), b, 1.2, None);
                rt.DrawLine(p(6.0, 2.5), p(11.0, 6.0), b, 1.2, None);
                rt.DrawLine(p(11.0, 6.0), p(6.0, 9.5), b, 1.2, None);
                rt.DrawLine(p(6.0, 9.5), p(1.0, 6.0), b, 1.2, None);
                rt.DrawLine(p(1.5, 10.5), p(10.5, 1.5), b, 1.3, None);
            }
            IconKind::Folder => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(1.0, 2.5), p(4.5, 2.5), b, 1.3, None);
                rt.DrawLine(p(4.5, 2.5), p(6.0, 4.0), b, 1.3, None);
                rt.DrawLine(p(6.0, 4.0), p(11.0, 4.0), b, 1.3, None);
                rt.DrawLine(p(11.0, 4.0), p(11.0, 10.0), b, 1.3, None);
                rt.DrawLine(p(11.0, 10.0), p(1.0, 10.0), b, 1.3, None);
                rt.DrawLine(p(1.0, 10.0), p(1.0, 2.5), b, 1.3, None);
            }
            IconKind::Pause => {
                rt.FillRectangle(
                    &D2D_RECT_F { left: ox + 2.0, top: oy + 1.5, right: ox + 4.5, bottom: oy + 10.5 },
                    b,
                );
                rt.FillRectangle(
                    &D2D_RECT_F { left: ox + 7.5, top: oy + 1.5, right: ox + 10.0, bottom: oy + 10.5 },
                    b,
                );
            }
            IconKind::Play => {
                let p = |x, y| D2D_POINT_2F { x: ox + x, y: oy + y };
                rt.DrawLine(p(2.5, 1.5), p(9.5, 6.0), b, 1.6, None);
                rt.DrawLine(p(9.5, 6.0), p(2.5, 10.5), b, 1.6, None);
                rt.DrawLine(p(2.5, 10.5), p(2.5, 1.5), b, 1.6, None);
            }
            IconKind::Stop => {
                rt.FillRectangle(
                    &D2D_RECT_F { left: ox + 2.0, top: oy + 2.0, right: ox + 10.0, bottom: oy + 10.0 },
                    b,
                );
            }
        }
    }
}

static BORDER_CLASS_ONCE: Once = Once::new();
static SNIP_CLASS_ONCE: Once = Once::new();
static PICKER_CLASS_ONCE: Once = Once::new();

struct RegionBorderWindow {
    hwnd: HWND,
    d2d: Rc<D2DContext>,
    surface: RefCell<LayeredSurface>,
}

impl RegionBorderWindow {
    fn create(d2d: Rc<D2DContext>, hinst: HINSTANCE) -> Self {
        let class_name = w!("QuickSnipRegionBorderClass");
        unsafe {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: hinst,
                lpszClassName: class_name,
                ..Default::default()
            };
            register_class_once(&BORDER_CLASS_ONCE, &wc);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
                class_name,
                w!("QuickSnip Region Border"),
                WS_POPUP,
                0,
                0,
                100,
                100,
                None,
                None,
                hinst,
                None,
            )
            .unwrap();
            Self {
                hwnd,
                d2d,
                surface: RefCell::new(LayeredSurface::new()),
            }
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn update_and_show(
        &self,
        phys_x: i32,
        phys_y: i32,
        phys_w: i32,
        phys_h: i32,
        app_name: Option<&str>,
    ) {
        let dpi = get_dpi_scale(self.hwnd);
        let lw = (phys_w.max(32) as f32) / dpi;
        let lh = (phys_h.max(32) as f32) / dpi;

        let is_app = app_name.is_some();
        let label = match app_name {
            Some(name) => format!("🪟 APP: {} [{}×{}]", name, phys_w, phys_h),
            None => format!("● VÙNG [{}×{}]", phys_w, phys_h),
        };
        let badge_w = self.d2d.measure_text(&label, &self.d2d.fmt_semi_10) + 12.0;

        self.surface.borrow_mut().render(
            &self.d2d,
            self.hwnd,
            phys_x,
            phys_y,
            lw,
            lh,
            dpi,
            |rt, bp| {
                let accent = if is_app { &bp.accent_cyan } else { &bp.accent_red };
                stroke_round_rect(rt, 1.0, 1.0, lw - 1.0, lh - 1.0, 3.0, 1.8, accent);
                fill_round_rect(rt, 3.0, 3.0, 3.0 + badge_w, 21.0, 3.0, &bp.bg_dark);
                stroke_round_rect(rt, 3.0, 3.0, 3.0 + badge_w, 21.0, 3.0, 1.0, accent);
                self.d2d.draw_text(
                    rt,
                    &label,
                    &self.d2d.fmt_semi_10,
                    D2D_RECT_F {
                        left: 3.0,
                        top: 3.0,
                        right: 3.0 + badge_w,
                        bottom: 21.0,
                    },
                    &bp.white,
                );
            },
        );

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    fn close(&self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

struct SnippingResult {
    is_full_screen: bool,
    selected_physical_region: Option<(i32, i32, i32, i32)>,
    selected_app: Option<CapturableWindowInfo>,
}

const SNIP_HINT: &str =
    "🎯 Click vào cửa sổ App để quay riêng App (gồm âm thanh App) hoặc Kéo chuột chọn vùng";
const SNIP_LABELS: [&str; 4] = ["Toàn Màn Hình", "1080p", "720p", "✕ Hủy (Esc)"];

struct SnippingOverlayState {
    hwnd: HWND,
    d2d: Rc<D2DContext>,
    surface: LayeredSurface,
    virt_rect: RECT,
    dpi: f32,
    lw: f32,
    lh: f32,
    bar_x: f32,
    bar_y: f32,
    total_bar_w: f32,
    hint_w: f32,
    preset_rects: [D2D_RECT_F; 4],
    windows: Vec<CapturableWindowInfo>,
    hovered_win: Option<usize>,
    is_dragging: bool,
    start_x: f32,
    start_y: f32,
    curr_x: f32,
    curr_y: f32,
    hovered_btn: Option<usize>,
    fmt_buf: String,
    done: bool,
    result: Option<SnippingResult>,
}

impl SnippingOverlayState {
    fn compute_layout(&mut self) {
        self.lw = (self.virt_rect.right - self.virt_rect.left) as f32 / self.dpi;
        self.lh = (self.virt_rect.bottom - self.virt_rect.top) as f32 / self.dpi;
        self.hint_w = self.d2d.measure_text(SNIP_HINT, &self.d2d.fmt_bold_11);
        let mut btn_w = [0.0f32; 4];
        let mut sum_w = 0.0f32;
        for (i, l) in SNIP_LABELS.iter().enumerate() {
            btn_w[i] = self.d2d.measure_text(l, &self.d2d.fmt_semi_11) + 20.0;
            sum_w += btn_w[i];
        }
        self.total_bar_w = 14.0 + self.hint_w + 12.0 + sum_w + 16.0 + 14.0;
        self.bar_x = ((self.lw - self.total_bar_w) / 2.0).max(10.0);
        self.bar_y = 20.0;
        let mut cx = self.bar_x + 14.0 + self.hint_w + 12.0;
        for i in 0..4 {
            self.preset_rects[i] = D2D_RECT_F {
                left: cx,
                top: self.bar_y + 6.0,
                right: cx + btn_w[i],
                bottom: self.bar_y + 30.0,
            };
            cx += btn_w[i] + 4.0;
        }
    }

    fn render(&mut self) {
        let lw = self.lw;
        let lh = self.lh;
        let virt = self.virt_rect;
        let dpi = self.dpi;
        let bar_x = self.bar_x;
        let bar_y = self.bar_y;
        let total_w = self.total_bar_w;
        let hint_w = self.hint_w;
        let presets = self.preset_rects;
        let hovered_btn = self.hovered_btn;
        let is_dragging = self.is_dragging;
        let start_x = self.start_x;
        let start_y = self.start_y;
        let curr_x = self.curr_x;
        let curr_y = self.curr_y;
        let hovered_win = self.hovered_win.and_then(|idx| self.windows.get(idx));
        let d2d = Rc::clone(&self.d2d);
        let mut fmt_buf = std::mem::take(&mut self.fmt_buf);

        self.surface.render(
            &d2d,
            self.hwnd,
            virt.left,
            virt.top,
            lw,
            lh,
            dpi,
            |rt, bp| {
                unsafe {
                    rt.FillRectangle(
                        &D2D_RECT_F { left: 0.0, top: 0.0, right: lw, bottom: lh },
                        &bp.overlay_bg,
                    );
                }

                let draw_pill_badge = |rt: &ID2D1DCRenderTarget, bp: &BrushPool, bx: f32, by: f32, txt: &str| {
                    let tw = d2d.measure_text(txt, &d2d.fmt_semi_11) + 14.0;
                    fill_round_rect(rt, bx, by, bx + tw, by + 22.0, 5.0, &bp.badge_bg);
                    stroke_round_rect(rt, bx, by, bx + tw, by + 22.0, 5.0, 1.0, &bp.border_hover);
                    d2d.draw_text(
                        rt,
                        txt,
                        &d2d.fmt_semi_11,
                        D2D_RECT_F { left: bx, top: by, right: bx + tw, bottom: by + 22.0 },
                        &bp.white,
                    );
                };

                if is_dragging {
                    let x = start_x.min(curr_x);
                    let y = start_y.min(curr_y);
                    let w = (curr_x - start_x).abs();
                    let h = (curr_y - start_y).abs();
                    if w >= 4.0 || h >= 4.0 {
                        fill_round_rect(rt, x, y, x + w, y + h, 4.0, &bp.sel_fill);
                        stroke_round_rect(rt, x, y, x + w, y + h, 4.0, 1.8, &bp.white);
                        let pw = ((w * dpi).round() as i32) & !1;
                        let ph = ((h * dpi).round() as i32) & !1;
                        fmt_buf.clear();
                        let _ = write!(fmt_buf, "📐 {} × {} px", pw, ph);
                        let by = if y > 30.0 { y - 28.0 } else { y + 6.0 };
                        draw_pill_badge(rt, bp, x + 6.0, by, &fmt_buf);
                    }
                } else if let Some(win) = hovered_win {
                    let r = win.physical_bounds;
                    let wx = (r.left - virt.left) as f32 / dpi;
                    let wy = (r.top - virt.top) as f32 / dpi;
                    let ww = (r.right - r.left) as f32 / dpi;
                    let wh = (r.bottom - r.top) as f32 / dpi;
                    fill_round_rect(rt, wx, wy, wx + ww, wy + wh, 4.0, &bp.snap_fill);
                    stroke_round_rect(rt, wx, wy, wx + ww, wy + wh, 4.0, 1.8, &bp.accent_cyan);

                    fmt_buf.clear();
                    let _ = write!(
                        fmt_buf,
                        "🪟 [{}] {}×{} • Click để quay riêng App + Âm thanh App",
                        win.process_name,
                        win.width(),
                        win.height()
                    );
                    let bx = (wx + 6.0).max(6.0);
                    let by = if wy > 32.0 { wy - 28.0 } else { wy + 6.0 };
                    draw_pill_badge(rt, bp, bx, by, &fmt_buf);
                }

                fill_round_rect(rt, bar_x, bar_y, bar_x + total_w, bar_y + 36.0, 18.0, &bp.bg_dark);
                stroke_round_rect(rt, bar_x, bar_y, bar_x + total_w, bar_y + 36.0, 18.0, 1.0, &bp.border_hover);
                d2d.draw_text(
                    rt,
                    SNIP_HINT,
                    &d2d.fmt_bold_11,
                    D2D_RECT_F {
                        left: bar_x + 14.0,
                        top: bar_y,
                        right: bar_x + 14.0 + hint_w,
                        bottom: bar_y + 36.0,
                    },
                    &bp.white,
                );

                for (i, rect) in presets.iter().enumerate() {
                    let is_hov = hovered_btn == Some(i);
                    let bg = if is_hov {
                        &bp.bg_hover
                    } else if i == 3 {
                        &bp.bg_active
                    } else {
                        &bp.bg_card
                    };
                    fill_round_rect(rt, rect.left, rect.top, rect.right, rect.bottom, 6.0, bg);
                    stroke_round_rect(rt, rect.left, rect.top, rect.right, rect.bottom, 6.0, 1.0, &bp.border_hover);
                    d2d.draw_text(rt, SNIP_LABELS[i], &d2d.fmt_semi_11, *rect, &bp.text_secondary);
                }
            },
        );
        self.fmt_buf = fmt_buf;
    }

    fn find_hovered_window_idx(&self, lx: f32, ly: f32) -> Option<usize> {
        let px = self.virt_rect.left + (lx * self.dpi).round() as i32;
        let py = self.virt_rect.top + (ly * self.dpi).round() as i32;
        for (i, w) in self.windows.iter().enumerate() {
            let r = w.physical_bounds;
            if px >= r.left && px <= r.right && py >= r.top && py <= r.bottom {
                return Some(i);
            }
        }
        None
    }

    fn set_preset(&mut self, target_w: i32, target_h: i32) {
        let primary = get_primary_screen_physical_rect();
        let w = target_w.min(primary.right) & !1;
        let h = target_h.min(primary.bottom) & !1;
        let x = ((primary.right - w) / 2).max(0);
        let y = ((primary.bottom - h) / 2).max(0);
        self.result = Some(SnippingResult {
            is_full_screen: false,
            selected_physical_region: Some((x, y, w, h)),
            selected_app: None,
        });
        self.done = true;
    }
}

fn run_snipping_overlay(
    d2d: Rc<D2DContext>,
    hinst: HINSTANCE,
    owner_hwnd: HWND,
) -> Option<SnippingResult> {
    let windows = enumerate_capturable_windows(owner_hwnd, HWND::default());
    let virt = get_virtual_screen_physical_rect();

    let class_name = w!("QuickSnipOverlayClass");
    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(snipping_wnd_proc),
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        register_class_once(&SNIP_CLASS_ONCE, &wc);
    }

    let state = Box::new(SnippingOverlayState {
        hwnd: HWND::default(),
        d2d,
        surface: LayeredSurface::new(),
        virt_rect: virt,
        dpi: 1.0,
        lw: 0.0,
        lh: 0.0,
        bar_x: 0.0,
        bar_y: 0.0,
        total_bar_w: 0.0,
        hint_w: 0.0,
        preset_rects: [D2D_RECT_F::default(); 4],
        windows,
        hovered_win: None,
        is_dragging: false,
        start_x: 0.0,
        start_y: 0.0,
        curr_x: 0.0,
        curr_y: 0.0,
        hovered_btn: None,
        fmt_buf: String::with_capacity(96),
        done: false,
        result: None,
    });
    let state_ptr = Box::into_raw(state);

    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("QuickSnip Selection"),
            WS_POPUP,
            virt.left,
            virt.top,
            virt.right - virt.left,
            virt.bottom - virt.top,
            None,
            None,
            hinst,
            None,
        )
        .unwrap();

        (*state_ptr).hwnd = hwnd;
        (*state_ptr).dpi = get_dpi_scale(hwnd);
        (*state_ptr).compute_layout();
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        (*state_ptr).render();
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);

        let mut msg = MSG::default();
        while !(*state_ptr).done {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = DestroyWindow(hwnd);
        let boxed = Box::from_raw(state_ptr);
        boxed.result
    }
}

unsafe extern "system" fn snipping_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SnippingOverlayState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let st = &mut *ptr;

    match msg {
        WM_KEYDOWN => {
            if wparam.0 as u16 == VK_ESCAPE.0 {
                if st.is_dragging {
                    st.is_dragging = false;
                    let _ = ReleaseCapture();
                }
                st.done = true;
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (lx, ly) = lparam_logical(lparam, st.dpi);

            if st.is_dragging {
                st.curr_x = lx;
                st.curr_y = ly;
                st.render();
            } else {
                let mut new_btn = None;
                for (i, r) in st.preset_rects.iter().enumerate() {
                    if pt_in_rect(lx, ly, *r) {
                        new_btn = Some(i);
                        break;
                    }
                }
                let on_bar = pt_in_rect(
                    lx,
                    ly,
                    D2D_RECT_F {
                        left: st.bar_x,
                        top: st.bar_y,
                        right: st.bar_x + st.total_bar_w,
                        bottom: st.bar_y + 36.0,
                    },
                );
                let new_win = if new_btn.is_some() || on_bar {
                    None
                } else {
                    st.find_hovered_window_idx(lx, ly)
                };
                if new_btn != st.hovered_btn || new_win != st.hovered_win {
                    st.hovered_btn = new_btn;
                    st.hovered_win = new_win;
                    st.render();
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (lx, ly) = lparam_logical(lparam, st.dpi);

            for (i, r) in st.preset_rects.iter().enumerate() {
                if pt_in_rect(lx, ly, *r) {
                    match i {
                        0 => {
                            st.result = Some(SnippingResult {
                                is_full_screen: true,
                                selected_physical_region: None,
                                selected_app: None,
                            });
                            st.done = true;
                        }
                        1 => st.set_preset(1920, 1080),
                        2 => st.set_preset(1280, 720),
                        _ => st.done = true,
                    }
                    return LRESULT(0);
                }
            }

            if pt_in_rect(
                lx,
                ly,
                D2D_RECT_F {
                    left: st.bar_x,
                    top: st.bar_y,
                    right: st.bar_x + st.total_bar_w,
                    bottom: st.bar_y + 36.0,
                },
            ) {
                return LRESULT(0);
            }

            st.is_dragging = true;
            st.start_x = lx;
            st.start_y = ly;
            st.curr_x = lx;
            st.curr_y = ly;
            SetCapture(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if !st.is_dragging {
                return LRESULT(0);
            }
            st.is_dragging = false;
            let _ = ReleaseCapture();

            let (lx, ly) = lparam_logical(lparam, st.dpi);

            let x = st.start_x.min(lx);
            let y = st.start_y.min(ly);
            let w = (lx - st.start_x).abs();
            let h = (ly - st.start_y).abs();

            let pw = ((w * st.dpi).round() as i32) & !1;
            let ph = ((h * st.dpi).round() as i32) & !1;

            if pw >= 32 && ph >= 32 {
                let phys_x = st.virt_rect.left + (x * st.dpi).round() as i32;
                let phys_y = st.virt_rect.top + (y * st.dpi).round() as i32;
                st.result = Some(SnippingResult {
                    is_full_screen: false,
                    selected_physical_region: Some((phys_x, phys_y, pw, ph)),
                    selected_app: None,
                });
                st.done = true;
                return LRESULT(0);
            }

            let idx_opt = st.hovered_win.or_else(|| st.find_hovered_window_idx(lx, ly));
            if let Some(idx) = idx_opt {
                if let Some(win) = st.windows.get(idx).cloned() {
                    st.result = Some(SnippingResult {
                        is_full_screen: false,
                        selected_physical_region: None,
                        selected_app: Some(win),
                    });
                    st.done = true;
                    return LRESULT(0);
                }
            }

            st.render();
            LRESULT(0)
        }
        WM_CLOSE => {
            if st.is_dragging {
                st.is_dragging = false;
                let _ = ReleaseCapture();
            }
            st.done = true;
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

struct AppPickerState {
    hwnd: HWND,
    owner_hwnd: HWND,
    d2d: Rc<D2DContext>,
    surface: LayeredSurface,
    dpi: f32,
    win_x: i32,
    win_y: i32,
    windows: Vec<CapturableWindowInfo>,
    scroll_y: f32,
    hovered_item: Option<usize>,
    hovered_btn: Option<usize>,
    tracking_mouse: bool,
    meta_buf: String,
    done: bool,
    selected: Option<CapturableWindowInfo>,
}

const PICKER_BTNS: [D2D_RECT_F; 2] = [
    D2D_RECT_F { left: 332.0, top: 14.0, right: 418.0, bottom: 38.0 },
    D2D_RECT_F { left: 424.0, top: 14.0, right: 506.0, bottom: 38.0 },
];

impl AppPickerState {
    fn refresh(&mut self) {
        self.windows = enumerate_capturable_windows(self.owner_hwnd, self.hwnd);
        self.scroll_y = 0.0;
        self.hovered_item = None;
        self.render();
    }

    fn render(&mut self) {
        let lw = 520.0;
        let lh = 420.0;
        let d2d = Rc::clone(&self.d2d);
        let hovered_btn = self.hovered_btn;
        let hovered_item = self.hovered_item;
        let windows = &self.windows;
        let scroll_y = self.scroll_y;
        let mut meta_buf = std::mem::take(&mut self.meta_buf);

        self.surface
            .render(&d2d, self.hwnd, self.win_x, self.win_y, lw, lh, self.dpi, |rt, bp| {
                fill_round_rect(rt, 0.0, 0.0, lw, lh, 14.0, &bp.bg_dark);
                stroke_round_rect(rt, 0.5, 0.5, lw - 0.5, lh - 0.5, 14.0, 1.0, &bp.border_hover);

                d2d.draw_text(
                    rt,
                    "🪟 Chọn App Quay Video & Thu Âm Riêng",
                    &d2d.fmt_bold_13_left,
                    D2D_RECT_F { left: 14.0, top: 14.0, right: 325.0, bottom: 38.0 },
                    &bp.white,
                );

                let btn_labels = ["🔄 Làm mới", "✕ Đóng"];
                for (i, r) in PICKER_BTNS.iter().enumerate() {
                    let bg = if hovered_btn == Some(i) {
                        &bp.bg_hover
                    } else if i == 1 {
                        &bp.bg_active
                    } else {
                        &bp.bg_card
                    };
                    let fg = if i == 1 { &bp.accent_red } else { &bp.text_secondary };
                    fill_round_rect(rt, r.left, r.top, r.right, r.bottom, 10.0, bg);
                    stroke_round_rect(rt, r.left, r.top, r.right, r.bottom, 10.0, 1.0, &bp.border_hover);
                    d2d.draw_text(rt, btn_labels[i], &d2d.fmt_semi_11, *r, fg);
                }

                d2d.draw_text(
                    rt,
                    "🔊 Khi chọn App, hệ thống dùng WASAPI Process Loopback chỉ thu âm thanh từ đúng App đó (loại bỏ mọi tạp âm từ ứng dụng khác).",
                    &d2d.fmt_wrap_11,
                    D2D_RECT_F { left: 14.0, top: 44.0, right: lw - 14.0, bottom: 82.0 },
                    &bp.accent_cyan,
                );

                let list_top = 88.0;
                let list_bottom = lh - 14.0;
                unsafe {
                    rt.PushAxisAlignedClip(
                        &D2D_RECT_F { left: 14.0, top: list_top, right: lw - 14.0, bottom: list_bottom },
                        D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    );
                }

                if windows.is_empty() {
                    d2d.draw_text(
                        rt,
                        "Không tìm thấy cửa sổ ứng dụng nào đang mở.",
                        &d2d.fmt_semi_11,
                        D2D_RECT_F { left: 14.0, top: list_top + 30.0, right: lw - 14.0, bottom: list_top + 60.0 },
                        &bp.text_muted,
                    );
                } else {
                    for (i, win) in windows.iter().enumerate() {
                        let iy = list_top + (i as f32) * 44.0 - scroll_y;
                        if iy + 38.0 < list_top || iy > list_bottom {
                            continue;
                        }
                        let is_hov = hovered_item == Some(i);
                        fill_round_rect(
                            rt,
                            14.0,
                            iy,
                            lw - 14.0,
                            iy + 38.0,
                            8.0,
                            if is_hov { &bp.bg_hover } else { &bp.bg_card },
                        );
                        stroke_round_rect(
                            rt,
                            14.0,
                            iy,
                            lw - 14.0,
                            iy + 38.0,
                            8.0,
                            1.0,
                            if is_hov { &bp.accent_cyan } else { &bp.border_dark },
                        );

                        let badge_w =
                            (d2d.measure_text(&win.process_name, &d2d.fmt_bold_11) + 14.0).min(130.0);
                        fill_round_rect(rt, 24.0, iy + 8.0, 24.0 + badge_w, iy + 30.0, 6.0, &bp.bg_active);
                        stroke_round_rect(rt, 24.0, iy + 8.0, 24.0 + badge_w, iy + 30.0, 6.0, 1.0, &bp.accent_cyan);
                        d2d.draw_text(
                            rt,
                            &win.process_name,
                            &d2d.fmt_bold_11,
                            D2D_RECT_F { left: 24.0, top: iy + 8.0, right: 24.0 + badge_w, bottom: iy + 30.0 },
                            &bp.accent_cyan,
                        );

                        meta_buf.clear();
                        let (w, h) = (win.width(), win.height());
                        if w == 0 || h == 0 {
                            let _ = write!(meta_buf, "Thu nhỏ • PID {}", win.audio_root_pid);
                        } else {
                            let _ = write!(meta_buf, "{}×{} • PID {}", w, h, win.audio_root_pid);
                        }
                        let meta_w = d2d.measure_text(&meta_buf, &d2d.fmt_semi_10) + 10.0;
                        let meta_left = (lw - 24.0 - meta_w).max(24.0 + badge_w + 40.0);
                        d2d.draw_text(
                            rt,
                            &meta_buf,
                            &d2d.fmt_semi_10,
                            D2D_RECT_F { left: meta_left, top: iy, right: lw - 24.0, bottom: iy + 38.0 },
                            &bp.text_muted,
                        );

                        d2d.draw_text(
                            rt,
                            &win.title,
                            &d2d.fmt_semi_12_left,
                            D2D_RECT_F { left: 24.0 + badge_w + 10.0, top: iy, right: meta_left - 8.0, bottom: iy + 38.0 },
                            &bp.white,
                        );
                    }
                }

                unsafe {
                    rt.PopAxisAlignedClip();
                }
            });
        self.meta_buf = meta_buf;
    }
}

fn run_app_picker(
    d2d: Rc<D2DContext>,
    hinst: HINSTANCE,
    owner_hwnd: HWND,
) -> Option<CapturableWindowInfo> {
    let class_name = w!("QuickSnipAppPickerClass");
    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(app_picker_wnd_proc),
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        register_class_once(&PICKER_CLASS_ONCE, &wc);
    }

    let primary = get_primary_screen_physical_rect();
    let mut state = Box::new(AppPickerState {
        hwnd: HWND::default(),
        owner_hwnd,
        d2d,
        surface: LayeredSurface::new(),
        dpi: 1.0,
        win_x: 0,
        win_y: 0,
        windows: Vec::new(),
        scroll_y: 0.0,
        hovered_item: None,
        hovered_btn: None,
        tracking_mouse: false,
        meta_buf: String::with_capacity(48),
        done: false,
        selected: None,
    });

    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Chọn Ứng Dụng Quay (Thu Âm Riêng App)"),
            WS_POPUP,
            0,
            0,
            520,
            420,
            None,
            None,
            hinst,
            None,
        )
        .unwrap();

        let dpi = get_dpi_scale(hwnd);
        let phys_w = (520.0 * dpi).ceil() as i32;
        let phys_h = (420.0 * dpi).ceil() as i32;
        state.hwnd = hwnd;
        state.dpi = dpi;
        state.win_x = ((primary.right - phys_w) / 2).max(0);
        state.win_y = ((primary.bottom - phys_h) / 2).max(0);

        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        (*state_ptr).refresh();
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);

        let mut msg = MSG::default();
        while !(*state_ptr).done {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = DestroyWindow(hwnd);
        let boxed = Box::from_raw(state_ptr);
        boxed.selected
    }
}

unsafe extern "system" fn app_picker_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppPickerState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let st = &mut *ptr;

    match msg {
        WM_KEYDOWN => {
            if wparam.0 as u16 == VK_ESCAPE.0 {
                st.done = true;
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) as i16) as f32;
            let max_scroll = ((st.windows.len() as f32) * 44.0 - 318.0).max(0.0);
            let next_scroll = (st.scroll_y - delta * 0.35).clamp(0.0, max_scroll);
            if (next_scroll - st.scroll_y).abs() > 0.01 {
                st.scroll_y = next_scroll;
                st.render();
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if !st.tracking_mouse {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme).is_ok() {
                    st.tracking_mouse = true;
                }
            }
            let (lx, ly) = lparam_logical(lparam, st.dpi);

            let mut new_btn = None;
            for (i, r) in PICKER_BTNS.iter().enumerate() {
                if pt_in_rect(lx, ly, *r) {
                    new_btn = Some(i);
                    break;
                }
            }

            let mut new_item = None;
            if lx >= 14.0 && lx <= 506.0 && ly >= 88.0 && ly <= 406.0 {
                let idx = ((ly - 88.0 + st.scroll_y) / 44.0).floor() as isize;
                if idx >= 0 && (idx as usize) < st.windows.len() {
                    let iy = 88.0 + (idx as f32) * 44.0 - st.scroll_y;
                    if ly <= iy + 38.0 {
                        new_item = Some(idx as usize);
                    }
                }
            }

            if new_btn != st.hovered_btn || new_item != st.hovered_item {
                st.hovered_btn = new_btn;
                st.hovered_item = new_item;
                st.render();
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            st.tracking_mouse = false;
            if st.hovered_btn.is_some() || st.hovered_item.is_some() {
                st.hovered_btn = None;
                st.hovered_item = None;
                st.render();
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (lx, ly) = lparam_logical(lparam, st.dpi);

            if pt_in_rect(lx, ly, PICKER_BTNS[0]) {
                st.refresh();
                return LRESULT(0);
            }
            if pt_in_rect(lx, ly, PICKER_BTNS[1]) {
                st.done = true;
                return LRESULT(0);
            }

            if lx >= 14.0 && lx <= 506.0 && ly >= 88.0 && ly <= 406.0 {
                let idx = ((ly - 88.0 + st.scroll_y) / 44.0).floor() as isize;
                if idx >= 0 && (idx as usize) < st.windows.len() {
                    let iy = 88.0 + (idx as f32) * 44.0 - st.scroll_y;
                    if ly <= iy + 38.0 {
                        st.selected = Some(st.windows[idx as usize].clone());
                        st.done = true;
                        return LRESULT(0);
                    }
                }
            }

            if ly < 44.0 {
                let _ = ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            st.done = true;
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

static MAIN_CLASS_ONCE: Once = Once::new();

enum CenterItem {
    StartRec { w: f32 },
    Pill { id: BtnId, icon: IconKind, label: Cow<'static, str>, active: bool, w: f32 },
    LiveBadge { w: f32, paused: bool },
    ActionBtn {
        id: BtnId,
        icon: Option<IconKind>,
        label: Cow<'static, str>,
        kind: ActionStyle,
        w: f32,
    },
    Saving { label: Cow<'static, str>, w: f32 },
}

#[derive(Clone, Copy)]
enum ActionStyle {
    AmberOutline,
    AmberSolid,
    WhiteSolid,
    RedOutline,
}

struct ToastLayout {
    tx: f32,
    ty: f32,
    tw: f32,
    play_r: D2D_RECT_F,
    dir_r: D2D_RECT_F,
    close_r: D2D_RECT_F,
    has_file: bool,
}

struct MainWindowState {
    hwnd: HWND,
    hinst: HINSTANCE,
    d2d: Rc<D2DContext>,
    surface: LayeredSurface,
    recorder: Arc<FFmpegRecorder>,
    fps_steps: FpsSteps,
    border_window: RegionBorderWindow,
    dpi: f32,
    win_x: i32,
    win_y: i32,
    centered_once: bool,
    layout_dirty: bool,
    total_w: f32,
    total_h: f32,
    bar_w: f32,
    brand_w: f32,
    div2_x: f32,
    min_rect: D2D_RECT_F,
    close_rect: D2D_RECT_F,
    toast_layout: Option<ToastLayout>,
    hit_rects: Vec<HitRect>,
    center_items: Vec<CenterItem>,
    center_rects: Vec<D2D_RECT_F>,
    hovered_id: Option<BtnId>,
    tracking_mouse: bool,
    cur_arrow: HCURSOR,
    cur_hand: HCURSOR,
    cur_sizeall: HCURSOR,
    rec_start_time: Option<Instant>,
    paused_elapsed_ms: u64,
    timer_buf: String,
    badge_buf: String,
    live_timer_w: f32,
    toast_text: Option<String>,
    toast_has_file: bool,
    last_saved_file: Option<String>,
    in_modal: bool,
    shutting_down: bool,
}

impl MainWindowState {
    #[inline]
    fn mark_dirty(&mut self) {
        self.layout_dirty = true;
    }

    fn update_timer_buf(&mut self) -> bool {
        let active_ms = self
            .rec_start_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let total = (self.paused_elapsed_ms + active_ms) / 1000;
        let h = total / 3600;
        let m = (total % 3600) / 60;
        let s = total % 60;
        let old_len = self.timer_buf.len();
        self.timer_buf.clear();
        if h > 0 {
            let _ = write!(self.timer_buf, "{:02}:{:02}:{:02}", h, m, s);
        } else {
            let _ = write!(self.timer_buf, "{:02}:{:02}", m, s);
        }
        self.timer_buf.len() != old_len
    }

    fn pulse_opacity(&self) -> f32 {
        let Some(t) = self.rec_start_time else {
            return 1.0;
        };
        let phase = (t.elapsed().as_millis() % 1200) as f32 / 600.0;
        let frac = if phase <= 1.0 { phase } else { 2.0 - phase };
        1.0 - 0.8 * frac
    }

    fn rebuild_layout(&mut self) {
        let state = self.recorder.state();
        let d2d = Rc::clone(&self.d2d);
        let margin = 10.0;
        let bar_h = 42.0;

        self.hit_rects.clear();
        self.center_items.clear();
        self.center_rects.clear();

        let mut x = margin + 12.0;
        let top = margin;

        let brand_w = 22.0 + d2d.measure_text("QuickSnip", &d2d.fmt_bold_12);
        self.brand_w = brand_w;
        self.hit_rects.push(HitRect {
            id: BtnId::DragBrand,
            rect: D2D_RECT_F {
                left: x,
                top,
                right: x + brand_w,
                bottom: top + bar_h,
            },
        });
        x += brand_w + 8.0 + 7.0;

        let pill_w = |label: &str| -> f32 { 20.0 + 17.0 + d2d.measure_text(label, &d2d.fmt_semi_11) };

        match state {
            RecorderState::Idle | RecorderState::Starting => {
                let start_w =
                    12.0 + 13.0 + d2d.measure_text("Bắt Đầu (F9)", &d2d.fmt_bold_11) + 12.0;
                self.center_items.push(CenterItem::StartRec { w: start_w });

                self.recorder.with_config(|cfg| {
                    let area_label: Cow<'static, str> = match cfg.mode {
                        CaptureTargetMode::CustomRegion => {
                            let (_, _, rw, rh) = cfg.selected_physical_region.unwrap_or((0, 0, 0, 0));
                            Cow::Owned(format!("Vùng {}×{}", rw, rh))
                        }
                        _ => Cow::Borrowed("Toàn Màn Hình"),
                    };
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::Area,
                        icon: IconKind::Area,
                        w: pill_w(&area_label),
                        label: area_label,
                        active: cfg.mode == CaptureTargetMode::CustomRegion,
                    });

                    let app_label: Cow<'static, str> = match (&cfg.mode, &cfg.target_app_window) {
                        (CaptureTargetMode::AppWindow, Some(app)) => {
                            Cow::Owned(format!("App: {}", app.short_badge()))
                        }
                        _ => Cow::Borrowed("Chọn App"),
                    };
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::AppPicker,
                        icon: IconKind::AppWin,
                        w: pill_w(&app_label),
                        label: app_label,
                        active: cfg.mode == CaptureTargetMode::AppWindow,
                    });

                    let (aud_icon, aud_label): (IconKind, Cow<'static, str>) = if cfg.record_audio {
                        (
                            IconKind::SpeakerOn,
                            if cfg.mode == CaptureTargetMode::AppWindow {
                                Cow::Borrowed("Âm Thanh App")
                            } else {
                                Cow::Borrowed("Âm Thanh")
                            },
                        )
                    } else {
                        (IconKind::SpeakerMute, Cow::Borrowed("Tắt Tiếng"))
                    };
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::Audio,
                        icon: aud_icon,
                        w: pill_w(&aud_label),
                        label: aud_label,
                        active: cfg.record_audio,
                    });

                    let cursor_lbl = Cow::Borrowed("Chuột");
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::Cursor,
                        icon: IconKind::Cursor,
                        w: pill_w(&cursor_lbl),
                        label: cursor_lbl,
                        active: cfg.draw_mouse,
                    });

                    let fps_label = Cow::Owned(format!("{} FPS", cfg.fps));
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::Fps,
                        icon: IconKind::Fps,
                        w: pill_w(&fps_label),
                        label: fps_label,
                        active: false,
                    });

                    let automin_lbl = Cow::Borrowed("Tự Ẩn");
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::AutoMin,
                        icon: IconKind::EyeHide,
                        w: pill_w(&automin_lbl),
                        label: automin_lbl,
                        active: cfg.auto_minimize_on_record,
                    });

                    let folder_lbl = Cow::Borrowed("Thư Mục");
                    self.center_items.push(CenterItem::Pill {
                        id: BtnId::Folder,
                        icon: IconKind::Folder,
                        w: pill_w(&folder_lbl),
                        label: folder_lbl,
                        active: false,
                    });
                });
            }
            RecorderState::Recording | RecorderState::Pausing | RecorderState::Paused | RecorderState::Resuming => {
                let paused = matches!(state, RecorderState::Paused | RecorderState::Pausing);
                self.update_timer_buf();
                self.badge_buf.clear();
                if paused {
                    self.badge_buf.push_str("⏸ ĐÃ TẠM DỪNG");
                } else {
                    let (enc_badge, ddagrab) = self.recorder.active_encoder_badge();
                    self.recorder.with_config(|cfg| {
                        let audio_tag = if cfg.record_audio {
                            if cfg.mode == CaptureTargetMode::AppWindow {
                                "🔊App"
                            } else {
                                "🔊Sys"
                            }
                        } else {
                            "🔇Mute"
                        };
                        match (&cfg.mode, &cfg.target_app_window, cfg.selected_physical_region) {
                            (CaptureTargetMode::AppWindow, Some(app), _) => {
                                let _ = write!(
                                    self.badge_buf,
                                    "{}FPS • {} • 🪟{}",
                                    cfg.fps,
                                    audio_tag,
                                    app.short_badge()
                                );
                            }
                            (CaptureTargetMode::CustomRegion, _, Some((_, _, rw, rh))) => {
                                let _ = write!(
                                    self.badge_buf,
                                    "{}FPS • {} • {}×{}",
                                    cfg.fps, audio_tag, rw, rh
                                );
                            }
                            _ if ddagrab => {
                                let _ = write!(
                                    self.badge_buf,
                                    "{}FPS • {} • DXGI•{}",
                                    cfg.fps, audio_tag, enc_badge
                                );
                            }
                            _ => {
                                let _ = write!(
                                    self.badge_buf,
                                    "{}FPS • {} • {}",
                                    cfg.fps, audio_tag, enc_badge
                                );
                            }
                        }
                    });
                }

                self.live_timer_w = d2d.measure_text(&self.timer_buf, &d2d.fmt_mono_12);
                let lw_badge = 10.0
                    + 13.0
                    + self.live_timer_w
                    + 8.0
                    + d2d.measure_text(&self.badge_buf, &d2d.fmt_semi_10)
                    + 10.0;
                self.center_items.push(CenterItem::LiveBadge {
                    w: lw_badge,
                    paused,
                });

                if paused {
                    let lbl = Cow::Borrowed("Tiếp Tục (F7)");
                    let w = 22.0 + 13.0 + d2d.measure_text(&lbl, &d2d.fmt_bold_11);
                    self.center_items.push(CenterItem::ActionBtn {
                        id: BtnId::Resume,
                        icon: Some(IconKind::Play),
                        label: lbl,
                        kind: ActionStyle::AmberSolid,
                        w,
                    });
                } else {
                    let lbl = Cow::Borrowed("Tạm Dừng (F7)");
                    let w = 22.0 + 13.0 + d2d.measure_text(&lbl, &d2d.fmt_bold_11);
                    self.center_items.push(CenterItem::ActionBtn {
                        id: BtnId::Pause,
                        icon: Some(IconKind::Pause),
                        label: lbl,
                        kind: ActionStyle::AmberOutline,
                        w,
                    });
                }

                let stop_lbl = Cow::Borrowed("Dừng (F9)");
                let stop_w = 22.0 + 13.0 + d2d.measure_text(&stop_lbl, &d2d.fmt_bold_11);
                self.center_items.push(CenterItem::ActionBtn {
                    id: BtnId::Stop,
                    icon: Some(IconKind::Stop),
                    label: stop_lbl,
                    kind: ActionStyle::WhiteSolid,
                    w: stop_w,
                });

                let cancel_lbl = Cow::Borrowed("✕ Hủy (F8)");
                let cancel_w = 22.0 + d2d.measure_text(&cancel_lbl, &d2d.fmt_bold_11);
                self.center_items.push(CenterItem::ActionBtn {
                    id: BtnId::Cancel,
                    icon: None,
                    label: cancel_lbl,
                    kind: ActionStyle::RedOutline,
                    w: cancel_w,
                });
            }
            RecorderState::Stopping => {
                let lbl = Cow::Borrowed("⏳ Đang đóng gói & lưu video...");
                let w = 24.0 + d2d.measure_text(&lbl, &d2d.fmt_semi_11);
                self.center_items.push(CenterItem::Saving { label: lbl, w });
            }
        }

        let item_count = self.center_items.len();
        for (idx, item) in self.center_items.iter().enumerate() {
            let w = match item {
                CenterItem::StartRec { w } => *w,
                CenterItem::Pill { w, .. } => *w,
                CenterItem::LiveBadge { w, .. } => *w,
                CenterItem::ActionBtn { w, .. } => *w,
                CenterItem::Saving { w, .. } => *w,
            };
            let rect = D2D_RECT_F {
                left: x,
                top: top + 6.0,
                right: x + w,
                bottom: top + 36.0,
            };
            self.center_rects.push(rect);
            match item {
                CenterItem::StartRec { .. } => {
                    self.hit_rects.push(HitRect { id: BtnId::StartRecord, rect })
                }
                CenterItem::Pill { id, .. } => self.hit_rects.push(HitRect { id: *id, rect }),
                CenterItem::ActionBtn { id, .. } => self.hit_rects.push(HitRect { id: *id, rect }),
                _ => {}
            }
            let gap = if idx + 1 == item_count { 6.0 } else { 5.0 };
            x += w + gap;
        }

        self.div2_x = x;
        x += 7.0;

        self.min_rect = D2D_RECT_F {
            left: x,
            top: top + 9.0,
            right: x + 24.0,
            bottom: top + 33.0,
        };
        self.hit_rects.push(HitRect { id: BtnId::Minimize, rect: self.min_rect });
        x += 27.0;

        self.close_rect = D2D_RECT_F {
            left: x,
            top: top + 9.0,
            right: x + 24.0,
            bottom: top + 33.0,
        };
        self.hit_rects.push(HitRect { id: BtnId::Close, rect: self.close_rect });
        x += 24.0 + 10.0;

        self.bar_w = x - margin;
        self.total_w = self.bar_w + margin * 2.0;

        self.toast_layout = None;
        self.total_h = if let Some(ref txt) = self.toast_text {
            let text_w = d2d.measure_text(txt, &d2d.fmt_semi_11);
            let ty = top + bar_h + 8.0;
            if self.toast_has_file {
                let play_w = 20.0 + d2d.measure_text("▶ Xem", &d2d.fmt_semi_10);
                let dir_w = 20.0 + d2d.measure_text("📁 Mở", &d2d.fmt_semi_10);
                let fixed_chrome = 12.0 + 10.0 + play_w + 4.0 + dir_w + 4.0 + 18.0 + 8.0;
                let max_toast_w = self.bar_w + 180.0;
                let toast_w = (fixed_chrome + text_w).min(max_toast_w);
                self.total_w = self.total_w.max(toast_w + margin * 2.0);
                let tx = ((self.total_w - toast_w) / 2.0).max(margin);
                let close_r = D2D_RECT_F {
                    left: tx + toast_w - 8.0 - 18.0,
                    top: ty + 8.0,
                    right: tx + toast_w - 8.0,
                    bottom: ty + 26.0,
                };
                let dir_r = D2D_RECT_F {
                    left: close_r.left - 4.0 - dir_w,
                    top: ty + 6.0,
                    right: close_r.left - 4.0,
                    bottom: ty + 28.0,
                };
                let play_r = D2D_RECT_F {
                    left: dir_r.left - 4.0 - play_w,
                    top: ty + 6.0,
                    right: dir_r.left - 4.0,
                    bottom: ty + 28.0,
                };
                self.hit_rects.push(HitRect { id: BtnId::ToastPlay, rect: play_r });
                self.hit_rects.push(HitRect { id: BtnId::ToastFolder, rect: dir_r });
                self.hit_rects.push(HitRect { id: BtnId::ToastClose, rect: close_r });
                self.toast_layout = Some(ToastLayout {
                    tx,
                    ty,
                    tw: toast_w,
                    play_r,
                    dir_r,
                    close_r,
                    has_file: true,
                });
            } else {
                let toast_w = (12.0 + text_w + 8.0 + 18.0 + 8.0).min(self.bar_w + 80.0);
                self.total_w = self.total_w.max(toast_w + margin * 2.0);
                let tx = ((self.total_w - toast_w) / 2.0).max(margin);
                let close_r = D2D_RECT_F {
                    left: tx + toast_w - 8.0 - 18.0,
                    top: ty + 8.0,
                    right: tx + toast_w - 8.0,
                    bottom: ty + 26.0,
                };
                let empty_r = D2D_RECT_F {
                    left: close_r.left - 8.0,
                    top: ty,
                    right: close_r.left - 8.0,
                    bottom: ty,
                };
                self.hit_rects.push(HitRect { id: BtnId::ToastClose, rect: close_r });
                self.toast_layout = Some(ToastLayout {
                    tx,
                    ty,
                    tw: toast_w,
                    play_r: empty_r,
                    dir_r: empty_r,
                    close_r,
                    has_file: false,
                });
            }
            margin * 2.0 + bar_h + 8.0 + 34.0
        } else {
            margin * 2.0 + bar_h
        };

        self.layout_dirty = false;
    }

    fn render(&mut self) {
        let new_dpi = get_dpi_scale(self.hwnd);
        if (new_dpi - self.dpi).abs() > 0.001 {
            self.dpi = new_dpi;
            self.layout_dirty = true;
        }
        if !self.layout_dirty && self.rec_start_time.is_some() && self.update_timer_buf() {
            self.layout_dirty = true;
        }
        if self.layout_dirty {
            self.rebuild_layout();
        }

        let margin = 10.0;
        let top = margin;
        let bar_h = 42.0;

        if !self.centered_once {
            let primary = get_primary_screen_physical_rect();
            let pw = (self.total_w * self.dpi).ceil() as i32;
            let ph = (self.total_h * self.dpi).ceil() as i32;
            self.win_x = ((primary.right - pw) / 2).max(0);
            self.win_y = ((primary.bottom - ph) / 2).max(0);
            self.centered_once = true;
        }

        let d2d = Rc::clone(&self.d2d);
        let hovered = self.hovered_id;
        let pulse = self.pulse_opacity();
        let bar_w = self.bar_w;
        let brand_w = self.brand_w;
        let div2_x = self.div2_x;
        let min_rect = self.min_rect;
        let close_rect = self.close_rect;
        let total_w = self.total_w;
        let total_h = self.total_h;
        let live_timer_w = self.live_timer_w;
        let timer_str = self.timer_buf.as_str();
        let badge_str = self.badge_buf.as_str();
        let center_items = &self.center_items;
        let center_rects = &self.center_rects;
        let toast_layout = &self.toast_layout;
        let toast_text = self.toast_text.as_deref();

        self.surface.render(
            &d2d,
            self.hwnd,
            self.win_x,
            self.win_y,
            total_w,
            total_h,
            self.dpi,
            |rt, bp| {
                fill_round_rect(rt, margin, top, margin + bar_w, top + bar_h, 21.0, &bp.bg_dark);
                stroke_round_rect(rt, margin, top, margin + bar_w, top + bar_h, 21.0, 1.0, &bp.border_dark);

                let bx = margin + 12.0;
                let by = top + 13.0;
                stroke_round_rect(rt, bx + 2.0, by + 1.5, bx + 14.0, by + 14.5, 3.0, 1.3, &bp.white);
                unsafe {
                    rt.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: D2D_POINT_2F { x: bx + 8.0, y: by + 8.5 },
                            radiusX: 2.0,
                            radiusY: 2.0,
                        },
                        &bp.white,
                    );
                }
                d2d.draw_text(
                    rt,
                    "QuickSnip",
                    &d2d.fmt_bold_12,
                    D2D_RECT_F { left: bx + 20.0, top, right: bx + brand_w, bottom: top + bar_h },
                    &bp.white,
                );

                let div1_x = bx + brand_w + 8.0;
                unsafe {
                    rt.FillRectangle(
                        &D2D_RECT_F { left: div1_x, top: top + 13.0, right: div1_x + 1.0, bottom: top + 29.0 },
                        &bp.border_dark,
                    );
                    rt.FillRectangle(
                        &D2D_RECT_F { left: div2_x, top: top + 13.0, right: div2_x + 1.0, bottom: top + 29.0 },
                        &bp.border_dark,
                    );
                }

                for (item, r) in center_items.iter().zip(center_rects.iter()) {
                    match item {
                        CenterItem::StartRec { .. } => {
                            let bg = if hovered == Some(BtnId::StartRecord) {
                                &bp.text_secondary
                            } else {
                                &bp.white
                            };
                            fill_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, bg);
                            unsafe {
                                rt.FillEllipse(
                                    &D2D1_ELLIPSE {
                                        point: D2D_POINT_2F { x: r.left + 15.5, y: r.top + 15.0 },
                                        radiusX: 3.5,
                                        radiusY: 3.5,
                                    },
                                    &bp.accent_red,
                                );
                            }
                            d2d.draw_text(
                                rt,
                                "Bắt Đầu (F9)",
                                &d2d.fmt_bold_11,
                                D2D_RECT_F { left: r.left + 22.0, top: r.top, right: r.right - 8.0, bottom: r.bottom },
                                &bp.black,
                            );
                        }
                        CenterItem::Pill { id, icon, label, active, .. } => {
                            let is_hov = hovered == Some(*id);
                            let bg = if is_hov {
                                &bp.bg_hover
                            } else if *active {
                                &bp.bg_active
                            } else {
                                &bp.bg_card
                            };
                            let brd = if is_hov {
                                &bp.border_hover
                            } else if *active {
                                &bp.border_active
                            } else {
                                &bp.border_dark
                            };
                            let fg = if is_hov || *active { &bp.white } else { &bp.text_secondary };
                            let ic = if is_hov || *active { &bp.white } else { &bp.text_muted };

                            fill_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, bg);
                            stroke_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, 1.0, brd);
                            draw_icon(rt, *icon, r.left + 10.0, r.top + 9.0, ic);
                            d2d.draw_text(
                                rt,
                                label,
                                &d2d.fmt_semi_11,
                                D2D_RECT_F { left: r.left + 25.0, top: r.top, right: r.right - 8.0, bottom: r.bottom },
                                fg,
                            );
                        }
                        CenterItem::LiveBadge { paused, .. } => {
                            let brd = if *paused { &bp.accent_amber } else { &bp.border_active };
                            fill_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, &bp.bg_card);
                            stroke_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, 1.0, brd);

                            let dot_b = if *paused {
                                &bp.accent_amber
                            } else {
                                let mut c = C_ACCENT_RED;
                                c.a = pulse;
                                unsafe { bp.scratch.SetColor(&c); }
                                &bp.scratch
                            };
                            unsafe {
                                rt.FillEllipse(
                                    &D2D1_ELLIPSE {
                                        point: D2D_POINT_2F { x: r.left + 13.5, y: r.top + 15.0 },
                                        radiusX: 3.5,
                                        radiusY: 3.5,
                                    },
                                    dot_b,
                                );
                            }
                            d2d.draw_text(
                                rt,
                                timer_str,
                                &d2d.fmt_mono_12,
                                D2D_RECT_F { left: r.left + 21.0, top: r.top, right: r.left + 21.0 + live_timer_w, bottom: r.bottom },
                                &bp.white,
                            );
                            d2d.draw_text(
                                rt,
                                badge_str,
                                &d2d.fmt_semi_10,
                                D2D_RECT_F { left: r.left + 21.0 + live_timer_w + 6.0, top: r.top, right: r.right - 8.0, bottom: r.bottom },
                                &bp.text_muted,
                            );
                        }
                        CenterItem::ActionBtn { id, icon, label, kind, .. } => {
                            let (mut base_c, fg, brd) = match kind {
                                ActionStyle::AmberOutline => (C_BG_CARD, &bp.accent_amber, Some(&bp.border_hover)),
                                ActionStyle::AmberSolid => (C_ACCENT_AMBER, &bp.black, None),
                                ActionStyle::WhiteSolid => (C_WHITE, &bp.black, None),
                                ActionStyle::RedOutline => (C_BG_CARD, &bp.accent_red, Some(&bp.border_hover)),
                            };
                            if hovered == Some(*id) {
                                base_c.a = 0.88;
                            }
                            unsafe { bp.scratch.SetColor(&base_c); }
                            fill_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, &bp.scratch);
                            if let Some(b) = brd {
                                stroke_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, 1.0, b);
                            }
                            let text_left = if let Some(ic) = icon {
                                draw_icon(rt, *ic, r.left + 10.0, r.top + 9.0, fg);
                                r.left + 23.0
                            } else {
                                r.left + 8.0
                            };
                            d2d.draw_text(
                                rt,
                                label,
                                &d2d.fmt_bold_11,
                                D2D_RECT_F { left: text_left, top: r.top, right: r.right - 8.0, bottom: r.bottom },
                                fg,
                            );
                        }
                        CenterItem::Saving { label, .. } => {
                            fill_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, &bp.bg_card);
                            stroke_round_rect(rt, r.left, r.top, r.right, r.bottom, 15.0, 1.0, &bp.border_active);
                            d2d.draw_text(rt, label, &d2d.fmt_semi_11, *r, &bp.white);
                        }
                    }
                }

                let min_hov = hovered == Some(BtnId::Minimize);
                if min_hov {
                    fill_round_rect(rt, min_rect.left, min_rect.top, min_rect.right, min_rect.bottom, 12.0, &bp.bg_hover);
                }
                unsafe {
                    rt.DrawLine(
                        D2D_POINT_2F { x: min_rect.left + 7.0, y: min_rect.top + 12.0 },
                        D2D_POINT_2F { x: min_rect.right - 7.0, y: min_rect.top + 12.0 },
                        if min_hov { &bp.white } else { &bp.text_muted },
                        1.4,
                        None,
                    );
                }

                let cls_hov = hovered == Some(BtnId::Close);
                if cls_hov {
                    fill_round_rect(rt, close_rect.left, close_rect.top, close_rect.right, close_rect.bottom, 12.0, &bp.accent_red);
                }
                let cb = if cls_hov { &bp.white } else { &bp.text_muted };
                unsafe {
                    rt.DrawLine(
                        D2D_POINT_2F { x: close_rect.left + 8.0, y: close_rect.top + 8.0 },
                        D2D_POINT_2F { x: close_rect.right - 8.0, y: close_rect.bottom - 8.0 },
                        cb,
                        1.4,
                        None,
                    );
                    rt.DrawLine(
                        D2D_POINT_2F { x: close_rect.right - 8.0, y: close_rect.top + 8.0 },
                        D2D_POINT_2F { x: close_rect.left + 8.0, y: close_rect.bottom - 8.0 },
                        cb,
                        1.4,
                        None,
                    );
                }

                if let (Some(tl), Some(txt)) = (toast_layout, toast_text) {
                    fill_round_rect(rt, tl.tx, tl.ty, tl.tx + tl.tw, tl.ty + 34.0, 17.0, &bp.bg_card);
                    stroke_round_rect(rt, tl.tx, tl.ty, tl.tx + tl.tw, tl.ty + 34.0, 17.0, 1.0, &bp.border_hover);
                    let text_right = if tl.has_file { tl.play_r.left - 8.0 } else { tl.close_r.left - 8.0 };
                    d2d.draw_text(
                        rt,
                        txt,
                        &d2d.fmt_semi_11,
                        D2D_RECT_F { left: tl.tx + 12.0, top: tl.ty, right: text_right, bottom: tl.ty + 34.0 },
                        &bp.white,
                    );
                    if tl.has_file {
                        fill_round_rect(rt, tl.play_r.left, tl.play_r.top, tl.play_r.right, tl.play_r.bottom, 11.0, &bp.white);
                        d2d.draw_text(rt, "▶ Xem", &d2d.fmt_semi_10, tl.play_r, &bp.black);
                        fill_round_rect(rt, tl.dir_r.left, tl.dir_r.top, tl.dir_r.right, tl.dir_r.bottom, 11.0, &bp.bg_hover);
                        stroke_round_rect(rt, tl.dir_r.left, tl.dir_r.top, tl.dir_r.right, tl.dir_r.bottom, 11.0, 1.0, &bp.border_hover);
                        d2d.draw_text(rt, "📁 Mở", &d2d.fmt_semi_10, tl.dir_r, &bp.white);
                    }
                    d2d.draw_text(rt, "✕", &d2d.fmt_semi_10, tl.close_r, &bp.text_muted);
                }
            },
        );
    }

    fn sync_rec_timer(&self) {
        unsafe {
            let visible = IsWindowVisible(self.hwnd).as_bool() && !IsIconic(self.hwnd).as_bool();
            if visible && self.recorder.state() == RecorderState::Recording {
                let _ = SetTimer(self.hwnd, TIMER_REC_ID, 250, None);
            } else {
                let _ = KillTimer(self.hwnd, TIMER_REC_ID);
            }
        }
    }

    fn start_record(&mut self) {
        if self.in_modal || self.recorder.state() != RecorderState::Idle {
            return;
        }
        self.toast_text = None;
        self.toast_has_file = false;
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_TOAST_ID);
        }
        self.border_window.hide();

        let (auto_min, mode) = self.recorder.with_config(|c| (c.auto_minimize_on_record, c.mode));
        if auto_min {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }

        if let Err(err) = self.recorder.start_recording() {
            if mode != CaptureTargetMode::FullScreen {
                self.border_window.show();
            }
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOW);
                let _ = SetForegroundWindow(self.hwnd);
            }
            self.mark_dirty();
            self.render();
            log_error(&format!("StartRecord failed: {}", err));
            let w_msg = to_wide_null(&format!("Không thể bắt đầu quay: {}", err));
            unsafe {
                MessageBoxW(
                    self.hwnd,
                    PCWSTR(w_msg.as_ptr()),
                    w!("Thông Báo"),
                    MB_OK | MB_ICONERROR,
                );
            }
        } else if !auto_min {
            self.mark_dirty();
            self.render();
        }
    }

    fn stop_record(&mut self, discard: bool) {
        if self.in_modal {
            return;
        }
        if self.recorder.stop_recording(discard) {
            self.reset_rec_timer_state();
            self.mark_dirty();
            self.render();
        }
    }

    fn toggle_recording(&mut self) {
        if self.in_modal {
            return;
        }
        match self.recorder.state() {
            RecorderState::Idle => self.start_record(),
            RecorderState::Recording
            | RecorderState::Pausing
            | RecorderState::Paused
            | RecorderState::Resuming => self.stop_record(false),
            _ => {}
        }
    }

    fn toggle_pause(&mut self) {
        if self.in_modal {
            return;
        }
        match self.recorder.state() {
            RecorderState::Recording => {
                if let Some(t) = self.rec_start_time.take() {
                    self.paused_elapsed_ms += t.elapsed().as_millis() as u64;
                }
                unsafe {
                    let _ = KillTimer(self.hwnd, TIMER_REC_ID);
                }
                if self.recorder.pause_recording() {
                    self.mark_dirty();
                    self.render();
                }
            }
            RecorderState::Paused => {
                if self.recorder.with_config(|c| c.auto_minimize_on_record) {
                    unsafe {
                        let _ = ShowWindow(self.hwnd, SW_HIDE);
                    }
                }
                if self.recorder.resume_recording() {
                    self.mark_dirty();
                    if !self.recorder.with_config(|c| c.auto_minimize_on_record) {
                        self.render();
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_selected_app(&mut self, mut app: CapturableWindowInfo) {
        let app_hwnd = HWND(app.handle as *mut _);
        unsafe {
            if IsIconic(app_hwnd).as_bool() {
                let _ = ShowWindow(app_hwnd, SW_RESTORE);
            }
        }
        if let Some(bounds) = try_get_window_physical_bounds(app_hwnd) {
            app.physical_bounds = bounds;
        }
        let b = app.physical_bounds;
        let pw = app.width() & !1;
        let ph = app.height() & !1;
        let name = app.process_name.clone();
        self.recorder.update_config(|c| {
            c.mode = CaptureTargetMode::AppWindow;
            c.target_app_window = Some(app);
            c.selected_physical_region = Some((b.left, b.top, pw, ph));
        });
        self.border_window.update_and_show(b.left, b.top, pw, ph, Some(&name));
    }

    fn reset_rec_timer_state(&mut self) {
        self.rec_start_time = None;
        self.paused_elapsed_ms = 0;
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_REC_ID);
        }
    }

    fn select_area_or_window(&mut self) {
        if self.in_modal || self.recorder.state() != RecorderState::Idle {
            return;
        }
        self.in_modal = true;
        self.border_window.hide();
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
        let res = run_snipping_overlay(Rc::clone(&self.d2d), self.hinst, self.hwnd);
        self.in_modal = false;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }

        let Some(r) = res else {
            if self.recorder.with_config(|c| c.mode != CaptureTargetMode::FullScreen) {
                self.border_window.show();
            }
            return;
        };
        if r.is_full_screen {
            self.recorder.update_config(|c| {
                c.mode = CaptureTargetMode::FullScreen;
                c.target_app_window = None;
                c.selected_physical_region = None;
            });
            self.border_window.hide();
        } else if let Some(app) = r.selected_app {
            self.apply_selected_app(app);
        } else if let Some((rx, ry, rw, rh)) = r.selected_physical_region {
            self.recorder.update_config(|c| {
                c.mode = CaptureTargetMode::CustomRegion;
                c.target_app_window = None;
                c.selected_physical_region = Some((rx, ry, rw, rh));
            });
            self.border_window.update_and_show(rx, ry, rw, rh, None);
        }
        self.mark_dirty();
        self.render();
    }

    fn select_specific_app(&mut self) {
        if self.in_modal || self.recorder.state() != RecorderState::Idle {
            return;
        }
        self.in_modal = true;
        unsafe {
            let _ = EnableWindow(self.hwnd, false);
        }
        let selected = run_app_picker(Rc::clone(&self.d2d), self.hinst, self.hwnd);
        self.in_modal = false;
        unsafe {
            let _ = EnableWindow(self.hwnd, true);
            let _ = SetForegroundWindow(self.hwnd);
        }

        let Some(app) = selected else {
            return;
        };
        self.apply_selected_app(app);
        self.mark_dirty();
        self.render();
    }

    fn handle_recorder_events(&mut self) {
        while let Some(ev) = self.recorder.pop_event() {
            match ev {
                RecorderEvent::Started => {
                    self.paused_elapsed_ms = 0;
                    self.rec_start_time = Some(Instant::now());
                    self.mark_dirty();
                    if self.recorder.with_config(|c| c.auto_minimize_on_record) {
                        unsafe {
                            let _ = ShowWindow(self.hwnd, SW_HIDE);
                        }
                    } else {
                        unsafe {
                            let _ = ShowWindow(self.hwnd, SW_RESTORE);
                            let _ = SetForegroundWindow(self.hwnd);
                        }
                        self.render();
                    }
                    self.sync_rec_timer();
                }
                RecorderEvent::Paused => {
                    unsafe {
                        let _ = KillTimer(self.hwnd, TIMER_REC_ID);
                        let _ = ShowWindow(self.hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(self.hwnd);
                    }
                    self.mark_dirty();
                    self.render();
                }
                RecorderEvent::Resumed => {
                    self.rec_start_time = Some(Instant::now());
                    self.mark_dirty();
                    if self.recorder.with_config(|c| c.auto_minimize_on_record) {
                        unsafe {
                            let _ = ShowWindow(self.hwnd, SW_HIDE);
                        }
                    } else {
                        self.render();
                    }
                    self.sync_rec_timer();
                }
                RecorderEvent::Completed(path) => {
                    self.reset_rec_timer_state();
                    let fname = Path::new(&path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("video.mp4");
                    self.toast_text = Some(format!("✅ Đã lưu: {}", fname));
                    self.toast_has_file = true;
                    self.last_saved_file = Some(path);
                    self.restore_after_stop();
                    unsafe {
                        let _ = SetTimer(self.hwnd, TIMER_TOAST_ID, 6000, None);
                    }
                }
                RecorderEvent::Cancelled => {
                    self.reset_rec_timer_state();
                    self.toast_text = Some("⚠️ Đã hủy bỏ quay video".to_string());
                    self.toast_has_file = false;
                    self.restore_after_stop();
                    unsafe {
                        let _ = SetTimer(self.hwnd, TIMER_TOAST_ID, 6000, None);
                    }
                }
                RecorderEvent::Failed(err) => {
                    self.reset_rec_timer_state();
                    self.restore_after_stop();
                    log_error(&format!("Recording failed: {}", err));
                    if !self.shutting_down {
                        let w_msg = to_wide_null(&format!("Quá trình quay gặp lỗi: {}", err));
                        unsafe {
                            MessageBoxW(
                                self.hwnd,
                                PCWSTR(w_msg.as_ptr()),
                                w!("Thông Báo"),
                                MB_OK | MB_ICONERROR,
                            );
                        }
                    }
                }
            }
        }
    }

    fn restore_after_stop(&mut self) {
        if self.recorder.with_config(|c| c.mode != CaptureTargetMode::FullScreen) {
            self.border_window.show();
        }
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.mark_dirty();
        self.render();
    }

    fn shutdown_immediately(&mut self, destroy_wnd: bool) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
            let _ = KillTimer(self.hwnd, TIMER_REC_ID);
            let _ = KillTimer(self.hwnd, TIMER_TOAST_ID);
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_F9);
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_F8);
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_F7);
            let _ = UnregisterHotKey(self.hwnd, HOTKEY_CTRL_SHIFT_R);
        }
        self.recorder.force_kill();
        self.border_window.close();
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            if destroy_wnd {
                let _ = DestroyWindow(self.hwnd);
            }
            PostQuitMessage(0);
        }
    }
}

fn run_app(recorder: Arc<FFmpegRecorder>) {
    let d2d = Rc::new(D2DContext::new());
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap().into() };
    let border_window = RegionBorderWindow::create(Rc::clone(&d2d), hinst);
    let app_icon = load_app_icon(hinst);

    let class_name = w!("QuickSnipMainWindowClass");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(main_wnd_proc),
        hInstance: hinst,
        hIcon: app_icon,
        lpszClassName: class_name,
        ..Default::default()
    };
    register_class_once(&MAIN_CLASS_ONCE, &wc);

    let (cur_arrow, cur_hand, cur_sizeall) = unsafe {
        (
            LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            LoadCursorW(None, IDC_HAND).unwrap_or_default(),
            LoadCursorW(None, IDC_SIZEALL).unwrap_or_default(),
        )
    };

    let state = Box::new(MainWindowState {
        hwnd: HWND::default(),
        hinst,
        d2d,
        surface: LayeredSurface::new(),
        recorder: Arc::clone(&recorder),
        fps_steps: get_dynamic_fps_steps(),
        border_window,
        dpi: 1.0,
        win_x: 0,
        win_y: 0,
        centered_once: false,
        layout_dirty: true,
        total_w: 0.0,
        total_h: 0.0,
        bar_w: 0.0,
        brand_w: 0.0,
        div2_x: 0.0,
        min_rect: D2D_RECT_F::default(),
        close_rect: D2D_RECT_F::default(),
        toast_layout: None,
        hit_rects: Vec::with_capacity(14),
        center_items: Vec::with_capacity(8),
        center_rects: Vec::with_capacity(8),
        hovered_id: None,
        tracking_mouse: false,
        cur_arrow,
        cur_hand,
        cur_sizeall,
        rec_start_time: None,
        paused_elapsed_ms: 0,
        timer_buf: String::with_capacity(16),
        badge_buf: String::with_capacity(64),
        live_timer_w: 0.0,
        toast_text: None,
        toast_has_file: false,
        last_saved_file: None,
        in_modal: false,
        shutting_down: false,
    });
    let state_ptr = Box::into_raw(state);

    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST,
            class_name,
            w!("QuickSnip"),
            WS_POPUP,
            0,
            0,
            600,
            80,
            None,
            None,
            hinst,
            None,
        )
        .unwrap();

        (*state_ptr).hwnd = hwnd;
        apply_window_icons(hwnd, hinst, app_icon);
        recorder.set_notify_hwnd(hwnd);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        let _ = RegisterHotKey(hwnd, HOTKEY_F9, HOT_KEY_MODIFIERS(0), VK_F9.0 as u32);
        let _ = RegisterHotKey(hwnd, HOTKEY_F8, HOT_KEY_MODIFIERS(0), VK_F8.0 as u32);
        let _ = RegisterHotKey(hwnd, HOTKEY_F7, HOT_KEY_MODIFIERS(0), VK_F7.0 as u32);
        let _ = RegisterHotKey(
            hwnd,
            HOTKEY_CTRL_SHIFT_R,
            HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_SHIFT.0),
            0x52,
        );

        (*state_ptr).render();
        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let _ = Box::from_raw(state_ptr);
    }
}

unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindowState;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let st = &mut *ptr;

    match msg {
        WM_APP_RECORDER_EVENT => {
            st.handle_recorder_events();
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_REC_ID {
                if !IsWindowVisible(hwnd).as_bool() {
                    let _ = KillTimer(hwnd, TIMER_REC_ID);
                } else if !IsIconic(hwnd).as_bool() {
                    st.render();
                }
            } else if wparam.0 == TIMER_TOAST_ID {
                st.toast_text = None;
                st.toast_has_file = false;
                let _ = KillTimer(hwnd, TIMER_TOAST_ID);
                st.mark_dirty();
                st.render();
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            st.mark_dirty();
            st.render();
            LRESULT(0)
        }
        WM_HOTKEY => {
            match wparam.0 as i32 {
                HOTKEY_F9 | HOTKEY_CTRL_SHIFT_R => st.toggle_recording(),
                HOTKEY_F7 => st.toggle_pause(),
                HOTKEY_F8 => st.stop_record(true),
                _ => {}
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            let ctrl = (GetKeyState(VK_CONTROL.0 as i32) & -128) != 0;
            let shift = (GetKeyState(VK_SHIFT.0 as i32) & -128) != 0;
            if vk == VK_F9.0 || (vk == 0x52 && ctrl && shift) {
                st.toggle_recording();
            } else if vk == VK_F7.0 {
                st.toggle_pause();
            } else if vk == VK_F8.0 || vk == VK_ESCAPE.0 {
                st.stop_record(true);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if !st.tracking_mouse {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                if TrackMouseEvent(&mut tme).is_ok() {
                    st.tracking_mouse = true;
                }
            }

            let (lx, ly) = lparam_logical(lparam, st.dpi);
            let mut new_hov = None;
            for hr in &st.hit_rects {
                if pt_in_rect(lx, ly, hr.rect) {
                    new_hov = Some(hr.id);
                    break;
                }
            }

            let cur = match new_hov {
                Some(BtnId::DragBrand) => st.cur_sizeall,
                Some(_) => st.cur_hand,
                None => st.cur_arrow,
            };
            if !cur.0.is_null() {
                SetCursor(cur);
            }

            if new_hov != st.hovered_id {
                st.hovered_id = new_hov;
                st.render();
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            st.tracking_mouse = false;
            if st.hovered_id.is_some() {
                st.hovered_id = None;
                st.render();
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (lx, ly) = lparam_logical(lparam, st.dpi);
            let mut clicked = None;
            for hr in &st.hit_rects {
                if pt_in_rect(lx, ly, hr.rect) {
                    clicked = Some(hr.id);
                    break;
                }
            }

            match clicked {
                Some(BtnId::StartRecord) => st.start_record(),
                Some(BtnId::Area) => st.select_area_or_window(),
                Some(BtnId::AppPicker) => st.select_specific_app(),
                Some(BtnId::Audio) => {
                    st.recorder.update_config(|c| c.record_audio = !c.record_audio);
                    st.mark_dirty();
                    st.render();
                }
                Some(BtnId::Cursor) => {
                    st.recorder.update_config(|c| c.draw_mouse = !c.draw_mouse);
                    st.mark_dirty();
                    st.render();
                }
                Some(BtnId::Fps) => {
                    let steps = st.fps_steps.slice();
                    st.recorder.update_config(|c| {
                        let idx = steps.iter().position(|&f| f == c.fps).unwrap_or(0);
                        c.fps = steps[(idx + 1) % steps.len()];
                    });
                    st.mark_dirty();
                    st.render();
                }
                Some(BtnId::AutoMin) => {
                    st.recorder
                        .update_config(|c| c.auto_minimize_on_record = !c.auto_minimize_on_record);
                    st.mark_dirty();
                    st.render();
                }
                Some(BtnId::Folder) => {
                    let dir = st.recorder.with_config(|c| c.output_directory.clone());
                    let _ = fs::create_dir_all(&dir);
                    shell_open(&dir.to_string_lossy());
                }
                Some(BtnId::Pause) | Some(BtnId::Resume) => st.toggle_pause(),
                Some(BtnId::Stop) => st.stop_record(false),
                Some(BtnId::Cancel) => st.stop_record(true),
                Some(BtnId::Minimize) => {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                Some(BtnId::Close) => st.shutdown_immediately(true),
                Some(BtnId::ToastPlay) => {
                    if let Some(ref p) = st.last_saved_file {
                        if Path::new(p).exists() {
                            shell_open(p);
                        }
                    }
                }
                Some(BtnId::ToastFolder) => {
                    if let Some(ref p) = st.last_saved_file {
                        if Path::new(p).exists() {
                            explorer_select_file(p);
                        } else {
                            let dir = st.recorder.with_config(|c| c.output_directory.clone());
                            shell_open(&dir.to_string_lossy());
                        }
                    }
                }
                Some(BtnId::ToastClose) => {
                    st.toast_text = None;
                    st.toast_has_file = false;
                    let _ = KillTimer(hwnd, TIMER_TOAST_ID);
                    st.mark_dirty();
                    st.render();
                }
                Some(BtnId::DragBrand) | None => {
                    let _ = ReleaseCapture();
                    SendMessageW(hwnd, WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
                    let mut r = RECT::default();
                    if GetWindowRect(hwnd, &mut r).is_ok() {
                        st.win_x = r.left;
                        st.win_y = r.top;
                    }
                    let new_dpi = get_dpi_scale(hwnd);
                    if (new_dpi - st.dpi).abs() > 0.001 {
                        st.mark_dirty();
                        st.render();
                    }
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            st.shutdown_immediately(true);
            LRESULT(0)
        }
        WM_DESTROY => {
            st.shutdown_immediately(false);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn clean_old_temp_files() {
    let temp = std::env::temp_dir();
    let Ok(entries) = fs::read_dir(temp) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let s = name.to_string_lossy();
        let is_stale = (s.starts_with("quicksnip_s") && s.ends_with(".mp4"))
            || (s.starts_with("concat_s") && s.ends_with(".txt"));
        if is_stale {
            if let Ok(meta) = entry.metadata() {
                if let Ok(created) = meta.created().or_else(|_| meta.modified()) {
                    if now.duration_since(created).unwrap_or_default() > Duration::from_secs(3600) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        log_error(&format!("Panic: {}", info));
    }));

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let mutex_res = CreateMutexW(None, true, w!("QuickSnipRecorder_SingleInstance_Mutex_2026"));
        let Ok(mutex) = mutex_res else {
            CoUninitialize();
            return;
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            if let Ok(existing) = FindWindowW(None, w!("QuickSnip")) {
                if !existing.0.is_null() {
                    let _ = ShowWindow(existing, SW_RESTORE);
                    let _ = SetForegroundWindow(existing);
                }
            }
            CoUninitialize();
            return;
        }

        warm_up_probe();
        thread::spawn(clean_old_temp_files);

        let recorder = FFmpegRecorder::new();
        run_app(recorder);

        let _ = ReleaseMutex(mutex);
        let _ = CloseHandle(mutex);
        CoUninitialize();
    }
}
