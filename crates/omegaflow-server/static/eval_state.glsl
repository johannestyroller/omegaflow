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

struct state {
    vec3 pos;
    vec3 acc_gravity;
    vec3 B_field;
    float terrain_h;
    float dist_earth;
    vec3 r_hat;
    float sin_lat;
    float cos_lat;
    float lon_rad;
    vec3 earth_center;
};

state eval_state(vec3 pos, float capacity) {
    state st;
    st.pos = pos;
    st.earth_center = vec3(WMM(0), WMM(1), WMM(2));
    vec3 r_vec = pos - st.earth_center;
    st.dist_earth = length(r_vec);
    st.r_hat = r_vec / max(st.dist_earth, 1.0);
    st.sin_lat = st.r_hat.z;
    st.cos_lat = sqrt(1.0 - st.sin_lat * st.sin_lat);
    st.lon_rad = atan(st.r_hat.y, st.r_hat.x);

    int mass_limit = int(capacity * 256.0);
    float mass_fade = 1.0 - fract(capacity * 256.0);
    st.acc_gravity = vec3(0.0);
    for (int i = 0; i < mass_limit; i++) {
        vec4 m = MASS(i);
        vec3 r = m.xyz - pos;
        float r3 = max(dot(r, r) * length(r), 1.0);
        vec3 effect = m.w * r / r3;
        if (i == mass_limit - 1 && mass_limit > 0) {
            st.acc_gravity += effect * mass_fade;
        } else {
            st.acc_gravity += effect;
        }
    }

    int mag_limit = int(capacity * 133.0);
    float mag_fade = 1.0 - fract(capacity * 133.0);
    float sin_theta = st.cos_lat;
    float cos_theta = st.sin_lat;
    float inv_sin_theta = 1.0 / max(sin_theta, 1e-6);
    float time_delta = WMM(3);
    float a_over_r = 6378137.0 / st.dist_earth;

    if (mag_limit <= 12) {
        float B_r = 0.0; float B_t = 0.0; float B_p = 0.0;
        for (int mm = 0; mm <= mag_limit; mm++) {
            float cml = cos(float(mm) * st.lon_rad);
            float sml = sin(float(mm) * st.lon_rad);
            float p_pp = 0.0; float p_pr = 0.0; float p_cu = 0.0;
            float arn = pow(a_over_r, float(mm + 2));
            for (int n = mm; n <= mag_limit; n++) {
                if (n == mm) {
                    if (mm == 0) { p_cu = 1.0; }
                    else { p_cu = sqrt(1.0 - 1.0 / (4.0 * float(mm) * float(mm))) * sin_theta * p_pr; }
                } else if (n == mm + 1) {
                    p_cu = cos_theta * p_pr;
                } else {
                    p_cu = (float(2*n-1) * cos_theta * p_pr - float(n+mm-1) * p_pp) / float(n-mm);
                }
                int ci = 4 + (n*(n+1)/2+mm-1)*4;
                float gt = WMM(ci) + time_delta * WMM(ci+2);
                float ht = WMM(ci+1) + time_delta * WMM(ci+3);
                float ch = gt*cml + ht*sml;
                float sh = gt*sml - ht*cml;
                float fd = (n < mag_limit) ? 1.0 : mag_fade;
                B_r += arn * float(n+1) * p_cu * ch * fd;
                float dP = (n > mm) ? (float(n)*cos_theta*p_cu - float(n+mm)*p_pp)*inv_sin_theta : float(n)*cos_theta*p_cu*inv_sin_theta;
                B_t -= arn * dP * ch * fd;
                B_p += arn * float(mm) * p_cu * sh * inv_sin_theta * fd;
                p_pp = p_pr; p_pr = p_cu;
                arn *= a_over_r;
            }
        }
        st.B_field = vec3(
            B_r*st.sin_lat*cos(st.lon_rad) + B_t*st.cos_lat*cos(st.lon_rad) - B_p*sin(st.lon_rad),
            B_r*st.sin_lat*sin(st.lon_rad) + B_t*st.cos_lat*sin(st.lon_rad) + B_p*cos(st.lon_rad),
            B_r*st.cos_lat - B_t*st.sin_lat
        );
    } else {
        vec2 B_r = vec2(0.0); vec2 B_t = vec2(0.0); vec2 B_p = vec2(0.0);
        for (int mm = 0; mm <= mag_limit; mm++) {
            float cml = cos(float(mm) * st.lon_rad);
            float sml = sin(float(mm) * st.lon_rad);
            vec2 p_pp = vec2(0.0); vec2 p_pr = vec2(0.0); vec2 p_cu = vec2(0.0);
            vec2 arn = vec2(pow(a_over_r, float(mm+2)), 0.0);
            for (int n = mm; n <= mag_limit; n++) {
                if (n == mm) {
                    if (mm == 0) { p_cu = vec2(1.0, 0.0); }
                    else { p_cu = df64_mul(df64_scale(p_pr, sqrt(1.0-1.0/(4.0*float(mm)*float(mm)))), vec2(sin_theta,0.0)); }
                } else if (n == mm + 1) {
                    p_cu = df64_mul(p_pr, vec2(cos_theta, 0.0));
                } else {
                    p_cu = df64_sub(df64_scale(df64_mul(p_pr, vec2(cos_theta,0.0)), float(2*n-1)/float(n-mm)), df64_scale(p_pp, float(n+mm-1)/float(n-mm)));
                }
                int ci = 4 + (n*(n+1)/2+mm-1)*4;
                float gt = WMM(ci) + time_delta * WMM(ci+2);
                float ht = WMM(ci+1) + time_delta * WMM(ci+3);
                float ch = gt*cml + ht*sml;
                float sh = gt*sml - ht*cml;
                float fd = (n < mag_limit) ? 1.0 : mag_fade;
                B_r = df64_add(B_r, df64_scale(df64_mul(p_cu, vec2(ch,0.0)), arn.x*float(n+1)*fd));
                vec2 dP = (n > mm) ? df64_sub(df64_scale(df64_mul(p_cu,vec2(cos_theta,0.0)),float(n)*inv_sin_theta), df64_scale(p_pp,float(n+mm)*inv_sin_theta)) : df64_scale(df64_mul(p_cu,vec2(cos_theta,0.0)),float(n)*inv_sin_theta);
                B_t = df64_sub(B_t, df64_scale(df64_mul(dP,vec2(ch,0.0)), arn.x*fd));
                B_p = df64_add(B_p, df64_scale(df64_mul(p_cu,vec2(sh,0.0)), arn.x*float(mm)*inv_sin_theta*fd));
                p_pp = p_pr; p_pr = p_cu;
                arn = df64_scale(arn, a_over_r);
            }
        }
        st.B_field = vec3(
            B_r.x*st.sin_lat*cos(st.lon_rad) + B_t.x*st.cos_lat*cos(st.lon_rad) - B_p.x*sin(st.lon_rad),
            B_r.x*st.sin_lat*sin(st.lon_rad) + B_t.x*st.cos_lat*sin(st.lon_rad) + B_p.x*cos(st.lon_rad),
            B_r.x*st.cos_lat - B_t.x*st.sin_lat
        );
    }

    float earth_radius = 6378137.0;
    float terrain_fade = smoothstep(earth_radius * 1.3, earth_radius, st.dist_earth);
    if (terrain_fade <= 0.0) {
        st.terrain_h = 0.0;
    } else {
        float lat = asin(st.r_hat.z);
        float lon = atan(st.r_hat.y, st.r_hat.x);
        float x = fract(lon * 57.2957795) * 1200.0;
        float y = (1.0 - fract(lat * 57.2957795)) * 1200.0;
        int x0 = int(clamp(floor(x), 0.0, 1199.0));
        int y0 = int(clamp(floor(y), 0.0, 1199.0));
        float fx = x - float(x0); float fy = y - float(y0);
        float h = TERRAIN(x0,y0)*(1.0-fx)*(1.0-fy) + TERRAIN(min(x0+1,1200),y0)*fx*(1.0-fy)
                + TERRAIN(x0,min(y0+1,1200))*(1.0-fx)*fy + TERRAIN(min(x0+1,1200),min(y0+1,1200))*fx*fy;
        float undulation = EGM96((lon*57.2957795+180.0)*0.0027777777, (lat*57.2957795+90.0)*0.0055555555);
        st.terrain_h = (h + undulation) * terrain_fade;
    }

    return st;
}
