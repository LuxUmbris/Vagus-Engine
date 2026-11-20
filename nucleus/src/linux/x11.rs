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

use std::cell::UnsafeCell;
use std::collections::HashMap;
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

type XimCallbackFn = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut c_char);

#[repr(C)]
struct XimCallback {
    client_data: *mut c_char,
    callback: Option<XimCallbackFn>,
}

#[repr(C)]
struct XimStyles {
    count_styles: u16,
    supported_styles: *mut c_ulong,
}

#[repr(C)]
union XimTextString {
    multi_byte: *mut c_char,
    wide_char: *mut libc::wchar_t,
}

#[repr(C)]
struct XimText {
    length: u16,
    feedback: *mut c_ulong,
    encoding_is_wchar: c_int,
    string: XimTextString,
}

#[repr(C)]
struct XimPreeditDrawCallbackStruct {
    caret: c_int,
    chg_first: c_int,
    chg_length: c_int,
    text: *mut XimText,
}

#[repr(C)]
struct XimPreeditCaretCallbackStruct {
    position: c_int,
    direction: c_int,
    style: c_int,
}

#[derive(Default)]
struct X11PreeditState {
    composition: Vec<char>,
    pending: Vec<Event>,
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
    cookie: XGenericEventCookie,
    pad: [c_long; 24],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XGenericEventCookie {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    extension: c_int,
    evtype: c_int,
    cookie: c_uint,
    data: *mut c_void,
}

#[repr(C)]
struct XIEventMask {
    deviceid: c_int,
    mask_len: c_int,
    mask: *mut u8,
}

#[repr(C)]
struct XIAnyClassInfo {
    type_: c_int,
    sourceid: c_int,
}

#[repr(C)]
struct XIValuatorClassInfo {
    type_: c_int,
    sourceid: c_int,
    number: c_int,
    label: c_ulong,
    min: f64,
    max: f64,
    value: f64,
    resolution: c_int,
    mode: c_int,
}

#[repr(C)]
struct XIScrollClassInfo {
    type_: c_int,
    sourceid: c_int,
    number: c_int,
    scroll_type: c_int,
    increment: f64,
    flags: c_int,
}

#[repr(C)]
struct XIDeviceInfo {
    deviceid: c_int,
    name: *mut c_char,
    use_: c_int,
    attachment: c_int,
    enabled: c_int,
    num_classes: c_int,
    classes: *mut *mut XIAnyClassInfo,
}

#[repr(C)]
struct XIButtonState {
    mask_len: c_int,
    mask: *mut u8,
}

#[repr(C)]
struct XIValuatorState {
    mask_len: c_int,
    mask: *mut u8,
    values: *mut f64,
}

#[repr(C)]
struct XIModifierState {
    base: c_int,
    latched: c_int,
    locked: c_int,
    effective: c_int,
}

#[repr(C)]
struct XIDeviceEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    extension: c_int,
    evtype: c_int,
    time: c_ulong,
    deviceid: c_int,
    sourceid: c_int,
    detail: c_int,
    root: XWindow,
    event: XWindow,
    child: XWindow,
    root_x: f64,
    root_y: f64,
    event_x: f64,
    event_y: f64,
    flags: c_int,
    buttons: XIButtonState,
    valuators: XIValuatorState,
    mods: XIModifierState,
    group: XIModifierState,
}

struct X11ScrollAxis {
    increment: f64,
    scroll_type: c_int,
    position: f64,
    position_valid: bool,
}

struct XInput2State {
    extension_opcode: c_int,
    devices: HashMap<c_int, Vec<(c_int, X11ScrollAxis)>>,
}

impl XInput2State {
    fn new(display: *mut Display, window: XWindow) -> Option<Self> {
        let mut extension_opcode = 0;
        let mut first_event = 0;
        let mut first_error = 0;
        if unsafe {
            XQueryExtension(
                display,
                b"XInputExtension\0".as_ptr().cast(),
                &mut extension_opcode,
                &mut first_event,
                &mut first_error,
            )
        } == 0
        {
            return None;
        }

        let (mut major, mut minor) = (2, 1);
        if unsafe { XIQueryVersion(display, &mut major, &mut minor) } != 0
            || (major, minor) < (2, 1)
        {
            return None;
        }

        let mut pointer_mask = [0u8; 1];
        for event in [XI_BUTTON_PRESS, XI_MOTION, 7] {
            pointer_mask[event as usize / 8] |= 1 << (event as usize % 8);
        }
        let mut device_mask = [0u8; 2];
        for event in [XI_DEVICE_CHANGED, XI_HIERARCHY_CHANGED] {
            device_mask[event as usize / 8] |= 1 << (event as usize % 8);
        }
        let mut event_masks = [
            XIEventMask {
                deviceid: XI_ALL_MASTER_DEVICES,
                mask_len: pointer_mask.len() as c_int,
                mask: pointer_mask.as_mut_ptr(),
            },
            XIEventMask {
                deviceid: XI_ALL_DEVICES,
                mask_len: device_mask.len() as c_int,
                mask: device_mask.as_mut_ptr(),
            },
        ];
        if unsafe { XISelectEvents(display, window, event_masks.as_mut_ptr(), 2) } != 0 {
            return None;
        }

        let mut state = Self {
            extension_opcode,
            devices: HashMap::new(),
        };
        state.refresh_devices(display);
        Some(state)
    }

    fn refresh_devices(&mut self, display: *mut Display) {
        let mut count = 0;
        let devices = unsafe { XIQueryDevice(display, XI_ALL_DEVICES, &mut count) };
        if devices.is_null() || count <= 0 {
            if !devices.is_null() {
                unsafe { XIFreeDeviceInfo(devices) };
            }
            self.devices.clear();
            return;
        }

        let infos = unsafe { std::slice::from_raw_parts(devices, count as usize) };
        let mut updated = HashMap::new();
        for info in infos {
            if !matches!(
                info.use_,
                XI_SLAVE_POINTER | XI_SLAVE_KEYBOARD | XI_FLOATING_SLAVE
            ) || info.num_classes <= 0
                || info.classes.is_null()
            {
                continue;
            }
            let classes =
                unsafe { std::slice::from_raw_parts(info.classes, info.num_classes as usize) };
            let mut axes = Vec::new();
            for &class in classes {
                if class.is_null() || unsafe { (*class).type_ } != XI_SCROLL_CLASS {
                    continue;
                }
                let scroll = unsafe { &*class.cast::<XIScrollClassInfo>() };
                if !matches!(
                    scroll.scroll_type,
                    XI_SCROLL_TYPE_VERTICAL | XI_SCROLL_TYPE_HORIZONTAL
                ) || !scroll.increment.is_finite()
                    || scroll.increment == 0.0
                {
                    continue;
                }
                axes.push((
                    scroll.number,
                    X11ScrollAxis {
                        increment: scroll.increment,
                        scroll_type: scroll.scroll_type,
                        position: 0.0,
                        position_valid: false,
                    },
                ));
            }
            for &class in classes {
                if class.is_null() || unsafe { (*class).type_ } != XI_VALUATOR_CLASS {
                    continue;
                }
                let valuator = unsafe { &*class.cast::<XIValuatorClassInfo>() };
                if let Some((_, axis)) = axes
                    .iter_mut()
                    .find(|(number, _)| *number == valuator.number)
                {
                    if valuator.value.is_finite() {
                        axis.position = valuator.value;
                        axis.position_valid = true;
                    }
                }
            }
            if !axes.is_empty() {
                updated.insert(info.deviceid, axes);
            }
        }
        unsafe { XIFreeDeviceInfo(devices) };
        self.devices = updated;
    }

    fn process_wheel_button(&self, source_id: c_int, detail: c_int) -> Option<Event> {
        let button = u32::try_from(detail).ok()?;
        let scroll_type = match button {
            4 | 5 => XI_SCROLL_TYPE_VERTICAL,
            6 | 7 => XI_SCROLL_TYPE_HORIZONTAL,
            _ => return None,
        };
        if self.devices.get(&source_id).is_some_and(|axes| {
            axes.iter()
                .any(|(_, axis)| axis.scroll_type == scroll_type && axis.position_valid)
        }) {
            return None;
        }
        x11_button_event(button, true)
    }

    fn process_motion(&mut self, event: &XIDeviceEvent) -> Vec<Event> {
        let state = &event.valuators;
        if state.mask_len <= 0 || state.mask.is_null() || state.values.is_null() {
            return Vec::new();
        }
        let Some(axes) = self.devices.get_mut(&event.sourceid) else {
            return Vec::new();
        };

        let valuator_count = (state.mask_len as usize).saturating_mul(8);
        if valuator_count > 8192 {
            return Vec::new();
        }
        let mut value_index = 0usize;
        let mut events = Vec::new();
        for number in 0..valuator_count {
            let mask_byte = unsafe { *state.mask.add(number / 8) };
            if mask_byte & (1 << (number % 8)) == 0 {
                continue;
            }
            let current = unsafe { *state.values.add(value_index) };
            value_index += 1;
            if !current.is_finite() {
                continue;
            }
            let Some((_, axis)) = axes.iter_mut().find(|(axis, _)| *axis as usize == number) else {
                continue;
            };
            if !axis.position_valid {
                axis.position = current;
                axis.position_valid = true;
                continue;
            }
            let previous = axis.position;
            axis.position = current;
            if let Some(event) =
                xinput_scroll_delta(previous, current, axis.increment, axis.scroll_type)
            {
                events.push(event);
            }
        }
        events
    }
}

unsafe extern "C" {
    fn mbstowcs(destination: *mut libc::wchar_t, source: *const c_char, count: usize) -> usize;
}

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    #[allow(dead_code)]
    fn XkbGetDetectableAutoRepeat(display: *mut Display, supported: *mut c_int) -> c_int;
    fn XkbSetDetectableAutoRepeat(
        display: *mut Display,
        detectable: c_int,
        supported: *mut c_int,
    ) -> c_int;
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
    fn XSetLocaleModifiers(modifiers: *const c_char) -> *mut c_char;
    fn XOpenIM(
        display: *mut Display,
        database: *mut c_void,
        resource_name: *mut c_char,
        resource_class: *mut c_char,
    ) -> *mut c_void;
    fn XGetIMValues(input_method: *mut c_void, ...) -> *mut c_char;
    fn XCreateIC(input_method: *mut c_void, ...) -> *mut c_void;
    fn XVaCreateNestedList(unused: c_int, ...) -> *mut c_void;
    fn XFree(data: *mut c_void) -> c_int;
    fn XDestroyIC(input_context: *mut c_void);
    fn XCloseIM(input_method: *mut c_void) -> c_int;
    fn Xutf8LookupString(
        input_context: *mut c_void,
        event: *mut XKeyEvent,
        buffer: *mut c_char,
        bytes_buffer: c_int,
        keysym_return: *mut c_ulong,
        status_return: *mut c_int,
    ) -> c_int;
    fn XFilterEvent(event: *mut XEvent, window: XWindow) -> c_int;
    fn XSetICFocus(input_context: *mut c_void);
    fn XUnsetICFocus(input_context: *mut c_void);
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
    #[allow(dead_code)]
    fn XSync(display: *mut Display, discard: c_int) -> c_int;
    fn XPending(display: *mut Display) -> c_int;
    fn XNextEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    fn XPeekEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    fn XQueryExtension(
        display: *mut Display,
        name: *const c_char,
        opcode: *mut c_int,
        first_event: *mut c_int,
        first_error: *mut c_int,
    ) -> c_int;
    fn XGetEventData(display: *mut Display, cookie: *mut XGenericEventCookie) -> c_int;
    fn XFreeEventData(display: *mut Display, cookie: *mut XGenericEventCookie);
    fn XDestroyWindow(display: *mut Display, window: XWindow) -> c_int;
    fn XCloseDisplay(display: *mut Display) -> c_int;
}

#[link(name = "Xi")]
unsafe extern "C" {
    fn XIQueryVersion(display: *mut Display, major: *mut c_int, minor: *mut c_int) -> c_int;
    fn XISelectEvents(
        display: *mut Display,
        window: XWindow,
        masks: *mut XIEventMask,
        count: c_int,
    ) -> c_int;
    fn XIQueryDevice(
        display: *mut Display,
        deviceid: c_int,
        count: *mut c_int,
    ) -> *mut XIDeviceInfo;
    fn XIFreeDeviceInfo(info: *mut XIDeviceInfo);
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
const GENERIC_EVENT: c_int = 35;
const XI_DEVICE_CHANGED: c_int = 1;
const XI_BUTTON_PRESS: c_int = 4;
const XI_MOTION: c_int = 6;
const XI_HIERARCHY_CHANGED: c_int = 11;
const XI_SCROLL_CLASS: c_int = 3;
const XI_VALUATOR_CLASS: c_int = 2;
const XI_ALL_DEVICES: c_int = 0;
const XI_ALL_MASTER_DEVICES: c_int = 1;

fn is_legacy_repeat_release(release: &XEvent, next: &XEvent) -> bool {
    unsafe {
        if release.type_ != KEY_RELEASE || next.type_ != KEY_PRESS {
            return false;
        }
        let release = release.key;
        let next = next.key;
        release.window == next.window
            && release.time == next.time
            && release.keycode == next.keycode
    }
}
const XI_SLAVE_POINTER: c_int = 3;
const XI_SLAVE_KEYBOARD: c_int = 4;
const XI_FLOATING_SLAVE: c_int = 5;
const XI_SCROLL_TYPE_VERTICAL: c_int = 1;
const XI_SCROLL_TYPE_HORIZONTAL: c_int = 2;
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
const X_LOOKUP_CHARS: c_int = 2;
const X_LOOKUP_BOTH: c_int = 4;
const X_BUFFER_OVERFLOW: c_int = -1;
const XIM_PREEDIT_NOTHING: c_ulong = 0x0008;
const XIM_PREEDIT_CALLBACKS: c_ulong = 0x0002;
const XIM_STATUS_NOTHING: c_ulong = 0x0400;

fn preedit_event(composition: &[char], caret: c_int) -> Event {
    let text: String = composition.iter().collect();
    let offset = if caret < 0 {
        -1
    } else {
        composition
            .iter()
            .take(caret as usize)
            .map(|character| character.len_utf8())
            .sum::<usize>()
            .min(i32::MAX as usize) as i32
    };
    Event::TextPreedit {
        text,
        cursor_begin: offset,
        cursor_end: offset,
    }
}

fn clear_preedit_after_focus_loss(state: &mut X11PreeditState, was_active: bool) {
    state.composition.clear();
    if was_active
        && !matches!(
            state.pending.last(),
            Some(Event::TextPreedit { text, .. }) if text.is_empty()
        )
    {
        state.pending.push(preedit_event(&state.composition, -1));
    }
}

fn xim_supports_preedit_callbacks(input_method: *mut c_void) -> bool {
    let mut styles = ptr::null_mut::<XimStyles>();
    let error = unsafe {
        XGetIMValues(
            input_method,
            b"queryInputStyle\0".as_ptr().cast::<c_char>(),
            &mut styles,
            ptr::null::<c_char>(),
        )
    };
    if !error.is_null() || styles.is_null() {
        if !styles.is_null() {
            unsafe { XFree(styles.cast()) };
        }
        return false;
    }
    let styles_ref = unsafe { &*styles };
    let supported = !styles_ref.supported_styles.is_null()
        && unsafe {
            std::slice::from_raw_parts(
                styles_ref.supported_styles,
                usize::from(styles_ref.count_styles),
            )
            .contains(&(XIM_PREEDIT_CALLBACKS | XIM_STATUS_NOTHING))
        };
    unsafe { XFree(styles.cast()) };
    supported
}

fn apply_preedit_change(
    composition: &mut Vec<char>,
    change_start: c_int,
    change_length: c_int,
    inserted: &[char],
    caret: c_int,
) -> Event {
    let start = (change_start.max(0) as usize).min(composition.len());
    let remove_end = start
        .saturating_add(change_length.max(0) as usize)
        .min(composition.len());
    composition.splice(start..remove_end, inserted.iter().copied());
    preedit_event(composition, caret)
}

fn decode_xim_text(text: *const XimText) -> Vec<char> {
    if text.is_null() {
        return Vec::new();
    }
    let text = unsafe { &*text };
    let length = usize::from(text.length);
    if length == 0 {
        return Vec::new();
    }
    unsafe {
        if text.encoding_is_wchar != 0 {
            let pointer = text.string.wide_char;
            if pointer.is_null() {
                return Vec::new();
            }
            std::slice::from_raw_parts(pointer, length)
                .iter()
                .filter_map(|&value| u32::try_from(value).ok().and_then(char::from_u32))
                .collect()
        } else {
            let pointer = text.string.multi_byte;
            if pointer.is_null() {
                return Vec::new();
            }
            let mut bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), length).to_vec();
            bytes.push(0);
            let mut wide = vec![0 as libc::wchar_t; length + 1];
            let converted = mbstowcs(wide.as_mut_ptr(), bytes.as_ptr().cast(), wide.len());
            if converted == usize::MAX {
                return Vec::new();
            }
            wide.truncate(converted);
            wide.into_iter()
                .filter_map(|value| u32::try_from(value).ok().and_then(char::from_u32))
                .collect()
        }
    }
}

fn with_preedit_state(client_data: *mut c_char, update: impl FnOnce(&mut X11PreeditState)) {
    if client_data.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        update(&mut *client_data.cast::<X11PreeditState>());
    }));
}

unsafe extern "C" fn x11_preedit_start_callback(
    _context: *mut c_void,
    client_data: *mut c_char,
    _call_data: *mut c_char,
) {
    with_preedit_state(client_data, |state| {
        state.composition.clear();
        state.pending.push(preedit_event(&state.composition, -1));
    });
}

unsafe extern "C" fn x11_preedit_done_callback(
    _context: *mut c_void,
    client_data: *mut c_char,
    _call_data: *mut c_char,
) {
    with_preedit_state(client_data, |state| {
        state.composition.clear();
        state.pending.push(preedit_event(&state.composition, -1));
    });
}

unsafe extern "C" fn x11_preedit_draw_callback(
    _context: *mut c_void,
    client_data: *mut c_char,
    call_data: *mut c_char,
) {
    if call_data.is_null() {
        return;
    }
    let draw = unsafe { &*call_data.cast::<XimPreeditDrawCallbackStruct>() };
    let inserted = decode_xim_text(draw.text);
    with_preedit_state(client_data, |state| {
        let event = apply_preedit_change(
            &mut state.composition,
            draw.chg_first,
            draw.chg_length,
            &inserted,
            draw.caret,
        );
        state.pending.push(event);
    });
}

unsafe extern "C" fn x11_preedit_caret_callback(
    _context: *mut c_void,
    client_data: *mut c_char,
    call_data: *mut c_char,
) {
    if call_data.is_null() {
        return;
    }
    let caret = unsafe { (*call_data.cast::<XimPreeditCaretCallbackStruct>()).position };
    with_preedit_state(client_data, |state| {
        state.pending.push(preedit_event(&state.composition, caret));
    });
}

pub struct Window {
    display: *mut Display,
    id: XWindow,
    detectable_keyboard_repeat: bool,
    input_method: *mut c_void,
    input_context: *mut c_void,
    preedit_callbacks: bool,
    preedit: Box<UnsafeCell<X11PreeditState>>,
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
    xinput2: Option<XInput2State>,
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
        // Avoid legacy X11 autorepeat release/press pairs creating false input transitions.
        let mut detectable_repeat_supported = 0;
        let detectable_keyboard_repeat = unsafe {
            XkbSetDetectableAutoRepeat(display, 1, &mut detectable_repeat_supported) != 0
                && detectable_repeat_supported != 0
        };
        let _screen = unsafe { XDefaultScreen(display) };
        let root = unsafe { XDefaultRootWindow(display) };
        let id = unsafe { XCreateSimpleWindow(display, root, 0, 0, width, height, 0, 0, 0) };
        if id == 0 {
            unsafe { XCloseDisplay(display) };
            return Err("XCreateSimpleWindow failed".into());
        }
        // Xlib invokes these callbacks synchronously while polling filtered events.
        let preedit = Box::new(UnsafeCell::new(X11PreeditState::default()));
        let preedit_client_data = preedit.get().cast::<c_char>();
        let (input_method, input_context, preedit_callbacks) = unsafe {
            libc::setlocale(libc::LC_CTYPE, b"\0".as_ptr().cast());
            XSetLocaleModifiers(b"\0".as_ptr().cast());
            let input_method = XOpenIM(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            let (input_context, preedit_callbacks) = if !input_method.is_null() {
                let mut start = XimCallback {
                    client_data: preedit_client_data,
                    callback: Some(x11_preedit_start_callback),
                };
                let mut done = XimCallback {
                    client_data: preedit_client_data,
                    callback: Some(x11_preedit_done_callback),
                };
                let mut draw = XimCallback {
                    client_data: preedit_client_data,
                    callback: Some(x11_preedit_draw_callback),
                };
                let mut caret = XimCallback {
                    client_data: preedit_client_data,
                    callback: Some(x11_preedit_caret_callback),
                };
                let nested = if xim_supports_preedit_callbacks(input_method) {
                    XVaCreateNestedList(
                        0,
                        b"preeditStartCallback\0".as_ptr().cast::<c_char>(),
                        &mut start,
                        b"preeditDoneCallback\0".as_ptr().cast::<c_char>(),
                        &mut done,
                        b"preeditDrawCallback\0".as_ptr().cast::<c_char>(),
                        &mut draw,
                        b"preeditCaretCallback\0".as_ptr().cast::<c_char>(),
                        &mut caret,
                        ptr::null::<c_char>(),
                    )
                } else {
                    ptr::null_mut()
                };
                let callback_context = if !nested.is_null() {
                    let context = XCreateIC(
                        input_method,
                        b"inputStyle\0".as_ptr().cast::<c_char>(),
                        XIM_PREEDIT_CALLBACKS | XIM_STATUS_NOTHING,
                        b"clientWindow\0".as_ptr().cast::<c_char>(),
                        id,
                        b"focusWindow\0".as_ptr().cast::<c_char>(),
                        id,
                        b"preeditAttributes\0".as_ptr().cast::<c_char>(),
                        nested,
                        ptr::null::<c_char>(),
                    );
                    XFree(nested);
                    context
                } else {
                    ptr::null_mut()
                };
                let (input_context, preedit_callbacks) = if !callback_context.is_null() {
                    (callback_context, true)
                } else {
                    (
                        XCreateIC(
                            input_method,
                            b"inputStyle\0".as_ptr().cast::<c_char>(),
                            XIM_PREEDIT_NOTHING | XIM_STATUS_NOTHING,
                            b"clientWindow\0".as_ptr().cast::<c_char>(),
                            id,
                            b"focusWindow\0".as_ptr().cast::<c_char>(),
                            id,
                            ptr::null::<c_char>(),
                        ),
                        false,
                    )
                };
                (input_context, preedit_callbacks)
            } else {
                (ptr::null_mut(), false)
            };
            if input_context.is_null() && !input_method.is_null() {
                XCloseIM(input_method);
                (ptr::null_mut(), ptr::null_mut(), false)
            } else {
                (input_method, input_context, preedit_callbacks)
            }
        };
        unsafe {
            XStoreName(display, id, title.as_ptr());
            XSelectInput(display, id, EVENT_MASK);
        }
        let xinput2 = XInput2State::new(display, id);
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
            detectable_keyboard_repeat,
            input_method,
            input_context,
            preedit_callbacks,
            preedit,
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
            xinput2,
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

    fn process_xinput2_event(&mut self, cookie: &mut XGenericEventCookie, events: &mut Vec<Event>) {
        let display = self.display;
        let window = self.id;
        let Some(input) = self.xinput2.as_mut() else {
            return;
        };
        if cookie.extension != input.extension_opcode
            || unsafe { XGetEventData(display, cookie) } == 0
        {
            return;
        }

        match cookie.evtype {
            XI_BUTTON_PRESS if !cookie.data.is_null() => {
                let event = unsafe { &*cookie.data.cast::<XIDeviceEvent>() };
                if event.event == window {
                    if let Some(translated) =
                        input.process_wheel_button(event.sourceid, event.detail)
                    {
                        events.push(translated);
                    }
                }
            }
            XI_MOTION if !cookie.data.is_null() => {
                let event = unsafe { &*cookie.data.cast::<XIDeviceEvent>() };
                if event.event == window {
                    events.extend(input.process_motion(event));
                }
            }
            XI_DEVICE_CHANGED | XI_HIERARCHY_CHANGED | 7 => input.refresh_devices(display),
            _ => {}
        }
        unsafe { XFreeEventData(display, cookie) };
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        while unsafe { XPending(self.display) } > 0 {
            let mut raw = XEvent { pad: [0; 24] };
            unsafe { XNextEvent(self.display, &mut raw) };
            if !self.detectable_keyboard_repeat
                && unsafe { raw.type_ == KEY_RELEASE && XPending(self.display) > 0 }
            {
                let mut next = XEvent { pad: [0; 24] };
                unsafe { XPeekEvent(self.display, &mut next) };
                if is_legacy_repeat_release(&raw, &next) {
                    continue;
                }
            }
            let filtered =
                !self.input_context.is_null() && unsafe { XFilterEvent(&mut raw, self.id) != 0 };
            let event_type = unsafe { raw.type_ };
            if event_type == GENERIC_EVENT {
                self.process_xinput2_event(unsafe { &mut raw.cookie }, &mut events);
            }
            if filtered && !matches!(event_type, KEY_PRESS | KEY_RELEASE) {
                if self.preedit_callbacks {
                    events.append(&mut self.preedit.get_mut().pending);
                }
                continue;
            }
            unsafe {
                match event_type {
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
                            if event.type_ == KEY_PRESS && !filtered {
                                let mut key = event;
                                let characters = if self.input_context.is_null() {
                                    let mut keysym = 0;
                                    let mut compose = XComposeStatus {
                                        compose_ptr: ptr::null_mut(),
                                        chars_matched: 0,
                                    };
                                    utf8_lookup_text(|buffer| {
                                        let count = XLookupString(
                                            &mut key,
                                            buffer.as_mut_ptr(),
                                            buffer.len() as c_int,
                                            &mut keysym,
                                            &mut compose,
                                        );
                                        (count, if count > 0 { X_LOOKUP_CHARS } else { 0 })
                                    })
                                } else {
                                    let mut keysym = 0;
                                    utf8_lookup_text(|buffer| {
                                        let mut status = 0;
                                        let count = Xutf8LookupString(
                                            self.input_context,
                                            &mut key,
                                            buffer.as_mut_ptr(),
                                            buffer.len() as c_int,
                                            &mut keysym,
                                            &mut status,
                                        );
                                        (count, status)
                                    })
                                };
                                events.extend(characters.into_iter().map(|character| {
                                    Event::TextInput {
                                        codepoint: character as u32,
                                    }
                                }));
                            }
                        }
                    }
                    BUTTON_PRESS | BUTTON_RELEASE => {
                        let event = raw.button;
                        if event.window == self.id {
                            if !(self.xinput2.is_some() && (4..=7).contains(&event.button)) {
                                if let Some(translated) =
                                    x11_button_event(event.button, event.type_ == BUTTON_PRESS)
                                {
                                    events.push(translated);
                                }
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
                            if !self.input_context.is_null() {
                                if event.type_ == FOCUS_IN {
                                    XSetICFocus(self.input_context);
                                } else {
                                    XUnsetICFocus(self.input_context);
                                    if self.preedit_callbacks {
                                        let was_active =
                                            !self.preedit.get_mut().composition.is_empty();
                                        clear_preedit_after_focus_loss(
                                            self.preedit.get_mut(),
                                            was_active,
                                        );
                                    }
                                }
                            }
                            events.push(Event::Focused {
                                focused: event.type_ == FOCUS_IN,
                            });
                        }
                    }
                    _ => {}
                }
            }
            if self.preedit_callbacks {
                events.append(&mut self.preedit.get_mut().pending);
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
            if !self.input_context.is_null() {
                XDestroyIC(self.input_context);
            }
            if !self.input_method.is_null() {
                XCloseIM(self.input_method);
            }
            XDestroyWindow(self.display, self.id);
            XCloseDisplay(self.display);
        }
    }
}

fn normalize_button_code(code: u32) -> u32 {
    match code {
        2 => 3,
        3 => 2,
        8 => 4,
        9 => 5,
        other => other,
    }
}

fn x11_button_event(code: u32, pressed: bool) -> Option<Event> {
    let scroll = match code {
        4 => Some((0.0, 1.0)),
        5 => Some((0.0, -1.0)),
        6 => Some((-1.0, 0.0)),
        7 => Some((1.0, 0.0)),
        _ => None,
    };

    match scroll {
        Some((delta_x, delta_y)) if pressed => Some(Event::Scroll { delta_x, delta_y }),
        Some(_) => None,
        None => Some(Event::PointerButton {
            button: normalize_button_code(code),
            pressed,
        }),
    }
}

fn xinput_scroll_delta(
    previous: f64,
    current: f64,
    increment: f64,
    scroll_type: c_int,
) -> Option<Event> {
    if !previous.is_finite() || !current.is_finite() || !increment.is_finite() || increment == 0.0 {
        return None;
    }
    let delta = -(current - previous) / increment;
    if delta == 0.0 {
        return None;
    }
    match scroll_type {
        XI_SCROLL_TYPE_VERTICAL => Some(Event::Scroll {
            delta_x: 0.0,
            delta_y: delta,
        }),
        XI_SCROLL_TYPE_HORIZONTAL => Some(Event::Scroll {
            delta_x: delta,
            delta_y: 0.0,
        }),
        _ => None,
    }
}

fn utf8_lookup_text(mut lookup: impl FnMut(&mut [c_char]) -> (c_int, c_int)) -> Vec<char> {
    let mut buffer = vec![0; 64];
    let (mut count, mut status) = lookup(&mut buffer);
    if status == X_BUFFER_OVERFLOW && count > 0 {
        buffer.resize(count as usize + 1, 0);
        (count, status) = lookup(&mut buffer);
    }
    if count <= 0
        || count as usize > buffer.len()
        || !matches!(status, X_LOOKUP_CHARS | X_LOOKUP_BOTH)
    {
        return Vec::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), count as usize) };
    std::str::from_utf8(bytes)
        .map(|text| text.chars().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        Event, X11PreeditState, XimPreeditCaretCallbackStruct, XimPreeditDrawCallbackStruct,
        XimText, XimTextString, apply_preedit_change, decode_xim_text, normalize_button_code,
        utf8_lookup_text, x11_preedit_done_callback, x11_preedit_draw_callback,
    };

    #[test]
    fn xim_multibyte_text_uses_the_active_locale() {
        let mut bytes = b"abc\0".to_vec();
        let text = XimText {
            length: 3,
            feedback: std::ptr::null_mut(),
            encoding_is_wchar: 0,
            string: XimTextString {
                multi_byte: bytes.as_mut_ptr().cast(),
            },
        };

        assert_eq!(decode_xim_text(&text), "abc".chars().collect::<Vec<_>>());
    }

    #[test]
    fn xim_caret_callback_converts_character_position_to_utf8_bytes() {
        let mut composition = X11PreeditState {
            composition: "かな".chars().collect(),
            pending: Vec::new(),
        };
        let mut caret = XimPreeditCaretCallbackStruct {
            position: 1,
            direction: 0,
            style: 0,
        };

        unsafe {
            super::x11_preedit_caret_callback(
                std::ptr::null_mut(),
                (&mut composition as *mut X11PreeditState).cast(),
                (&mut caret as *mut XimPreeditCaretCallbackStruct).cast(),
            );
        }

        assert_eq!(
            composition.pending,
            vec![Event::TextPreedit {
                text: "かな".into(),
                cursor_begin: 3,
                cursor_end: 3,
            }]
        );
    }

    #[test]
    fn xim_draw_callback_uses_the_native_change_payload() {
        let mut composition = X11PreeditState::default();
        let mut inserted = ['か' as libc::wchar_t, 'な' as libc::wchar_t];
        let mut text = XimText {
            length: 2,
            feedback: std::ptr::null_mut(),
            encoding_is_wchar: 1,
            string: XimTextString {
                wide_char: inserted.as_mut_ptr(),
            },
        };
        let mut draw = XimPreeditDrawCallbackStruct {
            caret: 2,
            chg_first: 0,
            chg_length: 0,
            text: &mut text,
        };

        unsafe {
            x11_preedit_draw_callback(
                std::ptr::null_mut(),
                (&mut composition as *mut X11PreeditState).cast(),
                (&mut draw as *mut XimPreeditDrawCallbackStruct).cast(),
            );
        }

        assert_eq!(
            composition.pending,
            vec![Event::TextPreedit {
                text: "かな".into(),
                cursor_begin: 6,
                cursor_end: 6,
            }]
        );

        unsafe {
            x11_preedit_done_callback(
                std::ptr::null_mut(),
                (&mut composition as *mut X11PreeditState).cast(),
                std::ptr::null_mut(),
            );
        }
        assert!(composition.composition.is_empty());
        assert_eq!(
            composition.pending.last(),
            Some(&Event::TextPreedit {
                text: String::new(),
                cursor_begin: -1,
                cursor_end: -1,
            })
        );
    }

    #[test]
    fn focus_loss_clears_active_xim_preedit_without_duplicate_clear() {
        let clear_event = Event::TextPreedit {
            text: String::new(),
            cursor_begin: -1,
            cursor_end: -1,
        };
        let mut state = X11PreeditState {
            composition: "かな".chars().collect(),
            pending: Vec::new(),
        };

        super::clear_preedit_after_focus_loss(&mut state, true);

        assert!(state.composition.is_empty());
        assert_eq!(state.pending, vec![clear_event.clone()]);

        state.composition = "か".chars().collect();
        state.pending.clear();
        state.pending.push(clear_event.clone());
        super::clear_preedit_after_focus_loss(&mut state, true);

        assert!(state.composition.is_empty());
        assert_eq!(state.pending, vec![clear_event]);
    }

    #[test]
    fn preedit_draw_replaces_character_ranges_and_reports_utf8_cursor_offsets() {
        let mut composition = Vec::new();
        let first = apply_preedit_change(&mut composition, 0, 0, &['か', 'な'], 2);
        assert_eq!(
            first,
            Event::TextPreedit {
                text: "かな".into(),
                cursor_begin: 6,
                cursor_end: 6,
            }
        );

        let updated = apply_preedit_change(&mut composition, 1, 1, &['に'], 2);
        assert_eq!(
            updated,
            Event::TextPreedit {
                text: "かに".into(),
                cursor_begin: 6,
                cursor_end: 6,
            }
        );
    }

    #[test]
    fn x11_buttons_use_the_shared_mouse_order() {
        assert_eq!(normalize_button_code(1), 1);
        assert_eq!(normalize_button_code(2), 3);
        assert_eq!(normalize_button_code(3), 2);
        assert_eq!(normalize_button_code(8), 4);
        assert_eq!(normalize_button_code(9), 5);
    }

    #[test]
    fn x11_wheel_buttons_emit_detents_without_mouse_button_transitions() {
        assert_eq!(
            super::x11_button_event(4, true),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            })
        );
        assert_eq!(
            super::x11_button_event(5, true),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: -1.0,
            })
        );
        assert_eq!(
            super::x11_button_event(6, true),
            Some(Event::Scroll {
                delta_x: -1.0,
                delta_y: 0.0,
            })
        );
        assert_eq!(
            super::x11_button_event(7, true),
            Some(Event::Scroll {
                delta_x: 1.0,
                delta_y: 0.0,
            })
        );
        assert_eq!(super::x11_button_event(4, false), None);
        assert_eq!(
            super::x11_button_event(1, true),
            Some(Event::PointerButton {
                button: 1,
                pressed: true,
            })
        );
        assert_eq!(
            super::x11_button_event(2, false),
            Some(Event::PointerButton {
                button: 3,
                pressed: false,
            })
        );
    }

    #[test]
    fn xinput_scroll_valuators_convert_to_fractional_wheel_steps() {
        assert_eq!(
            super::xinput_scroll_delta(12.0, 11.5, 2.0, super::XI_SCROLL_TYPE_VERTICAL),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: 0.25,
            })
        );
        assert_eq!(
            super::xinput_scroll_delta(12.0, 11.5, 2.0, super::XI_SCROLL_TYPE_HORIZONTAL),
            Some(Event::Scroll {
                delta_x: 0.25,
                delta_y: 0.0,
            })
        );
        assert_eq!(
            super::xinput_scroll_delta(1.0, 2.0, 0.0, super::XI_SCROLL_TYPE_VERTICAL),
            None
        );
    }

    #[test]
    fn xinput_wheel_buttons_fall_back_per_device_without_double_counting() {
        let state = super::XInput2State {
            extension_opcode: 0,
            devices: std::collections::HashMap::from([(
                7,
                vec![(
                    1,
                    super::X11ScrollAxis {
                        increment: 1.0,
                        scroll_type: super::XI_SCROLL_TYPE_VERTICAL,
                        position: 0.0,
                        position_valid: true,
                    },
                )],
            )]),
        };

        assert_eq!(state.process_wheel_button(7, 4), None);
        assert_eq!(
            state.process_wheel_button(8, 4),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            })
        );
        assert_eq!(
            state.process_wheel_button(8, 5),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: -1.0,
            })
        );
        assert_eq!(state.process_wheel_button(8, 1), None);
    }

    #[test]
    fn xinput_wheel_button_falls_back_until_its_matching_axis_has_a_baseline() {
        let state = super::XInput2State {
            extension_opcode: 0,
            devices: std::collections::HashMap::from([(
                7,
                vec![(
                    1,
                    super::X11ScrollAxis {
                        increment: 1.0,
                        scroll_type: super::XI_SCROLL_TYPE_VERTICAL,
                        position: 0.0,
                        position_valid: false,
                    },
                )],
            )]),
        };

        assert_eq!(
            state.process_wheel_button(7, 4),
            Some(Event::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            })
        );
        assert_eq!(
            state.process_wheel_button(7, 6),
            Some(Event::Scroll {
                delta_x: -1.0,
                delta_y: 0.0,
            })
        );
    }

    #[test]
    fn xinput_motion_tracks_only_changes_on_scroll_valuators() {
        let mut state = super::XInput2State {
            extension_opcode: 0,
            devices: std::collections::HashMap::from([(
                7,
                vec![(
                    1,
                    super::X11ScrollAxis {
                        increment: 2.0,
                        scroll_type: super::XI_SCROLL_TYPE_VERTICAL,
                        position: 0.0,
                        position_valid: false,
                    },
                )],
            )]),
        };
        let mut mask = [0b0000_0010];
        let mut values = [12.0];
        let event = super::XIDeviceEvent {
            type_: super::GENERIC_EVENT,
            serial: 0,
            send_event: 0,
            display: std::ptr::null_mut(),
            extension: 0,
            evtype: super::XI_MOTION,
            time: 0,
            deviceid: 3,
            sourceid: 7,
            detail: 0,
            root: 0,
            event: 1,
            child: 0,
            root_x: 0.0,
            root_y: 0.0,
            event_x: 0.0,
            event_y: 0.0,
            flags: 0,
            buttons: super::XIButtonState {
                mask_len: 0,
                mask: std::ptr::null_mut(),
            },
            valuators: super::XIValuatorState {
                mask_len: 1,
                mask: mask.as_mut_ptr(),
                values: values.as_mut_ptr(),
            },
            mods: super::XIModifierState {
                base: 0,
                latched: 0,
                locked: 0,
                effective: 0,
            },
            group: super::XIModifierState {
                base: 0,
                latched: 0,
                locked: 0,
                effective: 0,
            },
        };

        assert!(state.process_motion(&event).is_empty());
        unsafe { *event.valuators.values = 11.5 };
        assert_eq!(
            state.process_motion(&event),
            vec![Event::Scroll {
                delta_x: 0.0,
                delta_y: 0.25,
            }]
        );
        assert!(state.process_motion(&event).is_empty());
    }

    #[test]
    #[ignore = "requires a live X server and VAGUS_TEST_X11_INPUT=1"]
    fn xinput2_initializes_against_an_opted_in_x_server() {
        assert_eq!(std::env::var("VAGUS_TEST_X11_INPUT").as_deref(), Ok("1"));
        unsafe {
            let display = super::XOpenDisplay(std::ptr::null());
            assert!(
                !display.is_null(),
                "DISPLAY must be reachable for this test"
            );
            let root = super::XDefaultRootWindow(display);
            let window = super::XCreateSimpleWindow(display, root, 0, 0, 1, 1, 0, 0, 0);
            let state = super::XInput2State::new(display, window);
            super::XSync(display, 0);
            super::XDestroyWindow(display, window);
            super::XCloseDisplay(display);
            assert!(state.is_some(), "server must support XInput2 2.1 or newer");
        }
    }

    fn key_event(
        type_: i32,
        window: super::XWindow,
        time: std::ffi::c_ulong,
        keycode: u32,
    ) -> super::XEvent {
        super::XEvent {
            key: super::XKeyEvent {
                type_,
                serial: 0,
                send_event: 0,
                display: std::ptr::null_mut(),
                window,
                root: 0,
                subwindow: 0,
                time,
                x: 0,
                y: 0,
                x_root: 0,
                y_root: 0,
                state: 0,
                keycode,
                same_screen: 1,
            },
        }
    }

    #[test]
    fn legacy_autorepeat_release_matches_only_following_same_key_press() {
        let release = key_event(super::KEY_RELEASE, 7, 100, 38);
        let matching_press = key_event(super::KEY_PRESS, 7, 100, 38);

        assert!(super::is_legacy_repeat_release(&release, &matching_press));
        assert!(!super::is_legacy_repeat_release(
            &release,
            &key_event(super::KEY_PRESS, 7, 101, 38)
        ));
        assert!(!super::is_legacy_repeat_release(
            &release,
            &key_event(super::KEY_PRESS, 7, 100, 39)
        ));
        assert!(!super::is_legacy_repeat_release(
            &release,
            &key_event(super::KEY_PRESS, 8, 100, 38)
        ));
    }

    #[test]
    #[ignore = "requires a live X server and VAGUS_TEST_X11_INPUT=1"]
    fn x11_window_enables_detectable_keyboard_repeat() {
        assert_eq!(std::env::var("VAGUS_TEST_X11_INPUT").as_deref(), Ok("1"));
        let window = super::Window::create_window("Vagus repeat test", 1, 1).unwrap();
        let mut supported = 0;
        let enabled = unsafe { super::XkbGetDetectableAutoRepeat(window.display, &mut supported) };
        assert_ne!(
            supported, 0,
            "X server must support detectable keyboard repeat"
        );
        assert_ne!(
            enabled, 0,
            "X11 window must enable detectable keyboard repeat"
        );
    }

    #[test]
    #[ignore = "requires a live X server and VAGUS_TEST_X11_INPUT=1"]
    fn x11_fallback_suppresses_legacy_repeat_release_pairs() {
        assert_eq!(std::env::var("VAGUS_TEST_X11_INPUT").as_deref(), Ok("1"));
        let mut window = super::Window::create_window("Vagus repeat fallback test", 1, 1).unwrap();
        window.detectable_keyboard_repeat = false;

        let mut release = key_event(super::KEY_RELEASE, window.id, 100, 38);
        let mut press = key_event(super::KEY_PRESS, window.id, 100, 38);
        unsafe {
            release.key.display = window.display;
            press.key.display = window.display;
            assert_ne!(
                super::XSendEvent(window.display, window.id, 0, 0, &mut release),
                0
            );
            assert_ne!(
                super::XSendEvent(window.display, window.id, 0, 0, &mut press),
                0
            );
            super::XSync(window.display, 0);
        }

        let keys = window
            .poll_events()
            .into_iter()
            .filter_map(|event| match event {
                Event::Key { code, pressed } => Some((code, pressed)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(keys, [(38, true)]);
    }

    #[test]
    fn utf8_lookup_retries_buffer_overflow_and_preserves_unicode_text() {
        let expected = "漢🙂".repeat(12);
        let bytes = expected.as_bytes();
        let mut calls = 0;
        let decoded = utf8_lookup_text(|buffer| {
            calls += 1;
            if buffer.len() < bytes.len() {
                return (bytes.len() as i32, -1);
            }
            for (slot, byte) in buffer.iter_mut().zip(bytes) {
                *slot = *byte as i8;
            }
            (bytes.len() as i32, 2)
        });

        assert_eq!(calls, 2);
        assert_eq!(decoded, expected.chars().collect::<Vec<_>>());
    }

    #[test]
    fn utf8_lookup_ignores_keysyms_and_malformed_utf8() {
        assert!(
            utf8_lookup_text(|buffer| {
                buffer[0] = b'A' as i8;
                (1, 3)
            })
            .is_empty()
        );
        assert!(
            utf8_lookup_text(|buffer| {
                buffer[0] = 0xff_u8 as i8;
                (1, 2)
            })
            .is_empty()
        );
    }

    #[test]
    fn x11_input_context_lifecycle_is_safe_when_display_is_available() {
        let mut window = match super::Window::create_window("Vagus input test", 1, 1) {
            Ok(window) => window,
            Err(error) if error.starts_with("XOpenDisplay failed") => return,
            Err(error) => panic!("failed to create X11 input-test window: {error}"),
        };
        assert_eq!(
            window.input_context.is_null(),
            window.input_method.is_null()
        );
        assert_eq!(
            window.preedit_callbacks,
            !window.input_method.is_null()
                && super::xim_supports_preedit_callbacks(window.input_method)
        );
        let _ = window.poll_events();
    }
}
