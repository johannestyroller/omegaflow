vec3 eval_perception(vec2 uv, vec2 res, float scale, vec3 center,
    vec2 rotation, vec4 obs, vec4 accel, vec4 mag, vec4 local, vec4 geo) {

    float capacity = obs.w;
    float dwell = obs.x;
    float motion = obs.y;
    float lux = obs.z;
    float acoustic = local.x;
    float camera_lux = local.y;
    float temporal_certainty = local.z;
    float locality_certainty = local.w;

    float certainty = temporal_certainty * locality_certainty;

    float cosY = cos(rotation.x); float sinY = sin(rotation.x);
    float cosP = cos(rotation.y); float sinP = sin(rotation.y);

    vec3 offset = vec3((uv.x - 0.5) * res.x * scale, (uv.y - 0.5) * res.y * scale, 0.0);
    vec3 ry = vec3(offset.x*cosY + offset.z*sinY, offset.y, -offset.x*sinY + offset.z*cosY);
    vec3 pos = center + vec3(ry.x, ry.y*cosP - ry.z*sinP, ry.y*sinP + ry.z*cosP);

    state st = eval_state(pos, capacity);

    float g = length(st.acc_gravity);
    vec3 B = st.B_field + mag.xyz * 1e-5;
    float b = length(B);

    float g_energy = g / 9.81;
    float b_energy = b / 6.0e-5;

    float surface_dist = st.dist_earth - 6378137.0 - st.terrain_h;
    float atmo = clamp(1.0 - surface_dist / 100000.0, 0.0, 1.0);

    int cam_rot = int(geo.w);
    vec2 cam_uv = vec2(uv.x, 1.0 - uv.y);
    if (cam_rot == 1) cam_uv = vec2(1.0 - uv.y, uv.x);
    else if (cam_rot == 2) cam_uv = vec2(1.0 - uv.x, uv.y);
    else if (cam_rot == 3) cam_uv = vec2(uv.y, 1.0 - uv.x);
    vec3 cam = CAMERA(cam_uv);

    float gf = g_energy * capacity * certainty;
    vec3 gravity = vec3(
        gf,
        gf * 0.4 * (1.0 - gf),
        (1.0 - gf * 0.6) * gf
    );

    vec3 magnetic = B / max(b, 1e-10) * b_energy * capacity * certainty;

    vec3 atmosphere = vec3(0.1, 0.3, 0.8) * atmo * capacity * certainty;

    float cam_weight = max(0.0, 1.0 - length(gravity) - length(atmosphere));

    return cam * cam_weight * certainty + gravity + atmosphere + magnetic;
}
