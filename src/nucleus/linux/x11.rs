use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_ulong};
use std::ptr;

use crate::nucleus::Event;

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
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;
const EVENT_MASK: c_long =
    (1 << 17) | (1 << 15) | (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 6);
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
    motif_hints_atom: c_ulong,
    width: u32,
    height: u32,
    fullscreen: bool,
}

impl Window {
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
        let motif_hints = CString::new("_MOTIF_WM_HINTS").unwrap();
        let protocols_atom = unsafe { XInternAtom(display, protocols.as_ptr(), 0) };
        let delete_atom = unsafe { XInternAtom(display, delete.as_ptr(), 0) };
        let fullscreen_atom = unsafe { XInternAtom(display, fullscreen.as_ptr(), 0) };
        let net_wm_state_atom = unsafe { XInternAtom(display, net_wm_state.as_ptr(), 0) };
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
            motif_hints_atom,
            width,
            height,
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
                            self.width = event.width.max(0) as u32;
                            self.height = event.height.max(0) as u32;
                            events.push(Event::Resized {
                                width: self.width,
                                height: self.height,
                            });
                        }
                    }
                    KEY_PRESS | KEY_RELEASE => {
                        let event = raw.key;
                        if event.window == self.id {
                            events.push(Event::Key {
                                code: event.keycode,
                                pressed: event.type_ == KEY_PRESS,
                            });
                        }
                    }
                    BUTTON_PRESS | BUTTON_RELEASE => {
                        let event = raw.button;
                        if event.window == self.id {
                            events.push(Event::PointerButton {
                                button: event.button,
                                pressed: event.type_ == BUTTON_PRESS,
                            });
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
                    _ => {}
                }
            }
        }
        unsafe { XFlush(self.display) };
        events
    }

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
