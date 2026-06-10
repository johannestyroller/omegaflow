```wgsl
override HARDWARE_TIER: i32 = 0;
override N_MAX: i32 = 12; 
const LEGENDRE_ARRAY_SIZE: i32 = 105;

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
@group(0) @binding(3) var terrain_tex: texture_2d<f32>;
@group(0) @binding(4) var camera_tex: texture_2d<f32>;
@group(0) @binding(5) var camera_sampler: sampler;

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
    acc += vp.device_accel.xyz;
    return length(acc);
}

fn eval_magnetic_state(pos: vec3f) -> vec3f {
    let earth_center = vec3f(wmm[0], wmm[1], wmm[2]);
    let dipole_dir = vec3f(wmm[3], wmm[4], wmm[5]);
    
    let r_vec = pos - earth_center;
    let r = length(r_vec);
    let earth_radius = 6378137.0;
    
    if (r < earth_radius * 0.9) { return vec3f(0.0); }
    
    let r_hat = r_vec / r;
    let m = dipole_dir * 7.94e22; 
    
    let B = (3.0 * dot(m, r_hat) * r_hat - m) / pow(r, 3.0);
    return B;
}

@fragment fn fs(i: V) -> @location(0) vec4f {
    let w = vp.res_count.x;
    let h = vp.res_count.y;
    let scale = vp.center_scale.w;
    let yaw = vp.rotation.x;
    let pitch = vp.rotation.y;
    let acoustic_pressure = vp.device_local.x;
    let local_lux = vp.device_local.y;
    let temporal_certainty = vp.device_local.z;
    let locality_certainty = vp.device_local.w;

    let cosY = cos(yaw); let sinY = sin(yaw);
    let cosP = cos(pitch); let sinP = sin(pitch);

    let offset = vec3f((i.u.x - 0.5) * w * scale, (i.u.y - 0.5) * h * scale, 0.0);
    let rotated_y = vec3f(offset.x * cosY + offset.z * sinY, offset.y, -offset.x * sinY + offset.z * cosY);
    let rotated = vec3f(rotated_y.x, rotated_y.y * cosP - rotated_y.z * sinP, rotated_y.y * sinP + rotated_y.z * cosP);

    let pos = vec3f(vp.center_scale.x, vp.center_scale.y, vp.center_scale.z) + rotated;

    let dwellTime = vp.observer_state.x;
    let motion = vp.observer_state.y;
    let lux = vp.observer_state.z;
    let awareness = vp.observer_state.w;

    let noise = vec3f(sin(pos.x * 12.9898 + pos.y * 78.233), cos(pos.y * 43.758 + pos.z * 39.346), sin(pos.z * 23.456 + pos.x * 93.138));
    
    let total_disturbance = motion + acoustic_pressure * 10.0 + (1.0 - temporal_certainty) * 5.0 + (1.0 - locality_certainty) * 5.0;
    let noisy_pos = pos + noise * total_disturbance * scale * 0.01; 

    let g_omega = eval_gravitational_state(noisy_pos);

    let B_universe = eval_magnetic_state(noisy_pos);
    let B_local = vp.device_mag.xyz;
    let total_B = length(B_universe + B_local * 1e-5);
    
    let total_lux = lux + local_lux * 100.0;
    let omega = max(0.0, g_omega - total_lux * 0.001);

    let certainty = temporal_certainty * locality_certainty;
    let luxCompensation = 1.0 / (1.0 + total_lux * 0.0001);

    let g_norm = clamp(omega / 9.81, 0.0, 10.0);
    let gravity_alpha = smoothstep(0.5, 5.0, g_norm) * awareness * certainty * luxCompensation;
    let r = smoothstep(0.0, 0.5, g_norm) + smoothstep(0.8, 1.0, g_norm) * 0.5;
    let g = smoothstep(0.1, 0.8, g_norm);
    let b = smoothstep(0.0, 0.2, g_norm) * (1.0 - smoothstep(0.5, 1.0, g_norm));
    let gravity_col = vec3f(r, g, b) * luxCompensation;

    let B_norm = clamp(total_B / 6.0e-5, 0.0, 1.0);
    let mag_brightness = smoothstep(0.1, 0.6, B_norm);
    let mag_glow = vec3f(0.1, 0.9, 1.0) * mag_brightness * awareness * certainty * luxCompensation * 0.8;

    let earth_center = vec3f(wmm[0], wmm[1], wmm[2]);
    let dist_from_earth = length(pos - earth_center);
    let earth_radius = 6378137.0;
    let earth_atmo = smoothstep(1.02, 1.0, dist_from_earth / earth_radius);
    var atmo_col = vec3f(0.0);
    var atmo_alpha = 0.0;
    if (dist_from_earth < earth_radius * 1.02) {
        atmo_col = vec3f(0.1, 0.3, 0.8);
        atmo_alpha = earth_atmo * awareness * certainty;
    }

    let cam_rot = i32(vp.device_geo.w);
    var cam_uv = vec2f(i.u.x, 1.0 - i.u.y);
    if (cam_rot == 1) { cam_uv = vec2f(1.0 - i.u.y, i.u.x); }
    else if (cam_rot == 2) { cam_uv = vec2f(1.0 - i.u.x, i.u.y); }
    else if (cam_rot == 3) { cam_uv = vec2f(i.u.y, 1.0 - i.u.x); }
    let cam_color = textureSample(camera_tex, camera_sampler, cam_uv).rgb;
    let cam_alpha = (1.0 - gravity_alpha - atmo_alpha) * certainty;

    let final_output = cam_color * cam_alpha + gravity_col * gravity_alpha + atmo_col * atmo_alpha + mag_glow;

    return vec4f(final_output, 1.0);
}

