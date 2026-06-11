#version 300 es
precision highp float;
precision highp int;

#define MASS(i) texelFetch(massTex, ivec2(i, 0), 0)
#define WMM(i) texelFetch(wmmTex, ivec2(i, 0), 0).r
#define TERRAIN(x,y) texelFetch(terrainTex, ivec2(x, y), 0).r
#define EGM96(u,v) texture(egm96Tex, vec2(u, v)).r
#define CAMERA(uv) texture(cameraTex, uv).rgb
#define VP_FIELD(f, swizzle) f.swizzle

layout(std140) uniform VP {
    vec4 center_scale;
    vec4 res_count;
    vec4 observer_state;
    vec4 device_accel;
    vec4 device_mag;
    vec4 rotation;
    vec4 device_local;
    vec4 device_geo;
};

uniform sampler2D massTex;
uniform sampler2D wmmTex;
uniform sampler2D terrainTex;
uniform sampler2D egm96Tex;
uniform sampler2D cameraTex;

layout(location=0) out vec4 fragColor;
layout(location=0) in vec2 vUv;

STATE

PERCEPTION

void main() {
    vec3 perception = eval_perception(vUv, res_count.xy, center_scale.w, center_scale.xyz,
        rotation.xy, observer_state, device_accel, device_mag, device_local, device_geo);
    fragColor = vec4(perception, 1.0);
}
