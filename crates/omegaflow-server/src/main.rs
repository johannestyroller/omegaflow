use axum::extract::Query;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

#[derive(Deserialize)]
struct StreamReq { 
    jd: f64, cx: f64, cy: f64, cz: f64, scale: f64, 
    min_g: f32, n_max: i32, lat0: i32, lon0: i32 
}

async fn index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html")], include_str!("../static/index.html"))
}

async fn client_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], include_str!("../static/client.js"))
}

async fn webgpu_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], include_str!("../static/webgpu.js"))
}

async fn webgl_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], include_str!("../static/webgl.js"))
}

fn glsl_to_wgsl(glsl: &str) -> String {
    let mut s = glsl.to_string();
    s = s.replace("float ", "f32 ");
    s = s.replace("float(", "f32(");
    s = s.replace("vec2 ", "vec2f ");
    s = s.replace("vec2(", "vec2f(");
    s = s.replace("vec3 ", "vec3f ");
    s = s.replace("vec3(", "vec3f(");
    s = s.replace("vec4 ", "vec4f ");
    s = s.replace("vec4(", "vec4f(");
    s = s.replace("int ", "i32 ");
    s = s.replace("int(", "i32(");
    s = s.replace("uint(", "u32(");
    s = s.replace("const ", "let ");
    s
}

async fn eval_state_wgsl() -> impl IntoResponse {


    let state = glsl_to_wgsl(EVAL_STATE);
    let perception = glsl_to_wgsl(EVAL_PERCEPTION);
    let shader = DICT_WGSL.replace("STATE", &state).replace("PERCEPTION", &perception);
    ([(header::CONTENT_TYPE, "text/wgsl")], shader)
}

async fn eval_state_glsl() -> impl IntoResponse {
    let vertex = r#"#version 300 es
precision highp float;
layout(location=0) out vec2 vUv;
const vec2 pos[3] = vec2[3](vec2(-1,-1), vec2(3,-1), vec2(-1,3));
void main() {
    vec2 p = pos[gl_VertexID];
    vUv = vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    gl_Position = vec4(p, 0.0, 1.0);
}"#;
    let dict = DICT_GLSL.replace("STATE", EVAL_STATE).replace("PERCEPTION", EVAL_PERCEPTION);
    let fragment = format!("{}\n{}", vertex, dict);
    ([(header::CONTENT_TYPE, "text/glsl")], fragment)
}

async fn universe_stream(Query(params): Query<StreamReq>) -> impl IntoResponse {
    let t = (params.jd - 2451545.0) * 86400.0;
    let viewport_center = glam::DVec3::new(params.cx, params.cy, params.cz);
    
    let mut masses = omegaflow_core::masses_at(t, params.cx, params.cy, params.cz, params.scale);
    masses.sort_by(|a, b| b.gm.partial_cmp(&a.gm).unwrap_or(std::cmp::Ordering::Equal));
    masses.retain(|m| { let r2 = (m.pos - viewport_center).length_squared().max(1.0); (m.gm / r2) > params.min_g as f64 });

    let mass_data: Vec<f32> = masses.iter().flat_map(|m| {
        [m.pos.x as f32, m.pos.y as f32, m.pos.z as f32, m.gm as f32]
    }).collect();
    let mass_bytes: Vec<u8> = mass_data.iter().flat_map(|f| f.to_le_bytes()).collect();

    let wmm_bytes = match omegaflow_core::almanac().and_then(|alm| omegaflow_core::wmm_at(t, alm)) {
        Some(data) => {
            let effective_n_max = params.n_max.min(data.n_max);
            let wmm_coeffs = (effective_n_max * (effective_n_max + 3)) / 2;
            let mut out = Vec::new();
            out.extend_from_slice(&[data.earth_pos.x as f32, data.earth_pos.y as f32, data.earth_pos.z as f32].iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());
            out.extend_from_slice(&data.time_delta.to_le_bytes());
            for i in 0..wmm_coeffs as usize {
                let g = *data.g_mfc.get(i).unwrap_or(&0.0);
                let h = *data.h_mfc.get(i).unwrap_or(&0.0);
                let g_s = *data.g_svc.get(i).unwrap_or(&0.0);
                let h_s = *data.h_svc.get(i).unwrap_or(&0.0);
                out.extend_from_slice(&g.to_le_bytes());
                out.extend_from_slice(&h.to_le_bytes());
                out.extend_from_slice(&g_s.to_le_bytes());
                out.extend_from_slice(&h_s.to_le_bytes());
            }
            out
        },
        None => Vec::new()
    };

    let terrain_bytes = omegaflow_core::raw_hgt_tile(params.lat0, params.lon0);
    let egm_bytes = omegaflow_core::raw_egm96();

    let mut stream = Vec::new();
    stream.extend_from_slice(&(mass_bytes.len() as u32).to_le_bytes());
    stream.extend_from_slice(&(wmm_bytes.len() as u32).to_le_bytes());
    stream.extend_from_slice(&(terrain_bytes.len() as u32).to_le_bytes());
    stream.extend_from_slice(&(egm_bytes.len() as u32).to_le_bytes());
    
    stream.extend(mass_bytes);
    stream.extend(wmm_bytes);
    stream.extend(terrain_bytes);
    stream.extend(egm_bytes);

    ([(header::CONTENT_TYPE, "application/octet-stream")], stream)
}

async fn time() -> impl IntoResponse {
    let jd = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() / 86400.0 + 2440587.5;
    ([(header::CONTENT_TYPE, "text/plain")], jd.to_string())
}

async fn manifest() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/json")], MANIFEST)
}

async fn service_worker() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], SW)
}

#[tokio::main]
async fn main() {
    tokio::task::spawn_blocking(|| omegaflow_core::init()).await.ok();
    let app = Router::new()
        .route("/", get(index))
        .route("/eval_state.wgsl", get(eval_state_wgsl))
        .route("/eval_state.glsl", get(eval_state_glsl))
        .route("/stream", get(universe_stream))
        .route("/time", get(time))
        .route("/manifest.json", get(manifest))
        .route("/sw.js", get(service_worker))
        .route("/client.js", get(client_js))
        .route("/webgpu.js", get(webgpu_js))
        .route("/webgl.js", get(webgl_js));
    let port: u16 = std::env::var("PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(80);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

static EVAL_STATE: &str = include_str!("../static/eval_state.glsl");
static EVAL_PERCEPTION: &str = include_str!("../static/eval_perception.glsl");
static DICT_GLSL: &str = include_str!("../static/dict_glsl.glsl");
static DICT_WGSL: &str = include_str!("../static/dict_wgsl.wgsl");
static MANIFEST: &str = include_str!("../static/manifest.json");
static SW: &str = include_str!("../static/sw.js");
