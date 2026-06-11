export const S={
    cx:0,cy:0,cz:0,scale:0,
    yaw:0,pitch:0,camRot:0,
    jd:0,
    timeMultiplier:1.0,
    lastMoveTime:0,
    dwellTime:0,
    capacity:0,
    massCount:0,
    deviceAccX:0,deviceAccY:0,deviceAccZ:0,
    deviceMagX:0,deviceMagY:0,deviceMagZ:0,
    ambientLux:0,micVolume:0,cameraLux:0,
    obsLat:0,obsLon:0,obsAlt:0,
    drag:false,rdrag:false,lx:0,ly:0,
    touches:{},initialPinchDist:0,initialScale:0,
    initialAlpha:null,initialBeta:null,
    videoElement:null,
    awake:false,
    prev_cx:0,prev_cy:0,prev_cz:0,
    lastRenderTime:0,
    egmLoaded:false,
    streaming:false
};

export function clamp(v,mn,mx){return Math.max(mn,Math.min(mx,v))}

export function syncHere(){
    let t=S.jd-2451545.0;
    let ex=1.496e11*Math.cos(2*Math.PI*t/365.25);
    let ey=1.496e11*Math.sin(2*Math.PI*t/365.25);
    let lr=S.obsLat*Math.PI/180,onr=S.obsLon*Math.PI/180;
    let R=6378137.0+S.obsAlt;
    let ox=R*Math.cos(lr)*Math.cos(onr),oy=R*Math.cos(lr)*Math.sin(onr),oz=R*Math.sin(lr);
    let gmst=(280.46061837+360.98564736629*t)*Math.PI/180;
    S.cx=ex+Math.cos(gmst)*ox-Math.sin(gmst)*oy;
    S.cy=ey+Math.sin(gmst)*ox+Math.cos(gmst)*oy;
    S.cz=oz;S.scale=1e4;
}

export function buildVp(){
    return new Float32Array([S.cx,S.cy,S.cz,S.scale,0,0,S.massCount,0,S.dwellTime,0,S.ambientLux,S.capacity,S.deviceAccX,S.deviceAccY,S.deviceAccZ,0,S.deviceMagX,S.deviceMagY,S.deviceMagZ,0,S.yaw,S.pitch,0,0,S.micVolume,S.cameraLux,0,0,S.obsLat,S.obsLon,S.obsAlt,S.camRot]);
}

export function updateCapacity(dt){
    S.capacity=1.0/(1.0+dt/16.0);
    let tsm=Date.now()-S.lastMoveTime;
    S.dwellTime=clamp(tsm/20,0,100);
    S.jd+=(dt/1000/86400)*S.timeMultiplier;
    S.prev_cx=S.cx;S.prev_cy=S.cy;S.prev_cz=S.cz;
}

export async function fetchStream(upload){
    if(S.streaming)return;
    S.streaming=true;
    let fj=S.jd+(0.01*S.timeMultiplier);
    let mg=1e-8/Math.max(S.capacity,0.01);
    try{
        const r=await fetch(`/stream?jd=${fj}&cx=${S.cx}&cy=${S.cy}&cz=${S.cz}&scale=${S.scale}&min_g=${mg}&n_max=${Math.floor(1+S.capacity*132)+5}&lat0=${Math.floor(S.obsLat)}&lon0=${Math.floor(S.obsLon)}`);
        const b=await r.arrayBuffer();
        if(b.byteLength>=16){
            const v=new DataView(b);
            const ml=v.getUint32(0,true),wl=v.getUint32(4,true),tl=v.getUint32(8,true),el=v.getUint32(12,true);
            let off=16;
            if(ml>0){S.massCount=ml/16;upload.masses(b,off,ml);off+=ml;}
            if(wl>0){upload.wmm(b,off,wl);off+=wl;}
            if(tl>0&&!S.egmLoaded){upload.terrain(b,off,tl);off+=tl;}
            if(el>0&&!S.egmLoaded){upload.egm96(b,off,el);S.egmLoaded=true;}
        }
    }catch(e){console.error(e);}
    S.streaming=false;
}

export async function fetchTime(){
    try{const r=await fetch('/time');const t=await r.text();S.jd=parseFloat(t);}catch(e){}
}

export async function awaken(){
    if(S.awake)return;S.awake=true;
    try{const stream=await navigator.mediaDevices.getUserMedia({audio:true});const actx=new AudioContext();const src=actx.createMediaStreamSource(stream);const an=actx.createAnalyser();src.connect(an);const d=new Uint8Array(an.frequencyBinCount);setInterval(()=>{an.getByteTimeDomainData(d);let s=0;for(let i=0;i<d.length;i++){let v=(d[i]-128)/128;s+=v*v;}S.micVolume=Math.sqrt(s/d.length);},50);}catch(e){}
    try{const stream=await navigator.mediaDevices.getUserMedia({video:{width:640,height:480,facingMode:'environment'}});S.videoElement=document.createElement('video');S.videoElement.srcObject=stream;S.videoElement.play();}catch(e){}
    if('geolocation' in navigator)navigator.geolocation.watchPosition(p=>{S.obsLat=p.coords.latitude;S.obsLon=p.coords.longitude;S.obsAlt=p.coords.altitude||0;},e=>{},{enableHighAccuracy:true,maximumAge:0});
    if(!document.fullscreenElement)document.documentElement.requestFullscreen().catch(()=>{});
    await fetchTime();S.prev_cx=S.cx;S.prev_cy=S.cy;S.prev_cz=S.cz;
}

export function initInput(canvas){
    canvas.width=window.innerWidth;canvas.height=window.innerHeight;
    window.addEventListener('resize',()=>{canvas.width=window.innerWidth;canvas.height=window.innerHeight;});
    canvas.addEventListener('contextmenu',e=>e.preventDefault());
    canvas.addEventListener('mousedown',e=>{S.lastMoveTime=Date.now();canvas.focus();awaken();S.lx=e.clientX;S.ly=e.clientY;if(e.button===0)S.drag=true;if(e.button===2)S.rdrag=true;});
    canvas.addEventListener('mousemove',e=>{S.lastMoveTime=Date.now();if(S.drag){S.cx-=(e.clientX-S.lx)*S.scale;S.cy-=(e.clientY-S.ly)*S.scale;}if(S.rdrag){S.yaw-=(e.clientX-S.lx)*0.01;S.pitch+=(e.clientY-S.ly)*0.01;}S.lx=e.clientX;S.ly=e.clientY;});
    canvas.addEventListener('mouseup',e=>{if(e.button===0)S.drag=false;if(e.button===2)S.rdrag=false;});
    canvas.addEventListener('dblclick',()=>{if(!document.fullscreenElement)document.documentElement.requestFullscreen().catch(()=>{});else document.exitFullscreen();});
    canvas.addEventListener('wheel',e=>{e.preventDefault();if(e.shiftKey)S.jd+=e.deltaY*0.0001*S.timeMultiplier;else if(e.ctrlKey){S.scale*=e.deltaY>0?1.01:1/1.01;}else{let z=e.deltaMode===1?1.1:1.05;S.scale*=e.deltaY>0?z:1/z;}},{passive:false});
    window.addEventListener('keydown',e=>{S.lastMoveTime=Date.now();awaken();const st=S.scale*0.1,ts=0.01*S.timeMultiplier;
        if(e.key==='a'||e.key==='A')S.cx+=st;if(e.key==='d'||e.key==='D')S.cx-=st;
        if(e.key==='ArrowUp')S.cy-=st;if(e.key==='ArrowDown')S.cy+=st;
        if(e.key==='ArrowLeft')S.cx+=st;if(e.key==='ArrowRight')S.cx-=st;
        if(e.key==='q'||e.key==='Q')S.jd-=ts;if(e.key==='e'||e.key==='E')S.jd+=ts;
        if(e.key==='z'||e.key==='Z')S.timeMultiplier=Math.max(0.1,S.timeMultiplier/1.5);
        if(e.key==='x'||e.key==='X')S.timeMultiplier=Math.min(1e10,S.timeMultiplier*1.5);
        if(e.key==='c'||e.key==='C')S.yaw-=0.1;if(e.key==='v'||e.key==='V')S.yaw+=0.1;
        if(e.key==='b'||e.key==='B')S.pitch-=0.1;if(e.key==='n'||e.key==='N')S.pitch+=0.1;
        if(e.key==='f'||e.key==='F'){if(!document.fullscreenElement)document.documentElement.requestFullscreen().catch(()=>{});else document.exitFullscreen();}
        if(e.key==='1')S.camRot=0;if(e.key==='2')S.camRot=1;if(e.key==='3')S.camRot=2;if(e.key==='4')S.camRot=3;
        if(e.key==='h'||e.key==='H')syncHere();
        if(e.key==='t'||e.key==='T')S.jd=Date.now()/86400000.0+2440587.5;
    });
    canvas.addEventListener('touchstart',e=>{e.preventDefault();canvas.focus();S.lastMoveTime=Date.now();awaken();for(let t of e.changedTouches)S.touches[t.identifier]={x:t.clientX,y:t.clientY};if(e.touches.length===2){let t1=e.touches[0],t2=e.touches[1];S.initialPinchDist=Math.hypot(t1.clientX-t2.clientX,t1.clientY-t2.clientY);S.initialScale=S.scale;}},{passive:false});
    canvas.addEventListener('touchmove',e=>{e.preventDefault();S.lastMoveTime=Date.now();if(e.touches.length===1){let t=e.touches[0],p=S.touches[t.identifier];if(p){S.cx-=(t.clientX-p.x)*S.scale;S.cy-=(t.clientY-p.y)*S.scale;}S.touches[t.identifier]={x:t.clientX,y:t.clientY};}else if(e.touches.length===2){let t1=e.touches[0],t2=e.touches[1],p1=S.touches[t1.identifier],p2=S.touches[t2.identifier],cd=Math.hypot(t1.clientX-t2.clientX,t1.clientY-t2.clientY);if(S.initialPinchDist>0)S.scale=S.initialScale*(S.initialPinchDist/cd);if(p1&&p2){S.jd+=(t1.clientX-p1.x+t2.clientX-p2.x)/2*0.00005*S.timeMultiplier;let dy=(t1.clientY-p1.y+t2.clientY-p2.y)/2;S.timeMultiplier*=Math.pow(1.05,-dy);S.timeMultiplier=Math.max(0.1,Math.min(S.timeMultiplier,1e10));}S.touches[t1.identifier]={x:t1.clientX,y:t1.clientY};S.touches[t2.identifier]={x:t2.clientX,y:t2.clientY};}},{passive:false});
    canvas.addEventListener('touchend',e=>{for(let t of e.changedTouches)delete S.touches[t.identifier];});
    if('serviceWorker' in navigator)navigator.serviceWorker.register('/sw.js').catch(()=>{});
}
