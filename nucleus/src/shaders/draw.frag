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

#version 450
layout(location = 0) in vec4 vertex_color;
layout(location = 1) in vec2 vertex_uv;
layout(location = 2) in float vertex_lighting;
layout(set = 0, binding = 0) uniform sampler2D source_texture;
layout(location = 0) out vec4 fragment_color;
void main() {
    vec4 sampled_color = vertex_color * texture(source_texture, vertex_uv);
    fragment_color = vec4(sampled_color.rgb * vertex_lighting, sampled_color.a);
}
