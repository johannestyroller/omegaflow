use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 { eprintln!("omegaflow-pool <input> <output.omega>"); std::process::exit(1); }
    let input = &args[1];
    let output = &args[2];
    let lower = input.to_lowercase();
    let data = fs::read(input).expect("read");
    let mut out = Vec::new();
    out.extend_from_slice(b"OMEGA");
    out.push(1u8);
    if lower.ends_with(".bsp") { compile_spk(&data, &mut out); }
    else if lower.ends_with(".pca") { out.push(b'p'); w32(&mut out, data.len() as u32); out.extend_from_slice(&data); }
    else if lower.ends_with(".dac") { compile_egm96(&data, &mut out); }
    else if lower.contains("wmm") || lower.ends_with(".cof") { compile_wmm(&mut out); }
    else { eprintln!("unknown: {}", input); std::process::exit(1); }
    fs::write(output, &out).expect("write");
    eprintln!("{} -> {} ({} bytes)", input, output, out.len());
}

fn r64(d: &[u8], i: usize) -> f64 { f64::from_le_bytes(d[i..i+8].try_into().unwrap()) }
fn r32i(d: &[u8], i: usize) -> i32 { i32::from_le_bytes(d[i..i+4].try_into().unwrap()) }
fn w32(o: &mut Vec<u8>, v: u32) { o.extend_from_slice(&v.to_le_bytes()); }
fn wf32(o: &mut Vec<u8>, v: f32) { o.extend_from_slice(&v.to_le_bytes()); }
fn wf64(o: &mut Vec<u8>, v: f64) { o.extend_from_slice(&v.to_le_bytes()); }

fn compile_spk(data: &[u8], out: &mut Vec<u8>) {
    out.push(b's');
    let nd = r32i(data, 8) as usize;
    let ni = r32i(data, 12) as usize;
    let sb = nd * 8 + ni * 4;
    let sb = sb + (8 - sb % 8) % 8;

    let fsum = r32i(data, 76) as usize;
    let base = (fsum - 1) * 1024 + 104;

    let mut sums = Vec::new();
    let mut off = base;
    for _ in 0..200 {
        if off + sb > data.len() { break; }
        let dtype = r32i(data, off + 28);
        let target = r32i(data, off + 16);
        if dtype == 0 && target == 0 { break; }
        if dtype == 2 {
            let center = r32i(data, off + 20);
            let sstart = r64(data, off);
            let send = r64(data, off + 8);
            let si = r32i(data, off + 32) as usize;
            let ei = r32i(data, off + 36) as usize;
            sums.push((target, center, sstart, send, si, ei));
        }
        off += sb;
    }

    struct Seg { target: i32, center: i32, start: f64, end: f64, recs: Vec<(f64, f64, Vec<f64>, Vec<f64>, Vec<f64>)> }
    let mut segs = Vec::new();
    for &(target, center, start, end, si, ei) in &sums {
        let ds = (si - 1) * 8;
        let de = ei * 8;
        if de > data.len() { continue; }
        let d = &data[ds..de];
        let all: Vec<f64> = (0..d.len()/8).map(|i| r64(d, i*8)).collect();
        if all.len() < 5 { continue; }
        let rsize = all[all.len()-2] as usize;
        let nrec = all[all.len()-1] as usize;
        if rsize < 3 || nrec == 0 { continue; }
        let nc = (rsize - 2) / 3;
        let rd = &all[..all.len()-4];
        let mut recs = Vec::new();
        for i in 0..nrec {
            let s = i * rsize;
            let e = s + rsize;
            if e > rd.len() { break; }
            let r = &rd[s..e];
            recs.push((r[0], r[1], r[2..2+nc].to_vec(), r[2+nc..2+2*nc].to_vec(), r[2+2*nc..2+3*nc].to_vec()));
        }
        segs.push(Seg { target, center, start, end, recs });
    }

    w32(out, segs.len() as u32);
    for seg in &segs {
        w32(out, seg.target as u32);
        w32(out, seg.center as u32);
        wf64(out, seg.start);
        wf64(out, seg.end);
        wf64(out, 0.0); // domain_radius: filled by client, 0 = infinite for now
        w32(out, seg.recs.len() as u32);
        for (mid, rad, cx, cy, cz) in &seg.recs {
            wf64(out, *mid);
            wf64(out, *rad);
            w32(out, cx.len() as u32);
            for c in cx { wf64(out, *c); }
            for c in cy { wf64(out, *c); }
            for c in cz { wf64(out, *c); }
        }
    }
    let total: usize = segs.iter().map(|s| s.recs.len()).sum();
    eprintln!("spk: {} bodies, {} chebyshev records", segs.len(), total);
}

fn compile_wmm(out: &mut Vec<u8>) {
    omegaflow_core::init();
    let alm = omegaflow_core::almanac().expect("almanac");
    let wmm = omegaflow_core::wmm_at(0.0, alm).expect("wmm");
    out.push(b'w');
    w32(out, wmm.n_max as u32);
    wf32(out, wmm.time_delta);
    wf32(out, wmm.earth_pos.x as f32); wf32(out, wmm.earth_pos.y as f32); wf32(out, wmm.earth_pos.z as f32);
    let n = (wmm.n_max * (wmm.n_max + 3)) / 2;
    for i in 0..n as usize {
        wf32(out, wmm.g_mfc.get(i).copied().unwrap_or(0.0));
        wf32(out, wmm.h_mfc.get(i).copied().unwrap_or(0.0));
        wf32(out, wmm.g_svc.get(i).copied().unwrap_or(0.0));
        wf32(out, wmm.h_svc.get(i).copied().unwrap_or(0.0));
    }
    eprintln!("wmm: n_max={}", wmm.n_max);
}

fn compile_egm96(data: &[u8], out: &mut Vec<u8>) {
    out.push(b'e');
    let exp = 721*1440*2;
    if data.len() != exp { eprintln!("egm96: expected {} bytes", exp); std::process::exit(1); }
    let v: Vec<f32> = data.chunks_exact(2).map(|c| i16::from_be_bytes([c[0],c[1]]) as f32*0.01).collect();
    let mn = v.iter().cloned().fold(f32::MAX, f32::min);
    let mx = v.iter().cloned().fold(f32::MIN, f32::max);
    let r = (mx-mn).max(0.001);
    wf32(out, mn); wf32(out, r); w32(out, 721); w32(out, 1440);
    for &x in &v { wf32(out, (x-mn)/r); }
    eprintln!("egm96: {} values", v.len());
}
