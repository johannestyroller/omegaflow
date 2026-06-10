use axum::{Router, routing::get, http::header, response::IntoResponse, extract::Query};
use serde::Deserialize;

#[derive(Deserialize)]
struct MassesReq { jd: f64, cx: f64, cy: f64, cz: f64, scale: f64, observer_tier: i32 }

#[derive(Deserialize)]
struct WmmReq { jd: f64 }

#[derive(Deserialize)]
struct TerrainReq { lat: f64, lon: f64, size: f64 }

async fn index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html")], HTML)
}

async fn eval_state_wgsl() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/wgsl")], EVAL_STATE_SHADER)
}

async fn masses(Query(params): Query<MassesReq>) -> impl IntoResponse {
    let t = (params.jd - 2451545.0) * 86400.0;
    let masses = omegaflow_server::masses_at(t, params.cx, params.cy, params.cz, params.scale, params.observer_tier);
    let data: Vec<f32> = masses.iter().flat_map(|m| {
        [m.pos.x as f32, m.pos.y as f32, m.pos.z as f32, m.gm as f32]
    }).collect();
    let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
}

async fn wmm(Query(params): Query<WmmReq>) -> impl IntoResponse {
    let t = (params.jd - 2451545.0) * 86400.0;
    let Some(data) = omegaflow_server::wmm_at(t) else {
        return ([(header::CONTENT_TYPE, "application/octet-stream")], Vec::<u8>::new());
    };
    
    let n_max = data.n_max;
    let wmm_coeffs = (n_max * (n_max + 3)) / 2;

    let mut out = Vec::with_capacity(6 + 4 * wmm_coeffs as usize + 9 + 1);
    
    // wmm[0..2]: Erdzentrum im ICRF
    out.push(data.earth_pos.x as f32);
    out.push(data.earth_pos.y as f32);
    out.push(data.earth_pos.z as f32);

    // wmm[3..5]: Magnetischer Dipol im ICRF (Rotiert aus ITRF)
    let t_ut1 = params.jd - 2451545.0;
    let gmst_deg = 280.46061837 + 360.98564736629 * t_ut1;
    let gmst_rad = gmst_deg.to_radians();
    let cos_g = gmst_rad.cos() as f32;
    let sin_g = gmst_rad.sin() as f32;

    // Dipol in ITRF zeigt genähert nach Süden (Magnetisches Moment)
    let dipole_itrf_x: f32 = 0.0;
    let dipole_itrf_y: f32 = 0.0;
    let dipole_itrf_z: f32 = -1.0; 

    let dipole_x = cos_g * dipole_itrf_x - sin_g * dipole_itrf_y;
    let dipole_y = sin_g * dipole_itrf_x + cos_g * dipole_itrf_y;
    let dipole_z = dipole_itrf_z;

    out.push(dipole_x);
    out.push(dipole_y);
    out.push(dipole_z);

    // wmm[6]: Zeitdifferenz
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

    out.push(cos_g);  out.push(sin_g);  out.push(0.0);
    out.push(-sin_g); out.push(cos_g);  out.push(0.0);
    out.push(0.0);    out.push(0.0);    out.push(1.0);

    let bytes: Vec<u8> = out.iter().flat_map(|f| f.to_le_bytes()).collect();
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
}

async fn terrain(Query(params): Query<TerrainReq>) -> impl IntoResponse {
    let size = 256;
    let mut out = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            let lat = params.lat + (y as f64 / size as f64 - 0.5) * params.size;
            let lon = params.lon + (x as f64 / size as f64 - 0.5) * params.size;
            out.push(omegaflow_server::terrain_height(lat, lon));
        }
    }
    let bytes: Vec<u8> = out.iter().flat_map(|f| f.to_le_bytes()).collect();
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
}

async fn time() -> impl IntoResponse {
    let jd = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() / 86400.0 + 2440587.5;
    ([(header::CONTENT_TYPE, "text/plain")], jd.to_string())
}

#[tokio::main]
async fn main() {
    tokio::task::spawn_blocking(|| omegaflow_server::init()).await.ok();
    let app = Router::new()
        .route("/", get(index))
        .route("/eval_state.wgsl", get(eval_state_wgsl))
        .route("/masses", get(masses))
        .route("/wmm", get(wmm))
        .route("/terrain", get(terrain))
        .route("/time", get(time));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

static EVAL_STATE_SHADER: &str = include_str!("../static/eval_state.wgsl");

static HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Omegaflow</title>
<link rel="icon" href="data:,">
<style>*{margin:0;padding:0}body{background:#000;overflow:hidden}canvas{display:block;width:100vw;height:100vh;cursor:none}</style>
</head><body><canvas id="c" tabindex="0"></canvas><script>
(async()=>{
try {
const canvas=document.getElementById('c');
canvas.focus();
const adapter=await navigator.gpu.requestAdapter();
if(!adapter){document.body.innerText='No GPU';return;}
const device=await adapter.requestDevice();
if(!device){document.body.innerText='No Device';return;}
device.lost.then(info=>{document.body.innerText='GPU Lost: '+info.message;console.error(info)});
const ctx=canvas.getContext('webgpu');
const fmt=navigator.gpu.getPreferredCanvasFormat();
ctx.configure({device,format:fmt,alphaMode:'opaque'});
canvas.width=window.innerWidth;
canvas.height=window.innerHeight;
let RX=canvas.width,RY=canvas.height;
window.addEventListener('resize',()=>{canvas.width=window.innerWidth;canvas.height=window.innerHeight;RX=canvas.width;RY=canvas.height});
window.cx=0;window.cy=0;window.cz=0;window.scale=1e4;window.jd=2459945.0;
window.yaw=0;window.pitch=0;
window.currentJD=jd; window.currentDwell=0; window.currentTimeScale=1; window.observerTier=0; window.timeMultiplier=1.0; window.observerCapacity=1.0;
let drag=false,rdrag=false,lx=0,ly=0;
let lastMoveTime = Date.now();
let dwellTime = 0.0;
let timeScale = 1.0;
let timeMultiplier = 1.0;
let observerTier = 0;
let observerCapacity = 1.0;
let deviceAccX=0.0, deviceAccY=0.0, deviceAccZ=0.0;
let deviceMagX=0.0, deviceMagY=0.0, deviceMagZ=0.0;
let ambientLux = 50.0;
let micVolume = 0.0;
let cameraLux = 0.0;
let obsLat=47.0; let obsLon=11.0; let obsAlt=0.0; let camRot=0;
let touches = {};
let initialPinchDist = 0;
let initialScale = 0;
let smoothedCapacity = 0.1;
let smoothFactor = 0.05;
let initialAlpha = null;
let initialBeta = null;
let videoElement = null;
let observerAwake = false;
let prev_cx=0, prev_cy=0, prev_cz=0;

function syncHere() {
    let t_ut1 = jd - 2451545.0;
    let earth_x = 1.496e11 * Math.cos(2 * Math.PI * t_ut1 / 365.25);
    let earth_y = 1.496e11 * Math.sin(2 * Math.PI * t_ut1 / 365.25);
    let earth_z = 0.0;

    let lat_r = obsLat * Math.PI / 180;
    let lon_r = obsLon * Math.PI / 180;
    let R = 6378137.0 + obsAlt;
    let ox = R * Math.cos(lat_r) * Math.cos(lon_r);
    let oy = R * Math.cos(lat_r) * Math.sin(lon_r);
    let oz = R * Math.sin(lat_r);

    let gmst_deg = 280.46061837 + 360.98564736629 * t_ut1;
    let gmst_rad = gmst_deg * Math.PI / 180;
    let icrf_ox = Math.cos(gmst_rad) * ox - Math.sin(gmst_rad) * oy;
    let icrf_oy = Math.sin(gmst_rad) * ox + Math.cos(gmst_rad) * oy;
    let icrf_oz = oz;

    cx = earth_x + icrf_ox;
    cy = earth_y + icrf_oy;
    cz = earth_z + icrf_oz;
    scale = 1e4; 
}

window.addEventListener('devicemotion',e=>{
    let acc=e.accelerationIncludingGravity;
    if(acc){deviceAccX=acc.x||0; deviceAccY=acc.y||0; deviceAccZ=acc.z||0;}
});
if('AmbientLightSensor' in window){
    try{
        let als=new AmbientLightSensor();
        als.addEventListener('reading',()=>{ambientLux=als.illuminance;});
        als.start();
    }catch(e){}
}
if('Magnetometer' in window){
    try{
        let mag=new Magnetometer({frequency:60});
        mag.addEventListener('reading',()=>{deviceMagX=mag.x||0; deviceMagY=mag.y||0; deviceMagZ=mag.z||0;});
        mag.start();
    }catch(e){}
}
window.addEventListener('deviceorientation',e=>{
    if(initialAlpha===null){initialAlpha=e.alpha; initialBeta=e.beta;}
    let dAlpha = e.alpha - initialAlpha;
    if(dAlpha > 180) dAlpha -= 360;
    if(dAlpha < -180) dAlpha += 360;
    yaw = dAlpha * 0.02;
    pitch = (e.beta - initialBeta) * 0.02;
});

if('geolocation' in navigator){
    navigator.geolocation.watchPosition(p=>{
        obsLat=p.coords.latitude;
        obsLon=p.coords.longitude;
        obsAlt=p.coords.altitude||0.0;
        if(observerAwake) {
            syncHere();
            fetchTerrain();
        }
    }, e=>{}, {enableHighAccuracy:true, maximumAge:0});
}

try {
    const stream = await navigator.mediaDevices.getUserMedia({audio:true});
    const actx = new AudioContext();
    const source = actx.createMediaStreamSource(stream);
    const analyser = actx.createAnalyser();
    source.connect(analyser);
    const data = new Uint8Array(analyser.frequencyBinCount);
    setInterval(()=>{
        analyser.getByteTimeDomainData(data);
        let sum=0;
        for(let i=0;i<data.length;i++){let v=(data[i]-128)/128.0;sum+=v*v;}
        micVolume=Math.sqrt(sum/data.length);
    },50);
} catch(e){}

try {
    const stream = await navigator.mediaDevices.getUserMedia({video:{width:640, height:480}});
    videoElement = document.createElement('video');
    videoElement.srcObject = stream;
    videoElement.play();
    const vctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
    setInterval(()=>{
        try{
            vctx.canvas.width=1;vctx.canvas.height=1;
            vctx.drawImage(videoElement,0,0,1,1);
            const p=vctx.getImageData(0,0,1,1).data;
            cameraLux=(p[0]+p[1]+p[2])/765.0;
        }catch(e){}
    },100);
} catch(e){}

async function awaken() {
    if(observerAwake) return;
    observerAwake = true;
    if(!document.fullscreenElement){document.documentElement.requestFullscreen().catch(e=>{});}
    await fetchTime();
    syncHere();
    prev_cx=cx; prev_cy=cy; prev_cz=cz;
    fetchMasses(); fetchWmm(); fetchTerrain();
}

canvas.addEventListener('contextmenu',e=>e.preventDefault());
canvas.addEventListener('mousedown',e=>{
    lastMoveTime=Date.now();canvas.focus();
    awaken();
    lx=e.clientX;ly=e.clientY;
    if(e.button===0)drag=true;
    if(e.button===2)rdrag=true;
});
canvas.addEventListener('mousemove',e=>{
    lastMoveTime=Date.now();
    if(drag){cx-=(e.clientX-lx)*scale;cy-=(e.clientY-ly)*scale;}
    if(rdrag){yaw-=(e.clientX-lx)*0.01;pitch+=(e.clientY-ly)*0.01;}
    lx=e.clientX;ly=e.clientY;
});
canvas.addEventListener('mouseup',e=>{
    if(e.button===0)drag=false;
    if(e.button===2)rdrag=false;
});
canvas.addEventListener('dblclick',()=>{
    if(!document.fullscreenElement){document.documentElement.requestFullscreen().catch(e=>{});}
    else{document.exitFullscreen();}
});
canvas.addEventListener('wheel',e=>{
    e.preventDefault();
    if(e.shiftKey){
        jd+=e.deltaY*0.0001*timeMultiplier;
    } else if(e.ctrlKey){
        let z=1.01;
        scale*=e.deltaY>0?z:1.0/z;
    } else {
        let z=e.deltaMode===1?1.1:1.05;
        scale*=e.deltaY>0?z:1.0/z;
    }
},{passive:false});
window.addEventListener('keydown',e=>{
    lastMoveTime=Date.now();
    awaken();
    const step=scale*0.1;
    const tStep=0.01*timeMultiplier;
    if(e.key==='a'||e.key==='A'){cx+=step;}
    if(e.key==='d'||e.key==='D'){cx-=step;}
    if(e.key==='ArrowUp'){cy-=step;}
    if(e.key==='ArrowDown'){cy+=step;}
    if(e.key==='ArrowLeft'){cx+=step;}
    if(e.key==='ArrowRight'){cx-=step;}
    if(e.key==='q'||e.key==='Q'){jd-=tStep;}
    if(e.key==='e'||e.key==='E'){jd+=tStep;}
    if(e.key==='z'||e.key==='Z'){timeMultiplier=Math.max(0.1,timeMultiplier/1.5);}
    if(e.key==='x'||e.key==='X'){timeMultiplier=Math.min(1e10,timeMultiplier*1.5);}
    if(e.key==='c'||e.key==='C'){yaw-=0.1;}
    if(e.key==='v'||e.key==='V'){yaw+=0.1;}
    if(e.key==='b'||e.key==='B'){pitch-=0.1;}
    if(e.key==='n'||e.key==='N'){pitch+=0.1;}
    if(e.key==='f'||e.key==='F'){
        if(!document.fullscreenElement){document.documentElement.requestFullscreen().catch(e=>{});}
        else{document.exitFullscreen();}
    }
    if(e.key==='1'){camRot=0;}
    if(e.key==='2'){camRot=1;}
    if(e.key==='3'){camRot=2;}
    if(e.key==='4'){camRot=3;}
    if(e.key==='h'||e.key==='H'){syncHere();}
});

canvas.addEventListener('touchstart',e=>{
    e.preventDefault();canvas.focus();
    lastMoveTime=Date.now();
    awaken();
    for(let t of e.changedTouches){touches[t.identifier]={x:t.clientX,y:t.clientY};}
    if(e.touches.length===2){
        let t1=e.touches[0],t2=e.touches[1];
        initialPinchDist=Math.hypot(t1.clientX-t2.clientX,t1.clientY-t2.clientY);
        initialScale=scale;
    }
},{passive:false});
canvas.addEventListener('touchmove',e=>{
    e.preventDefault();
    lastMoveTime=Date.now();
    if(e.touches.length===1){
        let t=e.touches[0];
        let prev=touches[t.identifier];
        if(prev){cx-=(t.clientX-prev.x)*scale;cy-=(t.clientY-prev.y)*scale;}
        touches[t.identifier]={x:t.clientX,y:t.clientY};
    } else if(e.touches.length===2){
        let t1=e.touches[0],t2=e.touches[1];
        let prev1=touches[t1.identifier],prev2=touches[t2.identifier];
        let currentPinchDist=Math.hypot(t1.clientX-t2.clientX,t1.clientY-t2.clientY);
        if(initialPinchDist>0){scale=initialScale*(initialPinchDist/currentPinchDist);}
        if(prev1&&prev2){
            let dx1=t1.clientX-prev1.x,dx2=t2.clientX-prev2.x;
            let avgDx=(dx1+dx2)/2.0;
            jd+=avgDx*0.00005*timeMultiplier;
            let dy1=t1.clientY-prev1.y,dy2=t2.clientY-prev2.y;
            let avgDy=(dy1+dy2)/2.0;
            timeMultiplier*=Math.pow(1.05,-avgDy);
            timeMultiplier=Math.max(0.1,Math.min(timeMultiplier,1e10));
        }
        touches[t1.identifier]={x:t1.clientX,y:t1.clientY};
        touches[t2.identifier]={x:t2.clientX,y:t2.clientY};
    }
},{passive:false});
canvas.addEventListener('touchend',e=>{for(let t of e.changedTouches){delete touches[t.identifier];}});

const shaderResp = await fetch('/eval_state.wgsl');
const shader = await shaderResp.text();

const bgl=device.createBindGroupLayout({entries:[
  {binding:0,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'read-only-storage'}},
  {binding:1,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'uniform'}},
  {binding:2,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'read-only-storage'}},
  {binding:3,visibility:GPUShaderStage.FRAGMENT,texture:{sampleType:'unfilterable-float'}},
  {binding:4,visibility:GPUShaderStage.FRAGMENT,texture:{sampleType:'float'}},
  {binding:5,visibility:GPUShaderStage.FRAGMENT,sampler:{type:'filtering'}}
]});
const pl=device.createPipelineLayout({bindGroupLayouts:[bgl]});

let currentNMax = 8;

async function createPipe(nMax, tier) {
  const legendreSize = nMax == 133 ? 9045 : nMax == 12 ? 105 : 45;
  let patchedShader = shader.replace("const LEGENDRE_ARRAY_SIZE: i32 = 105;", "const LEGENDRE_ARRAY_SIZE: i32 = " + legendreSize + ";");
  const sm = device.createShaderModule({code: patchedShader});
  try {
    return await device.createRenderPipelineAsync({layout:pl,vertex:{module:sm,entryPoint:'vs'},fragment:{module:sm,entryPoint:'fs',targets:[{format:fmt}]},primitive:{topology:'triangle-list'},constants:{"HARDWARE_TIER":tier,"N_MAX":nMax}});
  } catch(e) {
    console.error("Pipeline failed:", e);
    document.body.innerText = "Shader Error: " + e.message;
    return null;
  }
}

let pipe = await createPipe(currentNMax, 0);
if(!pipe) return;

const massBuf=device.createBuffer({size:4096,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});
const vpBuf=device.createBuffer({size:128,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
const wmmBuf=device.createBuffer({size:65536,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});
let terrainTex = device.createTexture({size:[256,256],format:'r32float',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});
const cameraTex = device.createTexture({size:[640,480],format:'rgba8unorm',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST|GPUTextureUsage.RENDER_ATTACHMENT});
const cameraSampler = device.createSampler({magFilter:'linear',minFilter:'linear'});

let bg=device.createBindGroup({layout:bgl,entries:[
  {binding:0,resource:{buffer:massBuf}},
  {binding:1,resource:{buffer:vpBuf}},
  {binding:2,resource:{buffer:wmmBuf}},
  {binding:3,resource:terrainTex.createView()},
  {binding:4,resource:cameraTex.createView()},
  {binding:5,resource:cameraSampler}
]});
let massCount=0;

async function fetchMasses(){
  try{
    const r=await fetch(`/masses?jd=${jd}&cx=${cx}&cy=${cy}&cz=${cz}&scale=${scale}&observer_tier=${observerTier}`);
    const b=await r.arrayBuffer();
    const d=new Float32Array(b);
    massCount=d.length/4;
    device.queue.writeBuffer(massBuf,0,d);
  }catch(e){console.error(e)}
}

async function fetchWmm(){
  try{
    const r=await fetch('/wmm?jd='+jd);
    const b=await r.arrayBuffer();
    if(b.byteLength>0){
      const d=new Float32Array(b);
      device.queue.writeBuffer(wmmBuf,0,d);
    }
  }catch(e){console.error(e)}
}

async function fetchTerrain(){
  try{
    const r=await fetch(`/terrain?lat=${obsLat}&lon=${obsLon}&size=1.0`);
    const b=await r.arrayBuffer();
    if(b.byteLength>0){
      device.queue.writeTexture({texture:terrainTex},b,{bytesPerRow:1024},{width:256,height:256,depthOrArrayLayers:1});
    }
  }catch(e){console.error(e)}
}

async function fetchTime(){
  try{
    const r=await fetch('/time');
    const t=await r.text();
    jd=parseFloat(t);
    window.jd=jd;
  }catch(e){console.error(e)}
}

function render(){
  try{
    if(videoElement && videoElement.readyState >= videoElement.HAVE_CURRENT_DATA){
        device.queue.copyExternalImageToTexture({source:videoElement},{texture:cameraTex},{width:640,height:480});
    }

    if(!observerAwake){
        const enc=device.createCommandEncoder();
        const pass=enc.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:{r:0,g:0,b:0,a:1},loadOp:'clear',storeOp:'store'}]});
        pass.setPipeline(pipe);pass.setBindGroup(0,bg);pass.draw(3);pass.end();
        device.queue.submit([enc.finish()]);
        return;
    }

    const timeSinceMove = Date.now() - lastMoveTime;
    let motion = Math.sqrt(deviceAccX**2 + deviceAccY**2 + deviceAccZ**2);
    let motionFactor = Math.max(0.1, 1.0 - (motion / 20.0));
    let dwellFactor = 0.1;
    if (timeSinceMove > 2000) {
        dwellTime = Math.min(dwellTime + 1.0, 100.0);
        observerTier = 2;
        dwellFactor = 1.0;
    } else if (timeSinceMove > 500) {
        dwellTime = Math.min(dwellTime + 0.5, 100.0);
        observerTier = 1;
        dwellFactor = 0.5;
    } else {
        dwellTime = Math.max(dwellTime - 2.0, 0.0);
        observerTier = 0;
        dwellFactor = 0.1;
    }

    let targetCapacity = motionFactor * dwellFactor;
    smoothedCapacity += (targetCapacity - smoothedCapacity) * smoothFactor;
    observerCapacity = smoothedCapacity;

    let timeDilation = Math.pow(scale / 1e4, 1.5);
    jd += timeDilation * 0.000001 * timeScale * timeMultiplier;

    let realNow = Date.now() / 86400000.0 + 2440587.5;
    let deltaT = Math.abs(jd - realNow);
    let temporalCertainty = Math.exp(-deltaT * 20.0);

    let dx = cx - prev_cx; let dy = cy - prev_cy; let dz = cz - prev_cz;
    let viewVelocity = Math.sqrt(dx*dx + dy*dy + dz*dz) / scale;
    let localityCertainty = Math.exp(-viewVelocity * 5.0);
    prev_cx=cx; prev_cy=cy; prev_cz=cz;

    window.currentJD = jd; 
    window.currentDwell = dwellTime; 
    window.currentTimeScale = timeScale;
    window.observerTier = observerTier;
    window.timeMultiplier = timeMultiplier;
    window.observerCapacity = observerCapacity;
    window.deviceMotion = motion;
    window.ambientLux = ambientLux;

    const vp=new Float32Array([
        cx,cy,cz,scale, 
        RX,RY,massCount,0.0, 
        dwellTime,motion,ambientLux,observerCapacity, 
        deviceAccX,deviceAccY,deviceAccZ,0.0, 
        deviceMagX,deviceMagY,deviceMagZ,0.0,
        yaw,pitch,0.0,0.0,
        micVolume,cameraLux,temporalCertainty,localityCertainty,
        obsLat,obsLon,obsAlt,camRot
    ]);
    device.queue.writeBuffer(vpBuf,0,vp);
    const enc=device.createCommandEncoder();
    const pass=enc.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:{r:0,g:0,b:0,a:1},loadOp:'clear',storeOp:'store'}]});
    pass.setPipeline(pipe);pass.setBindGroup(0,bg);pass.draw(3);pass.end();
    device.queue.submit([enc.finish()]);
  }catch(e){console.error(e)}
}

async function loop(){render();requestAnimationFrame(loop);}
setInterval(()=>{if(observerAwake){fetchMasses();fetchWmm();}},1000);
loop();
} catch(e) { document.body.innerText = e.message; console.error(e); }
})();
</script></body></html>"#;


