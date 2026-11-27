import {checkHero,checkInstalledMotionPreference} from './hero.mjs';
import {mkdtempSync,readFileSync,writeFileSync,copyFileSync,mkdirSync,existsSync,rmSync,appendFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {checkMaintenance,checkUpdateReview} from './maintenance.mjs';
import {checkDisplay} from './display.mjs';
import {checkNetwork} from './network.mjs';
import {checkMigration} from './migration.mjs';
import {checkStorage} from './storage.mjs';
import {checkUpdateProgress,checkUpdateWindow} from './updates.mjs';
import {checkApplicationTerminal,checkBuildProgress,checkWorkAdmission,checkWorkloadRefresh,checkWorkloadViews} from './workloads.mjs';
const source=resolve(process.env.ANIMA_UI_SOURCE||resolve(import.meta.dir,'../../console')),out=resolve(process.argv[2]||mkdtempSync(join(tmpdir(),'anima-ui-results-')));mkdirSync(out,{recursive:true});
const folder=mkdtempSync(join(tmpdir(),'anima-ui-'));
for(const name of ['index.html','style.css','lock-screen.css','ui.js','telemetry.js','workloads.js','hero-points.js','hero-motion.js'])copyFileSync(join(source,name),join(folder,name));
for(const name of ['dialog.js','remote-access.js','maintenance.js','activity.js','presentation.js','pages.js','display-settings.js','storage.js', 'work-admission.js', 'log-snapshot.js', 'updates.js', 'enrollment.js'])if(existsSync(join(source,name)))copyFileSync(join(source,name),join(folder,name));
copyFileSync(resolve(import.meta.dir,'../../branding/wordmark.svg'),join(folder,'wordmark.svg'));
copyFileSync(resolve(import.meta.dir,'../../branding/hero.png'),join(folder,'hero.png'));
copyFileSync(join(import.meta.dir,'fixture.js'),join(folder,'fixture.js'));
const fontDirectory=process.env.ANIMA_TEST_FONT_DIRECTORY;
if(fontDirectory)for(const [weight,name] of [[300,'Light'],[400,'Regular'],[500,'Medium']]){
 copyFileSync(join(fontDirectory,`IBMPlexSans-${name}.ttf`),join(folder,`${weight}.ttf`));
 appendFileSync(join(folder,'style.css'),`\n@font-face{font-family:'IBM Plex Sans';font-weight:${weight};src:url('${weight}.ttf') format('truetype');}\n`);
}

writeFileSync(join(folder,'index.html'),readFileSync(join(folder,'index.html'),'utf8').replace('qrc:///qtwebchannel/qwebchannel.js','fixture.js'));
const profile=mkdtempSync(join(tmpdir(),'anima-chromium-'));
const child=Bun.spawn([process.env.ANIMA_TEST_CHROMIUM||'chromium','--headless','--disable-background-networking','--disable-component-update','--disable-sync','--disable-gpu','--no-first-run','--no-default-browser-check','--remote-debugging-port=0','--remote-allow-origins=*','--user-data-dir='+profile,'file://'+join(folder,'index.html')],{stdout:'ignore',stderr:'ignore'});
try{
 for(let i=0;i<100&&!existsSync(join(profile,'DevToolsActivePort'));i++)await Bun.sleep(50);
 const port=readFileSync(join(profile,'DevToolsActivePort'),'utf8').split('\n')[0];
 let target;
 for(let i=0;i<100&&!target;i++){
  const targets=await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  target=targets.find(entry=>entry.type==='page');
  if(!target)await Bun.sleep(50);
 }
 if(!target)throw Error('Chromium did not expose the console page.');
 const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws.onopen=r);
 let next=0;const waiting=new Map();ws.onmessage=e=>{const r=JSON.parse(e.data);if(r.id){waiting.get(r.id)?.(r);waiting.delete(r.id);}};
 const cmd=(method,params={})=>new Promise((resolve,reject)=>{const id=++next;const timer=setTimeout(()=>{waiting.delete(id);reject(Error('Browser command timed out: '+method));},20000);waiting.set(id,r=>{clearTimeout(timer);r.error?reject(Error(JSON.stringify(r.error))):resolve(r.result);});ws.send(JSON.stringify({id,method,params}));});
 const js=async expression=>{const r=await cmd('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.text+JSON.stringify(r.exceptionDetails));return r.result.value;};
 const until=async expression=>{for(let i=0;i<100;i++){if(await js(expression))return true;await Bun.sleep(50);}return false;};
 const click=async selector=>{
  const activated=`(()=>{const control=[...document.querySelectorAll(${JSON.stringify(selector)})].find(node=>
    !node.closest('[inert]')&&!node.matches(':disabled')&&node.checkVisibility({visibilityProperty:true}));
    if(!control)return false;control.focus();control.click();return true;})()`;
  if(!await until(activated))throw Error('Control not available '+selector);
  await until('fixture.pending <= (fixture.download?1:0)');await Bun.sleep(50);
 };
 const assert=async(expression,message)=>{if(!await until(expression))throw Error(message);};
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
 await Bun.sleep(300);
 await assert(`!!document.querySelector('.overlay [data-action=screen-off]')?.checkVisibility()`,'Locked console exposes a visible screen-off action without a password');
 await cmd('Emulation.setDeviceMetricsOverride',{width:320,height:480,deviceScaleFactor:1,mobile:false});
 await assert(`(()=>{const r=document.querySelector('.overlay [data-action=screen-off]').getBoundingClientRect();return r.top>=0&&r.bottom<=innerHeight&&r.left>=0&&r.right<=innerWidth})()`,'Locked screen-off control fits a 320px display without scrolling');
 const lockedShot=await cmd('Page.captureScreenshot',{format:'png'});writeFileSync(join(out,'locked-320x480.png'),Buffer.from(lockedShot.data,'base64'));
 await click('.overlay [data-action=screen-off]');
 await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 await assert(`fixture.screenOff===1&&!!document.querySelector('#unlock-form')&&!fixture.calls.includes('unlock')&&!fixture.calls.includes('shutdown')&&!fixture.calls.includes('restart')`,'Screen-off while locked needs no unlock or power operation');
 await js(`fixture.failUnlock=true;document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await assert(`(()=>{const input=document.querySelector('#unlock-form input'),alert=document.getElementById(input.getAttribute('aria-describedby'));
  return document.activeElement===input&&input.value===''&&alert?.getAttribute('role')==='alert'&&alert.textContent.includes('The local request could not finish.');})()`,
  'Failed unlock announces recovery at the cleared password field and keeps keyboard focus there');
 await js(`delete fixture.failUnlock;document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await assert(`document.querySelector('.overlay').hidden&&document.activeElement===document.querySelector('main h1')`,'First unlock puts keyboard focus on the current page heading');
 await assert(`document.querySelector('#telemetry-readings').textContent.includes('7%')&&document.querySelector('#battery-status .battery')`,'Overview shows the available observation');
 await js(`fixture.telemetrySnapshot=null;consoleBridge.telemetry()`);
 await assert(`document.querySelector('#telemetry-readings').textContent.includes('Local readings unavailable')&&!document.querySelector('#battery-status .battery')&&document.querySelector('header [data-remote-connections]').textContent.includes('unavailable')`,'An unavailable publication clears readings, battery badges and transport counts');
 await js(`delete fixture.telemetrySnapshot;consoleBridge.telemetry()`);
 await assert(`document.querySelector('#telemetry-readings').textContent.includes('7%')&&document.querySelector('#battery-status .battery')`,'A fresh observation restores the console readings');
 await assert(`JSON.stringify([...document.querySelectorAll('nav [data-page]')].map(e=>e.dataset.page))===JSON.stringify(['Overview','Workloads','Activity','Network','Storage','Settings'])`,'Console destinations include Activity');
 await click('nav [data-page=Activity]');
 await assert(`fixture.calls.includes('activity')&&document.querySelector('[data-service-events] time')?.dateTime==='2026-11-24T11:00:00.000Z'&&document.querySelector('main').textContent.includes('Private link')`,'Activity presents serialized service transitions with a machine-readable time');
 await assert(`document.querySelector('[data-action-history]')?.textContent.includes('admin · Accepted')&&document.querySelector('[data-action-history] time')?.dateTime==='2026-11-24T11:00:00.001Z'`,'Activity shows the local account and recorded action result');
 await js(`fixture.actions={entries:[{action:'Restart',account:'<img data-untrusted-actor>',outcome:'Outcome unavailable',atMs:1795518000001}],complete:false}`);
 await click('main [data-action=activity-refresh]');
 await assert(`document.querySelector('[data-action-history]').textContent.includes('<img data-untrusted-actor> · Outcome unavailable')&&!document.querySelector('[data-untrusted-actor]')&&document.querySelector('[data-action-history]').textContent.includes('Some local actions could not be recorded.')`,'Activity escapes account names and distinguishes unconfirmed and incomplete history');
 await js('delete fixture.actions');
 await click('main [data-page=Recovery]');
 await assert(`document.querySelector('main h1').textContent==='Diagnostics & recovery'`,'Activity opens local diagnostics');
 await js(`fixture.attention={entries:[{workspace:'abc12345-0000-4000-8000-000000000001',generation:3,waitingThreads:1}],complete:true}`);
 await click('nav [data-page=Overview]');
 await assert(`document.querySelector('main [data-page=Activity]')?.textContent.includes('Needs you')`,'Overview shows a link for sessions waiting for a response');
 await click('main [data-page=Activity]');
 await assert(`document.querySelector('main').textContent.includes('Workspace abc12345')&&document.querySelector('main').textContent.includes('Open Code in an Anima app')`,'Activity scopes the request count and directs replies to Code');
 await js(`fixture.attention={entries:[],complete:false}`);
 await click('main [data-action=activity-refresh]');
 await assert(`document.querySelector('main').textContent.includes('Some coding sessions could not be checked.')&&!document.querySelector('main').textContent.includes('No coding sessions are waiting')`,'Unobserved workspaces are not treated as idle');
 await click('nav [data-page=Overview]');
 await assert(`document.querySelector('main [data-page=Activity]')?.textContent.includes('Current status unavailable')`,'An incomplete check keeps the previous Needs you signal without claiming a current count');
 await click('main [data-page=Activity]');
 await js(`delete fixture.attention`);
 await click('main [data-action=activity-refresh]');
 await assert(`document.querySelector('main').textContent.includes('No coding sessions are waiting for a response.')&&!document.querySelector('main').textContent.includes('Workspace abc12345')`,'A complete zero count clears the earlier prompt');
 await click('nav [data-page=Overview]');
 await assert(`!document.querySelector('main [data-page=Activity]')`,'A complete empty sample clears the Overview signal');
 await click('[data-action=sign-in]');await assert(`fixture.calls.includes('sign-in')`,'One action starts sign-in');await click('[data-action=cancel-sign-in]');
 await click('[data-action=selfhost]');await assert(`fixture.enrollments.length===0`,'Opening enrollment does not connect');
 await assert(`document.querySelector('[name=installCode]').type==='password'&&document.querySelector('.dialog').textContent.includes('private SSH, RDP and VNC')`,'Private enrollment starts with the install-code field');
 await click('[data-action=selfhost-direct]');
 await assert(`document.querySelector('[name=hub]').required&&document.querySelector('[name=code]').required&&fixture.enrollments.length===0`,'Monitoring enrollment remains a distinct form');
 await click('.dialog [data-action=selfhost]');
 await js(`document.querySelector('[name=installCode]').value='synthetic-install-code';document.querySelector('#install-code-form').requestSubmit()`);
 await until('fixture.pending===0');
 await assert(`fixture.calls.includes('enroll-install')&&fixture.enrollments.length===1&&fixture.enrollments[0].code==='synthetic-install-code'&&!document.querySelector('[name=installCode]')&&!document.body.textContent.includes('synthetic-install-code')`,'Explicit submit sends the install code and clears it');
 await js(`fixture.remotePorts=[22,3389];fixture.remoteChecks={RDP:'no-reply'}`);
 await click('main [data-action=connection-details]');
 await assert(`(()=>{
  const dialog=document.querySelector('.dialog');
  return ['Enrollment','Monitoring','Local protocol check passed','Service did not respond in time','Private access off']
   .every(text=>dialog.textContent.includes(text));
 })()`,'Connection details exposes partial remote access immediately after enrollment');
 await js('fixture.failRemoteCheck=true');
 await click('.dialog [data-action=remote-access-status]');
 await assert(`document.querySelector('[data-remote-check]').textContent.includes('previous observation')&&
  document.querySelector('[data-remote-access-results]').textContent.includes('Service did not respond in time')&&
  document.activeElement.dataset.action==='remote-access-status'&&fixture.enrollments.length===1`,
 'A failed recheck keeps the partial observation and retry focus without reenrolling');
 await click('.dialog [data-action=close]');
 await js('delete fixture.failRemoteCheck');
 await js('fixture.enrolled=false');

 await click('[data-page=Settings]');await checkDisplay({js,click,assert});
 const previousRemoteReads=await js(`fixture.calls.filter(op=>op==='remote-access-status').length`);
 await js(`fixture.remotePorts=[22,3389];fixture.remoteChecks={RDP:'unexpected'}`);
 await click('main [data-action=remote-access-status]');
 await assert(`(()=>{const dialog=document.querySelector('.dialog');return ['Local protocol check passed','Unexpected protocol or security settings','Private access off','verify private-link access and sign-in'].every(text=>dialog.textContent.includes(text));})()`,'Private access shows distinct local observations and keeps sign-in separate');
 await js(`fixture.remoteChecks={RDP:'unavailable'}`);
 await click('.dialog [data-action=remote-access-status]');
 await assert(`document.querySelector('.dialog').textContent.includes('Service unavailable')&&!document.querySelector('.dialog').textContent.includes('Unexpected protocol')&&fixture.calls.filter(op=>op==='remote-access-status').length===${previousRemoteReads+2}&&!fixture.calls.includes('remote-access-set')&&document.activeElement.dataset.action==='remote-access-status'`,'Checking again refreshes observations without writing access choices or moving focus');
 await click('.dialog [data-action=close]');
 await js(`fixture.remoteOrigin='pending-defaults';fixture.remoteConfigured=false;fixture.remotePorts=[];delete fixture.remoteChecks;`);
 await click('main [data-action=remote-access-status]');
 await assert(`!!document.querySelector('[data-action=remote-access-retry-defaults]')&&!fixture.calls.includes('remote-access-retry-defaults')`,'Pending defaults offer an explicit retry');
 await click('.dialog [data-action=remote-access-retry-defaults]');
 await assert(`fixture.calls.filter(op=>op==='remote-access-retry-defaults').length===1&&
  document.querySelectorAll('[data-remote-access-results] .row').length===3&&
  !document.querySelector('[data-action=remote-access-retry-defaults]')&&fixture.enrollments.length===1`,
 'Retry refreshes the protocol observations without a second enrollment');
 await click('.dialog [data-action=close]');
 await js(`delete fixture.remotePorts;delete fixture.remoteChecks;delete fixture.remoteConfigured;delete fixture.remoteOrigin`);
 await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 const shot=async name=>{const image=await cmd('Page.captureScreenshot',{format:'png'});writeFileSync(join(out,name+'.png'),Buffer.from(image.data,'base64'));};
 await checkNetwork({js,click,assert,until,shot});
 await checkStorage({js,click,assert});
 await checkMigration({js,click,assert,shot});
 await checkBuildProgress({js,click,assert});
 await checkApplicationTerminal({js,click,assert});
 await checkWorkloadViews({js,click,assert,cmd});
 await checkWorkloadRefresh({js,click,assert});
 await checkWorkAdmission({js,click,assert});
 await checkMaintenance({js,click,assert});
 await checkUpdateReview({js,click,assert});
 await checkUpdateWindow({js,click,assert,cmd,shot});
 await checkUpdateProgress({js,click,assert});
 await click('nav [data-page=Overview]');await js('fixture.enrolled=true');await Bun.sleep(20);await click('nav [data-page=Settings]');await click('[data-page=Recovery]');await click('[data-action=diagnostics]');await assert(`document.querySelector('.dialog').textContent.includes('inactive')`,'Diagnostics remains reachable');await click('[data-action=close]');
 await click('nav [data-page=Overview]');
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});await click('nav [data-page=Settings]');await Bun.sleep(350);await assert(`document.querySelectorAll('.page-outgoing').length===0`,'Finished transitions leave no hidden page');await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
 await js(`fixture.scan=[{device:'wlan0',bssid:'00:11:22:33:44:01',ssid:'WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW',signal:'95',security:'WPA3'},{device:'wlan0',bssid:'00:11:22:33:44:02',ssid:'Guest',signal:'64',security:'--'}]`);
 await click('nav [data-page=Network]');await click('[data-action=scan]');
 await cmd('Emulation.setDeviceMetricsOverride',{width:320,height:640,deviceScaleFactor:1,mobile:false});await Bun.sleep(100);
 await assert(`[...document.querySelectorAll('nav button')].every(button=>button.getAttribute('aria-label')&&button.title&&button.querySelector('.glyph').checkVisibility()&&button.offsetHeight>=44&&button.getBoundingClientRect().right<=innerWidth)`,'All compact navigation controls have visible icons, names and tooltips within the viewport');
 await assert(`[...document.querySelectorAll('.wifi-network,.network-connect,.network-heading button')].every(e=>{const r=e.getBoundingClientRect(),m=document.querySelector('main').getBoundingClientRect();return r.left>=m.left&&r.right<=m.right;})`,'Wi-Fi controls remain inside narrow screen with unbroken network name');
 await assert(`[...document.querySelectorAll('.wifi-network')].every(e=>e.getBoundingClientRect().height>=44)`,'Wi-Fi row targets remain usable on narrow screen');await shot('network-320x640');
 const records=[];
 for(const [width,height] of [[1280,800],[800,600],[640,480],[800,1280],[1920,1080]]){
  await cmd('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});await Bun.sleep(100);
  for(const page of ['Overview','Workloads','Activity','Network','Storage','Settings']){
   await click(`nav [data-page=${page}]`);
   const geometry=await js(`({overflow:document.documentElement.scrollWidth>innerWidth,nav:[...document.querySelectorAll('nav button')].map(e=>{const r=e.getBoundingClientRect();return {label:e.textContent,width:r.width,height:r.height,x:r.x,right:r.right};}),footer:document.querySelector('footer').getBoundingClientRect().bottom,main:document.querySelector('main').getBoundingClientRect().height})`);
   if(geometry.overflow||geometry.footer>height+1||geometry.main<50||geometry.nav.some(n=>n.width<24||n.height<24||n.x<0||n.right>width+1))throw Error('Viewport geometry '+JSON.stringify({width,height,page,geometry}));
   if(page==='Network')await assert(`document.querySelector('.wifi-network').getBoundingClientRect().top+44<=document.querySelector('main').getBoundingClientRect().bottom`,'First Wi-Fi network is visible without scrolling at supported sizes');
   const shot=await cmd('Page.captureScreenshot',{format:'png'});writeFileSync(join(out,`${page.toLowerCase()}-${width}x${height}.png`),Buffer.from(shot.data,'base64'));records.push({width,height,page,...geometry});
  }
 }
 await assert(`document.querySelector('footer [data-action=screen-off]').textContent.includes('Turn screen off')`,'Footer screen-off action has a visible text label');
 await click('footer [data-action=screen-off]');await assert(`fixture.screenOff===2&&!!document.querySelector('#unlock-form')&&!fixture.calls.includes('shutdown')&&!fixture.calls.includes('restart')`,'Screen-off uses the native display action and locks without a power operation');
 await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);await until('fixture.pending===0');
 await click('nav [data-page=Activity]');
 await cmd('Input.dispatchKeyEvent',{type:'keyDown',key:'l',code:'KeyL',modifiers:3});
 await assert(`!!document.querySelector('#unlock-form')&&!document.querySelector('main time')`,'Keyboard lock clears recorded activity from the rendered console');
 await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await assert(`document.querySelector('[data-service-events] time')?.dateTime==='2026-11-24T11:00:00.000Z'`,'Unlock refreshes the Activity page');
 await click('nav [data-page=Network]');
 await cmd('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
 const lockState=await js(`(()=>{
  document.querySelector('nav [data-page=Storage]').click();
  const outgoing=!!document.querySelector('main .page-outgoing');
  const storage=document.querySelector('main .page-content').textContent.includes('/dev/nvme0n1');
  consoleBridge.lock();
  return {outgoing,storage,empty:!document.querySelector('main').firstElementChild,locked:!!document.querySelector('#unlock-form')};
 })()`);
 if(!lockState.outgoing||!lockState.storage||!lockState.empty||!lockState.locked)throw Error('Lock must immediately remove the current and outgoing pages: '+JSON.stringify(lockState));
 await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await assert(`document.querySelector('.overlay').hidden&&document.querySelector('main').textContent.includes('/dev/nvme0n1')`,'Unlock reloads the selected Storage page');
 await click('nav [data-page=Settings]');
 await click('main [data-action=remote-access-status]');
 const dismissState=await js(`(()=>{
  const overlay=document.querySelector('.overlay');
  overlay.querySelector('[data-action=close]').click();
  const active=document.querySelector('main').inert;
  return {closing:!overlay.hidden,backgroundInert:active,dialogInert:overlay.querySelector('.dialog')?.inert};
 })()`);
 if(!dismissState.closing||!dismissState.backgroundInert||!dismissState.dialogInert)throw Error('A fading dialog must stop accepting input: '+JSON.stringify(dismissState));
 await assert(`document.querySelector('.overlay').hidden&&document.activeElement.dataset.action==='remote-access-status'`,'Animated dismissal returns focus to Manage');
 await click('nav [data-page=Overview]');
 const readings=await js(`(()=>{
  const panel=document.querySelector('#telemetry-readings'),metric=panel.querySelector('.metric');
  fixture.telemetrySnapshot={cpu:31,memoryTotal:16*1024**3,memoryUsed:3*1024**3,uptime:3900};
  consoleBridge.telemetry();
  return {same:panel.querySelector('.metric')===metric,current:panel.querySelector('.metric strong')?.textContent,
    fading:panel.querySelector('.metric .metric-previous')?.textContent,meter:panel.querySelector('.metric meter')?.value};
 })()`);
 if(!readings.same||readings.current!=='31%'||readings.fading!=='7%'||readings.meter!==31)throw Error('Readings crossfade between samples in place: '+JSON.stringify(readings));
 await assert(`!document.querySelector('.metric-previous')`,'The preceding reading is removed when the crossfade finishes');
 const blanked=await js(`(()=>{
  document.querySelector('footer [data-action=screen-off]').click();
  return fixture.screenOff===3&&!!document.querySelector('#unlock-form')&&!document.querySelector('.lock-art canvas');
 })()`);
 if(!blanked)throw Error('Screen-off must lock without starting a particle transition');
 await js(`delete fixture.telemetrySnapshot;consoleBridge.telemetry();document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
 await assert(`document.querySelector('.overlay').hidden`,'Unlock settles before another lock');
 await click('header [data-action=lock]');
 await checkHero({js,assert,cmd,shot,until});
 const carryPreference=()=>appendFileSync(join(folder,'fixture.js'),'\nwindow.animaInstallerReducedMotion = true;\n');
 await checkInstalledMotionPreference({js,assert,cmd,click,carryPreference});
 writeFileSync(join(out,'results.json'),JSON.stringify({status:'passed',fontDirectory:fontDirectory||'Host font fallback; installed appliance uses IBM Plex',heroChecks:['loaded still after relock','bounded normal transition','reduced motion','mid-transition preference change','rapid unlock and re-lock','screen-off skips animation','installed default and operator override','portrait and short-display control bounds'],consoleMotionChecks:['immediate current and outgoing page removal on lock','selected page reload','inert exit and focus return','observed value crossfade'],workloadChecks:['Local Hosting setup and recovery','Compose grouping and standalone containers','320px project bounds','per-container logs and terminal review','view focus and refresh persistence','state, health and Docker ownership filters','whole-project unhealthy count and recovery','cached snapshot on refresh failure','late reply discarded on lock'],buildChecks:['stage observations','failure stage','unavailable progress retains receipt','attempt-scoped logs from Workloads and Activity'],migrationChecks:['runtime storage and ownership mappings','missing runtime observations','network topology','escaped network name','missing versus empty networks'],networkChecks:['adapter absence','radio off and automatic scan','inline scan failure','scan navigation','strongest access point','escaped SSID','secured and open connection','adapter loss and reattach','lock during scan','320px row bounds'],remoteAccessChecks:['per-protocol observations and owner choice','read-only recheck retains focus'],scope:'Synthetic transport, real product HTML/CSS/JS in headless Chromium; not appliance or hardware acceptance',checks:records},null,2));
 console.log('PASS: direct sign-in, Activity and diagnostics, settings/back focus, network availability/inline scan/connection/lifecycle, concurrent update progress, legacy network inventory, maintenance review and drain, workload views and application terminal selection, 30 responsive screenshots. Evidence: '+out);
 ws.close();
}finally{child.kill();await child.exited;rmSync(profile,{recursive:true,force:true});rmSync(folder,{recursive:true,force:true});}
