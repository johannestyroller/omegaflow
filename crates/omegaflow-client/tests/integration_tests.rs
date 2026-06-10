use axum::{
    body::Body,
    http::{Request, StatusCode, header},
    Router,
    extract::Query,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Deserialize)]
struct MassesReq { jd: f64 }

async fn index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html")], "")
}

async fn eval_state_wgsl() -> impl IntoResponse {
    let shader = include_str!("../../omegaflow-client/static/eval_state.wgsl");
    ([(header::CONTENT_TYPE, "text/wgsl")], shader)
}

async fn masses(Query(params): Query<MassesReq>) -> impl IntoResponse {
    let t = (params.jd - 2451545.0) * 86400.0;
    let masses = omegaflow_server::masses_at(t);
    let data: Vec<f32> = masses.iter().flat_map(|m| {
        [m.pos.x as f32, m.pos.y as f32, m.pos.z as f32, m.gm as f32]
    }).collect();
    let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
}

async fn wmm(Query(params): Query<MassesReq>) -> impl IntoResponse {
    let t = (params.jd - 2451545.0) * 86400.0;
    let Some(data) = omegaflow_server::wmm_at(t) else {
        return ([(header::CONTENT_TYPE, "application/octet-stream")], Vec::<u8>::new());
    };
    let n_max = data.n_max;
    let wmm_coeffs = (n_max * (n_max + 3)) / 2;
    let mut out = Vec::with_capacity(4 + 4 * wmm_coeffs as usize + 9);
    out.push(data.earth_pos.x as f32);
    out.push(data.earth_pos.y as f32);
    out.push(data.earth_pos.z as f32);
    out.push(data.time_delta);
    let pad = |v: &Vec<f32>, len: usize| -> Vec<f32> {
        let mut p = v.clone();
        p.resize(len, 0.0);
        p
    };
    out.extend(pad(&data.g_mfc, wmm_coeffs as usize));
    out.extend(pad(&data.h_mfc, wmm_coeffs as usize));
    out.extend(pad(&data.g_svc, wmm_coeffs as usize));
    out.extend(pad(&data.h_svc, wmm_coeffs as usize));
    let t_ut1 = params.jd - 2451545.0;
    let gmst_deg = 280.46061837 + 360.98564736629 * t_ut1;
    let gmst_rad = gmst_deg.to_radians();
    let cos_g = gmst_rad.cos() as f32;
    let sin_g = gmst_rad.sin() as f32;
    out.push(cos_g); out.push(sin_g); out.push(0.0);
    out.push(-sin_g); out.push(cos_g); out.push(0.0);
    out.push(0.0); out.push(0.0); out.push(1.0);
    let bytes: Vec<u8> = out.iter().flat_map(|f| f.to_le_bytes()).collect();
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
}

fn app() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/eval_state.wgsl", get(eval_state_wgsl))
        .route("/masses", get(masses))
        .route("/wmm", get(wmm))
}

async fn ensure_init() {
    static INIT: std::sync::LazyLock<tokio::sync::OnceCell<()>> = std::sync::LazyLock::new(tokio::sync::OnceCell::new);
    INIT.get_or_init(|| async {
        std::env::set_current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent().unwrap().parent().unwrap()
        ).ok();
        tokio::task::spawn_blocking(|| omegaflow_server::init()).await.ok();
    }).await;
}

#[tokio::test]
async fn test_index_status() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(ct.contains("text/html"));
}

#[tokio::test]
async fn test_eval_state_wgsl_status() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/eval_state.wgsl").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(ct.contains("text/wgsl"));
}

#[tokio::test]
async fn test_masses_status() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/masses?jd=2460000.5").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(ct.contains("application/octet-stream"));
}

#[tokio::test]
async fn test_masses_body_alignment() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/masses?jd=2460000.5").body(Body::empty()).unwrap()).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    assert!(!body.is_empty());
    assert_eq!(body.len() % 16, 0, "body len {} not multiple of 16", body.len());
}

#[tokio::test]
async fn test_masses_gm_positive() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/masses?jd=2460000.5").body(Body::empty()).unwrap()).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    let floats: Vec<f32> = body.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let count = floats.len() / 4;
    assert!(count > 0);
    for i in 0..count {
        let gm = floats[i * 4 + 3];
        assert!(gm.is_finite() && gm > 0.0, "mass[{}].gm = {}", i, gm);
    }
}

#[tokio::test]
async fn test_masses_different_epochs() {
    ensure_init().await;
    async fn fetch(jd: &str) -> Vec<u8> {
        let resp = app().oneshot(Request::builder().uri(&format!("/masses?jd={jd}")).body(Body::empty()).unwrap()).await.unwrap();
        axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap().to_vec()
    }
    let a = fetch("2460000.5").await;
    let b = fetch("2460100.5").await;
    assert_ne!(a, b);
}

#[tokio::test]
async fn test_masses_missing_jd() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/masses").body(Body::empty()).unwrap()).await.unwrap();
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_wmm_status() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/wmm?jd=2460000.5").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_wmm_body_structure() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/wmm?jd=2460000.5").body(Body::empty()).unwrap()).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    if body.is_empty() { return; }
    assert_eq!(body.len() % 4, 0);
    let floats: Vec<f32> = body.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    assert!(floats[0].is_finite());
    assert!(floats[1].is_finite());
    assert!(floats[2].is_finite());
    assert!(floats[3].is_finite());
    assert!(floats.len() >= 13);
    let len = floats.len();
    let rot = &floats[len - 9..];
    let r0 = (rot[0]*rot[0] + rot[1]*rot[1] + rot[2]*rot[2]).sqrt();
    let r1 = (rot[3]*rot[3] + rot[4]*rot[4] + rot[5]*rot[5]).sqrt();
    let r2 = (rot[6]*rot[6] + rot[7]*rot[7] + rot[8]*rot[8]).sqrt();
    assert!((r0 - 1.0).abs() < 0.01, "row0 len = {}", r0);
    assert!((r1 - 1.0).abs() < 0.01, "row1 len = {}", r1);
    assert!((r2 - 1.0).abs() < 0.01, "row2 len = {}", r2);
}

#[tokio::test]
async fn test_wmm_missing_jd() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/wmm").body(Body::empty()).unwrap()).await.unwrap();
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_unknown_route() {
    ensure_init().await;
    let resp = app().oneshot(Request::builder().uri("/nonexistent").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_server_masses_at_j2000() {
    ensure_init().await;
    let masses = omegaflow_server::masses_at(0.0);
    assert!(!masses.is_empty());
    for (i, m) in masses.iter().enumerate() {
        assert!(m.pos.is_finite(), "mass[{}].pos = {:?}", i, m.pos);
        assert!(m.gm.is_finite() && m.gm > 0.0, "mass[{}].gm = {}", i, m.gm);
    }
}

#[tokio::test]
async fn test_server_wmm_at_epoch() {
    ensure_init().await;
    let t = (2460000.5 - 2451545.0) * 86400.0;
    let Some(data) = omegaflow_server::wmm_at(t) else { return; };
    assert!(data.earth_pos.is_finite());
    assert!(data.time_delta.is_finite());
    assert!(data.n_max > 0);
    assert!(!data.g_mfc.is_empty());
}
