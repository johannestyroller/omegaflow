// --- VERTEX ---
#version 300 es
precision highp float;
layout(location=0) out vec2 vUv;
const vec2 pos[3] = vec2[3](vec2(-1,-1), vec2(3,-1), vec2(-1,3));
void main() {
    vec2 p = pos[gl_VertexID];
    vUv = vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    gl_Position = vec4(p, 0.0, 1.0);
}

// --- FRAGMENT ---
#version 300 es
precision highp float;
precision highp int;

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

vec2 df64_add(vec2 a, vec2 b) {
    float s = a.x + b.x;
    float v = s - a.x;
    float e = (a.x - (s - v)) + (b.x - v) + a.y + b.y;
    return vec2(s + e, e - (s + e - s));
}
vec2 df64_sub(vec2 a, vec2 b) {
    float s = a.x - b.x;
    float v = s - a.x;
    float e = (a.x - (s - v)) - (b.x + v) + a.y - b.y;
    return vec2(s + e, e - (s + e - s));
}
vec2 df64_mul(vec2 a, vec2 b) {
    float p = a.x * b.x;
    float e = ((a.x * b.x - p) + a.y * b.x + a.x * b.y) + a.y * b.y;
    return vec2(p + e, e - (p + e - p));
}
vec2 df64_scale(vec2 a, float s) {
    float p = a.x * s;
    float e = (a.x * s - p) + a.y * s;
    return vec2(p + e, e - (p + e - p));
}

float eval_gravity(vec3 pos, float capacity) {
    float mass_limit_f = 5.0 + capacity * 251.0;
    int current_mass_limit = int(mass_limit_f);
    float limit_fade = 1.0 - fract(mass_limit_f);
    vec3 acc = vec3(0.0);
    for (int i = 0; i < current_mass_limit; i++) {
        vec4 m = texelFetch(massTex, ivec2(i, 0), 0);
        vec3 r_vec = m.xyz - pos;
        float r = length(r_vec);
        float r_cubed = max(r * r * r, 1.0);
        vec3 effect = m.w * r_vec / r_cubed;
        if (i == current_mass_limit - 1 && current_mass_limit > 0) {
            acc += effect * limit_fade;
        } else {
            acc += effect;
        }
    }
    acc += device_accel.xyz;
    return length(acc);
}

float wmm_at(int idx) {
    return texelFetch(wmmTex, ivec2(idx, 0), 0).r;
}

vec3 eval_magnetic(vec3 pos, vec3 earth_center, float sin_lat, float cos_lat, float lon_rad, float capacity) {
    vec3 r_vec = pos - earth_center;
    float r = length(r_vec);
    if (r < 6378137.0 * 0.9) return vec3(0.0);

    float sin_theta = cos_lat;
    float cos_theta = sin_lat;
    float inv_sin_theta = 1.0 / max(sin_theta, 1e-6);
    float time_delta = wmm_at(3);
    float a_over_r = 6378137.0 / r;

    float mag_limit_f = 1.0 + capacity * 132.0;
    int current_mag_limit = int(mag_limit_f);
    float limit_fade = 1.0 - fract(mag_limit_f);

    if (current_mag_limit <= 12) {
        float B_r = 0.0; float B_theta = 0.0; float B_phi = 0.0;
        for (int m = 0; m <= current_mag_limit; m++) {
            float cos_m_lon = cos(float(m) * lon_rad);
            float sin_m_lon = sin(float(m) * lon_rad);
            float p_pp = 0.0; float p_pr = 0.0; float p_cu = 0.0;
            float a_r_n = pow(a_over_r, float(m + 2));
            for (int n = m; n <= current_mag_limit; n++) {
                if (n == m) {
                    if (m == 0) { p_cu = 1.0; }
                    else { p_cu = sqrt(1.0 - 1.0 / (4.0 * float(m) * float(m))) * sin_theta * p_pr; }
                } else if (n == m + 1) {
                    p_cu = cos_theta * p_pr;
                } else {
                    p_cu = (float(2*n-1) * cos_theta * p_pr - float(n+m-1) * p_pp) / float(n-m);
                }

                int idx = n * (n + 1) / 2 + m - 1;
                int ci = 4 + idx * 4;
                float g = wmm_at(ci); float h = wmm_at(ci+1);
                float g_svc = wmm_at(ci+2); float h_svc = wmm_at(ci+3);
                float g_t = g + time_delta * g_svc; float h_t = h + time_delta * h_svc;

                float ch = g_t * cos_m_lon + h_t * sin_m_lon;
                float sh = g_t * sin_m_lon - h_t * cos_m_lon;

                float fade = (n < current_mag_limit) ? 1.0 : limit_fade;

                B_r += a_r_n * float(n + 1) * p_cu * ch * fade;
                float dP = 0.0;
                if (n > m) { dP = (float(n) * cos_theta * p_cu - float(n+m) * p_pp) * inv_sin_theta; }
                else { dP = float(n) * cos_theta * p_cu * inv_sin_theta; }
                B_theta -= a_r_n * dP * ch * fade;
                B_phi += a_r_n * float(m) * p_cu * sh * inv_sin_theta * fade;

                p_pp = p_pr; p_pr = p_cu;
                a_r_n *= a_over_r;
            }
        }
        return vec3(
            B_r * sin_lat * cos(lon_rad) + B_theta * cos_lat * cos(lon_rad) - B_phi * sin(lon_rad),
            B_r * sin_lat * sin(lon_rad) + B_theta * cos_lat * sin(lon_rad) + B_phi * cos(lon_rad),
            B_r * cos_lat - B_theta * sin_lat
        );
    } else {
        vec2 B_r = vec2(0.0); vec2 B_theta = vec2(0.0); vec2 B_phi = vec2(0.0);
        for (int m = 0; m <= current_mag_limit; m++) {
            float cos_m_lon = cos(float(m) * lon_rad);
            float sin_m_lon = sin(float(m) * lon_rad);
            vec2 p_pp = vec2(0.0); vec2 p_pr = vec2(0.0); vec2 p_cu = vec2(0.0);
            vec2 a_r_n = vec2(pow(a_over_r, float(m + 2)), 0.0);
            for (int n = m; n <= current_mag_limit; n++) {
                if (n == m) {
                    if (m == 0) { p_cu = vec2(1.0, 0.0); }
                    else { p_cu = df64_mul(df64_scale(p_pr, sqrt(1.0 - 1.0 / (4.0 * float(m) * float(m)))), vec2(sin_theta, 0.0)); }
                } else if (n == m + 1) {
                    p_cu = df64_mul(p_pr, vec2(cos_theta, 0.0));
                } else {
                    p_cu = df64_sub(df64_scale(df64_mul(p_pr, vec2(cos_theta, 0.0)), float(2*n-1) / float(n-m)), df64_scale(p_pp, float(n+m-1) / float(n-m)));
                }

                int idx = n * (n + 1) / 2 + m - 1;
                int ci = 4 + idx * 4;
                float g = wmm_at(ci); float h = wmm_at(ci+1);
                float g_svc = wmm_at(ci+2); float h_svc = wmm_at(ci+3);
                float g_t = g + time_delta * g_svc; float h_t = h + time_delta * h_svc;

                float ch = g_t * cos_m_lon + h_t * sin_m_lon;
                float sh = g_t * sin_m_lon - h_t * cos_m_lon;

                float fade = (n < current_mag_limit) ? 1.0 : limit_fade;

                B_r = df64_add(B_r, df64_scale(df64_mul(p_cu, vec2(ch, 0.0)), a_r_n.x * float(n + 1) * fade));
                vec2 dP = vec2(0.0);
                if (n > m) { dP = df64_sub(df64_scale(df64_mul(p_cu, vec2(cos_theta, 0.0)), float(n) * inv_sin_theta), df64_scale(p_pp, float(n+m) * inv_sin_theta)); }
                else { dP = df64_scale(df64_mul(p_cu, vec2(cos_theta, 0.0)), float(n) * inv_sin_theta); }
                B_theta = df64_sub(B_theta, df64_scale(df64_mul(dP, vec2(ch, 0.0)), a_r_n.x * fade));
                B_phi = df64_add(B_phi, df64_scale(df64_mul(p_cu, vec2(sh, 0.0)), a_r_n.x * float(m) * inv_sin_theta * fade));

                p_pp = p_pr; p_pr = p_cu;
                a_r_n = df64_scale(a_r_n, a_over_r);
            }
        }
        float b_r_f = B_r.x; float b_theta_f = B_theta.x; float b_phi_f = B_phi.x;
        return vec3(
            b_r_f * sin_lat * cos(lon_rad) + b_theta_f * cos_lat * cos(lon_rad) - b_phi_f * sin(lon_rad),
            b_r_f * sin_lat * sin(lon_rad) + b_theta_f * cos_lat * sin(lon_rad) + b_phi_f * cos(lon_rad),
            b_r_f * cos_lat - b_theta_f * sin_lat
        );
    }
}

float eval_terrain(vec3 pos, vec3 earth_center, vec3 r_hat, float dist) {
    float earth_radius = 6378137.0;
    float terrain_fade = smoothstep(earth_radius * 1.5, earth_radius, dist);
    if (terrain_fade <= 0.0) return 0.0;

    float lat = asin(r_hat.z);
    float lon = atan(r_hat.y, r_hat.x);

    float lat0_deg = floor(lat * 57.2957795);
    float lon0_deg = floor(lon * 57.2957795);
    float local_lat = (lat * 57.2957795) - lat0_deg;
    float local_lon = (lon * 57.2957795) - lon0_deg;

    float x = local_lon * 1200.0;
    float y = (1.0 - local_lat) * 1200.0;

    int x0 = int(clamp(floor(x), 0.0, 1199.0));
    int y0 = int(clamp(floor(y), 0.0, 1199.0));
    int x1 = min(x0 + 1, 1200);
    int y1 = min(y0 + 1, 1200);

    float fx = x - float(x0);
    float fy = y - float(y0);

    float h00 = texelFetch(terrainTex, ivec2(x0, y0), 0).r;
    float h10 = texelFetch(terrainTex, ivec2(x1, y0), 0).r;
    float h01 = texelFetch(terrainTex, ivec2(x0, y1), 0).r;
    float h11 = texelFetch(terrainTex, ivec2(x1, y1), 0).r;

    float h = h00*(1.0-fx)*(1.0-fy) + h10*fx*(1.0-fy) + h01*(1.0-fx)*fy + h11*fx*fy;

    float egm_u = (lon * 57.2957795 + 180.0) * 0.0027777777;
    float egm_v = (lat * 57.2957795 + 90.0) * 0.0055555555;
    float undulation = texture(egm96Tex, vec2(egm_u, egm_v)).r;

    return (h + undulation) * terrain_fade;
}

void main() {
    float w = res_count.x;
    float h = res_count.y;
    float scale = center_scale.w;
    float yaw = rotation.x;
    float pitch = rotation.y;
    float acoustic_pressure = device_local.x;
    float local_lux = device_local.y;
    float temporal_certainty = device_local.z;
    float locality_certainty = device_local.w;
    float capacity = observer_state.w;

    float cosY = cos(yaw); float sinY = sin(yaw);
    float cosP = cos(pitch); float sinP = sin(pitch);

    vec3 offset = vec3((vUv.x - 0.5) * w * scale, (vUv.y - 0.5) * h * scale, 0.0);
    vec3 ry = vec3(offset.x*cosY + offset.z*sinY, offset.y, -offset.x*sinY + offset.z*cosY);
    vec3 rotated = vec3(ry.x, ry.y*cosP - ry.z*sinP, ry.y*sinP + ry.z*cosP);

    vec3 pos = center_scale.xyz + rotated;

    vec3 earth_center = vec3(wmm_at(0), wmm_at(1), wmm_at(2));
    vec3 r_vec = pos - earth_center;
    float dist = length(r_vec);
    vec3 r_hat = r_vec / max(dist, 1.0);
    float sin_lat = r_hat.z;
    float cos_lat = sqrt(1.0 - sin_lat * sin_lat);
    float lon_rad = atan(r_hat.y, r_hat.x);

    vec3 noise = vec3(sin(pos.x*12.9898+pos.y*78.233), cos(pos.y*43.758+pos.z*39.346), sin(pos.z*23.456+pos.x*93.138));
    float total_disturbance = observer_state.y + acoustic_pressure * 10.0 + (1.0-temporal_certainty) * 5.0 + (1.0-locality_certainty) * 5.0;
    vec3 noisy_pos = pos + noise * total_disturbance * scale * 0.01;

    float g_omega = eval_gravity(noisy_pos, capacity);
    vec3 B_universe = eval_magnetic(noisy_pos, earth_center, sin_lat, cos_lat, lon_rad, capacity);
    vec3 B_local = device_mag.xyz;
    float total_B = length(B_universe + B_local * 1e-5);

    float total_lux = observer_state.z + local_lux * 100.0;
    float omega = max(0.0, g_omega - total_lux * 0.001);

    float certainty = temporal_certainty * locality_certainty;
    float luxComp = 1.0 / (1.0 + total_lux * 0.0001);

    float g_norm = clamp(omega / 9.81, 0.0, 10.0);
    float gravity_alpha = smoothstep(0.5, 5.0, g_norm) * capacity * certainty * luxComp;

    float t = clamp(g_norm / 5.0, 0.0, 1.0);
    float r_col = smoothstep(0.2, 0.6, t);
    float g_col = smoothstep(0.0, 0.3, t) * (1.0 - smoothstep(0.6, 0.8, t));
    float b_col = 1.0 - smoothstep(0.0, 0.4, t);
    float white_add = smoothstep(0.8, 1.0, t);
    vec3 gravity_col = (vec3(r_col, g_col, b_col) + white_add) * luxComp;

    float B_norm = clamp(total_B / 6.0e-5, 0.0, 1.0);
    float mag_brightness = smoothstep(0.1, 0.6, B_norm);
    vec3 mag_glow = vec3(0.1, 0.9, 1.0) * mag_brightness * capacity * certainty * luxComp * 0.8;

    float earth_radius = 6378137.0;
    float terrain_height = eval_terrain(pos, earth_center, r_hat, dist);
    float surface_dist = dist - earth_radius - terrain_height;
    float earth_atmo = smoothstep(10000.0, 0.0, surface_dist);
    vec3 atmo_col = vec3(0.1, 0.3, 0.8);
    float atmo_alpha = earth_atmo * capacity * certainty;

    int cam_rot = int(device_geo.w);
    vec2 cam_uv = vec2(vUv.x, 1.0 - vUv.y);
    if (cam_rot == 1) cam_uv = vec2(1.0 - vUv.y, vUv.x);
    else if (cam_rot == 2) cam_uv = vec2(1.0 - vUv.x, vUv.y);
    else if (cam_rot == 3) cam_uv = vec2(vUv.y, 1.0 - vUv.x);
    vec3 cam_sample = texture(cameraTex, cam_uv).rgb;

    vec3 cam_color = cam_sample;
    float cam_lum = dot(cam_sample, vec3(0.299, 0.587, 0.114));
    if (cam_lum < 0.01) cam_color = vec3(0.02, 0.02, 0.05);

    float cam_alpha = (1.0 - gravity_alpha - atmo_alpha) * certainty;

    fragColor = vec4(cam_color * cam_alpha + gravity_col * gravity_alpha + atmo_col * atmo_alpha + mag_glow, 1.0);
}
