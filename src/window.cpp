#include <SDL2/SDL.h>
#include <glad/glad.h>
#include "window.hpp"

void InitWindow(int width, int height) 
{
    SDL_Init(SDL_INIT_VIDEO);
    SDL_Window* window = SDL_CreateWindow("Title", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, width, height, SDL_WINDOW_OPENGL);
    SDL_GLContext context = SDL_GL_CreateContext(window);
    gladLoadGLLoader((GLADloadproc)SDL_GL_GetProcAddress);
}

void InitWindowFullscreen(bool borderless) 
{
    SDL_Init(SDL_INIT_VIDEO);
    Uint32 flags = SDL_WINDOW_OPENGL | (borderless ? SDL_WINDOW_BORDERLESS : 0);
    SDL_Window* window = SDL_CreateWindow("Title", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, 0, 0, flags | SDL_WINDOW_FULLSCREEN_DESKTOP);
    SDL_GLContext context = SDL_GL_CreateContext(window);
    gladLoadGLLoader((GLADloadproc)SDL_GL_GetProcAddress);
}

void SetWindowFullscreen(bool borderless)
{
    SDL_Window* window = SDL_GL_GetCurrentWindow();
    if (window) 
    {
        Uint32 flags = SDL_WINDOW_OPENGL | (borderless ? SDL_WINDOW_BORDERLESS : 0);
        SDL_SetWindowFullscreen(window, flags | SDL_WINDOW_FULLSCREEN_DESKTOP);
    }
}

void SetWindowSize(int width, int height)
{
    SDL_Window* window = SDL_GL_GetCurrentWindow();
    if (window) 
    {
        SDL_SetWindowSize(window, width, height);
    }
}

void SetWindowTitle(const char* title) 
{
    SDL_Window* window = SDL_GL_GetCurrentWindow();
    if (window) 
    {
        SDL_SetWindowTitle(window, title);
    }
}

void SetWindowPosition(int x, int y) 
{
    SDL_Window* window = SDL_GL_GetCurrentWindow();
    if (window) 
    {
        SDL_SetWindowPosition(window, x, y);
    }
}

void GetWindowPosition(int* x, int* y) 
{
    SDL_Window* window = SDL_GL_GetCurrentWindow();
    if (window) 
    {
        SDL_GetWindowPosition(window, x, y);
    }
}