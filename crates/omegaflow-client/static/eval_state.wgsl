override HARDWARE_TIER: i32 = 0;
override N_MAX: i32 = 12; 
const LEGENDRE_ARRAY_SIZE: i32 = 105;

struct VP { center_scale: vec4f, res_count: vec4f }

@group(0) @binding(0) var<storage, read> masses: array<vec4f>;
@group(0) @binding(1) var<uniform> vp: VP;
@group(0) @binding(2) var<storage, read> wmm: array<f32>;

var<private> P: array<f32, LEGENDRE_ARRAY_SIZE>;

struct V { @builtin(position) p: vec4f, @location(0) u: vec2f }

@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    var p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    var o: V;
    o.p = vec4f(p[i], 0.0, 1.0);
    o.u = vec2f(p[i].x * 0.5 + 0.5, 0.5 - p[i].y * 0.5);
    return o;
}

fn eval_gravitational_state(pos: vec3f) -> f32 {
    var acc = vec3f(0.0);
    for (var i: i32 = 0; i < i32(vp.res_count.z); i++) {
        let r_vec = masses[i].xyz - pos;
        let r2_s = max(dot(r_vec, r_vec), 1.0);
        acc += masses[i].w * r_vec / (r2_s * sqrt(r2_s));
    }
    return length(acc);
}

fn eval_magnetic_state(pos: vec3f) -> vec3f {
    if (HARDWARE_TIER < 2) { return vec3f(0.0); }
    return vec3f(0.0);
}

@fragment fn fs(i: V) -> @location(0) vec4f {
    let w = vp.res_count.x;
    let h = vp.res_count.y;
    let scale = vp.center_scale.w;
    let pos = vec3f(vp.center_scale.x + (i.u.x - 0.5) * w * scale, vp.center_scale.y - (i.u.y - 0.5) * h * scale, vp.center_scale.z);

    let g_omega = eval_gravitational_state(pos);
    let B = eval_magnetic_state(pos);
    let omega = g_omega + length(B);

    let brightness = clamp(log(1.0 + omega / 9.81) / log(11.0), 0.0, 1.0);
    let B_norm = clamp(length(B) / 65000.0, 0.0, 1.0);
    let g_norm = clamp(g_omega / 9.81, 0.0, 1.0);

    let grav_col = mix(vec3f(0.0, 0.1, 0.8), vec3f(1.0, 0.9, 0.0), g_norm * g_norm);
    let mag_col = vec3f(B_norm, 0.0, B_norm * 0.7);
    let final_col = mix(grav_col, mag_col, B_norm * 0.6);

    return vec4f(final_col * brightness, 1.0);
}

