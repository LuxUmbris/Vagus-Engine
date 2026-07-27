#ifndef VAGUS_ENGINE_RENDER_HPP
#define VAGUS_ENGINE_RENDER_HPP

#include "window.hpp"

void StartFrame();
void EndFrame(Window& window);
void VSync(bool enabled);
void SetTargetFPS(int fps);
int GetTargetFPS(int fps);
double DeltaTime();

#endif // VAGUS_ENGINE_RENDER_HPP