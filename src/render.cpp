#include <SDL2/SDL.h>
#include <glad/glad.h>
#include "render.hpp"
#include "window.hpp"

void StartFrame() 
{
    // Clear the screen
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
}

void EndFrame(Window& window) 
{
    Uint64 now = SDL_GetPerformanceCounter();
    double freq = (double)SDL_GetPerformanceFrequency();

    delta_time = (double)(now - last_time) / freq;

    if (delta_time < target_frame_time) {
        double delay = (target_frame_time - delta_time) * 1000.0;
        SDL_Delay((Uint32)delay);

        now = SDL_GetPerformanceCounter();
        delta_time = (double)(now - last_time) / freq;
    }

    last_time = now;

    SDL_GL_SwapWindow(window.sdl_window);
}

void VSync(bool enabled) 
{
    SDL_GL_SetSwapInterval(enabled ? 1 : 0);
}

void SetTargetFPS(int fps) 
{
    target_fps = fps;
    target_frame_time = 1.0 / (double)fps;
}

int GetTargetFPS(int fps) 
{
    return target_fps;
}

double DeltaTime() 
{
    return delta_time;
}