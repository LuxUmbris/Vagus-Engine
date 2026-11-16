//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::ptr;

use crate::Event;

type XWindow = c_ulong;
#[repr(C)]
struct Display {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XAnyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: XWindow,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XKeyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: XWindow,
    root: XWindow,
    subwindow: XWindow,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XButtonEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: XWindow,
    root: XWindow,
    subwindow: XWindow,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    button: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XMotionEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: XWindow,
    root: XWindow,
    subwindow: XWindow,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    is_hint: c_char,
    same_screen: c_int,
}

#[repr(C)]
struct XComposeStatus {
    compose_ptr: *mut c_char,
    chars_matched: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    event: XWindow,
    window: XWindow,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    above: XWindow,
    override_redirect: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
union ClientData {
    b: [c_char; 20],
    s: [i16; 10],
    l: [c_long; 5],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: XWindow,
    message_type: c_ulong,
    format: c_int,
    data: ClientData,
}

#[repr(C)]
struct XSizeHints {
    flags: c_long,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    min_width: c_int,
    min_height: c_int,
    max_width: c_int,
    max_height: c_int,
    width_inc: c_int,
    height_inc: c_int,
    min_aspect_x: c_int,
    min_aspect_y: c_int,
    max_aspect_x: c_int,
    max_aspect_y: c_int,
    base_width: c_int,
    base_height: c_int,
    win_gravity: c_int,
}

#[repr(C)]
struct MotifWmHints {
    flags: c_ulong,
    functions: c_ulong,
    decorations: c_ulong,
    input_mode: c_long,
    status: c_ulong,
}

#[repr(C)]
union XEvent {
    type_: c_int,
    any: XAnyEvent,
    key: XKeyEvent,
    button: XButtonEvent,
    motion: XMotionEvent,
    configure: XConfigureEvent,
    client: XClientMessageEvent,
    crossing: XAnyEvent,
    pad: [c_long; 24],
}

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XDefaultScreen(display: *mut Display) -> c_int;
    fn XDefaultRootWindow(display: *mut Display) -> XWindow;
    fn XCreateSimpleWindow(
        display: *mut Display,
        parent: XWindow,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        border_width: c_uint,
        border: c_ulong,
        background: c_ulong,
    ) -> XWindow;
    fn XStoreName(display: *mut Display, window: XWindow, name: *const c_char) -> c_int;
    fn XResizeWindow(
        display: *mut Display,
        window: XWindow,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XMoveWindow(display: *mut Display, window: XWindow, x: c_int, y: c_int) -> c_int;
    fn XLookupString(
        event: *mut XKeyEvent,
        buffer: *mut c_char,
        bytes_buffer: c_int,
        keysym_return: *mut c_ulong,
        status_in_out: *mut XComposeStatus,
    ) -> c_int;
    fn XSelectInput(display: *mut Display, window: XWindow, mask: c_long) -> c_int;
    fn XInternAtom(display: *mut Display, name: *const c_char, only_if_exists: c_int) -> c_ulong;
    fn XSetWMProtocols(
        display: *mut Display,
        window: XWindow,
        protocols: *mut c_ulong,
        count: c_int,
    ) -> c_int;
    fn XSetWMNormalHints(display: *mut Display, window: XWindow, hints: *mut XSizeHints);
    fn XChangeProperty(
        display: *mut Display,
        window: XWindow,
        property: c_ulong,
        property_type: c_ulong,
        format: c_int,
        mode: c_int,
        data: *const u8,
        elements: c_int,
    ) -> c_int;
    fn XSendEvent(
        display: *mut Display,
        window: XWindow,
        propagate: c_int,
        event_mask: c_long,
        event: *mut XEvent,
    ) -> c_int;
    fn XMapWindow(display: *mut Display, window: XWindow) -> c_int;
    fn XUnmapWindow(display: *mut Display, window: XWindow) -> c_int;
    fn XMapRaised(display: *mut Display, window: XWindow) -> c_int;
    fn XIconifyWindow(display: *mut Display, window: XWindow, screen: c_int) -> c_int;
    fn XSetInputFocus(
        display: *mut Display,
        focus: XWindow,
        revert_to: c_int,
        time: c_ulong,
    ) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XPending(display: *mut Display) -> c_int;
    fn XNextEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    fn XDestroyWindow(display: *mut Display, window: XWindow) -> c_int;
    fn XCloseDisplay(display: *mut Display) -> c_int;
}

const KEY_PRESS: c_int = 2;
const KEY_RELEASE: c_int = 3;
const BUTTON_PRESS: c_int = 4;
const BUTTON_RELEASE: c_int = 5;
const MOTION_NOTIFY: c_int = 6;
const ENTER_NOTIFY: c_int = 7;
const LEAVE_NOTIFY: c_int = 8;
const FOCUS_IN: c_int = 9;
const FOCUS_OUT: c_int = 10;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;
const EVENT_MASK: c_long = (1 << 17)
    | (1 << 15)
    | (1 << 0)
    | (1 << 1)
    | (1 << 2)
    | (1 << 3)
    | (1 << 4)
    | (1 << 5)
    | (1 << 6)
    | (1 << 21);
const SUBSTRUCTURE_NOTIFY_MASK: c_long = 1 << 19;
const SUBSTRUCTURE_REDIRECT_MASK: c_long = 1 << 20;
const PROP_MODE_REPLACE: c_int = 0;
const P_MIN_SIZE: c_long = 1 << 4;
const P_MAX_SIZE: c_long = 1 << 5;

pub struct Window {
    display: *mut Display,
    id: XWindow,
    protocols_atom: c_ulong,
    delete_atom: c_ulong,
    root: XWindow,
    fullscreen_atom: c_ulong,
    net_wm_state_atom: c_ulong,
    maximize_h_atom: c_ulong,
    maximize_v_atom: c_ulong,
    motif_hints_atom: c_ulong,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    fullscreen: bool,
}

impl Window {
    pub fn vulkan_handles(&self) -> (*mut c_void, u64) {
        (self.display.cast(), self.id as u64)
    }

    pub fn create_window(title: &str, width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("window dimensions must be non-zero".into());
        }
        let title = CString::new(title).map_err(|_| "window title contains NUL")?;
        let display = unsafe { XOpenDisplay(ptr::null()) };
        if display.is_null() {
            return Err("XOpenDisplay failed (check DISPLAY and X server)".into());
        }
        let _screen = unsafe { XDefaultScreen(display) };
        let root = unsafe { XDefaultRootWindow(display) };
        let id = unsafe { XCreateSimpleWindow(display, root, 0, 0, width, height, 0, 0, 0) };
        if id == 0 {
            unsafe { XCloseDisplay(display) };
            return Err("XCreateSimpleWindow failed".into());
        }
        unsafe {
            XStoreName(display, id, title.as_ptr());
            XSelectInput(display, id, EVENT_MASK);
        }
        let protocols = CString::new("WM_PROTOCOLS").unwrap();
        let delete = CString::new("WM_DELETE_WINDOW").unwrap();
        let fullscreen = CString::new("_NET_WM_STATE_FULLSCREEN").unwrap();
        let net_wm_state = CString::new("_NET_WM_STATE").unwrap();
        let maximize_h = CString::new("_NET_WM_STATE_MAXIMIZED_HORZ").unwrap();
        let maximize_v = CString::new("_NET_WM_STATE_MAXIMIZED_VERT").unwrap();
        let motif_hints = CString::new("_MOTIF_WM_HINTS").unwrap();
        let protocols_atom = unsafe { XInternAtom(display, protocols.as_ptr(), 0) };
        let delete_atom = unsafe { XInternAtom(display, delete.as_ptr(), 0) };
        let fullscreen_atom = unsafe { XInternAtom(display, fullscreen.as_ptr(), 0) };
        let net_wm_state_atom = unsafe { XInternAtom(display, net_wm_state.as_ptr(), 0) };
        let maximize_h_atom = unsafe { XInternAtom(display, maximize_h.as_ptr(), 0) };
        let maximize_v_atom = unsafe { XInternAtom(display, maximize_v.as_ptr(), 0) };
        let motif_hints_atom = unsafe { XInternAtom(display, motif_hints.as_ptr(), 0) };
        unsafe {
            XSetWMProtocols(display, id, &mut (delete_atom as c_ulong), 1);
            XMapWindow(display, id);
            XFlush(display);
        }
        Ok(Self {
            display,
            id,
            protocols_atom,
            delete_atom,
            root,
            fullscreen_atom,
            net_wm_state_atom,
            maximize_h_atom,
            maximize_v_atom,
            motif_hints_atom,
            width,
            height,
            x: 0,
            y: 0,
            fullscreen: false,
        })
    }

    pub fn apply_cfg(
        &mut self,
        fullscreen: bool,
        resizable: bool,
        borderless: bool,
    ) -> Result<(), String> {
        let mut hints = XSizeHints {
            flags: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            min_width: 0,
            min_height: 0,
            max_width: 0,
            max_height: 0,
            width_inc: 0,
            height_inc: 0,
            min_aspect_x: 0,
            min_aspect_y: 0,
            max_aspect_x: 0,
            max_aspect_y: 0,
            base_width: 0,
            base_height: 0,
            win_gravity: 0,
        };
        if !resizable {
            hints.flags = P_MIN_SIZE | P_MAX_SIZE;
            hints.min_width = self.width as c_int;
            hints.min_height = self.height as c_int;
            hints.max_width = self.width as c_int;
            hints.max_height = self.height as c_int;
        }
        unsafe { XSetWMNormalHints(self.display, self.id, &mut hints) };

        let mut motif_hints = MotifWmHints {
            flags: 1 << 1,
            functions: 0,
            decorations: if borderless { 0 } else { 1 },
            input_mode: 0,
            status: 0,
        };
        if unsafe {
            XChangeProperty(
                self.display,
                self.id,
                self.motif_hints_atom,
                self.motif_hints_atom,
                32,
                PROP_MODE_REPLACE,
                (&mut motif_hints as *mut MotifWmHints).cast(),
                5,
            )
        } == 0
        {
            return Err("setting X11 window decorations failed".into());
        }

        if fullscreen != self.fullscreen {
            let mut event = XEvent {
                client: XClientMessageEvent {
                    type_: CLIENT_MESSAGE,
                    serial: 0,
                    send_event: 1,
                    display: self.display,
                    window: self.id,
                    message_type: self.net_wm_state_atom,
                    format: 32,
                    data: ClientData {
                        l: [
                            if fullscreen { 1 } else { 0 },
                            self.fullscreen_atom as c_long,
                            0,
                            1,
                            0,
                        ],
                    },
                },
            };
            if unsafe {
                XSendEvent(
                    self.display,
                    self.root,
                    0,
                    SUBSTRUCTURE_REDIRECT_MASK | SUBSTRUCTURE_NOTIFY_MASK,
                    &mut event,
                )
            } == 0
            {
                return Err("X11 window manager did not accept fullscreen request".into());
            }
            self.fullscreen = fullscreen;
        }
        unsafe { XFlush(self.display) };
        Ok(())
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), String> {
        let title = CString::new(title).map_err(|_| "window title contains NUL")?;
        if unsafe { XStoreName(self.display, self.id, title.as_ptr()) } == 0 {
            return Err("XStoreName failed".into());
        }
        unsafe { XFlush(self.display) };
        Ok(())
    }

    pub fn set_size(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err("window dimensions must be positive".into());
        }
        unsafe { XResizeWindow(self.display, self.id, width, height) };
        unsafe { XFlush(self.display) };
        Ok(())
    }

    pub fn set_position(&mut self, x: i32, y: i32) -> Result<(), String> {
        unsafe { XMoveWindow(self.display, self.id, x, y) };
        unsafe { XFlush(self.display) };
        Ok(())
    }

    pub fn set_visible(&mut self, visible: bool) {
        unsafe {
            if visible {
                XMapWindow(self.display, self.id);
            } else {
                XUnmapWindow(self.display, self.id);
            }
            XFlush(self.display);
        }
    }

    pub fn set_window_state(&mut self, minimized: bool, maximized: bool) -> Result<(), String> {
        if minimized && maximized {
            return Err("a window cannot be minimized and maximized simultaneously".into());
        }
        if minimized {
            let screen = unsafe { XDefaultScreen(self.display) };
            if unsafe { XIconifyWindow(self.display, self.id, screen) } == 0 {
                return Err("X11 window manager rejected the minimize request".into());
            }
        } else {
            let mut event = XEvent {
                client: XClientMessageEvent {
                    type_: CLIENT_MESSAGE,
                    serial: 0,
                    send_event: 1,
                    display: self.display,
                    window: self.id,
                    message_type: self.net_wm_state_atom,
                    format: 32,
                    data: ClientData {
                        l: [
                            if maximized { 1 } else { 0 },
                            self.maximize_h_atom as c_long,
                            self.maximize_v_atom as c_long,
                            1,
                            0,
                        ],
                    },
                },
            };
            if unsafe {
                XSendEvent(
                    self.display,
                    self.root,
                    0,
                    SUBSTRUCTURE_REDIRECT_MASK | SUBSTRUCTURE_NOTIFY_MASK,
                    &mut event,
                )
            } == 0
            {
                return Err("X11 window manager rejected the maximize/restore request".into());
            }
            if !maximized {
                unsafe { XMapRaised(self.display, self.id) };
            }
        }
        unsafe { XFlush(self.display) };
        Ok(())
    }

    pub fn request_focus(&mut self) {
        unsafe {
            XSetInputFocus(self.display, self.id, 2, 0);
            XFlush(self.display);
        }
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        while unsafe { XPending(self.display) } > 0 {
            let mut raw = XEvent { pad: [0; 24] };
            unsafe { XNextEvent(self.display, &mut raw) };
            unsafe {
                match raw.type_ {
                    CLIENT_MESSAGE => {
                        let event = raw.client;
                        if event.window == self.id
                            && event.message_type == self.protocols_atom
                            && event.format == 32
                            && event.data.l[0] as c_ulong == self.delete_atom
                        {
                            events.push(Event::CloseRequested);
                        }
                    }
                    CONFIGURE_NOTIFY => {
                        let event = raw.configure;
                        if event.window == self.id {
                            if self.x != event.x || self.y != event.y {
                                self.x = event.x;
                                self.y = event.y;
                                events.push(Event::Moved {
                                    x: self.x,
                                    y: self.y,
                                });
                            }
                            let (width, height) =
                                (event.width.max(0) as u32, event.height.max(0) as u32);
                            if self.width != width || self.height != height {
                                self.width = width;
                                self.height = height;
                                events.push(Event::Resized { width, height });
                            }
                        }
                    }
                    KEY_PRESS | KEY_RELEASE => {
                        let event = raw.key;
                        if event.window == self.id {
                            events.push(Event::Key {
                                code: event.keycode,
                                pressed: event.type_ == KEY_PRESS,
                            });
                            if event.type_ == KEY_PRESS {
                                let mut key = event;
                                let mut bytes = [0i8; 64];
                                let mut keysym = 0;
                                let mut compose = XComposeStatus {
                                    compose_ptr: ptr::null_mut(),
                                    chars_matched: 0,
                                };
                                let count = XLookupString(
                                    &mut key,
                                    bytes.as_mut_ptr(),
                                    bytes.len() as c_int,
                                    &mut keysym,
                                    &mut compose,
                                );
                                if count > 0 {
                                    let bytes = std::slice::from_raw_parts(
                                        bytes.as_ptr().cast::<u8>(),
                                        count as usize,
                                    );
                                    if let Ok(text) = std::str::from_utf8(bytes) {
                                        events.extend(text.chars().map(|character| {
                                            Event::TextInput {
                                                codepoint: character as u32,
                                            }
                                        }));
                                    }
                                }
                            }
                        }
                    }
                    BUTTON_PRESS | BUTTON_RELEASE => {
                        let event = raw.button;
                        if event.window == self.id {
                            if event.type_ == BUTTON_PRESS {
                                let scroll = match event.button {
                                    4 => Some((0.0, 1.0)),
                                    5 => Some((0.0, -1.0)),
                                    6 => Some((-1.0, 0.0)),
                                    7 => Some((1.0, 0.0)),
                                    _ => None,
                                };
                                if let Some((delta_x, delta_y)) = scroll {
                                    events.push(Event::Scroll { delta_x, delta_y });
                                } else {
                                    events.push(Event::PointerButton {
                                        button: event.button,
                                        pressed: true,
                                    });
                                }
                            } else if !(4..=7).contains(&event.button) {
                                events.push(Event::PointerButton {
                                    button: event.button,
                                    pressed: false,
                                });
                            }
                        }
                    }
                    MOTION_NOTIFY => {
                        let event = raw.motion;
                        if event.window == self.id {
                            events.push(Event::PointerMoved {
                                x: event.x,
                                y: event.y,
                            });
                        }
                    }
                    ENTER_NOTIFY | LEAVE_NOTIFY => {
                        let event = raw.crossing;
                        if event.window == self.id {
                            events.push(if event.type_ == ENTER_NOTIFY {
                                Event::PointerEntered
                            } else {
                                Event::PointerLeft
                            });
                        }
                    }
                    FOCUS_IN | FOCUS_OUT => {
                        let event = raw.crossing;
                        if event.window == self.id {
                            events.push(Event::Focused {
                                focused: event.type_ == FOCUS_IN,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        unsafe { XFlush(self.display) };
        events
    }

    #[allow(dead_code)]
    pub fn id(&self) -> c_ulong {
        self.id
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            XDestroyWindow(self.display, self.id);
            XCloseDisplay(self.display);
        }
    }
}
