use glam::DVec3;
use std::sync::OnceLock;
use anise::prelude::*;
use anise::constants::frames::SSB_J2000;
use hifitime::Epoch;
use world_magnetic_model::wmm_models::select_models;
use world_magnetic_model::time::Date;
use std::fs::File;
use std::io::Read;

static ALMANAC: OnceLock<Almanac> = OnceLock::new();
static MASS_IDS: OnceLock<Vec<i32>> = OnceLock::new();
static HGT_DATA: OnceLock<Vec<i16>> = OnceLock::new();

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
    
    if let Ok(mut f) = File::open("data/N47E011.hgt") {
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).ok() == Some(2884802) {
            let data: Vec<i16> = buf.chunks_exact(2).map(|c| i16::from_be_bytes([c[0], c[1]])).collect();
            let _ = HGT_DATA.set(data);
        }
    }
}

pub fn masses_at(t: f64, cx: f64, cy: f64, cz: f64, scale: f64, observer_tier: i32) -> Vec<Mass> {
    let Some(alm) = ALMANAC.get() else { return Vec::new() };
    let Some(ids) = MASS_IDS.get() else { return Vec::new() };
    let epoch = Epoch::from_tdb_seconds(t);
    let viewport_center = DVec3::new(cx, cy, cz);
    let mut out = Vec::new();
    for &id in ids {
        let frame = Frame::from_ephem_j2000(id);
        let gm = match alm.frame_info(frame).ok().and_then(|f| f.mu_km3_s2) {
            Some(gm) => gm * 1e9,
            None => continue,
        };
        let Ok(state) = alm.translate(frame, SSB_J2000, epoch, None) else { continue };
        let pos = DVec3::new(state.radius_km.x * 1e3, state.radius_km.y * 1e3, state.radius_km.z * 1e3);
        
        let dist = (pos - viewport_center).length();
        let influence_radius = scale * 10.0; 
        if dist > influence_radius && id != 10 { continue; } 
        
        out.push(Mass { pos, gm });
    }
    out.sort_by(|a, b| b.gm.partial_cmp(&a.gm).unwrap_or(std::cmp::Ordering::Equal));
    
    let max_masses = if observer_tier > 0 { 15 } else { 5 };
    out.truncate(max_masses);
    
    out
}

pub fn terrain_height(lat: f64, lon: f64) -> f32 {
    let Some(data) = HGT_DATA.get() else { return 0.0 };
    let lat0 = 47.0;
    let lon0 = 11.0;
    let x = ((lon - lon0) * 1200.0) as usize;
    let y = ((lat0 + 1.0 - lat) * 1200.0) as usize;
    if x >= 1201 || y >= 1201 { return 0.0; }
    let val = data[y * 1201 + x];
    if val == -32768 { 0.0 } else { val as f32 }
}

pub struct WmmData {
    pub earth_pos: DVec3,
    pub time_delta: f32,
    pub n_max: i32,
    pub g_mfc: Vec<f32>,
    pub h_mfc: Vec<f32>,
    pub g_svc: Vec<f32>,
    pub h_svc: Vec<f32>,
}

pub fn wmm_at(t: f64) -> Option<WmmData> {
    let epoch = Epoch::from_tdb_seconds(t);
    let year = epoch.year();
    let day_of_year = epoch.day_of_year() as u16;
    let date = Date::from_ordinal_date(year, day_of_year).ok()?;
    
    let (model, _error_model) = select_models(date).ok()?;
    
    let alm = ALMANAC.get()?;
    let earth_frame = Frame::from_ephem_j2000(3);
    let state = alm.translate(earth_frame, SSB_J2000, epoch, None).ok()?;
    let earth_pos = DVec3::new(state.radius_km.x * 1e3, state.radius_km.y * 1e3, state.radius_km.z * 1e3);
    
    let n_max = (((8 * model.g_mfc.len() as i32 + 9) as f64).sqrt() as i32 - 3) / 2;
    let g_mfc: Vec<f32> = model.g_mfc.iter().map(|&x| x as f32).collect();
    let h_mfc: Vec<f32> = model.h_mfc.iter().map(|&x| x as f32).collect();
    let g_svc: Vec<f32> = model.g_svc.iter().map(|&x| x as f32).collect();
    let h_svc: Vec<f32> = model.h_svc.iter().map(|&x| x as f32).collect();
    
    let time_delta = ((year - 2020) as f32) + (day_of_year as f32 / 365.0);
    
    Some(WmmData {
        earth_pos,
        time_delta,
        n_max,
        g_mfc,
        h_mfc,
        g_svc,
        h_svc,
    })
}

