/* Deterministic 45-second film. Real frames; editorial timing, not a benchmark. */
'use strict';
(async () => {
  const params = new URLSearchParams(location.search);
  const reference = params.get('reference') === '1';
  const recording = params.get('record') === '1';
  const minimalMode = recording && params.get('minimal') === '1' && ['qr12','qr40','pipe'].includes(params.get('mode')) ? params.get('mode') : null;
  if (recording) document.body.classList.add('record');
  const canvas = document.querySelector('#film');
  const c = canvas.getContext('2d', { alpha: false });
  const $ = selector => document.querySelector(selector);
  const C = { bg:'#111313', panel:'#171b1a', line:'#34413c', ink:'#f2eee7', muted:'#a3b1ac', coral:'#ff876e', mint:'#a9d9ba', dim:'#25302c' };
  const DURATION = minimalMode ? 10 : 45;
  const starts = [0, 5, 15, 25, 36];
  const titles = ['Through the terminal.', 'A file, written in light.', 'A denser kind of signal.', 'Now, let the characters flow.', 'Every byte. Back where it belongs.'];
  const captions = ['One image. A terminal is all it takes.', 'QR · V12-M / 224-byte shards / CRC32C + Reed–Solomon', 'QR · V40-L / 2,893-byte shards / the same recovery protocol', 'Base64 · SI1 frames / tmux pipe-pane / no QR decoding', 'One original. One recovered copy. The same SHA-256.'];
  let data = window.SHADOW_INK_PAYLOAD;
  let ready = false, playing = false, seconds = 0, lastClock = 0, raf = 0;
  const qrCache = new Map();
  let base64Frames;
  const tileRanks = Array.from({length:400}, (_,i)=>i);
  let tileSeed=2026;
  for(let i=399;i>0;i--){tileSeed=(Math.imul(tileSeed,1664525)+1013904223)>>>0;const j=tileSeed%(i+1);[tileRanks[i],tileRanks[j]]=[tileRanks[j],tileRanks[i]];}
  let image;
  function text(value, x, y, size = 22, color = C.ink, face = 'Sora', weight = 400) {
    c.fillStyle = color; c.font = `${weight} ${size}px "${face}"`; c.textBaseline = 'alphabetic'; c.fillText(value, x, y);
  }
  function mono(value, x, y, size = 18, color = C.muted) { text(value, x, y, size, color, 'JetBrains Mono'); }
  function line(x1,y1,x2,y2,color=C.line,width=1) { c.strokeStyle=color;c.lineWidth=width;c.beginPath();c.moveTo(x1,y1);c.lineTo(x2,y2);c.stroke(); }
  const clamp = x => Math.max(0, Math.min(1, x));
  const smooth = x => { x=clamp(x); return x*x*(3-2*x); };
  function rect(x,y,w,h,color){c.fillStyle=color;c.fillRect(x,y,w,h);}
  function chapterAt(t){ return t<5?0:t<15?1:t<25?2:t<36?3:4; }
  function portrait(x,y,size,progress=1,source=false,t=0){
    const cols=20, tile=size/cols, n=Math.floor(clamp(progress)*cols*cols);
    rect(x,y,size,size,'#202824');
    c.save();c.beginPath();c.rect(x,y,size,size);c.clip();
    if(source || progress>=1){c.drawImage(image,0,0,image.width,image.height,x,y,size,size);}
    else {
      // Each complete tile stands for restored data; order is deliberately editorial.
      for(let i=0;i<cols*cols;i++){
        const rank=tileRanks[i], dx=(i%cols)*tile,dy=Math.floor(i/cols)*tile;
        if(rank<n){
          c.drawImage(image,(i%cols)*image.width/cols,Math.floor(i/cols)*image.height/cols,image.width/cols,image.height/cols,x+dx,y+dy,tile+.3,tile+.3);
        }else if(rank<n+9){rect(x+dx,y+dy,tile,tile,C.coral);}
        else {rect(x+dx+1,y+dy+1,tile-2,tile-2,((i*31)%7===0)?'#2c3932':'#202824');}
      }
      const scanY=y+((t*.19)%1)*size;
      rect(x,scanY,size,1,'#a9d9ba');
    }
    c.restore();
    // Registration marks belong to the image field, not a decorative card.
    const k=13;
    [[x-7,y-7,1,1],[x+size+7,y-7,-1,1],[x-7,y+size+7,1,-1],[x+size+7,y+size+7,-1,-1]].forEach(([a,b,h,v])=>{line(a,b,a+h*k,b,C.line);line(a,b,a,b+v*k,C.line);});
  }
  function qrImage(mode,idx){
    const key=`${mode}:${idx}`;
    if(qrCache.has(key))return qrCache.get(key);
    const rows=data.modes[mode].frames[idx].rows,n=rows.length+8;
    const qr=document.createElement('canvas');qr.width=n;qr.height=n;
    const q=qr.getContext('2d');q.fillStyle='#f2eee7';q.fillRect(0,0,n,n);q.fillStyle='#111313';
    rows.forEach((row,y)=>{let start=-1;for(let x=0;x<=row.length;x++){if(row[x]==='1'&&start<0)start=x;if(row[x]!=='1'&&start>=0){q.fillRect(start+4,y+4,x-start,1);start=-1;}}});
    qrCache.set(key,qr);return qr;
  }
  function header(ch,t){
    rect(0,0,1920,1080,C.bg);
    // The paired brackets are the mark: a sender and a receiver.
    line(102,72,88,86,C.coral,3);line(88,86,102,100,C.coral,3);
    line(124,72,138,86,C.coral,3);line(138,86,124,100,C.coral,3);
    text('shadow-ink',158,101,38,C.ink,'Sora',600);
    mono('TERMINAL  →  TERMINAL',1390,91,18,C.muted);
    text(titles[ch],100,208,ch===4?63:68,C.ink,'Sora',500);
    const subtitles=['A file can travel wherever a terminal can speak.','An image becomes independently checked protocol frames.','Change the symbol. Keep the guarantee.','The binary frame stays. The QR image goes.','Resumable transport. Verifiable arrival.'];
    text(subtitles[ch],102,253,25,C.muted);
    const cuts=[5,15,25,36,45];
    for(let i=0;i<5;i++){const x=100+i*347;rect(x,286,332,2,C.line);rect(x,286,332*clamp((t-starts[i])/(cuts[i]-starts[i])),2,i===ch?C.coral:C.mint);}
  }
  function terminal(ch){
    rect(100,319,1720,578,C.panel);
    line(100,319,1820,319);line(100,365,1820,365);line(100,319,100,897);line(1820,319,1820,897);
    line(960,365,960,856);
    mono('%0  sender / remote',126,349,18,C.ink);
    mono('%1  receiver / local',987,349,18,C.ink);
    mono(ch===4?'complete':'tmux · split-window -h',1538,349,15,ch===4?C.mint:C.muted);
    rect(100,856,1720,41,'#283e34');
    mono('shadow-ink  0:transfer*',124,883,17,C.mint);
    mono('RS 64/96',846,883,17,C.mint);
    mono('q: quit    space: pause    [local]',1400,883,15,C.mint);
  }
  function packets(t,progress,active){
    if(!active)return;
    const y=623;
    rect(932,575,57,90,C.panel);
    line(933,y,988,y,C.line);
    line(983,y-5,989,y,C.coral,1.5);line(989,y,983,y+5,C.coral,1.5);
    for(let i=0;i<3;i++){
      const phase=((t*1.3+i/3)%1),x=934+phase*48;
      rect(x,y-3,6,6,i===0?C.mint:C.coral);
    }
    mono('TX',907,602,13,C.coral);mono('RX',991,602,13,C.mint);
  }
  function progressBar(progress,ch,t){
    const x=1135,w=470,y=807;
    text(progress>=1?'File restored':'Reconstructing image',x,y-17,19,progress>=1?C.mint:C.muted);
    mono(String(Math.floor(progress*100)).padStart(3,' ')+'%',x+w-57,y-17,18,progress>=1?C.mint:C.ink);
    rect(x,y,w,3,C.line);rect(x,y,w*progress,3,progress>=1?C.mint:C.coral);
    if(ch>0&&progress<1)mono('illustrative reveal · frames are real',1135,839,14,C.muted);
    else mono('original_sha256 matches',1135,839,15,C.mint);
  }
  function intro(t){
    mono('$ shadow-ink remote '+data.filename,130,402,19,C.mint);
    portrait(335,436,350,1,true,t);
    mono(data.filename,335,823,17,C.ink);
    mono(`${data.bytes.toLocaleString('en-US')} bytes  ·  immutable snapshot`,1055,404,18,C.muted);
    portrait(1205,436,330,clamp((t-1.4)/3.3)*.12,false,t);
    text('Waiting on the other side.',1135,825,22,C.muted);
    packets(t,0,t>1.3);
  }
  function transmission(ch,t){
    const start=starts[ch],len=ch===3?11:10,u=t-start;
    const p=smooth((u-.9)/(len-2.0));
    const mode=ch===1?'qr12':ch===2?'qr40':'pipe',set=data.modes[mode];
    const idx=Math.floor(Math.max(0,u)*((ch===1)?8:18))%set.frames.length,frame=set.frames[idx];
    const cmd=ch===3?`$ shadow-ink remote ${data.filename} --backend pipe`:`$ shadow-ink remote ${data.filename} --backend qr --speed ${ch===1?'safe':'fast'}`;
    mono(cmd,130,402,17,C.mint);
    if(ch<3){
      const qr=qrImage(mode,idx),scale=ch===1?5:2,size=qr.width*scale;
      const x=530-size/2,y=448+(370-size)/2;
      c.imageSmoothingEnabled=false;c.drawImage(qr,x,y,size,size);c.imageSmoothingEnabled=true;
      mono(`V${set.version}-${set.ecc}   ${set.symbolBytes.toLocaleString('en-US')} B / shard`,300,841,17,C.muted);
      mono(`seq ${String(frame.sequence).padStart(6,'0')}  g ${frame.generation}  s ${frame.symbol}`,145,463,14,C.muted);
    }else{
      c.save();c.beginPath();c.rect(129,447,798,369);c.clip();
      const f=set.frames[Math.floor(u*19)%set.frames.length];
      for(let row=0;row<17;row++){
        // Actual contiguous envelope bytes, soft-wrapped with no invented prefix.
        const content=f.line.slice(row*74,(row+1)*74);
        mono(content,132,467+row*21,15,row===15?C.coral:row%4===0?C.mint:'#9caea3');
      }
      c.restore();
      mono('Base64 / CRC32C / tmux pipe-pane',247,841,17,C.muted);
      mono('SI1 envelope · soft-wrapped excerpt',132,435,13,C.muted);
    }
    mono('$ shadow-ink recv --source '+(ch===3?'pipe:%0':'tmux:%0'),989,402,18,C.mint);
    portrait(1205,436,330,p,false,t);
    progressBar(p,ch,t);
    packets(t,p,p<1);
    // Concise state appears in the receiver's actual code vocabulary.
    mono(p>=1?'complete: true':`cached_symbols: ${Math.floor(p*64)}  /  recovering`,990,427,14,p>=1?C.mint:C.muted);
  }
  function finale(t){
    mono('$ shadow-ink status ./rx-state',130,402,19,C.mint);
    text('Transfer complete.',148,487,46,C.ink,'Sora',500);
    mono('complete: true',150,537,22,C.mint);
    mono(`original_bytes: ${data.bytes.toLocaleString('en-US')}`,150,576,19,C.muted);
    mono('original_sha256',150,636,17,C.muted);
    mono(data.sha256.slice(0,32),150,674,21,C.ink);
    mono(data.sha256.slice(32),150,709,21,C.ink);
    line(150,749,843,749);
    text('One command. Pick up where you left off.',150,798,23,C.muted);
    mono('byte-for-byte',989,402,18,C.mint);
    portrait(1205,436,330,1,true,t);
    progressBar(1,4,t);
  }
  function drawMinimal(t){
    const set=data.modes[minimalMode];
    const p=smooth((t-.4)/8.1);
    const idx=Math.floor(t*(minimalMode==='qr12'?8:18))%set.frames.length;
    rect(0,0,1920,1080,C.bg);
    rect(72,72,1776,936,C.panel);
    line(72,72,1848,72);line(72,124,1848,124);
    line(72,72,72,1008);line(1848,72,1848,1008);
    line(960,124,960,960);
    mono(minimalMode==='pipe'?'%0  Base64':`%0  QR V${set.version}-${set.ecc}`,100,107,20,C.ink);
    mono('%1  receiver',996,107,20,C.ink);
    if(minimalMode==='pipe'){
      if(!base64Frames){
        // Prefer actual data-rich shards over padding-only FEC shards in the film.
        const rich=set.frames.filter(frame=>frame.object===1 && new Set(frame.line.slice(84,1984)).size>=40);
        base64Frames=rich.length?rich:set.frames;
      }
      const frame=base64Frames[idx%base64Frames.length];
      c.save();c.beginPath();c.rect(112,214,806,672);c.clip();
      for(let row=0;row<25;row++){
        mono(frame.line.slice(row*76,(row+1)*76),112,238+row*26,17,row===23?C.coral:row%4===0?C.mint:C.muted);
      }
      c.restore();
    }else{
      const qr=qrImage(minimalMode,idx),scale=minimalMode==='qr12'?9:3,size=qr.width*scale;
      c.imageSmoothingEnabled=false;
      c.drawImage(qr,516-Math.floor(size/2),550-Math.floor(size/2),size,size);
      c.imageSmoothingEnabled=true;
    }
    portrait(1070,225,650,p,false,t);
    if(p<1){
      rect(888,518,144,64,C.panel);
      line(896,550,1027,550,C.line,2);
      line(1020,544,1028,550,C.coral,2);line(1028,550,1020,556,C.coral,2);
      for(let i=0;i<4;i++){
        const phase=(t*1.3+i/4)%1;
        rect(898+phase*118,546,8,8,i===0?C.mint:C.coral);
      }
    }
    rect(1070,915,650,3,C.line);
    rect(1070,915,650*p,3,p>=1?C.mint:C.coral);
    rect(72,960,1776,48,'#283e34');
    mono('shadow-ink  0:transfer*',100,992,19,C.mint);
  }
  function draw(t){
    if(minimalMode){drawMinimal(t);return;}
    const ch=chapterAt(t);header(ch,t);terminal(ch);
    if(ch===0)intro(t);else if(ch===4)finale(t);else transmission(ch,t);
    text(captions[ch],100,963,27,ch===4?C.mint:C.ink);
    mono('Real protocol frames · choreographed playback · not a benchmark',102,1005,17,C.muted);
    mono('github.com/Chivier/shadow-ink',1370,1005,17,C.muted);
    if(!recording){
      $('#seek').value=t;$('#time').value=`00:${String(Math.floor(t)).padStart(2,'0')} / 00:45`;
      if($('#caption').textContent!==captions[ch])$('#caption').textContent=captions[ch];
      document.querySelectorAll('[data-time]').forEach((button,i)=>button.setAttribute('aria-current',String(i===ch)));
    }
  }
  function setPlaying(value){
    if(!ready)return;
    playing=value;lastClock=performance.now();
    $('#play span').textContent=value?'Pause':'Play';$('#play').setAttribute('aria-label',value?'Pause film':'Play film');
    $('#play svg').innerHTML=value?'<path d="M7 5h3v14H7zm7 0h3v14h-3z"/>':'<path d="M8 5l11 7-11 7z"/>';
    if(value&&!raf)raf=requestAnimationFrame(tick);
  }
  function tick(now){
    raf=0;if(!playing)return;
    seconds=Math.min(DURATION,seconds+(now-lastClock)/1000);lastClock=now;draw(seconds);
    if(seconds>=DURATION)setPlaying(false);else raf=requestAnimationFrame(tick);
  }
  function seek(value){if(!ready)return seconds;seconds=Math.max(0,Math.min(DURATION,Number(value)));lastClock=performance.now();draw(seconds);return seconds;}
  $('#source').value=reference?'reference':'sample';
  $('#source').addEventListener('change',event=>{const next=new URLSearchParams(location.search);if(event.target.value==='reference')next.set('reference','1');else next.delete('reference');location.search=next.toString();});
  $('#retry').addEventListener('click',()=>location.reload());
  $('#play').addEventListener('click',()=>{if(seconds>=DURATION)seek(0);setPlaying(!playing);});
  $('#replay').addEventListener('click',()=>{seek(0);setPlaying(true);});
  $('#seek').addEventListener('input',e=>seek(e.target.value));
  document.querySelectorAll('[data-time]').forEach(button=>button.addEventListener('click',()=>seek(button.dataset.time)));
  $('#fullscreen').addEventListener('click',async()=>{try{if(document.fullscreenElement)await document.exitFullscreen();else await $('.film-wrap').requestFullscreen();}catch{ $('#caption').textContent='Fullscreen is unavailable. Open the demo in a browser window.'; }});
  document.addEventListener('keydown',event=>{if(!ready)return;if(['INPUT','BUTTON','SUMMARY','A','SELECT'].includes(event.target.tagName))return;if(event.code==='Space'){event.preventDefault();setPlaying(!playing);}if(event.code==='ArrowRight'){event.preventDefault();seek(seconds+5);}if(event.code==='ArrowLeft'){event.preventDefault();seek(seconds-5);}});
  try{
    await Promise.all([document.fonts.load('500 68px "Sora"'),document.fonts.load('400 19px "JetBrains Mono"')]);
    if(reference){
      await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='assets/local-payload.js';script.onload=resolve;script.onerror=reject;document.head.append(script);});
      data=window.SHADOW_INK_PAYLOAD;
    }
    const raw=new Image();raw.src=reference?'assets/reference.png':'assets/sample.svg';await raw.decode();
    // Crop is a display choice only: the original screenshot bytes are the payload.
    image=document.createElement('canvas');image.width=600;image.height=600;
    const crop=reference?(data.crop||[0,0,raw.width,raw.height]):[0,0,raw.width,raw.height];
    image.getContext('2d').drawImage(raw,...crop,0,0,600,600);
    window.demo={seek,pause:()=>setPlaying(false),play:()=>setPlaying(true),get time(){return seconds;},get payload(){return data;},duration:DURATION,canvas};
    ready=true;
    document.querySelectorAll('[data-needs-ready]').forEach(control=>control.disabled=false);
    $('#loading').hidden=true;
    seek(Number(params.get('t')||0));
    window.DEMO_READY=true;
    if(params.get('autoplay')==='1'&&!matchMedia('(prefers-reduced-motion: reduce)').matches)setPlaying(true);
  }catch(error){$('#loading').textContent='Could not load this image. Switch to the bundled sample below, or prepare the local reference with demo/prepare.py and retry.';$('#source').disabled=false;$('#retry').hidden=false;console.error(error);}
})();
