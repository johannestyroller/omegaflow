use axum::{Router, routing::get, http::header, response::IntoResponse, extract::Query};
use serde::Deserialize;

#[derive(Deserialize)]
struct MassesReq { jd: f64 }

async fn index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html")], HTML)
}

async fn masses(Query(params): Query<MassesReq>) -> impl IntoResponse {
    let masses = nebra_core::masses_at(params.jd);
    let data: Vec<f32> = masses.iter().flat_map(|m| {
        [m.pos.x as f32, m.pos.y as f32, m.pos.z as f32, m.gm as f32]
    }).collect();
    let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes)
}

#[tokio::main]
async fn main() {
    println!("Loading ephemeris...");
    tokio::task::spawn_blocking(|| nebra_core::init()).await.ok();
    println!("Ready. http://localhost:3000");
    let app = Router::new().route("/", get(index)).route("/masses", get(masses));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

static HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Nebra v3</title>
<style>*{margin:0;padding:0}body{background:#000;overflow:hidden}canvas{display:block;width:100vw;height:100vh}</style>
</head><body><canvas id="c"></canvas><script>
(async()=>{
const canvas=document.getElementById('c');
const adapter=await navigator.gpu.requestAdapter();
const device=await adapter.requestDevice();
const ctx=canvas.getContext('webgpu');
const fmt=navigator.gpu.getPreferredCanvasFormat();
ctx.configure({device,format:fmt,alphaMode:'opaque'});
canvas.width=window.innerWidth;
canvas.height=window.innerHeight;
let RX=canvas.width,RY=canvas.height;
window.addEventListener('resize',()=>{canvas.width=window.innerWidth;canvas.height=window.innerHeight;RX=canvas.width;RY=canvas.height});
let cx=0,cy=0,cz=0,scale=3e10,jd=2460000.5;
let drag=false,lx=0,ly=0;
canvas.addEventListener('mousedown',e=>{drag=true;lx=e.clientX;ly=e.clientY});
canvas.addEventListener('mousemove',e=>{if(!drag)return;cx-=(e.clientX-lx)*scale;cy+=(e.clientY-ly)*scale;lx=e.clientX;ly=e.clientY});
canvas.addEventListener('mouseup',()=>drag=false);
canvas.addEventListener('wheel',e=>{e.preventDefault();scale*=e.deltaY>0?1.1:0.9});
const shader=`
struct VP{center_scale:vec4f,res_count:vec4f};
@group(0)@binding(0) var<storage,read> masses:array<vec4f>;
@group(0)@binding(1) var<uniform> vp:VP;
struct V{@builtin(position) p:vec4f,@location(0) u:vec2f}
@vertex fn vs(@builtin(vertex_index) i:u32)->V{
  var p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));
  var o:V;o.p=vec4f(p[i],0,1);o.u=vec2f(p[i].x*0.5+0.5,0.5-p[i].y*0.5);return o;
}
@fragment fn fs(i:V)->@location(0) vec4f{
  let count=u32(vp.res_count.z);
  let scale=vp.center_scale.w;
  let w=vp.res_count.x;
  let h=vp.res_count.y;
  let pixel_pos=vec3f(
    vp.center_scale.x+(i.u.x-0.5)*w*scale,
    vp.center_scale.y-(i.u.y-0.5)*h*scale,
    vp.center_scale.z);
  var omega=0.0f;
  for(var j=0u;j<count;j=j+1u){
    let m=masses[j];
    let delta=m.xyz-pixel_pos;
    let dist=length(delta);
    if(dist>1.0){omega=omega+m.w/(dist*dist);}
  }
  if(omega<=0.0){discard;}
  let t2=clamp((log2(omega)+14.0)/22.0,0.0,1.0);
  let c=mix(vec3f(0.0,0.02,0.1),vec3f(0.0,0.3,0.8),clamp(t2*4.0,0.0,1.0));
  let c2=mix(c,vec3f(0.2,0.8,1.0),clamp((t2-0.25)*4.0,0.0,1.0));
  let c3=mix(c2,vec3f(1.0,0.7,0.1),clamp((t2-0.5)*4.0,0.0,1.0));
  let c4=mix(c3,vec3f(1.0,1.0,1.0),clamp((t2-0.75)*4.0,0.0,1.0));
  return vec4f(c4,1.0);
}`;
const sm=device.createShaderModule({code:shader});
const bgl=device.createBindGroupLayout({entries:[
  {binding:0,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'read-only-storage'}},
  {binding:1,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'uniform'}}
]});
const pl=device.createPipelineLayout({bindGroupLayouts:[bgl]});
const pipe=device.createRenderPipeline({layout:pl,vertex:{module:sm,entryPoint:'vs'},fragment:{module:sm,entryPoint:'fs',targets:[{format:fmt}]},primitive:{topology:'triangle-list'}});
const maxMassBuf=device.createBuffer({size:2048,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});
const vpBuf=device.createBuffer({size:32,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
let bg=device.createBindGroup({layout:bgl,entries:[{binding:0,resource:{buffer:maxMassBuf}},{binding:1,resource:{buffer:vpBuf}}]});
let massCount=0;
async function fetchMasses(){
  try{
    const r=await fetch('/masses?jd='+jd);
    const b=await r.arrayBuffer();
    const d=new Float32Array(b);
    massCount=d.length/4;
    device.queue.writeBuffer(maxMassBuf,0,d);
    console.log('masses: '+massCount);
  }catch(e){console.log('fetch error',e);}
}
function render(){
  const vp=new Float32Array([cx,cy,cz,scale,RX,RY,massCount,0]);
  device.queue.writeBuffer(vpBuf,0,vp);
  const enc=device.createCommandEncoder();
  const pass=enc.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:{r:0,g:0,b:0,a:1},loadOp:'clear',storeOp:'store'}]});
  pass.setPipeline(pipe);pass.setBindGroup(0,bg);pass.draw(3);pass.end();
  device.queue.submit([enc.finish()]);
}
async function loop(){
  render();
  requestAnimationFrame(loop);
}
setInterval(()=>{jd+=0.001;fetchMasses();},1000);
await fetchMasses();
loop();
})();
</script></body></html>"#;
