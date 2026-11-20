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
layout(location = 0) in vec3 position;
layout(location = 1) in vec4 color;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 normal;
layout(location = 4) in vec4 light_parameters;
layout(location = 5) in vec4 instance_model_0;
layout(location = 6) in vec4 instance_model_1;
layout(location = 7) in vec4 instance_model_2;
layout(location = 8) in vec4 instance_model_3;
layout(location = 9) in vec4 instance_normal_0;
layout(location = 10) in vec4 instance_normal_1;
layout(location = 11) in vec4 instance_normal_2;
layout(location = 12) in vec4 instance_flags;
layout(push_constant) uniform CameraProjection {
    vec4 origin;
    vec4 right;
    vec4 up;
    vec4 forward;
    vec4 parameters;
} camera_projection;
layout(location = 0) out vec4 vertex_color;
layout(location = 1) out vec2 vertex_uv;
layout(location = 2) out float vertex_lighting;
void main() {
    vec3 surface_normal = normal.xyz;
    vec3 vertex_position = position;
    if (instance_flags.x > 0.5) {
        mat4 model = mat4(
            instance_model_0,
            instance_model_1,
            instance_model_2,
            instance_model_3
        );
        mat3 normal_matrix = mat3(
            instance_normal_0.xyz,
            instance_normal_1.xyz,
            instance_normal_2.xyz
        );
        vertex_position = (model * vec4(position, 1.0)).xyz;
        surface_normal = normalize(normal_matrix * normal.xyz);
    }
    // Sprite instances already contain clip-space transforms.
    if (instance_flags.y > 0.5) {
        gl_Position = vec4(vertex_position, 1.0);
        vertex_uv = uv;
    // The normal's spare component marks camera-space vertices.
    } else if (instance_flags.x > 0.5 || normal.w < -0.5) {
        float horizontal_extent = camera_projection.parameters.x;
        float vertical_extent = camera_projection.parameters.y;
        float near_plane = camera_projection.parameters.z;
        float far_plane = camera_projection.parameters.w;
        vec3 camera_position = vertex_position;
        if (camera_projection.origin.w > 0.5) {
            vec3 relative_position = vertex_position - camera_projection.origin.xyz;
            camera_position = vec3(
                dot(relative_position, camera_projection.right.xyz),
                dot(relative_position, camera_projection.up.xyz),
                dot(relative_position, camera_projection.forward.xyz)
            );
        }
        float camera_x = camera_position.x;
        float camera_y = camera_position.y;
        float camera_depth = camera_position.z;
        float clip_depth = far_plane * (camera_depth - near_plane) / (far_plane - near_plane);
        gl_Position = vec4(
            camera_x / horizontal_extent,
            -camera_y / vertical_extent,
            clip_depth,
            camera_depth
        );
        vertex_uv = uv;
    } else {
        gl_Position = vec4(position, 1.0);
        vertex_uv = uv;
    }
    vertex_color = color;
    if (light_parameters.w >= 1.0) {
        vertex_lighting = 1.0;
    } else {
        float diffuse = clamp(dot(surface_normal, light_parameters.xyz), 0.0, 1.0);
        vertex_lighting = light_parameters.w + (1.0 - light_parameters.w) * diffuse;
    }
}
