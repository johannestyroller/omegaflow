use wasm_bindgen::prelude::*;
use std::sync::OnceLock;

struct OmegaBody {
    target: u32,
    #[allow(dead_code)]
    center: u32,
    start: f64,
    end: f64,
    domain_radius: f64,
    records: Vec<ChebRecord>,
}

struct ChebRecord {
    midpoint: f64,
    radius: f64,
    n_coeffs: usize,
    coeffs: Vec<f64>,
}

struct Pool {
    bodies: Vec<OmegaBody>,
}

static POOL: OnceLock<Pool> = OnceLock::new();

fn read_u32(d: &[u8], o: &mut usize) -> u32 {
    let v = u32::from_le_bytes(d[*o..*o+4].try_into().unwrap());
    *o += 4;
    v
}

fn read_f64(d: &[u8], o: &mut usize) -> f64 {
    let v = f64::from_le_bytes(d[*o..*o+8].try_into().unwrap());
    *o += 8;
    v
}

#[wasm_bindgen]
pub fn init(omega_bytes: &[u8]) -> bool {
    if omega_bytes.len() < 10 || &omega_bytes[0..5] != b"OMEGA" {
        return false;
    }
    let mut o = 6;
    let tag = omega_bytes[o];
    o += 1;
    if tag != b's' { return false; }

    let mut bodies = Vec::new();
    let n_bodies = read_u32(omega_bytes, &mut o) as usize;
    for _ in 0..n_bodies {
        let target = read_u32(omega_bytes, &mut o);
        let center = read_u32(omega_bytes, &mut o);
        let start = read_f64(omega_bytes, &mut o);
        let end = read_f64(omega_bytes, &mut o);
        let domain_radius = read_f64(omega_bytes, &mut o);
        let n_records = read_u32(omega_bytes, &mut o) as usize;
        let mut records = Vec::with_capacity(n_records);
        for _ in 0..n_records {
            let midpoint = read_f64(omega_bytes, &mut o);
            let radius = read_f64(omega_bytes, &mut o);
            let nc = read_u32(omega_bytes, &mut o) as usize;
            let mut coeffs = Vec::with_capacity(nc * 3);
            for _ in 0..nc*3 {
                coeffs.push(read_f64(omega_bytes, &mut o));
            }
            records.push(ChebRecord { midpoint, radius, n_coeffs: nc, coeffs });
        }
        bodies.push(OmegaBody { target, center, start, end, domain_radius, records });
    }

    let _ = POOL.set(Pool { bodies });
    true
}

fn clenshaw(t: f64, coeffs: &[f64]) -> f64 {
    let n = coeffs.len();
    if n == 0 { return 0.0; }
    if n == 1 { return coeffs[0]; }
    let mut b2 = 0.0f64;
    let mut b1 = 0.0f64;
    let two_t = 2.0 * t;
    for i in (1..n).rev() {
        let b0 = coeffs[i] + two_t * b1 - b2;
        b2 = b1;
        b1 = b0;
    }
    coeffs[0] + t * b1 - b2
}

fn eval_body(body: &OmegaBody, t: f64) -> Option<(f64, f64, f64)> {
    if t < body.start || t > body.end { return None; }
    let records = &body.records;
    let mut lo = 0usize;
    let mut hi = records.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let half = records[mid].radius;
        if t < records[mid].midpoint - half {
            hi = mid;
        } else if t > records[mid].midpoint + half {
            lo = mid + 1;
        } else {
            lo = mid;
            break;
        }
    }
    if lo >= records.len() { lo = records.len() - 1; }
    let rec = &records[lo];
    let normalized = (t - rec.midpoint) / rec.radius;
    let nc = rec.n_coeffs;
    let x = clenshaw(normalized, &rec.coeffs[0..nc]);
    let y = clenshaw(normalized, &rec.coeffs[nc..2*nc]);
    let z = clenshaw(normalized, &rec.coeffs[2*nc..3*nc]);
    Some((x, y, z))
}

const GM_SUN: f64 = 1.32712440041939400e20;
const GM_MERCURY: f64 = 2.20318685514000000e13;
const GM_VENUS: f64 = 3.24858592000000000e14;
const GM_EARTH_MOON: f64 = 4.03503235502259700e14;
const GM_MARS: f64 = 4.28283736206990900e13;
const GM_JUPITER: f64 = 1.26712764100000000e17;
const GM_SATURN: f64 = 3.79405852400000000e16;
const GM_URANUS: f64 = 5.79454900700000000e15;
const GM_NEPTUNE: f64 = 6.83652710058002400e15;
const GM_PLUTO: f64 = 9.75500000000000000e11;
const GM_MOON: f64 = 4.90279821844000000e12;
const GM_EARTH: f64 = 3.98600435507000000e14;

fn gm_for(target: u32) -> f64 {
    match target {
        10 => GM_SUN,
        1 => GM_EARTH_MOON * 0.0,
        199 => GM_MERCURY,
        299 => GM_VENUS,
        3 => GM_EARTH_MOON,
        4 => GM_MARS,
        5 => GM_JUPITER,
        6 => GM_SATURN,
        7 => GM_URANUS,
        8 => GM_NEPTUNE,
        9 => GM_PLUTO,
        301 => GM_MOON,
        399 => GM_EARTH,
        _ => 0.0,
    }
}

#[wasm_bindgen]
pub fn masses_at(t_sec: f64, cx: f64, cy: f64, cz: f64, scale: f64) -> Vec<f32> {
    let Some(pool) = POOL.get() else { return Vec::new() };
    let mut out = Vec::new();
    for body in &pool.bodies {
        let gm = gm_for(body.target);
        if gm == 0.0 { continue; }
        if t_sec < body.start || t_sec > body.end { continue; }
        let Some((x, y, z)) = eval_body(body, t_sec) else { continue; };
        let px = x * 1e3;
        let py = y * 1e3;
        let pz = z * 1e3;
        let dist = ((px - cx).powi(2) + (py - cy).powi(2) + (pz - cz).powi(2)).sqrt();
        let domain = if body.domain_radius > 0.0 { body.domain_radius } else { scale * 10.0 };
        if dist > domain { continue; }
        out.push(px as f32);
        out.push(py as f32);
        out.push(pz as f32);
        out.push(gm as f32);
    }
    out
}

#[wasm_bindgen]
pub fn observer_potential(t_sec: f64, cx: f64, cy: f64, cz: f64) -> f64 {
    let Some(pool) = POOL.get() else { return 0.0 };
    let mut potential = 0.0;
    for body in &pool.bodies {
        let gm = gm_for(body.target);
        if gm == 0.0 { continue; }
        if t_sec < body.start || t_sec > body.end { continue; }
        let Some((x, y, z)) = eval_body(body, t_sec) else { continue; };
        let px = x * 1e3;
        let py = y * 1e3;
        let pz = z * 1e3;
        let dist = ((px - cx).powi(2) + (py - cy).powi(2) + (pz - cz).powi(2)).sqrt();
        let domain = if body.domain_radius > 0.0 { body.domain_radius } else { 1e15 };
        if dist > domain { continue; }
        if dist > 1.0 {
            potential -= gm / dist;
        }
    }
    potential
}

#[wasm_bindgen]
pub fn jd_now() -> f64 {
    js_sys::Date::now() / 86400000.0 + 2440587.5
}
