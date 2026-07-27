#ifndef VAGUS_ENGINE_WINDOW_HPP
#define VAGUS_ENGINE_WINDOW_HPP

typedef struct Window {
    SDL_Window* sdl_window;
    SDL_GLContext gl_context;
} Window;

static int target_fps = 60;
static double target_frame_time = 1.0 / 60.0;

static Uint64 last_time = 0;
static double delta_time = 0.0;

void InitWindow(int width, int height);
void InitWindowFullscreen(bool borderless);
void SetWindowFullscreen(bool borderless);
void SetWindowSize(int width, int height);
void SetWindowTitle(const char* title);
void SetWindowPosition(int x, int y);
void GetWindowPosition(int* x, int* y);
void CloseWindow();
bool WindowShouldClose();
void GetWindowSize(int* w, int* h);

#endif // VAGUS_ENGINE_WINDOW_HPP