pub mod audio;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use crate::Event;

type Handle = *mut c_void;
type HInstance = Handle;
type HIcon = Handle;
type HCursor = Handle;
type HBrush = Handle;
type WParam = usize;
type LParam = isize;
type LResult = isize;

type WindowProc = Option<unsafe extern "system" fn(Handle, u32, WParam, LParam) -> LResult>;

#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: WindowProc,
    cls_extra: i32,
    wnd_extra: i32,
    instance: HInstance,
    icon: HIcon,
    cursor: HCursor,
    background: HBrush,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[repr(C)]
struct Msg {
    hwnd: Handle,
    message: u32,
    wparam: WParam,
    lparam: LParam,
    time: u32,
    point: Point,
    private: u32,
}

#[repr(C)]
struct TrackMouseEvent {
    size: u32,
    flags: u32,
    handle: Handle,
    hover_time: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HInstance;
    fn GetLastError() -> u32;
    fn SetLastError(error: u32);
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Handle,
        menu: Handle,
        instance: HInstance,
        param: *const c_void,
    ) -> Handle;
    fn DefWindowProcW(hwnd: Handle, message: u32, wparam: WParam, lparam: LParam) -> LResult;
    fn DestroyWindow(hwnd: Handle) -> i32;
    fn ShowWindow(hwnd: Handle, command: i32) -> i32;
    fn SetWindowTextW(hwnd: Handle, text: *const u16) -> i32;
    fn SetForegroundWindow(hwnd: Handle) -> i32;
    fn PeekMessageW(message: *mut Msg, hwnd: Handle, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Msg) -> i32;
    fn DispatchMessageW(message: *const Msg) -> LResult;
    fn TrackMouseEvent(event: *mut TrackMouseEvent) -> i32;
    fn GetWindowLongPtrW(hwnd: Handle, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Handle, index: i32, value: isize) -> isize;
    fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn MonitorFromWindow(hwnd: Handle, flags: u32) -> Handle;
    fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
    fn SetWindowPos(
        hwnd: Handle,
        insert_after: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
}

const CLASS_NAME: &[u16] = &[86, 97, 103, 117, 115, 87, 105, 110, 100, 111, 119, 0];
const CS_HREDRAW: u32 = 0x0002;
const CS_VREDRAW: u32 = 0x0001;
const CW_USEDEFAULT: i32 = i32::MIN;
const WS_OVERLAPPEDWINDOW: u32 = 0x00cf0000;
const WS_THICKFRAME: u32 = 0x00040000;
const WS_MAXIMIZEBOX: u32 = 0x00010000;
const WS_POPUP: u32 = 0x80000000;
const WS_CONFIG_STYLE_MASK: u32 = WS_OVERLAPPEDWINDOW | WS_POPUP;
const GWL_STYLE: i32 = -16;
const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOOWNERZORDER: u32 = 0x0200;
const SWP_FRAMECHANGED: u32 = 0x0020;
const SW_SHOW: i32 = 5;
const SW_HIDE: i32 = 0;
const SW_MINIMIZE: i32 = 6;
const SW_MAXIMIZE: i32 = 3;
const SW_RESTORE: i32 = 9;
const PM_REMOVE: u32 = 1;
const WM_CLOSE: u32 = 0x0010;
const WM_SIZE: u32 = 0x0005;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MOUSEWHEEL: u32 = 0x020a;
const WM_XBUTTONDOWN: u32 = 0x020b;
const WM_XBUTTONUP: u32 = 0x020c;
const WM_MOUSEHWHEEL: u32 = 0x020e;
const WM_MOUSELEAVE: u32 = 0x02a3;
const WM_MOVE: u32 = 0x0003;
const WM_SETFOCUS: u32 = 0x0007;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_CHAR: u32 = 0x0102;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_SYSCHAR: u32 = 0x0106;
const TME_LEAVE: u32 = 0x00000002;
const WHEEL_DELTA: f64 = 120.0;
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;

thread_local! {
    static EVENTS: RefCell<HashMap<isize, VecDeque<Event>>> = RefCell::new(HashMap::new());
    static POINTER_INSIDE: RefCell<HashSet<isize>> = RefCell::new(HashSet::new());
    static PENDING_HIGH_SURROGATE: RefCell<HashMap<isize, u16>> = RefCell::new(HashMap::new());
}

pub struct Window {
    handle: Handle,
    instance: HInstance,
    fullscreen: bool,
    windowed_rect: Option<Rect>,
}

impl Window {
    pub fn create_window(title: &str, width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 {
            return Err("window dimensions must be positive".into());
        }
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        if instance.is_null() {
            return Err(format!("GetModuleHandleW failed: {}", unsafe {
                GetLastError()
            }));
        }
        let class = WndClassW {
            style: CS_HREDRAW | CS_VREDRAW,
            wnd_proc: Some(window_proc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: ptr::null_mut(),
            cursor: ptr::null_mut(),
            background: ptr::null_mut(),
            menu_name: ptr::null(),
            class_name: CLASS_NAME.as_ptr(),
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_CLASS_ALREADY_EXISTS {
                return Err(format!("RegisterClassW failed: {error}"));
            }
        }
        let title: Vec<u16> = OsStr::new(title)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe {
            CreateWindowExW(
                0,
                CLASS_NAME.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            )
        };
        if handle.is_null() {
            return Err(format!("CreateWindowExW failed: {}", unsafe {
                GetLastError()
            }));
        }
        EVENTS.with(|events| {
            events.borrow_mut().insert(handle as isize, VecDeque::new());
        });
        unsafe { ShowWindow(handle, SW_SHOW) };
        Ok(Self {
            handle,
            instance,
            fullscreen: false,
            windowed_rect: None,
        })
    }

    pub fn vulkan_handles(&self) -> (Handle, Handle) {
        (self.instance, self.handle)
    }

    pub fn apply_cfg(
        &mut self,
        fullscreen: bool,
        resizable: bool,
        borderless: bool,
    ) -> Result<(), String> {
        let style = window_style(self.handle, fullscreen, resizable, borderless)?;

        if fullscreen && !self.fullscreen {
            let mut windowed_rect = Rect::default();
            if unsafe { GetWindowRect(self.handle, &mut windowed_rect) } == 0 {
                return Err(last_error("GetWindowRect"));
            }
            let monitor = unsafe { MonitorFromWindow(self.handle, MONITOR_DEFAULTTONEAREST) };
            if monitor.is_null() {
                return Err(last_error("MonitorFromWindow"));
            }
            let mut info = MonitorInfo {
                size: std::mem::size_of::<MonitorInfo>() as u32,
                monitor: Rect::default(),
                work: Rect::default(),
                flags: 0,
            };
            if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
                return Err(last_error("GetMonitorInfoW"));
            }
            set_window_style(self.handle, style)?;
            set_window_position(
                self.handle,
                info.monitor.left,
                info.monitor.top,
                info.monitor.right - info.monitor.left,
                info.monitor.bottom - info.monitor.top,
                SWP_FRAMECHANGED | SWP_NOOWNERZORDER,
            )?;
            self.windowed_rect = Some(windowed_rect);
        } else if !fullscreen && self.fullscreen {
            let rect = self
                .windowed_rect
                .ok_or_else(|| "missing saved windowed position".to_owned())?;
            set_window_style(self.handle, style)?;
            set_window_position(
                self.handle,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_FRAMECHANGED | SWP_NOOWNERZORDER,
            )?;
            self.windowed_rect = None;
        } else {
            set_window_style(self.handle, style)?;
            set_window_position(
                self.handle,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOOWNERZORDER | SWP_NOMOVE | SWP_NOSIZE,
            )?;
        }
        self.fullscreen = fullscreen;
        Ok(())
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), String> {
        let title: Vec<u16> = OsStr::new(title)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        if unsafe { SetWindowTextW(self.handle, title.as_ptr()) } == 0 {
            return Err(last_error("SetWindowTextW"));
        }
        Ok(())
    }

    pub fn set_size(&mut self, width: u32, height: u32) -> Result<(), String> {
        let width =
            i32::try_from(width).map_err(|_| "window width exceeds the Windows API limit")?;
        let height =
            i32::try_from(height).map_err(|_| "window height exceeds the Windows API limit")?;
        if width <= 0 || height <= 0 {
            return Err("window dimensions must be positive".into());
        }
        let mut rect = Rect::default();
        if unsafe { GetWindowRect(self.handle, &mut rect) } == 0 {
            return Err(last_error("GetWindowRect"));
        }
        set_window_position(
            self.handle,
            rect.left,
            rect.top,
            width,
            height,
            SWP_NOOWNERZORDER,
        )
    }

    pub fn set_position(&mut self, x: i32, y: i32) -> Result<(), String> {
        set_window_position(self.handle, x, y, 0, 0, SWP_NOOWNERZORDER | SWP_NOSIZE)
    }

    pub fn set_visible(&mut self, visible: bool) {
        unsafe { ShowWindow(self.handle, if visible { SW_SHOW } else { SW_HIDE }) };
    }

    pub fn set_window_state(&mut self, minimized: bool, maximized: bool) -> Result<(), String> {
        if minimized && maximized {
            return Err("a window cannot be minimized and maximized simultaneously".into());
        }
        let command = if minimized {
            SW_MINIMIZE
        } else if maximized {
            SW_MAXIMIZE
        } else {
            SW_RESTORE
        };
        unsafe { ShowWindow(self.handle, command) };
        Ok(())
    }

    pub fn request_focus(&mut self) -> Result<(), String> {
        if unsafe { SetForegroundWindow(self.handle) } == 0 {
            return Err("Windows declined the foreground activation request".into());
        }
        Ok(())
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        let mut message: Msg = unsafe { std::mem::zeroed() };
        unsafe {
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        EVENTS.with(|events| {
            events
                .borrow_mut()
                .entry(self.handle as isize)
                .or_default()
                .drain(..)
                .collect()
        })
    }

    pub fn handle(&self) -> *mut c_void {
        self.handle
    }
}

fn window_style(
    handle: Handle,
    fullscreen: bool,
    resizable: bool,
    borderless: bool,
) -> Result<u32, String> {
    unsafe { SetLastError(0) };
    let current = unsafe { GetWindowLongPtrW(handle, GWL_STYLE) };
    let error = unsafe { GetLastError() };
    if current == 0 && error != 0 {
        return Err(format!("GetWindowLongPtrW failed: {error}"));
    }
    let mut style = current as u32 & !WS_CONFIG_STYLE_MASK;
    if fullscreen || borderless {
        style |= WS_POPUP;
    } else {
        style |= WS_OVERLAPPEDWINDOW;
        if !resizable {
            style &= !(WS_THICKFRAME | WS_MAXIMIZEBOX);
        }
    }
    Ok(style)
}

fn set_window_style(handle: Handle, style: u32) -> Result<(), String> {
    unsafe { SetLastError(0) };
    let previous = unsafe { SetWindowLongPtrW(handle, GWL_STYLE, style as isize) };
    let error = unsafe { GetLastError() };
    if previous == 0 && error != 0 {
        return Err(format!("SetWindowLongPtrW failed: {error}"));
    }
    Ok(())
}

fn set_window_position(
    handle: Handle,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    flags: u32,
) -> Result<(), String> {
    if unsafe { SetWindowPos(handle, ptr::null_mut(), x, y, width, height, flags) } == 0 {
        return Err(last_error("SetWindowPos"));
    }
    Ok(())
}

fn last_error(operation: &str) -> String {
    format!("{operation} failed: {}", unsafe { GetLastError() })
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.handle) };
        EVENTS.with(|events| {
            events.borrow_mut().remove(&(self.handle as isize));
        });
        POINTER_INSIDE.with(|windows| {
            windows.borrow_mut().remove(&(self.handle as isize));
        });
        PENDING_HIGH_SURROGATE.with(|units| {
            units.borrow_mut().remove(&(self.handle as isize));
        });
    }
}

unsafe extern "system" fn window_proc(
    handle: Handle,
    message: u32,
    wparam: WParam,
    lparam: LParam,
) -> LResult {
    let event = match message {
        WM_CLOSE => Some(Event::CloseRequested),
        WM_SIZE => Some(Event::Resized {
            width: lparam as u32 & 0xffff,
            height: (lparam as u32 >> 16) & 0xffff,
        }),
        WM_KEYDOWN | WM_SYSKEYDOWN => Some(Event::Key {
            code: wparam as u32,
            pressed: true,
        }),
        WM_KEYUP | WM_SYSKEYUP => Some(Event::Key {
            code: wparam as u32,
            pressed: false,
        }),
        WM_MOVE => Some(Event::Moved {
            x: lparam as u16 as i16 as i32,
            y: (lparam as u32 >> 16) as u16 as i16 as i32,
        }),
        WM_SETFOCUS => Some(Event::Focused { focused: true }),
        WM_KILLFOCUS => Some(Event::Focused { focused: false }),
        WM_MOUSEMOVE => Some(Event::PointerMoved {
            x: lparam as u16 as i16 as i32,
            y: (lparam as u32 >> 16) as u16 as i16 as i32,
        }),
        WM_LBUTTONDOWN => Some(Event::PointerButton {
            button: 1,
            pressed: true,
        }),
        WM_LBUTTONUP => Some(Event::PointerButton {
            button: 1,
            pressed: false,
        }),
        WM_RBUTTONDOWN => Some(Event::PointerButton {
            button: 2,
            pressed: true,
        }),
        WM_RBUTTONUP => Some(Event::PointerButton {
            button: 2,
            pressed: false,
        }),
        WM_MBUTTONDOWN => Some(Event::PointerButton {
            button: 3,
            pressed: true,
        }),
        WM_MBUTTONUP => Some(Event::PointerButton {
            button: 3,
            pressed: false,
        }),
        WM_XBUTTONDOWN | WM_XBUTTONUP => Some(Event::PointerButton {
            button: ((wparam >> 16) as u16 as u32) + 3,
            pressed: message == WM_XBUTTONDOWN,
        }),
        WM_MOUSEWHEEL => Some(Event::Scroll {
            delta_x: 0.0,
            delta_y: ((wparam >> 16) as u16 as i16) as f64 / WHEEL_DELTA,
        }),
        WM_MOUSEHWHEEL => Some(Event::Scroll {
            delta_x: ((wparam >> 16) as u16 as i16) as f64 / WHEEL_DELTA,
            delta_y: 0.0,
        }),
        _ => None,
    };
    if let Some(event) = event {
        queue_event(handle, event);
    }
    if message == WM_MOUSEMOVE {
        let first_move =
            POINTER_INSIDE.with(|windows| windows.borrow_mut().insert(handle as isize));
        if first_move {
            let mut tracking = TrackMouseEvent {
                size: std::mem::size_of::<TrackMouseEvent>() as u32,
                flags: TME_LEAVE,
                handle,
                hover_time: 0,
            };
            unsafe { TrackMouseEvent(&mut tracking) };
            queue_event(handle, Event::PointerEntered);
        }
    } else if message == WM_MOUSELEAVE {
        let was_inside =
            POINTER_INSIDE.with(|windows| windows.borrow_mut().remove(&(handle as isize)));
        if was_inside {
            queue_event(handle, Event::PointerLeft);
        }
    }
    if message == WM_CHAR || message == WM_SYSCHAR {
        let unit = wparam as u16;
        let decoded = PENDING_HIGH_SURROGATE.with(|pending| {
            let mut pending = pending.borrow_mut();
            if (0xd800..=0xdbff).contains(&unit) {
                pending.insert(handle as isize, unit);
                None
            } else if (0xdc00..=0xdfff).contains(&unit) {
                pending
                    .remove(&(handle as isize))
                    .and_then(|high| char::decode_utf16([high, unit]).next().and_then(Result::ok))
            } else {
                pending.remove(&(handle as isize));
                char::from_u32(unit as u32)
            }
        });
        if let Some(character) = decoded {
            queue_event(
                handle,
                Event::TextInput {
                    codepoint: character as u32,
                },
            );
        }
    }
    let result = unsafe { DefWindowProcW(handle, message, wparam, lparam) };
    if message == WM_XBUTTONDOWN || message == WM_XBUTTONUP {
        1
    } else {
        result
    }
}

fn queue_event(handle: Handle, event: Event) {
    EVENTS.with(|events| {
        if let Some(queue) = events.borrow_mut().get_mut(&(handle as isize)) {
            queue.push_back(event);
        }
    });
}
