import{S,clamp,buildVp,updateCapacity,fetchStream,fetchTime,awaken,initInput} from'/client.js';
const canvas=document.getElementById('c');
canvas.focus();
const gl=canvas.getContext('webgl2');
const glslSrc=await(await fetch('/eval_state.glsl')).text();
const vsSrc=glslSrc.substring(0,glslSrc.indexOf('// --- FRAGMENT'));
const fsSrc=glslSrc.substring(glslSrc.indexOf('// --- FRAGMENT'));
function cs(s,t){const sh=gl.createShader(t);gl.shaderSource(sh,s);gl.compileShader(sh);if(!gl.getShaderParameter(sh,gl.COMPILE_STATUS)){console.error(gl.getShaderInfoLog(sh));return null;}return sh;}
const vs=cs(vsSrc,gl.VERTEX_SHADER),fs=cs(fsSrc,gl.FRAGMENT_SHADER);
const prog=gl.createProgram();gl.attachShader(prog,vs);gl.attachShader(prog,fs);gl.linkProgram(prog);
if(!gl.getProgramParameter(prog,gl.LINK_STATUS)){console.error(gl.getProgramInfoLog(prog));throw new Error('link');}
gl.useProgram(prog);
initInput(canvas);
const vpLoc=gl.getUniformBlockIndex(prog,'VP');gl.uniformBlockBinding(prog,vpLoc,0);
const vpBuf=gl.createBuffer();gl.bindBufferBase(gl.UNIFORM_BUFFER,0,vpBuf);
function mt(u,w,h,i,f,t){const tx=gl.createTexture();gl.activeTexture(gl.TEXTURE0+u);gl.bindTexture(gl.TEXTURE_2D,tx);
    gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D,0,i,w,h,0,f,t,null);
    gl.uniform1i(gl.getUniformLocation(prog,['massTex','wmmTex','terrainTex','egm96Tex','cameraTex'][u]),u);return tx;}
const massTex=mt(0,4096,1,gl.RGBA32F,gl.RGBA,gl.FLOAT);
const wmmTex=mt(1,4096,1,gl.RGBA32F,gl.RGBA,gl.FLOAT);
const terrainTex=mt(2,1201,1201,gl.R32F,gl.RED,gl.FLOAT);
const egm96Tex=mt(3,1440,721,gl.R32F,gl.RED,gl.FLOAT);
const camTex=mt(4,640,480,gl.RGBA,gl.UNSIGNED_BYTE);
gl.createVertexArray();gl.bindVertexArray(gl.createVertexArray());
const upload={
    masses(b,o,n){gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,massTex);gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,n/16,1,gl.RGBA,gl.FLOAT,new Float32Array(b,o,n/4));},
    wmm(b,o,n){gl.activeTexture(gl.TEXTURE1);gl.bindTexture(gl.TEXTURE_2D,wmmTex);gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,n/16,1,gl.RGBA,gl.FLOAT,new Float32Array(b,o,n/4));},
    terrain(b,o,n){gl.activeTexture(gl.TEXTURE2);gl.bindTexture(gl.TEXTURE_2D,terrainTex);gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,1201,1201,gl.RED,gl.FLOAT,new Float32Array(b,o,n/4));},
    egm96(b,o,n){gl.activeTexture(gl.TEXTURE3);gl.bindTexture(gl.TEXTURE_2D,egm96Tex);gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,1440,721,gl.RED,gl.FLOAT,new Float32Array(b,o,n/4));}
};
function render(){
    try{
        if(S.videoElement&&S.videoElement.readyState>=S.videoElement.HAVE_CURRENT_DATA){gl.activeTexture(gl.TEXTURE4);gl.bindTexture(gl.TEXTURE_2D,camTex);gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,640,480,gl.RGBA,gl.UNSIGNED_BYTE,S.videoElement);}
        if(!S.awake){gl.clearColor(0,0,0.05,1);gl.clear(gl.COLOR_BUFFER_BIT);return;}
        const now=performance.now();const dt=now-S.lastRenderTime;S.lastRenderTime=now;
        updateCapacity(dt);
        gl.bindBuffer(gl.UNIFORM_BUFFER,vpBuf);gl.bufferData(gl.UNIFORM_BUFFER,buildVp(),gl.DYNAMIC_DRAW);
        gl.clearColor(0,0,0,1);gl.clear(gl.COLOR_BUFFER_BIT);
        gl.drawArrays(gl.TRIANGLES,0,3);
        fetchStream(upload);
    }catch(e){console.error(e);}
}
await fetchTime();
function loop(){render();requestAnimationFrame(loop);}
loop();
