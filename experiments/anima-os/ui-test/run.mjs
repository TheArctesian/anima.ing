import {mkdtempSync,readFileSync,writeFileSync,copyFileSync,mkdirSync,existsSync,rmSync,appendFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const source=resolve(process.env.ANIMA_UI_SOURCE||resolve(import.meta.dir,'../console')),out=resolve(process.argv[2]||mkdtempSync(join(tmpdir(),'anima-ui-results-')));mkdirSync(out,{recursive:true});
const folder=mkdtempSync(join(tmpdir(),'anima-ui-'));
for(const name of ['index.html','style.css','ui.js','telemetry.js'])copyFileSync(join(source,name),join(folder,name));
copyFileSync(resolve(import.meta.dir,'../branding/wordmark.svg'),join(folder,'wordmark.svg'));
copyFileSync(join(import.meta.dir,'fixture.js'),join(folder,'fixture.js'));
const fontDirectory=process.env.ANIMA_TEST_FONT_DIRECTORY;
if(fontDirectory)for(const [weight,name] of [[300,'Light'],[400,'Regular'],[500,'Medium']]){
 copyFileSync(join(fontDirectory,`IBMPlexSans-${name}.ttf`),join(folder,`${weight}.ttf`));
 appendFileSync(join(folder,'style.css'),`\n@font-face{font-family:'IBM Plex Sans';font-weight:${weight};src:url('${weight}.ttf') format('truetype');}\n`);
}

writeFileSync(join(folder,'index.html'),readFileSync(join(folder,'index.html'),'utf8').replace('qrc:///qtwebchannel/qwebchannel.js','fixture.js'));
const profile=mkdtempSync(join(tmpdir(),'anima-chromium-'));
const child=Bun.spawn(['chromium','--headless','--disable-background-networking','--disable-component-update','--disable-sync','--disable-gpu','--no-first-run','--no-default-browser-check','--remote-debugging-port=0','--remote-allow-origins=*','--user-data-dir='+profile,'file://'+join(folder,'index.html')],{stdout:'ignore',stderr:'ignore'});
try{
 for(let i=0;i<100&&!existsSync(join(profile,'DevToolsActivePort'));i++)await Bun.sleep(50);
 const port=readFileSync(join(profile,'DevToolsActivePort'),'utf8').split('\n')[0];
 const targets=await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
 const target=targets.find(t=>t.type==='page');const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws.onopen=r);
 let next=0;const waiting=new Map();ws.onmessage=e=>{const r=JSON.parse(e.data);if(r.id){waiting.get(r.id)?.(r);waiting.delete(r.id);}};
 const cmd=(method,params={})=>new Promise((resolve,reject)=>{const id=++next;const timer=setTimeout(()=>{waiting.delete(id);reject(Error('Browser command timed out: '+method));},20000);waiting.set(id,r=>{clearTimeout(timer);r.error?reject(Error(JSON.stringify(r.error))):resolve(r.result);});ws.send(JSON.stringify({id,method,params}));});
 const js=async expression=>{const r=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.text+JSON.stringify(r.exceptionDetails));return r.result.value;};
 const until=async expression=>{for(let i=0;i<100;i++){if(await js(expression))return true;await Bun.sleep(50);}return false;};
 const click=async selector=>{if(!await until(`!!document.querySelector(${JSON.stringify(selector)})`))throw Error('Missing control '+selector);await js(`document.querySelector(${JSON.stringify(selector)}).focus();document.querySelector(${JSON.stringify(selector)}).click()`);await until('fixture.pending <= (fixture.download?1:0)');await Bun.sleep(50);};
 const assert=async(expression,message)=>{if(!await until(expression))throw Error(message);};
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
 await Bun.sleep(300);await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);await Bun.sleep(200);
 await assert(`JSON.stringify([...document.querySelectorAll('nav [data-page]')].map(e=>e.dataset.page))===JSON.stringify(['Overview','Network','Storage','Settings'])`,'Four destinations');
 await click('[data-action=sign-in]');await assert(`fixture.calls.includes('sign-in')`,'One action starts sign-in');await click('[data-action=cancel-sign-in]');
 await click('[data-page=Settings]');await click('[data-page=Display]');await assert(`document.querySelector('h1').textContent==='Display & accessibility'`,'Display reachable');await click('[data-action=settings-back]');await assert(`document.activeElement.dataset.page==='Display'`,'Settings back restores focus');
 await click('[data-page=Network]');await click('[data-action=scan]');await assert(`document.querySelector('main').textContent.includes('No networks found')`,'Empty scan differs from not scanned');
 await js('fixture.failScan=true');await click('[data-action=scan]');await click('[data-action=close]');await assert(`document.querySelector('main').textContent.includes('scan could not finish')`,'Scan failure remains visible');
 await click('[data-page=Settings]');await click('[data-page=Updates]');await click('[data-action=update-check]');await click('[data-action=update-download]');await click('nav [data-page=Network]');await Bun.sleep(1100);await assert(`document.querySelector('#operation-status').textContent.includes('Downloading')`,'Real progress available across navigation');await assert('!fixture.download','Download finishes');
 await click('nav [data-page=Overview]');await js('fixture.enrolled=true');await Bun.sleep(20);await click('nav [data-page=Settings]');await click('[data-page=Recovery]');await click('[data-action=diagnostics]');await assert(`document.querySelector('.dialog').textContent.includes('inactive')`,'Diagnostics remains reachable');await click('[data-action=close]');
 await click('nav [data-page=Overview]');
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});await click('nav [data-page=Settings]');await Bun.sleep(350);await assert(`document.querySelectorAll('.page-outgoing').length===0`,'Finished transitions leave no hidden page');await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
 const records=[];
 for(const [width,height] of [[1280,800],[800,600],[640,480],[800,1280],[1920,1080]]){
  await cmd('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});await Bun.sleep(100);
  for(const page of ['Overview','Network','Storage','Settings']){
   await click(`nav [data-page=${page}]`);
   const geometry=await js(`({overflow:document.documentElement.scrollWidth>innerWidth,nav:[...document.querySelectorAll('nav button')].map(e=>{const r=e.getBoundingClientRect();return {label:e.textContent,width:r.width,height:r.height,x:r.x,right:r.right};}),footer:document.querySelector('footer').getBoundingClientRect().bottom,main:document.querySelector('main').getBoundingClientRect().height})`);
   if(geometry.overflow||geometry.footer>height+1||geometry.main<50||geometry.nav.some(n=>n.width<24||n.height<24||n.x<0||n.right>width+1))throw Error('Viewport geometry '+JSON.stringify({width,height,page,geometry}));
   const shot=await cmd('Page.captureScreenshot',{format:'png'});writeFileSync(join(out,`${page.toLowerCase()}-${width}x${height}.png`),Buffer.from(shot.data,'base64'));records.push({width,height,page,...geometry});
  }
 }
 await cmd('Input.dispatchKeyEvent',{type:'keyDown',key:'l',code:'KeyL',modifiers:3});await assert(`!!document.querySelector('#unlock-form')`,'Keyboard lock remains available');
 writeFileSync(join(out,'results.json'),JSON.stringify({status:'passed',fontDirectory:fontDirectory||'Host font fallback; installed appliance uses IBM Plex',scope:'Synthetic transport, real product HTML/CSS/JS in headless Chromium; not appliance or hardware acceptance',checks:records},null,2));
 console.log('PASS: direct sign-in, four destinations, settings/back focus, scan empty/failure, concurrent update progress, diagnostics, 20 responsive screenshots. Evidence: '+out);
 ws.close();
}finally{child.kill();await child.exited;rmSync(profile,{recursive:true,force:true});rmSync(folder,{recursive:true,force:true});}
