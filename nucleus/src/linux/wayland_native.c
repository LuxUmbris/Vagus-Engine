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
} VagusEvent;

typedef struct
{
    struct wl_display *display;
    struct wl_compositor *compositor;
    struct wl_surface *surface;
    struct wl_seat *seat;
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
    VagusEvent events[512];
    size_t event_count;
    int configured;
    int32_t width;
    int32_t height;
    int fullscreen;
} VagusWaylandWindow;

static void push_event(VagusWaylandWindow *window, VagusEvent event)
{
    if (window->event_count < sizeof(window->events) / sizeof(window->events[0]))
        window->events[window->event_count++] = event;
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
        uint32_t v = version < 7 ? version : 7;
        window->seat = wl_registry_bind(registry, name, &wl_seat_interface, v);
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
    (void)pointer;
    (void)time;
    double delta = -wl_fixed_to_double(value);
    VagusEvent event = {.kind = 7};
    if (axis == WL_POINTER_AXIS_HORIZONTAL_SCROLL)
        event.delta_x = delta;
    else if (axis == WL_POINTER_AXIS_VERTICAL_SCROLL)
        event.delta_y = delta;
    else
        return;
    push_event(data, event);
}
static void pointer_frame(void *data, struct wl_pointer *pointer)
{
    (void)data;
    (void)pointer;
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
    (void)data;
    (void)pointer;
    (void)axis;
    (void)discrete;
}
static void pointer_axis_value120(void *data, struct wl_pointer *pointer, uint32_t axis, int32_t value120)
{
    (void)data;
    (void)pointer;
    (void)axis;
    (void)value120;
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
static void keyboard_enter(void *data, struct wl_keyboard *keyboard, uint32_t serial,
                           struct wl_surface *surface, struct wl_array *keys)
{
    (void)keyboard;
    (void)serial;
    (void)surface;
    (void)keys;
    push_event(data, (VagusEvent){.kind = 8, .pressed = 1});
}
static void keyboard_leave(void *data, struct wl_keyboard *keyboard, uint32_t serial,
                           struct wl_surface *surface)
{
    (void)keyboard;
    (void)serial;
    (void)surface;
    push_event(data, (VagusEvent){.kind = 8, .pressed = 0});
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
        if (state == WL_KEYBOARD_KEY_STATE_PRESSED)
        {
            uint32_t codepoint = xkb_state_key_get_utf32(window->xkb_state, keycode);
            if (codepoint)
                push_event(window, (VagusEvent){.kind = 9, .code = codepoint});
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
    if (window->xkb_state)
        xkb_state_unref(window->xkb_state);
    if (window->xkb_keymap)
        xkb_keymap_unref(window->xkb_keymap);
    if (window->xkb_context)
        xkb_context_unref(window->xkb_context);
    if (window->display)
        wl_display_disconnect(window->display);
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
    int32_t count = window->event_count < (size_t)capacity ? (int32_t)window->event_count : capacity;
    memcpy(output, window->events, (size_t)count * sizeof(*output));
    memmove(window->events, window->events + count,
            (window->event_count - (size_t)count) * sizeof(*output));
    window->event_count -= (size_t)count;
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
    free(window);
}
