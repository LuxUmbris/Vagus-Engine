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

#include <poll.h>
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "xdg-decoration-client-protocol.h"
#include "xdg-shell-client-protocol.h"
#include "text-input-unstable-v3-client-protocol.h"

#define VAGUS_EVENT_SCROLL_VALUE120 10
#define VAGUS_EVENT_SCROLL_DISCRETE 11
#define VAGUS_EVENT_KEYSYM 12
#define VAGUS_EVENT_TEXT_COMMIT 13
#define VAGUS_EVENT_TEXT_PREEDIT 14
#define VAGUS_EVENT_TEXT_DELETE_SURROUNDING 15
#define VAGUS_EVENT_TEXT_COMMIT_CHUNK 16
#define VAGUS_PREEDIT_TEXT_CAPACITY 512
#define VAGUS_EVENT_QUEUE_INITIAL_CAPACITY 512

typedef struct
{
    uint32_t kind;
    uint32_t code;
    int32_t x;
    int32_t y;
    uint32_t width;
    uint32_t height;
    uint32_t pressed;
    double delta_x;
    double delta_y;
    char text[VAGUS_PREEDIT_TEXT_CAPACITY];
    int32_t cursor_begin;
    int32_t cursor_end;
    uint32_t delete_before;
    uint32_t delete_after;
} VagusEvent;

typedef struct
{
    VagusEvent *items;
    size_t count;
    size_t capacity;
} VagusEventQueue;

typedef struct
{
    struct wl_display *display;
    struct wl_compositor *compositor;
    struct wl_surface *surface;
    struct wl_seat *seat;
    struct zwp_text_input_manager_v3 *text_input_manager;
    struct zwp_text_input_v3 *text_input;
    struct wl_pointer *pointer;
    struct wl_keyboard *keyboard;
    struct xkb_context *xkb_context;
    struct xkb_keymap *xkb_keymap;
    struct xkb_state *xkb_state;
    struct xdg_wm_base *wm_base;
    struct zxdg_decoration_manager_v1 *decoration_manager;
    struct xdg_surface *xdg_surface;
    struct xdg_toplevel *toplevel;
    struct zxdg_toplevel_decoration_v1 *decoration;
    VagusEventQueue events;
    double pending_axis_x;
    double pending_axis_y;
    int pending_axis_x_valid;
    int pending_axis_y_valid;
    int pending_axis_x_has_wheel_delta;
    int pending_axis_y_has_wheel_delta;
    VagusEventQueue pending_text_events;
    int event_queue_failed;
    int configured;
    int32_t width;
    int32_t height;
    int fullscreen;
    int text_input_active;
    char pending_preedit[VAGUS_PREEDIT_TEXT_CAPACITY];
    int32_t preedit_cursor_begin;
    int32_t preedit_cursor_end;
    int preedit_pending;
} VagusWaylandWindow;

size_t vagus_wayland_event_size(void)
{
    return sizeof(VagusEvent);
}

int vagus_wayland_should_emit_raw_axis_scroll(int has_wheel_delta)
{
    return !has_wheel_delta;
}

void vagus_wayland_filter_raw_axis_scroll(double axis_x, int axis_x_valid,
                                          int axis_x_has_wheel_delta, double axis_y,
                                          int axis_y_valid, int axis_y_has_wheel_delta,
                                          double *delta_x, double *delta_y)
{
    *delta_x = axis_x_valid &&
                       vagus_wayland_should_emit_raw_axis_scroll(axis_x_has_wheel_delta)
                   ? axis_x
                   : 0.0;
    *delta_y = axis_y_valid &&
                       vagus_wayland_should_emit_raw_axis_scroll(axis_y_has_wheel_delta)
                   ? axis_y
                   : 0.0;
}

int vagus_wayland_should_defer_axis_scroll(uint32_t pointer_version)
{
    return pointer_version >= 5;
}

static int queue_push(VagusEventQueue *queue, VagusEvent event)
{
    if (queue->count == queue->capacity)
    {
        size_t capacity = queue->capacity ? queue->capacity : VAGUS_EVENT_QUEUE_INITIAL_CAPACITY;
        if (queue->capacity)
        {
            if (capacity > SIZE_MAX / 2)
                return 0;
            capacity *= 2;
        }
        if (capacity > SIZE_MAX / sizeof(*queue->items))
            return 0;
        VagusEvent *items = realloc(queue->items, capacity * sizeof(*items));
        if (!items)
            return 0;
        queue->items = items;
        queue->capacity = capacity;
    }
    queue->items[queue->count++] = event;
    return 1;
}

static void free_event_queues(VagusWaylandWindow *window)
{
    free(window->events.items);
    free(window->pending_text_events.items);
    window->events = (VagusEventQueue){0};
    window->pending_text_events = (VagusEventQueue){0};
}

static void push_event(VagusWaylandWindow *window, VagusEvent event)
{
    if (!queue_push(&window->events, event))
        window->event_queue_failed = 1;
}

size_t vagus_wayland_test_event_queue_burst(size_t event_count)
{
    VagusWaylandWindow window = {0};
    for (size_t index = 0; index < event_count; ++index)
        push_event(&window, (VagusEvent){.kind = 1, .code = (uint32_t)index});
    size_t queued = window.events.count;
    for (size_t index = 0; index < queued; ++index)
    {
        if (window.events.items[index].code != (uint32_t)index)
        {
            queued = 0;
            break;
        }
    }
    free_event_queues(&window);
    return queued;
}

static int push_pending_text_event(VagusWaylandWindow *window, VagusEvent event)
{
    if (!queue_push(&window->pending_text_events, event))
    {
        window->event_queue_failed = 1;
        return 0;
    }
    return 1;
}

static void registry_global(void *data, struct wl_registry *registry, uint32_t name,
                            const char *interface, uint32_t version)
{
    VagusWaylandWindow *window = data;
    if (strcmp(interface, wl_compositor_interface.name) == 0)
    {
        uint32_t v = version < 4 ? version : 4;
        window->compositor = wl_registry_bind(registry, name, &wl_compositor_interface, v);
    }
    else if (strcmp(interface, wl_seat_interface.name) == 0)
    {
        uint32_t v = version < 8 ? version : 8;
        window->seat = wl_registry_bind(registry, name, &wl_seat_interface, v);
    }
    else if (strcmp(interface, zwp_text_input_manager_v3_interface.name) == 0)
    {
        window->text_input_manager = wl_registry_bind(
            registry, name, &zwp_text_input_manager_v3_interface, 1);
    }
    else if (strcmp(interface, xdg_wm_base_interface.name) == 0)
    {
        uint32_t v = version < 3 ? version : 3;
        window->wm_base = wl_registry_bind(registry, name, &xdg_wm_base_interface, v);
    }
    else if (strcmp(interface, zxdg_decoration_manager_v1_interface.name) == 0)
    {
        window->decoration_manager = wl_registry_bind(
            registry, name, &zxdg_decoration_manager_v1_interface, 1);
    }
}

static void registry_remove(void *data, struct wl_registry *registry, uint32_t name)
{
    (void)data;
    (void)registry;
    (void)name;
}
static const struct wl_registry_listener registry_listener = {registry_global, registry_remove};

static void wm_ping(void *data, struct xdg_wm_base *wm_base, uint32_t serial)
{
    (void)data;
    xdg_wm_base_pong(wm_base, serial);
}
static const struct xdg_wm_base_listener wm_listener = {wm_ping};

static void surface_configure(void *data, struct xdg_surface *surface, uint32_t serial)
{
    VagusWaylandWindow *window = data;
    xdg_surface_ack_configure(surface, serial);
    window->configured = 1;
}
static const struct xdg_surface_listener xdg_surface_listener = {surface_configure};

static void toplevel_configure(void *data, struct xdg_toplevel *toplevel,
                               int32_t width, int32_t height, struct wl_array *states)
{
    VagusWaylandWindow *window = data;
    (void)toplevel;
    (void)states;
    if (width > 0 && height > 0)
    {
        window->width = width;
        window->height = height;
        VagusEvent event = {.kind = 1, .width = (uint32_t)width, .height = (uint32_t)height};
        push_event(window, event);
    }
}
static void toplevel_close(void *data, struct xdg_toplevel *toplevel)
{
    (void)toplevel;
    push_event(data, (VagusEvent){.kind = 0});
}
static void toplevel_configure_bounds(void *data, struct xdg_toplevel *toplevel,
                                      int32_t width, int32_t height)
{
    (void)data;
    (void)toplevel;
    (void)width;
    (void)height;
}
static void toplevel_wm_capabilities(void *data, struct xdg_toplevel *toplevel,
                                     struct wl_array *capabilities)
{
    (void)data;
    (void)toplevel;
    (void)capabilities;
}
static const struct xdg_toplevel_listener toplevel_listener = {
    toplevel_configure, toplevel_close, toplevel_configure_bounds, toplevel_wm_capabilities};

static void decoration_configure(void *data,
                                 struct zxdg_toplevel_decoration_v1 *decoration,
                                 uint32_t mode)
{
    (void)data;
    (void)decoration;
    (void)mode;
}
static const struct zxdg_toplevel_decoration_v1_listener decoration_listener = {
    decoration_configure};

static void pointer_enter(void *data, struct wl_pointer *pointer, uint32_t serial,
                          struct wl_surface *surface, wl_fixed_t x, wl_fixed_t y)
{
    (void)pointer;
    (void)serial;
    (void)surface;
    push_event(data, (VagusEvent){.kind = 5, .x = wl_fixed_to_int(x), .y = wl_fixed_to_int(y)});
}
static void pointer_leave(void *data, struct wl_pointer *pointer, uint32_t serial,
                          struct wl_surface *surface)
{
    (void)pointer;
    (void)serial;
    (void)surface;
    push_event(data, (VagusEvent){.kind = 6});
}
static void pointer_motion(void *data, struct wl_pointer *pointer, uint32_t time,
                           wl_fixed_t x, wl_fixed_t y)
{
    (void)pointer;
    (void)time;
    VagusEvent event = {.kind = 3, .x = wl_fixed_to_int(x), .y = wl_fixed_to_int(y)};
    push_event(data, event);
}
static void pointer_button(void *data, struct wl_pointer *pointer, uint32_t serial,
                           uint32_t time, uint32_t button, uint32_t state)
{
    (void)pointer;
    (void)serial;
    (void)time;
    VagusEvent event = {.kind = 4, .code = button, .pressed = state == WL_POINTER_BUTTON_STATE_PRESSED};
    push_event(data, event);
}
static void pointer_axis(void *data, struct wl_pointer *pointer, uint32_t time,
                         uint32_t axis, wl_fixed_t value)
{
    VagusWaylandWindow *window = data;
    (void)time;
    double delta = wl_fixed_to_double(value);
    if (axis != WL_POINTER_AXIS_HORIZONTAL_SCROLL && axis != WL_POINTER_AXIS_VERTICAL_SCROLL)
        return;

    if (!vagus_wayland_should_defer_axis_scroll(wl_pointer_get_version(pointer)))
    {
        VagusEvent event = {.kind = 7};
        if (axis == WL_POINTER_AXIS_HORIZONTAL_SCROLL)
            event.delta_x = delta;
        else
            event.delta_y = delta;
        push_event(window, event);
        return;
    }

    if (axis == WL_POINTER_AXIS_HORIZONTAL_SCROLL)
    {
        window->pending_axis_x += delta;
        window->pending_axis_x_valid = 1;
    }
    else
    {
        window->pending_axis_y += delta;
        window->pending_axis_y_valid = 1;
    }
}
static void pointer_frame(void *data, struct wl_pointer *pointer)
{
    (void)pointer;
    VagusWaylandWindow *window = data;
    VagusEvent event = {.kind = 7};
    vagus_wayland_filter_raw_axis_scroll(
        window->pending_axis_x, window->pending_axis_x_valid,
        window->pending_axis_x_has_wheel_delta, window->pending_axis_y,
        window->pending_axis_y_valid, window->pending_axis_y_has_wheel_delta,
        &event.delta_x, &event.delta_y);
    if (event.delta_x != 0.0 || event.delta_y != 0.0)
        push_event(window, event);
    window->pending_axis_x = 0.0;
    window->pending_axis_y = 0.0;
    window->pending_axis_x_valid = 0;
    window->pending_axis_y_valid = 0;
    window->pending_axis_x_has_wheel_delta = 0;
    window->pending_axis_y_has_wheel_delta = 0;
}
static void pointer_axis_source(void *data, struct wl_pointer *pointer, uint32_t source)
{
    (void)data;
    (void)pointer;
    (void)source;
}
static void pointer_axis_stop(void *data, struct wl_pointer *pointer, uint32_t time, uint32_t axis)
{
    (void)data;
    (void)pointer;
    (void)time;
    (void)axis;
}
static void pointer_axis_discrete(void *data, struct wl_pointer *pointer, uint32_t axis, int32_t discrete)
{
    if (wl_pointer_get_version(pointer) >= 8)
        return;
    VagusEvent event = {.kind = VAGUS_EVENT_SCROLL_DISCRETE};
    if (axis == WL_POINTER_AXIS_HORIZONTAL_SCROLL)
    {
        ((VagusWaylandWindow *)data)->pending_axis_x_has_wheel_delta = 1;
        event.delta_x = discrete;
    }
    else if (axis == WL_POINTER_AXIS_VERTICAL_SCROLL)
    {
        ((VagusWaylandWindow *)data)->pending_axis_y_has_wheel_delta = 1;
        event.delta_y = discrete;
    }
    else
        return;
    push_event(data, event);
}
static void pointer_axis_value120(void *data, struct wl_pointer *pointer, uint32_t axis, int32_t value120)
{
    if (wl_pointer_get_version(pointer) < 8)
        return;
    VagusEvent event = {.kind = VAGUS_EVENT_SCROLL_VALUE120};
    if (axis == WL_POINTER_AXIS_HORIZONTAL_SCROLL)
    {
        ((VagusWaylandWindow *)data)->pending_axis_x_has_wheel_delta = 1;
        event.delta_x = value120;
    }
    else if (axis == WL_POINTER_AXIS_VERTICAL_SCROLL)
    {
        ((VagusWaylandWindow *)data)->pending_axis_y_has_wheel_delta = 1;
        event.delta_y = value120;
    }
    else
        return;
    push_event(data, event);
}
static const struct wl_pointer_listener pointer_listener = {
    pointer_enter, pointer_leave, pointer_motion, pointer_button, pointer_axis,
    pointer_frame, pointer_axis_source, pointer_axis_stop, pointer_axis_discrete,
    pointer_axis_value120};

static void keyboard_keymap(void *data, struct wl_keyboard *keyboard, uint32_t format,
                            int32_t fd, uint32_t size)
{
    (void)keyboard;
    VagusWaylandWindow *window = data;
    if (fd < 0)
        return;
    if (format != WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 || size == 0 || !window->xkb_context)
    {
        close(fd);
        return;
    }
    char *mapped = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
    close(fd);
    if (mapped == MAP_FAILED)
        return;
    char *text = malloc((size_t)size + 1);
    if (!text)
    {
        munmap(mapped, size);
        return;
    }
    memcpy(text, mapped, size);
    text[size] = '\0';
    munmap(mapped, size);
    struct xkb_keymap *keymap = xkb_keymap_new_from_string(
        window->xkb_context, text, XKB_KEYMAP_FORMAT_TEXT_V1, XKB_KEYMAP_COMPILE_NO_FLAGS);
    free(text);
    if (!keymap)
        return;
    struct xkb_state *state = xkb_state_new(keymap);
    if (!state)
    {
        xkb_keymap_unref(keymap);
        return;
    }
    if (window->xkb_state)
        xkb_state_unref(window->xkb_state);
    if (window->xkb_keymap)
        xkb_keymap_unref(window->xkb_keymap);
    window->xkb_keymap = keymap;
    window->xkb_state = state;
}
static void text_input_enter(void *data, struct zwp_text_input_v3 *text_input,
                             struct wl_surface *surface)
{
    (void)data;
    (void)text_input;
    (void)surface;
}

static void text_input_leave(void *data, struct zwp_text_input_v3 *text_input,
                             struct wl_surface *surface)
{
    (void)text_input;
    (void)surface;
    ((VagusWaylandWindow *)data)->text_input_active = 0;
}

static void text_input_preedit(void *data, struct zwp_text_input_v3 *text_input,
                               const char *text, int32_t cursor_begin, int32_t cursor_end)
{
    VagusWaylandWindow *window = data;
    (void)text_input;
    size_t length = text ? strlen(text) : 0;
    size_t copied = length < sizeof(window->pending_preedit)
                        ? length
                        : sizeof(window->pending_preedit) - 1;
    if (copied < length)
    {
        while (copied > 0 && (((unsigned char)text[copied] & 0xc0) == 0x80))
            --copied;
    }
    if (copied)
        memcpy(window->pending_preedit, text, copied);
    window->pending_preedit[copied] = '\0';
    window->preedit_cursor_begin = copied < length ? -1 : cursor_begin;
    window->preedit_cursor_end = copied < length ? -1 : cursor_end;
    if (!text)
    {
        window->preedit_cursor_begin = -1;
        window->preedit_cursor_end = -1;
    }
    window->preedit_pending = 1;
}

int vagus_wayland_decode_utf8_scalar(const unsigned char *bytes, size_t available,
                                     uint32_t *codepoint_out, size_t *consumed_out)
{
    if (consumed_out)
        *consumed_out = 0;
    if (!bytes || !codepoint_out || !consumed_out || available == 0)
        return 0;

    *consumed_out = 1;
    uint32_t codepoint;
    size_t length;
    if (bytes[0] < 0x80)
    {
        codepoint = bytes[0];
        length = 1;
    }
    else if (bytes[0] >= 0xc2 && bytes[0] <= 0xdf)
    {
        codepoint = bytes[0] & 0x1f;
        length = 2;
    }
    else if (bytes[0] >= 0xe0 && bytes[0] <= 0xef)
    {
        codepoint = bytes[0] & 0x0f;
        length = 3;
    }
    else if (bytes[0] >= 0xf0 && bytes[0] <= 0xf4)
    {
        codepoint = bytes[0] & 0x07;
        length = 4;
    }
    else
    {
        return 0;
    }

    if (length > available)
        return 0;
    for (size_t index = 1; index < length; ++index)
    {
        if ((bytes[index] & 0xc0) != 0x80)
            return 0;
        codepoint = (codepoint << 6) | (bytes[index] & 0x3f);
    }
    if ((length == 2 && codepoint < 0x80) ||
        (length == 3 && codepoint < 0x800) ||
        (length == 4 && codepoint < 0x10000) ||
        codepoint > 0x10ffff || (codepoint >= 0xd800 && codepoint <= 0xdfff))
        return 0;

    *codepoint_out = codepoint;
    *consumed_out = length;
    return 1;
}

size_t vagus_wayland_encode_committed_utf8(const char *text, VagusEvent *output,
                                           size_t capacity)
{
    if (text == NULL || (output == NULL && capacity != 0))
        return (size_t)-1;

    const unsigned char *cursor = (const unsigned char *)text;
    size_t remaining = strlen(text);
    size_t event_count = 0;
    size_t chunk_length = 0;
    VagusEvent event = {.kind = VAGUS_EVENT_TEXT_COMMIT_CHUNK};
    while (remaining > 0)
    {
        uint32_t codepoint;
        size_t consumed;
        int valid = vagus_wayland_decode_utf8_scalar(cursor, remaining, &codepoint, &consumed);
        if (valid)
        {
            if (chunk_length + consumed >= sizeof(event.text))
            {
                if (event_count == capacity)
                    return (size_t)-1;
                event.text[chunk_length] = '\0';
                output[event_count++] = event;
                event = (VagusEvent){.kind = VAGUS_EVENT_TEXT_COMMIT_CHUNK};
                chunk_length = 0;
            }
            memcpy(event.text + chunk_length, cursor, consumed);
            chunk_length += consumed;
        }
        cursor += consumed;
        remaining -= consumed;
    }

    if (chunk_length > 0)
    {
        if (event_count == capacity)
            return (size_t)-1;
        event.text[chunk_length] = '\0';
        output[event_count++] = event;
    }
    return event_count;
}

static void push_committed_utf8(VagusWaylandWindow *window, const char *text)
{
    if (!text)
        return;

    const unsigned char *cursor = (const unsigned char *)text;
    size_t remaining = strlen(text);
    size_t chunk_length = 0;
    VagusEvent event = {.kind = VAGUS_EVENT_TEXT_COMMIT_CHUNK};
    while (remaining > 0)
    {
        uint32_t codepoint;
        size_t consumed;
        int valid = vagus_wayland_decode_utf8_scalar(cursor, remaining, &codepoint, &consumed);
        if (valid)
        {
            if (chunk_length + consumed >= sizeof(event.text))
            {
                event.text[chunk_length] = '\0';
                if (!push_pending_text_event(window, event))
                    return;
                event = (VagusEvent){.kind = VAGUS_EVENT_TEXT_COMMIT_CHUNK};
                chunk_length = 0;
            }
            memcpy(event.text + chunk_length, cursor, consumed);
            chunk_length += consumed;
        }
        cursor += consumed;
        remaining -= consumed;
    }

    if (chunk_length > 0)
    {
        event.text[chunk_length] = '\0';
        push_pending_text_event(window, event);
    }
}

size_t vagus_wayland_test_committed_text_burst(size_t expected_chunks)
{
    const size_t chunk_capacity = sizeof(((VagusEvent *)0)->text) - 1;
    const size_t bytes_per_chunk = chunk_capacity - (chunk_capacity % 2);
    if (expected_chunks > (SIZE_MAX - 1) / bytes_per_chunk)
        return 0;
    size_t text_length = expected_chunks * bytes_per_chunk;
    char *text = malloc(text_length + 1);
    if (!text)
        return 0;
    for (size_t index = 0; index < text_length; index += 2)
    {
        text[index] = (char)0xc3;
        text[index + 1] = (char)0xa9;
    }
    text[text_length] = '\0';

    VagusWaylandWindow window = {0};
    push_committed_utf8(&window, text);
    int valid = !window.event_queue_failed &&
                window.pending_text_events.count == expected_chunks;
    size_t recovered_length = 0;
    for (size_t index = 0; valid && index < window.pending_text_events.count; ++index)
    {
        VagusEvent *event = &window.pending_text_events.items[index];
        if (event->kind != VAGUS_EVENT_TEXT_COMMIT_CHUNK)
        {
            valid = 0;
            break;
        }
        size_t chunk_length = 0;
        while (chunk_length < sizeof(event->text) && event->text[chunk_length] != '\0')
            ++chunk_length;
        if (chunk_length == sizeof(event->text) || chunk_length % 2 != 0)
            valid = 0;
        for (size_t offset = 0; valid && offset < chunk_length; offset += 2)
        {
            if ((unsigned char)event->text[offset] != 0xc3 ||
                (unsigned char)event->text[offset + 1] != 0xa9)
                valid = 0;
        }
        recovered_length += chunk_length;
    }
    valid = valid && recovered_length == text_length;
    size_t result = valid ? window.pending_text_events.count : 0;
    free(text);
    free_event_queues(&window);
    return result;
}

static void text_input_commit_string(void *data, struct zwp_text_input_v3 *text_input,
                                     const char *text)
{
    (void)text_input;
    if (text)
        push_committed_utf8(data, text);
}

static void text_input_delete_surrounding(void *data, struct zwp_text_input_v3 *text_input,
                                          uint32_t before_length, uint32_t after_length)
{
    (void)text_input;
    VagusWaylandWindow *window = data;
    push_pending_text_event(window, (VagusEvent){
        .kind = VAGUS_EVENT_TEXT_DELETE_SURROUNDING,
        .delete_before = before_length,
        .delete_after = after_length,
    });
}

static void text_input_done(void *data, struct zwp_text_input_v3 *text_input, uint32_t serial)
{
    VagusWaylandWindow *window = data;
    (void)text_input;
    (void)serial;
    push_event(window, (VagusEvent){
        .kind = VAGUS_EVENT_TEXT_PREEDIT,
        .cursor_begin = -1,
        .cursor_end = -1,
    });
    for (size_t index = 0; index < window->pending_text_events.count; ++index)
    {
        if (window->pending_text_events.items[index].kind == VAGUS_EVENT_TEXT_DELETE_SURROUNDING)
            push_event(window, window->pending_text_events.items[index]);
    }
    for (size_t index = 0; index < window->pending_text_events.count; ++index)
    {
        if (window->pending_text_events.items[index].kind == VAGUS_EVENT_TEXT_COMMIT ||
            window->pending_text_events.items[index].kind == VAGUS_EVENT_TEXT_COMMIT_CHUNK)
            push_event(window, window->pending_text_events.items[index]);
    }
    window->pending_text_events.count = 0;
    VagusEvent event = {.kind = VAGUS_EVENT_TEXT_PREEDIT};
    if (window->preedit_pending)
    {
        event.cursor_begin = window->preedit_cursor_begin;
        event.cursor_end = window->preedit_cursor_end;
        memcpy(event.text, window->pending_preedit, sizeof(event.text));
    }
    if (window->preedit_pending)
        push_event(window, event);
    window->preedit_pending = 0;
}

int vagus_wayland_test_preedit_truncates_on_utf8_boundary(void)
{
    char text[VAGUS_PREEDIT_TEXT_CAPACITY + 3] = {0};
    size_t prefix_length = VAGUS_PREEDIT_TEXT_CAPACITY - 2;
    memset(text, 'a', prefix_length);
    text[prefix_length] = (char)0xf0;
    text[prefix_length + 1] = (char)0x9f;
    text[prefix_length + 2] = (char)0x99;
    text[prefix_length + 3] = (char)0x82;
    text[prefix_length + 4] = '\0';

    VagusWaylandWindow window = {0};
    text_input_preedit(&window, NULL, text, (int32_t)prefix_length,
                       (int32_t)(prefix_length + 4));
    text_input_done(&window, NULL, 0);

    int valid = !window.event_queue_failed && window.events.count == 2;
    if (valid)
    {
        VagusEvent *preedit = &window.events.items[1];
        valid = preedit->kind == VAGUS_EVENT_TEXT_PREEDIT &&
                strlen(preedit->text) == prefix_length &&
                preedit->cursor_begin == -1 && preedit->cursor_end == -1 &&
                memcmp(preedit->text, text, prefix_length) == 0;
    }
    free_event_queues(&window);
    return valid;
}

static void text_input_action(void *data, struct zwp_text_input_v3 *text_input,
                              uint32_t action, uint32_t serial)
{
    (void)data;
    (void)text_input;
    (void)action;
    (void)serial;
}

static const struct zwp_text_input_v3_listener text_input_listener = {
    .enter = text_input_enter,
    .leave = text_input_leave,
    .preedit_string = text_input_preedit,
    .commit_string = text_input_commit_string,
    .delete_surrounding_text = text_input_delete_surrounding,
    .done = text_input_done,
    .action = text_input_action,
};

static void keyboard_enter(void *data, struct wl_keyboard *keyboard, uint32_t serial,
                           struct wl_surface *surface, struct wl_array *keys)
{
    (void)keyboard;
    (void)serial;
    (void)surface;
    (void)keys;
    VagusWaylandWindow *window = data;
    push_event(window, (VagusEvent){.kind = 8, .pressed = 1});
    if (window->text_input)
    {
        zwp_text_input_v3_enable(window->text_input);
        zwp_text_input_v3_commit(window->text_input);
        window->text_input_active = 1;
    }
}

static void keyboard_leave(void *data, struct wl_keyboard *keyboard, uint32_t serial,
                           struct wl_surface *surface)
{
    (void)keyboard;
    (void)serial;
    (void)surface;
    VagusWaylandWindow *window = data;
    push_event(window, (VagusEvent){.kind = 8, .pressed = 0});
    window->text_input_active = 0;
    if (window->text_input)
    {
        zwp_text_input_v3_disable(window->text_input);
        zwp_text_input_v3_commit(window->text_input);
    }
}
static void keyboard_key(void *data, struct wl_keyboard *keyboard, uint32_t serial,
                         uint32_t time, uint32_t key, uint32_t state)
{
    (void)keyboard;
    (void)serial;
    (void)time;
    VagusEvent event = {.kind = 2, .code = key, .pressed = state == WL_KEYBOARD_KEY_STATE_PRESSED};
    push_event(data, event);
    VagusWaylandWindow *window = data;
    if (window->xkb_state)
    {
        xkb_keycode_t keycode = key + 8;
        xkb_state_update_key(window->xkb_state, keycode,
                             state == WL_KEYBOARD_KEY_STATE_PRESSED ? XKB_KEY_DOWN : XKB_KEY_UP);
        if (state == WL_KEYBOARD_KEY_STATE_PRESSED && !window->text_input_active)
        {
            xkb_keysym_t keysym = xkb_state_key_get_one_sym(window->xkb_state, keycode);
            if (keysym != XKB_KEY_NoSymbol)
                push_event(window, (VagusEvent){.kind = VAGUS_EVENT_KEYSYM, .code = keysym});
        }
    }
}
static void keyboard_modifiers(void *data, struct wl_keyboard *keyboard, uint32_t serial,
                               uint32_t depressed, uint32_t latched, uint32_t locked,
                               uint32_t group)
{
    (void)keyboard;
    (void)serial;
    VagusWaylandWindow *window = data;
    if (window->xkb_state)
        xkb_state_update_mask(window->xkb_state, depressed, latched, locked, 0, 0, group);
}
static void keyboard_repeat(void *data, struct wl_keyboard *keyboard, int32_t rate, int32_t delay)
{
    (void)data;
    (void)keyboard;
    (void)rate;
    (void)delay;
}
static const struct wl_keyboard_listener keyboard_listener = {
    keyboard_keymap, keyboard_enter, keyboard_leave, keyboard_key,
    keyboard_modifiers, keyboard_repeat};

static void seat_capabilities(void *data, struct wl_seat *seat, uint32_t capabilities)
{
    VagusWaylandWindow *window = data;
    if ((capabilities & WL_SEAT_CAPABILITY_POINTER) && !window->pointer)
    {
        window->pointer = wl_seat_get_pointer(seat);
        wl_pointer_add_listener(window->pointer, &pointer_listener, window);
    }
    if ((capabilities & WL_SEAT_CAPABILITY_KEYBOARD) && !window->keyboard)
    {
        window->keyboard = wl_seat_get_keyboard(seat);
        wl_keyboard_add_listener(window->keyboard, &keyboard_listener, window);
    }
}
static void seat_name(void *data, struct wl_seat *seat, const char *name)
{
    (void)data;
    (void)seat;
    (void)name;
}
static const struct wl_seat_listener seat_listener = {seat_capabilities, seat_name};

void *vagus_wayland_create(const char *title, int32_t width, int32_t height)
{
    if (width <= 0 || height <= 0)
        return NULL;
    VagusWaylandWindow *window = calloc(1, sizeof(*window));
    if (!window)
        return NULL;
    window->xkb_context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    window->display = wl_display_connect(NULL);
    if (!window->display)
        goto fail;
    struct wl_registry *registry = wl_display_get_registry(window->display);
    if (!registry)
        goto fail;
    wl_registry_add_listener(registry, &registry_listener, window);
    if (wl_display_roundtrip(window->display) < 0 || !window->compositor || !window->wm_base)
        goto fail;
    if (window->seat)
        wl_seat_add_listener(window->seat, &seat_listener, window);
    xdg_wm_base_add_listener(window->wm_base, &wm_listener, window);
    window->surface = wl_compositor_create_surface(window->compositor);
    if (!window->surface)
        goto fail;
    if (window->text_input_manager && window->seat)
    {
        window->text_input = zwp_text_input_manager_v3_get_text_input(
            window->text_input_manager, window->seat);
        if (!window->text_input ||
            zwp_text_input_v3_add_listener(window->text_input, &text_input_listener, window) < 0)
            goto fail;
    }
    window->xdg_surface = xdg_wm_base_get_xdg_surface(window->wm_base, window->surface);
    if (!window->xdg_surface)
        goto fail;
    xdg_surface_add_listener(window->xdg_surface, &xdg_surface_listener, window);
    window->toplevel = xdg_surface_get_toplevel(window->xdg_surface);
    if (!window->toplevel)
        goto fail;
    xdg_toplevel_add_listener(window->toplevel, &toplevel_listener, window);
    window->width = width;
    window->height = height;
    if (window->decoration_manager)
    {
        window->decoration = zxdg_decoration_manager_v1_get_toplevel_decoration(
            window->decoration_manager, window->toplevel);
        if (!window->decoration)
            goto fail;
        zxdg_toplevel_decoration_v1_add_listener(
            window->decoration, &decoration_listener, window);
        zxdg_toplevel_decoration_v1_set_mode(
            window->decoration, ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
    }
    xdg_toplevel_set_title(window->toplevel, title);
    xdg_toplevel_set_app_id(window->toplevel, "vagus");
    xdg_toplevel_set_min_size(window->toplevel, 0, 0);
    wl_surface_commit(window->surface);
    if (wl_display_roundtrip(window->display) < 0 || !window->configured)
        goto fail;
    wl_display_flush(window->display);
    wl_registry_destroy(registry);
    return window;
fail:
    if (window->text_input)
        zwp_text_input_v3_destroy(window->text_input);
    if (window->text_input_manager)
        zwp_text_input_manager_v3_destroy(window->text_input_manager);
    if (window->xkb_state)
        xkb_state_unref(window->xkb_state);
    if (window->xkb_keymap)
        xkb_keymap_unref(window->xkb_keymap);
    if (window->xkb_context)
        xkb_context_unref(window->xkb_context);
    if (window->display)
        wl_display_disconnect(window->display);
    free_event_queues(window);
    free(window);
    return NULL;
}

int32_t vagus_wayland_poll(void *opaque, VagusEvent *output, int32_t capacity)
{
    VagusWaylandWindow *window = opaque;
    if (!window || !output || capacity <= 0)
        return 0;
    wl_display_dispatch_pending(window->display);
    wl_display_flush(window->display);
    struct pollfd descriptor = {.fd = wl_display_get_fd(window->display), .events = POLLIN};
    int ready;
    do
    {
        ready = poll(&descriptor, 1, 0);
    } while (ready < 0 && errno == EINTR);
    if (ready > 0)
        wl_display_dispatch(window->display);
    if (window->event_queue_failed)
    {
        window->event_queue_failed = 0;
        errno = ENOMEM;
        return -1;
    }
    int32_t count = window->events.count < (size_t)capacity
                        ? (int32_t)window->events.count
                        : capacity;
    if (count > 0)
    {
        memcpy(output, window->events.items, (size_t)count * sizeof(*output));
        size_t remaining = window->events.count - (size_t)count;
        if (remaining > 0)
            memmove(window->events.items, window->events.items + count,
                    remaining * sizeof(*output));
        window->events.count = remaining;
    }
    return count;
}

void *vagus_wayland_display(void *opaque)
{
    VagusWaylandWindow *window = opaque;
    return window ? window->display : NULL;
}
void *vagus_wayland_surface(void *opaque)
{
    VagusWaylandWindow *window = opaque;
    return window ? window->surface : NULL;
}
int32_t vagus_wayland_set_title(void *opaque, const char *title)
{
    VagusWaylandWindow *window = opaque;
    if (!window || !title)
        return -1;
    xdg_toplevel_set_title(window->toplevel, title);
    if (wl_display_flush(window->display) < 0 && errno != EAGAIN)
        return -2;
    return 0;
}
int32_t vagus_wayland_set_surrounding_text(void *opaque, const char *text,
                                           int32_t cursor, int32_t anchor)
{
    VagusWaylandWindow *window = opaque;
    if (!window || !text || !window->text_input || cursor < 0 || anchor < 0)
        return -1;
    size_t length = strlen(text);
    if (length > 4000 || (size_t)cursor > length || (size_t)anchor > length)
        return -1;

    zwp_text_input_v3_set_surrounding_text(window->text_input, text, cursor, anchor);
    if (window->text_input_active)
        zwp_text_input_v3_commit(window->text_input);
    if (wl_display_flush(window->display) < 0 && errno != EAGAIN)
        return -2;
    return 0;
}
int32_t vagus_wayland_apply_cfg(void *opaque, int32_t fullscreen,
                                int32_t resizable, int32_t borderless)
{
    VagusWaylandWindow *window = opaque;
    if (!window)
        return -2;
    if (borderless && !window->decoration)
        return -1;

    if (window->decoration)
    {
        zxdg_toplevel_decoration_v1_set_mode(
            window->decoration,
            borderless ? ZXDG_TOPLEVEL_DECORATION_V1_MODE_CLIENT_SIDE
                       : ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
    }
    if (fullscreen && !window->fullscreen)
        xdg_toplevel_set_fullscreen(window->toplevel, NULL);
    else if (!fullscreen && window->fullscreen)
        xdg_toplevel_unset_fullscreen(window->toplevel);

    if (resizable)
    {
        xdg_toplevel_set_min_size(window->toplevel, 0, 0);
        xdg_toplevel_set_max_size(window->toplevel, 0, 0);
    }
    else
    {
        xdg_toplevel_set_min_size(window->toplevel, window->width, window->height);
        xdg_toplevel_set_max_size(window->toplevel, window->width, window->height);
    }
    wl_surface_commit(window->surface);
    if (wl_display_flush(window->display) < 0 && errno != EAGAIN)
        return -2;
    window->fullscreen = fullscreen;
    return 0;
}
int32_t vagus_wayland_set_window_state(void *opaque, int32_t minimized,
                                       int32_t maximized)
{
    VagusWaylandWindow *window = opaque;
    if (!window)
        return -2;
    if (minimized && maximized)
        return -1;

    if (minimized)
        xdg_toplevel_set_minimized(window->toplevel);
    else if (maximized)
        xdg_toplevel_set_maximized(window->toplevel);
    else
        xdg_toplevel_unset_maximized(window->toplevel);
    wl_surface_commit(window->surface);
    if (wl_display_flush(window->display) < 0 && errno != EAGAIN)
        return -2;
    return 0;
}
void vagus_wayland_destroy(void *opaque)
{
    VagusWaylandWindow *window = opaque;
    if (!window)
        return;
    if (window->pointer)
        wl_pointer_destroy(window->pointer);
    if (window->keyboard)
        wl_keyboard_destroy(window->keyboard);
    if (window->text_input)
        zwp_text_input_v3_destroy(window->text_input);
    if (window->text_input_manager)
        zwp_text_input_manager_v3_destroy(window->text_input_manager);
    if (window->xkb_state)
        xkb_state_unref(window->xkb_state);
    if (window->xkb_keymap)
        xkb_keymap_unref(window->xkb_keymap);
    if (window->xkb_context)
        xkb_context_unref(window->xkb_context);
    if (window->decoration)
        zxdg_toplevel_decoration_v1_destroy(window->decoration);
    if (window->toplevel)
        xdg_toplevel_destroy(window->toplevel);
    if (window->xdg_surface)
        xdg_surface_destroy(window->xdg_surface);
    if (window->surface)
        wl_surface_destroy(window->surface);
    if (window->seat)
        wl_seat_destroy(window->seat);
    if (window->decoration_manager)
        zxdg_decoration_manager_v1_destroy(window->decoration_manager);
    if (window->wm_base)
        xdg_wm_base_destroy(window->wm_base);
    if (window->compositor)
        wl_compositor_destroy(window->compositor);
    wl_display_flush(window->display);
    wl_display_disconnect(window->display);
    free_event_queues(window);
    free(window);
}
