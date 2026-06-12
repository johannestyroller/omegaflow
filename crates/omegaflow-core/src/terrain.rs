use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::{Mutex, OnceLock};

static HGT_CACHE: OnceLock<Mutex<HashMap<(i32, i32), Vec<u8>>>> = OnceLock::new();
static EGM96_RAW: OnceLock<Vec<u8>> = OnceLock::new();

pub fn init() {
    let _ = HGT_CACHE.set(Mutex::new(HashMap::new()));
    if let Ok(mut f) = File::open("data/WW15MGH.DAC") {
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).ok() == Some(721 * 1440 * 2) {
            let mut out = Vec::with_capacity(721 * 1440 * 4);
            for chunk in buf.chunks_exact(2) {
                let h = i16::from_be_bytes([chunk[0], chunk[1]]);
                let undulation = h as f32 * 0.01;
                out.extend_from_slice(&undulation.to_le_bytes());
            }
            let _ = EGM96_RAW.set(out);
        }
    }
}

pub fn raw_hgt_tile(lat0: i32, lon0: i32) -> Vec<u8> {
    let cache = match HGT_CACHE.get() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let mut guard = cache.lock().unwrap();
    
    if !guard.contains_key(&(lat0, lon0)) {
        let ns = if lat0 >= 0 { 'N' } else { 'S' };
        let ew = if lon0 >= 0 { 'E' } else { 'W' };
        let filename = format!("{}{:02}{}{:03}.hgt", ns, lat0.abs(), ew, lon0.abs());
        let path = format!("data/{}", filename);
        
        let mut data = None;
        if let Ok(mut f) = File::open(&path) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).ok() == Some(2884802) { data = Some(buf); }
        }
        
        if data.is_none() {
            let server_url = std::env::var("OMEGAFLOW_DEM_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
            let url = format!("{}/{}", server_url, filename);
            if let Ok(resp) = reqwest::blocking::Client::new().get(&url).send() {
                if resp.status().is_success() {
                    if let Ok(buf) = resp.bytes() {
                        if buf.len() == 2884802 {
                            let _ = std::fs::write(&path, &buf);
                            data = Some(buf.to_vec());
                        }
                    }
                }
            }
        }
        guard.insert((lat0, lon0), data.unwrap_or_default());
    }
    guard.get(&(lat0, lon0)).cloned().unwrap_or_default()
}

pub fn raw_egm96() -> Vec<u8> {
    EGM96_RAW.get().map(|v| v.clone()).unwrap_or_default()
}

