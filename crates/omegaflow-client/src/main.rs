use axum::{Router, routing::get, http::header, response::IntoResponse, extract::Query};
use serde::Deserialize;

#[derive(Deserialize)]
struct MassesReq { jd: f64, cx: f64, cy: f64, cz: f64, scale: f64 }

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
    let masses = omegaflow_server::masses_at(t, params.cx, params.cy, params.cz, params.scale);
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

#[tokio::main]
async fn main() {
    tokio::task::spawn_blocking(|| omegaflow_server::init()).await.ok();
    let app = Router::new()
        .route("/", get(index))
        .route("/eval_state.wgsl", get(eval_state_wgsl))
        .route("/masses", get(masses))
        .route("/wmm", get(wmm))
        .route("/terrain", get(terrain));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

static EVAL_STATE_SHADER: &str = include_str!("../static/eval_state.wgsl");

static HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Omegaflow</title>
<style>*{margin:0;padding:0}body{background:#000;overflow:hidden}canvas{display:block;width:100vw;height:100vh}</style>
</head><body><canvas id="c"></canvas><script>
(async()=>{
try {
const canvas=document.getElementById('c');
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
window.cx=0;window.cy=0;window.cz=0;window.scale=3e8;window.jd=2459945.0;
let drag=false,lx=0,ly=0;
canvas.addEventListener('mousedown',e=>{drag=true;lx=e.clientX;ly=e.clientY});
canvas.addEventListener('mousemove',e=>{if(!drag)return;cx-=(e.clientX-lx)*scale;cy+=(e.clientY-ly)*scale;lx=e.clientX;ly=e.clientY});
canvas.addEventListener('mouseup',()=>drag=false);
canvas.addEventListener('wheel',e=>{e.preventDefault();scale*=e.deltaY>0?1.1:0.9},{passive:false});

const shaderResp = await fetch('/eval_state.wgsl');
const shader = await shaderResp.text();

const bgl=device.createBindGroupLayout({entries:[
  {binding:0,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'read-only-storage'}},
  {binding:1,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'uniform'}},
  {binding:2,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'read-only-storage'}},
  {binding:3,visibility:GPUShaderStage.FRAGMENT,texture:{sampleType:'unfilterable-float'}}
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
const vpBuf=device.createBuffer({size:32,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
const wmmBuf=device.createBuffer({size:65536,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});
let terrainTex = device.createTexture({size:[256,256],format:'r32float',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});

let bg=device.createBindGroup({layout:bgl,entries:[
  {binding:0,resource:{buffer:massBuf}},
  {binding:1,resource:{buffer:vpBuf}},
  {binding:2,resource:{buffer:wmmBuf}},
  {binding:3,resource:terrainTex.createView()}
]});
let massCount=0;

async function fetchMasses(){
  try{
    const r=await fetch(`/masses?jd=${jd}&cx=${cx}&cy=${cy}&cz=${cz}&scale=${scale}`);
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
    const r=await fetch('/terrain?lat=47.0&lon=11.0&size=1.0');
    const b=await r.arrayBuffer();
    if(b.byteLength>0){
      device.queue.writeTexture({texture:terrainTex},b,{bytesPerRow:1024},{width:256,height:256,depthOrArrayLayers:1});
    }
  }catch(e){console.error(e)}
}

function render(){
  try{
    const vp=new Float32Array([cx,cy,cz,scale,RX,RY,massCount,0]);
    device.queue.writeBuffer(vpBuf,0,vp);
    const enc=device.createCommandEncoder();
    const pass=enc.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:{r:0,g:0,b:0,a:1},loadOp:'clear',storeOp:'store'}]});
    pass.setPipeline(pipe);pass.setBindGroup(0,bg);pass.draw(3);pass.end();
    device.queue.submit([enc.finish()]);
  }catch(e){console.error(e)}
}

async function loop(){render();requestAnimationFrame(loop);}
setInterval(()=>{jd+=0.001;fetchMasses();fetchWmm();},1000);
await fetchMasses();
await fetchWmm();
await fetchTerrain();
loop();
} catch(e) { document.body.innerText = e.message; console.error(e); }
})();
</script></body></html>"#;

