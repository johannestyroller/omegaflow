struct VP { 
    center_scale: vec4f, 
    res_count: vec4f, 
    observer_state: vec4f, 
    device_accel: vec4f, 
    device_mag: vec4f, 
    rotation: vec4f,
    device_local: vec4f,
    device_geo: vec4f
}

@group(0) @binding(0) var<storage, read> masses: array<vec4f>;
@group(0) @binding(1) var<uniform> vp: VP;
@group(0) @binding(2) var<storage, read> wmm: array<f32>;
@group(0) @binding(3) var terrain_raw: texture_2d<i32>;
@group(0) @binding(4) var egm96_tex: texture_2d<f32>;
@group(0) @binding(5) var camera_tex: texture_2d<f32>;
@group(0) @binding(6) var camera_sampler: sampler;

#define MASS(i) masses[i]
#define WMM(i) wmm[i]
#define TERRAIN(x,y) f32(textureLoad(terrain_raw, vec2u(u32(x), u32(y)), 0).x)
#define EGM96(u,v) textureSample(egm96_tex, camera_sampler, vec2f(u, v)).r
#define CAMERA(uv) textureSample(camera_tex, camera_sampler, uv).rgb
#define VP_FIELD(f, swizzle) f.swizzle

struct V { @builtin(position) p: vec4f, @location(0) u: vec2f }

@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    var p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    var o: V;
    o.p = vec4f(p[i], 0.0, 1.0);
    o.u = vec2f(p[i].x * 0.5 + 0.5, 0.5 - p[i].y * 0.5);
    return o;
}

STATE

PERCEPTION

@fragment fn fs(i: V) -> @location(0) vec4f {
    let perception = eval_perception(i.u, vp.res_count.xy, vp.center_scale.w, vp.center_scale.xyz,
        vp.rotation.xy, vp.observer_state, vp.device_accel, vp.device_mag, vp.device_local, vp.device_geo);
    return vec4f(perception, 1.0);
}
