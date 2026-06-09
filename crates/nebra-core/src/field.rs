use glam::DVec3;
use std::sync::OnceLock;
use anise::prelude::*;
use anise::constants::frames::SSB_J2000;
use hifitime::Epoch;

static ALMANAC: OnceLock<Almanac> = OnceLock::new();
static MASS_IDS: OnceLock<Vec<i32>> = OnceLock::new();

pub struct Mass {
    pub pos: DVec3,
    pub gm: f64,
}

pub fn init() {
    let alm = Almanac::new("data/de440s.bsp")
        .and_then(|a| a.load("data/pck08.pca"));
    if let Ok(alm) = alm {
        let ids: Vec<i32> = (0..1000)
            .filter(|&id| {
                let frame = Frame::from_ephem_j2000(id);
                alm.frame_info(frame).ok().and_then(|f| f.mu_km3_s2).is_some()
            })
            .collect();
        let _ = MASS_IDS.set(ids);
        let _ = ALMANAC.set(alm);
    }
}

pub fn masses_at(jd: f64) -> Vec<Mass> {
    let Some(alm) = ALMANAC.get() else { return Vec::new() };
    let Some(ids) = MASS_IDS.get() else { return Vec::new() };
    let epoch = Epoch::from_tdb_seconds((jd - 2451545.0) * 86400.0);
    let mut out = Vec::new();
    for &id in ids {
        let frame = Frame::from_ephem_j2000(id);
        let gm = match alm.frame_info(frame).ok().and_then(|f| f.mu_km3_s2) {
            Some(gm) => gm * 1e9,
            None => continue,
        };
        let Ok(state) = alm.translate(frame, SSB_J2000, epoch, None) else { continue };
        out.push(Mass {
            pos: DVec3::new(state.radius_km.x * 1e3, state.radius_km.y * 1e3, state.radius_km.z * 1e3),
            gm,
        });
    }
    out
}

pub fn universe(jd: f64, pos: DVec3) -> (f64, DVec3) {
    let g = gravity(jd, pos);
    let e = electromagnetism(jd, pos);
    let w = weak_force(jd, pos);
    (g.0 + e.0 + w.0, g.1 + e.1 + w.1)
}

pub fn gravity(jd: f64, pos: DVec3) -> (f64, DVec3) {
    let masses = masses_at(jd);
    let mut omega = 0.0_f64;
    let mut flow = DVec3::ZERO;
    for m in &masses {
        let delta = m.pos - pos;
        let dist = delta.length();
        if dist < 1.0 { continue; }
        let g = m.gm / (dist * dist);
        omega += g;
        flow += delta.normalize() * g;
    }
    (omega, flow)
}

pub fn electromagnetism(_jd: f64, _pos: DVec3) -> (f64, DVec3) {
    (0.0, DVec3::ZERO)
}

pub fn weak_force(_jd: f64, _pos: DVec3) -> (f64, DVec3) {
    (0.0, DVec3::ZERO)
}
