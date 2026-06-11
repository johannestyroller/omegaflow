import{S,clamp,buildVp,updateCapacity,fetchStream,fetchTime,awaken,initInput} from'/client.js';
const canvas=document.getElementById('c');
canvas.focus();
const adapter=await navigator.gpu.requestAdapter();
const device=await adapter.requestDevice();
const ctx=canvas.getContext('webgpu');
const fmt=navigator.gpu.getPreferredCanvasFormat();
ctx.configure({device,format:fmt,alphaMode:'opaque'});
device.lost.then(i=>console.error(i));
initInput(canvas);
const sm=device.createShaderModule({code:await(await fetch('/eval_state.wgsl')).text()});
const pipe=await device.createRenderPipelineAsync({layout:'auto',vertex:{module:sm,entryPoint:'vs'},fragment:{module:sm,entryPoint:'fs',targets:[{format:fmt}]},primitive:{topology:'triangle-list'}});
const massBuf=device.createBuffer({size:65536,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});
const vpBuf=device.createBuffer({size:128,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
const wmmBuf=device.createBuffer({size:65536,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST});
const terrainTex=device.createTexture({size:[1201,1201],format:'r16sint',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});
const egm96Tex=device.createTexture({size:[1440,721],format:'r32float',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST});
const cameraTex=device.createTexture({size:[640,480],format:'rgba8unorm',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST|GPUTextureUsage.RENDER_ATTACHMENT});
const camSamp=device.createSampler({magFilter:'linear',minFilter:'linear'});
const bg=device.createBindGroup({layout:pipe.getBindGroupLayout(0),entries:[
    {binding:0,resource:{buffer:massBuf}},{binding:1,resource:{buffer:vpBuf}},{binding:2,resource:{buffer:wmmBuf}},
    {binding:3,resource:terrainTex.createView()},{binding:4,resource:egm96Tex.createView()},
    {binding:5,resource:cameraTex.createView()},{binding:6,resource:camSamp}]});
const upload={
    masses(b,o,n){device.queue.writeBuffer(massBuf,0,new Uint8Array(b,o,n));},
    wmm(b,o,n){device.queue.writeBuffer(wmmBuf,0,new Uint8Array(b,o,n));},
    terrain(b,o,n){device.queue.writeTexture({texture:terrainTex},new Uint8Array(b,o,n),{bytesPerRow:2402},{width:1201,height:1201});},
    egm96(b,o,n){device.queue.writeTexture({texture:egm96Tex},new Uint8Array(b,o,n),{bytesPerRow:5760},{width:1440,height:721});}
};
function render(){
    try{
        if(S.videoElement&&S.videoElement.readyState>=S.videoElement.HAVE_CURRENT_DATA)
            device.queue.copyExternalImageToTexture({source:S.videoElement},{texture:cameraTex},{width:640,height:480});
        if(!S.awake){const e=device.createCommandEncoder();const p=e.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:{r:0,g:0,b:0.05,a:1},loadOp:'clear',storeOp:'store'}]});p.setPipeline(pipe);p.setBindGroup(0,bg);p.draw(3);p.end();device.queue.submit([e.finish()]);return;}
        const now=performance.now();const dt=now-S.lastRenderTime;S.lastRenderTime=now;
        updateCapacity(dt);
        device.queue.writeBuffer(vpBuf,0,buildVp());
        const e=device.createCommandEncoder();
        const p=e.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:{r:0,g:0,b:0,a:1},loadOp:'clear',storeOp:'store'}]});
        p.setPipeline(pipe);p.setBindGroup(0,bg);p.draw(3);p.end();
        device.queue.submit([e.finish()]);
        fetchStream(upload);
    }catch(e){console.error(e);}
}
await fetchTime();
function loop(){render();requestAnimationFrame(loop);}
loop();
