
(()=>{
const root=document.getElementById('anima-console'),main=root.querySelector('main'),nav=root.querySelector('nav'),overlay=root.querySelector('.overlay');
const names=['Overview','Workloads','Activity','Network','Storage','Settings'];
const settingsPages=['Display','Updates','Recovery'];
const preference=matchMedia('(prefers-reduced-motion: reduce)');
const savedMotion=localStorage.getItem('motion');
let motionChoice=['system','reduced'].includes(savedMotion)?savedMotion:(window.animaInstallerReducedMotion?'reduced':'system');
const activeMotion=new Set();
const reduced=()=>motionChoice==='reduced'||(motionChoice==='system'&&preference.matches);
const role={press:100,fast:150,base:200};
function animate(element,frames,duration=role.base){
 if(reduced())return Promise.resolve();
 const animation=element.animate(frames,{duration,easing:'cubic-bezier(.2,0,0,1)'});
 activeMotion.add(animation);
 return animation.finished.catch(()=>{}).finally(()=>activeMotion.delete(animation));
}
function syncMotion(){
 root.dataset.reduced=String(reduced());
 if(reduced()){AnimaHero.stop();for(const animation of activeMotion)animation.finish();}
}
preference.addEventListener('change',syncMotion);
syncMotion();
const motion={animate,reduced,role};
const pages=AnimaPages.create({main,...motion});

let page='Overview';
const {icon,utility,button,row,head,subhead,pageLink}=AnimaPresentation;
root.querySelector('.identity').innerHTML='<span>Local session · admin</span>'+utility('Lock screen (Ctrl+Alt+L) — node keeps running','lock','locked');
root.querySelector('footer').innerHTML='<span id="node-label">Anima OS</span><div class="actions">'+utility('Administration terminal','terminal','terminal')+utility('Turn screen off','screen-off','screen',true)+utility('Restart computer','restart','renew')+utility('Shut down computer','shutdown','power')+'</div>';

nav.innerHTML='<small>This computer</small>'+names.map(n=>`<button data-page="${n}" aria-label="${n}" title="${n}">${icon(n)}<span>${n}</span></button>`).join('');
let state=null,available=[],signedIn=false,email='',locked=true,pending=0,bridge=null,poll=null,selected=null,scanState='not-scanned',scanVersion=0,networkScans=0,refreshFailed=false;
const dialog=AnimaDialog.create({root,icon,animate,reduced,role,canDismiss:()=>!locked&&!pending});
const {close,modal}=dialog;
const waiting=new Map();
let lockEpoch=0,screenOffPending=false;
const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const enrollment=AnimaEnrollment.create({modal,button,icon,escape:esc,busy,refresh});
const display=AnimaDisplaySettings.create({call,draw,active:()=>!locked&&page==='Display',escape:esc});
const maintenance=AnimaMaintenance.create({call,dialog,unlocked:()=>!locked,row,button,esc,changed:()=>{if(!locked)void refreshPage(page);}});
const {remoteConnectionText,showConnectionStatus}=AnimaRemoteAccess.connectionIndicator(root);
const checkRemoteAccess=AnimaRemoteAccess.checks({root,call,dialog,button,row,escape:esc,connectionText:remoteConnectionText});
const workloadDialogs=AnimaWorkloads.dialogs({call,dialog,button,escape:esc,active:()=>!locked&&['Workloads','Activity'].includes(page),open:(id,name,user)=>bridge.applicationTerminal(id,name,user)});
const workAdmission=AnimaWorkAdmission.create(call,()=>{
 const panel=main.querySelector('.page-content [data-overview-admission]');
 if(!locked&&page==='Overview'&&panel)panel.innerHTML=workAdmission.render();
});
const size=value=>(value/1024**3).toFixed(1)+' GiB';
const storage=AnimaStorage.create({call,redraw:()=>{if(!locked&&page==='Storage')draw();},escape:esc,row,head,size});
const updates=AnimaUpdates.create({call,redraw:draw,escape:esc,size,dialog,
 view:()=>({panel:main.querySelector('.page-content #operation-status'),onUpdates:page==='Updates'})});
const unitLabel=value=>({active:'Running',inactive:'Stopped',failed:'Needs attention',activating:'Starting',deactivating:'Stopping',unknown:'Unavailable'}[value]||'Not yet checked');
const networkLabel=()=>state?.devices?.find(d=>d.state==='connected')?.connection||'No active connection';
function connectionPanel(){
 if(state?.enrolled)return `<section class="connection"><div><h2>Enrollment saved</h2><p>Monitoring is ${esc(unitLabel(state.daemon).toLowerCase())}. Hub reachability has not been verified.</p></div>${button('Connection details','connection-details')}</section>`;
 if(signedIn)return `<section class="connection"><div><h2>Add this computer</h2><p>Signed in as ${esc(email)}. Add ${esc(state?.hostname||'this computer')} to your account’s hub.</p></div><div class="actions">${button('Add this computer','add-computer',true)}${button('Use another account','switch-account')}</div></section><p class="hint">This connects monitoring, SSH and remote desktop through your private hub. Remote sessions require your admin credentials and the hub’s access permissions.</p>`;
 return `<section class="connection"><div><h2>Connect to Anima</h2><p>Sign in, then confirm that you want to add this computer.</p></div><div class="actions">${button('Sign in to Anima','sign-in',true)}${button('Connect to your hub','selfhost')}</div></section>`;
}
const wifiReady=()=>state?.devices?.some(d=>d.type==='wifi')&&state.radio==='enabled'&&state.supplicant!=='failed';
const networkSecurity=n=>!n.security||n.security==='--'?'Open network':/802\.1X|EAP/.test(n.security)?'Enterprise network':'Secured network';
function wifiNotice(){
 if(!state)return ['Network information unavailable','Try opening Network again in a moment.'];
 if(!state.devices.some(d=>d.type==='wifi'))return ['No Wi-Fi adapter detected','Use Ethernet, or connect a Wi-Fi adapter to join a wireless network.'];
 if(state.radio==='disabled')return ['Wi-Fi is off','Turn on Wi-Fi to see available networks.'];
 if(state.supplicant==='failed')return ['Wi-Fi needs attention','Open Settings → Diagnostics & recovery to check the Wi-Fi service.'];
 if(state.radio!=='enabled')return ['Wi-Fi status unavailable','Try opening Network again in a moment.'];
 if(scanState==='scanning')return ['Looking for networks…','Available networks will appear here.'];
 if(scanState==='failed')return ['Couldn’t refresh networks','Try again, or check Diagnostics & recovery in Settings.'];
 if(scanState==='empty')return ['No networks found','Move closer to your router, then refresh.'];
 return ['Looking for networks…','Available networks will appear here.'];
}
function wifiRow(n,i){
 const signal=Math.max(0,Math.min(100,Number(n.signal)||0)),level=Math.max(1,Math.ceil(signal/25));
 return `<button class="wifi-network" data-action="wifi-${i}"><span class="wifi-signal" aria-hidden="true">${[1,2,3,4].map(bar=>`<i class="${bar<=level?'lit':''}"></i>`).join('')}</span><span class="network-name"><strong>${esc(n.ssid||'Hidden network')}</strong><small>${networkSecurity(n)} · ${signal}% signal</small></span><span class="network-connect">${networkSecurity(n)==='Enterprise network'?'Details':'Connect'}${icon('arrow-right')}</span></button>`;
}
function networkScreen(){
 const ethernet=(state?.devices||[]).filter(d=>d.type==='ethernet'),wifi=(state?.devices||[]).filter(d=>d.type==='wifi');
 const connected=wifi.some(d=>d.state==='connected'),scanning=scanState==='scanning',notice=wifiNotice();
 const wired=ethernet.map(d=>`<section class="network-card wired"><div class="network-heading">${icon('network-3')}<div><h2>Ethernet</h2><small>${esc(d.connection&&d.connection!=='--'?d.connection:d.device)}</small></div><span class="connection-state ${d.state==='connected'?'connected':''}">${d.state==='connected'?'Connected':d.state.startsWith('connecting')?'Connecting…':'Not connected'}</span></div></section>`).join('');
 const control=wifiReady()?`<button class="quiet" data-action="scan" ${scanning?'disabled':''}>${icon('renew')}${scanning?'Refreshing…':'Refresh'}</button>`:wifi.length&&state?.radio==='disabled'?button('Turn on','radio-on'):'';
 const list=wifiReady()&&available.length&&!scanning?`<div class="wifi-list" aria-label="Available Wi-Fi networks">${available.map(wifiRow).join('')}</div>`:`<div class="network-empty" role="status"><strong>${notice[0]}</strong><p>${notice[1]}</p></div>`;
 return head('Network','Manage your Ethernet and Wi-Fi connections.')+`<div class="network-settings">${wired}<section class="network-card"><div class="network-heading">${icon('connection-signal')}<div><h2>Wi-Fi</h2><small>${connected?'Connected':wifi.length?state?.radio==='disabled'?'Off':'Wireless networks':'Not available'}</small></div>${control}</div>${list}</section></div>`;
}
async function scanNetworks(){
 if(!wifiReady()||scanState==='scanning'||locked)return;
 const version=++scanVersion;networkScans++;scanState='scanning';available=[];if(page==='Network')draw();
 try{
  const networks=await call('scan');if(locked||version!==scanVersion||!wifiReady())return;
  const strongest=new Map();
  for(const n of networks.sort((a,b)=>Number(b.signal)-Number(a.signal))){const key=JSON.stringify([n.device,n.ssid||n.bssid,n.security]);if(!strongest.has(key))strongest.set(key,n);}
  available=[...strongest.values()];scanState=available.length?'found':'empty';
 }catch(error){if(locked||version!==scanVersion)return;scanState='failed';available=[];}
 finally{networkScans--;if(!locked&&version===scanVersion&&page==='Network')draw();}
}
function wifiDialog(network){
 selected=network;if(!selected)return;
 const security=networkSecurity(selected),title=esc(selected.ssid||'Hidden network');
 if(security==='Enterprise network'){modal('Enterprise Wi-Fi',`<p>${title}</p><p>This network requires advanced configuration. Open Settings → Diagnostics & recovery for recovery options.</p>`,button('Close','close'));return;}
 modal('Connect to Wi-Fi',`<p class="wifi-dialog-name">${title}</p><form id="wifi-form">${security==='Open network'?'<p>This is an open network. No password is required.</p>':'<label>Network password<input name="password" type="password" autocomplete="off"></label>'}<button class="primary">${icon('checkmark')}Connect</button></form>`,button('Cancel','close'));
}
function draw(){
 if(locked)return;
 root.querySelector('#node-label').textContent=state?.hostname?state.hostname+' · Anima OS':'Anima OS';
 nav.querySelectorAll('button').forEach(b=>b.setAttribute('aria-current',b.dataset.page===(settingsPages.includes(page)?'Settings':page)?'page':'false'));
 const screens={
 Overview:()=>head(esc(state?.hostname||'This computer'),'Local management · Anima OS')+connectionPanel()+'<div data-attention-overview aria-live="polite">'+AnimaActivity.overview(pageLink)+'</div>'+'<div data-overview-admission>'+workAdmission.render()+'</div>'+(state?.daemon==='failed'?`<div class="notice warning">Monitoring needs attention. ${pageLink('Recovery','Open diagnostics','Check the local service')}</div>`:'')+(updates.ready?`<div class="notice">An OS update is ready. ${pageLink('Updates','Review update','Restart only when you are ready')}</div>`:'')+`<section class="section"><h2>Local resources</h2><div id="telemetry-readings"></div></section><div class="summary-grid"><section><h2>Network</h2><p>${esc(networkLabel())}</p>${pageLink('Network','Network settings','Adapters and Wi-Fi')}</section><section><h2>Persistent storage</h2><p>${state?size(state.storage.available??state.storage.total-state.storage.used)+' available of '+size(state.storage.total):'Not yet checked'}</p>${pageLink('Storage','Storage details','Disks and capacity')}</section></div>`,
 Workloads:()=>AnimaWorkloads.render(esc),
 Activity:()=>AnimaActivity.render(esc,button,pageLink),
 Network:networkScreen,
 Storage:()=>storage.render(state),
 Settings:()=>head('Settings','Display preferences, system updates and local recovery.')+row('Private remote access','SSH, RDP and VNC through your enrolled device',button('Manage','remote-access-status'))+`<div class="settings-list">${pageLink('Display','Display & accessibility','Orientation and reduced motion')}${pageLink('Updates','OS updates','Check, stage or roll back a verified update')}${pageLink('Recovery','Diagnostics & recovery','Check services and find recovery options')}</div>`,
 Display:()=>display.render(motionChoice),
 Updates:updates.render,
 Recovery:()=>subhead('Diagnostics & recovery','Check this computer even when the hub is unavailable.')+row('System diagnostics','Radio, network and monitoring checks',button('Run checks','diagnostics'))+row('Maintenance','Review running work and power readiness',button('Review','maintenance-review'))+pageLink('Updates','OS rollback','Review the previous system and restart deliberately')+'<details><summary>Advanced recovery</summary><p>Press Ctrl+Alt+F2 and log in as admin. Return with Ctrl+Alt+F7. OS rollback is also available from the boot menu.</p><p>These options do not restore backups or erase your data.</p></details>'};
 pages.show(page,'<section id="operation-status" class="operation-status" role="status" hidden></section>'+(refreshFailed?'<p class="notice warning" role="status">The last refresh failed. Shown readings may be out of date.</p>':'')+screens[page]());
 renderTelemetry(icon,motion);updates.renderProgress();
}
function call(op,args={}){
 if(!bridge)return Promise.reject(Error('The local service is starting.'));
 const id=op==='unlock'?'unlock':crypto.randomUUID();
 if(waiting.has(id))return Promise.reject(Error('Wait for the current request.'));
 const quiet=op==='code-attention'||op==='workloads'||op==='work-admission'||op==='data-volumes'||op==='workload-logs'||op==='workload-build-logs'||op==='remote-access-status'||op==='update-status';if(!quiet)pending++;
 return new Promise((resolve,reject)=>{waiting.set(id,{resolve,reject,quiet});bridge.request(JSON.stringify({id,op,args}));});
}
function errorMessage(error){
 let message=overlay.querySelector('.message');
 if(overlay.hidden){modal('Unable to finish','<p class="message"></p>',button('Close','close'));message=overlay.querySelector('.message');}
 if(!message){message=document.createElement('p');message.className='message';overlay.querySelector('.dialog').append(message);}
 message.textContent=error.message;
 if(!locked&&!overlay.querySelector('[data-action=close]'))overlay.querySelector('.actions').innerHTML=button('Close','close');
}
function lock(heroMotion=true){
 AnimaWorkloads.clear();
 workAdmission.clear();
 storage.clear();
 AnimaActivity.clear();
 display.clear();
 pages.clear();
 locked=true;signedIn=false;email='';state=null;available=[];selected=null;
 scanState='not-scanned';scanVersion++;enrollment.clear();updates.clear();refreshFailed=false;
 root.querySelector('#node-label').textContent='Anima OS';
 clearInterval(poll);poll=null;
 modal('Local session locked','<p class="lock-summary">Your node keeps working.</p><p class="remote-connections" data-remote-connections role="status">'+remoteConnectionText()+'</p><p class="lock-description">Unlock to manage this computer. Turning off the display leaves applications and remote access running.</p><form id="unlock-form"><label>Local admin password<input type="password" name="password" autocomplete="current-password" aria-label="Local admin password" required></label><button class="primary" type="submit">'+icon('password')+'Unlock</button></form>',button('Turn screen off','screen-off'),heroMotion);
}
const networkSignature=value=>JSON.stringify([value?.radio,value?.supplicant,(value?.devices||[]).filter(d=>d.type==='wifi').map(d=>d.device).sort()]);
const attentionPage=()=>page==='Overview'||page==='Activity';
const refreshAttention=()=>AnimaActivity.refreshAttention(call,()=>{
 if(!locked&&attentionPage())AnimaActivity.updateAttention(main,esc,pageLink);
});
async function refresh(redraw=true){
 if(!locked&&attentionPage())void refreshAttention();
 if(!locked&&page==='Overview')void workAdmission.refresh();
 try{
  const next=await call('status');if(locked)return;
  if(networkSignature(state)!==networkSignature(next)){scanVersion++;scanState='not-scanned';available=[];}
  state=next;refreshFailed=false;
 }catch(error){refreshFailed=true;throw error;}
 finally{if(redraw&&!locked){draw();if(page==='Network'&&scanState==='not-scanned')void scanNetworks();}}
}
async function go(n,focus='h1'){
 if(!names.includes(n)&&!settingsPages.includes(n))return;page=n;close(true);main.scrollTop=0;draw();
 if(n==='Overview')void refreshAttention();
 if(n==='Updates')await updates.refresh();
 if(n==='Network'){await refresh(false);if(!locked&&page===n){draw();if(scanState==='not-scanned')void scanNetworks();}}
 await refreshPage(n);
 if(!locked&&page===n)main.querySelector(focus)?.focus({preventScroll:true});
}
async function refreshPage(n){
 if(n==='Overview')await workAdmission.refresh();
 if(n==='Workloads')await AnimaWorkloads.refresh(call,()=>{if(!locked&&page===n)draw();});
 if(n==='Activity')await refreshActivity();
 if(n==='Display')await display.refresh();
 if(n==='Storage')await storage.refresh();
}
async function refreshActivity(){
 void refreshAttention();await AnimaActivity.refresh(call);if(!locked&&page==='Activity')draw();
}
async function busy(op,args,label){
 modal(label,'<p>Please wait. This is a real operation on this computer.</p>','');
 try{const result=await call(op,args);close(true);return result;}catch(error){errorMessage(error);throw error;}
}
root.addEventListener('click',async e=>{
 const b=e.target.closest('button');if(!b||b.closest('[inert]'))return;
 const a=b.dataset.action;
 if(a==='lock'){bridge.lock();return;}
 if(a==='screen-off'&&bridge){screenOffPending=true;AnimaHero.stop();bridge.screenOff();return;}
 if(locked)return;
 if(a==='terminal'){bridge.terminal();return;}
 if(a==='storage-refresh'){if(page==='Storage')await storage.refresh();return;}
 if(a==='workloads-refresh'){if(page==='Workloads'&&b.getAttribute('aria-disabled')!=='true')await refreshPage('Workloads');return;}
 if(pending&&!updates.working()&&!b.dataset.page)return;
 try{
 const maintenanceAction=maintenance.handle(a,b.dataset);
 if(maintenanceAction){await maintenanceAction;return;}
 if(a==='migration-accounts'){
  const version=dialog.version,result=await call('migration-accounts');
  if(!locked&&version===dialog.version)modal('Legacy Podman inventory','<p>'+esc(result.note)+'</p>'+result.accounts.map(account=>'<div class="row"><div><strong>'+esc(account.user)+'</strong><small>UID '+account.uid+' · '+(account.defaultStorageDetected?'Storage detected':'No standard storage')+'</small></div><button data-action="migration-inventory" data-uid="'+account.uid+'" '+(account.defaultStorageDetected?'':'disabled')+'>Inspect</button></div>').join(''),button('Close','close'));return;
 }
 if(a==='migration-inventory'){
  const version=dialog.version,result=await call('migration-inventory',{uid:Number(b.dataset.uid)});
  if(!locked&&version===dialog.version)modal('Migration inventory',AnimaWorkloads.legacy(result,esc),button('Close','close'));return;
 }
 if(a==='activity-refresh'){await refreshActivity();return;}
 const workloadAction=workloadDialogs(a,b.dataset);
 if(workloadAction){await workloadAction;return;}
  if(b.dataset.page){await go(b.dataset.page);return;}
  if(a==='close'){close();return;}
  if(a==='remote-disconnect-confirm'){
   const protocol=b.dataset.protocol;if(!['RDP','VNC'].includes(protocol))return;
   modal('Disconnect '+protocol+' clients?','<p>This restarts only the graphical transport. You may be disconnected. The shared console and applications keep running, and its lock state stays unchanged. Allowed clients can reconnect.</p>',button('Cancel','close')+'<button class="danger" data-action="remote-disconnect" data-protocol="'+protocol+'">Disconnect clients</button>');return;
  }
  if(a==='remote-disconnect'){
   await busy('remote-disconnect',{protocol:b.dataset.protocol,confirmed:true},'Disconnecting clients');
   if(!locked)await checkRemoteAccess();return;
  }
  if(a==='remote-access-status'){await checkRemoteAccess();return;}
  if(a==='remote-access-retry-defaults'){await checkRemoteAccess({retryDefaults:true});return;}
  if(a==='remote-access-select'){modal('Change private access','<p>'+esc(b.dataset.enabled==='true'?'Allow new '+b.dataset.protocol+' connections through your private link? Your hub permissions still apply.':'Disable '+b.dataset.protocol+' and disconnect its sessions through the private link?')+'</p><button data-action="remote-access-apply" data-port="'+esc(b.dataset.port)+'" data-enabled="'+esc(b.dataset.enabled)+'">Confirm</button>',button('Cancel','close'));return;}
  if(a==='remote-access-apply'){
   const version=dialog.version,access=await call('remote-access-set',{port:Number(b.dataset.port),enabled:b.dataset.enabled==='true',confirmed:true});
   if(!locked&&version===dialog.version)await checkRemoteAccess({access});return;
  }
  if(a==='settings-back'){const previous=page;await go('Settings',`[data-page="${previous}"]`);return;}
  if(a==='connection-details'){await checkRemoteAccess({monitoring:unitLabel(state?.daemon)});return;}
  if(a==='scan'){await scanNetworks();return;}
  if(a==='radio-on'){await busy('radio-on',{},'Enabling Wi-Fi');state=await call('status');scanState='not-scanned';await go('Network');return;}
  if(a?.startsWith('wifi-')){wifiDialog(available[Number(a.slice(5))]);return;}
  if(a==='update-check'){await updates.check(()=>busy('update-check',{},'Checking for updates'));}
  if(a==='update-download'){await updates.download();}
  if(a==='update-window-open'){updates.scheduleDialog();return;}
  if(a==='update-window-cancel'){updates.cancelDialog();return;}
  if(a==='update-window-cancel-confirm'){await updates.cancelWindow();return;}
  if(a==='selfhost'||a==='selfhost-direct'){enrollment.show(a==='selfhost-direct');return;}
  if(a==='switch-account'){await call('cancel-sign-in');signedIn=false;email='';draw();}
  if(a==='sign-in'||a==='switch-account'){
   await busy('sign-in',{},'Opening secure sign-in');
   modal('Continue in your browser','<p>Sign in here, or choose QR / code in the secure sign-in window to use your phone or another computer. Anima will return here automatically.</p>',button('Cancel sign-in','cancel-sign-in'));
   poll=setInterval(async()=>{if(pending||locked)return;try{const result=await call('sign-in-poll');if(result.signedIn){clearInterval(poll);poll=null;signedIn=true;email=result.email;close(true);draw();}}catch(error){clearInterval(poll);poll=null;errorMessage(error);}},1000);
  }
  if(a==='cancel-sign-in'){clearInterval(poll);poll=null;await call('cancel-sign-in');close();draw();}
  if(a==='add-computer'){await busy('enroll',{},'Adding this computer');signedIn=false;await refresh();}
  if(a==='diagnostics'){const result=await busy('diagnostics',{},'Checking the system');modal('System diagnostics',Object.entries(result).map(([k,v])=>row(esc(k),'',esc(v))).join(''),button('Done','close',true));}
  if(a==='keep-rotation'||a==='revert'){await call(a);if(!locked){close();await display.refresh();}}
 }catch(error){if(a==='add-computer'){signedIn=false;email='';draw();}if(!locked)errorMessage(error);}
});
root.addEventListener('submit',async e=>{
 e.preventDefault();const form=e.target,epoch=lockEpoch;if(form.closest('[inert]')||(pending&&!updates.working()))return;
 try{
  if(form.id==='unlock-form'){
   const password=form.elements.password.value;form.elements.password.value='';
   await call('unlock',{password});if(epoch!==lockEpoch)return;
   locked=false;close();void updates.refresh();
   await refresh();
   await refreshPage(page);
   return;
  }
  if(locked)return;
  if(form.id==='activation-window-form'){await updates.saveWindow(form);return;}
  if(form.id==='install-code-form'||form.id==='selfhost-form'){await enrollment.submit(form);return;}
  if(form.id==='wifi-form'){
   const password=form.elements.password?.value||'';if(form.elements.password)form.elements.password.value='';
   await busy('connect',{device:selected.device,bssid:selected.bssid,password},'Connecting to Wi-Fi');await refresh();
  }
  if(form.id==='display-form')await display.preview(form,dialog);
 }catch(error){if(epoch===lockEpoch||form.id==='unlock-form'&&form.isConnected&&locked)errorMessage(error);}
});
document.addEventListener('keydown',e=>{if(e.ctrlKey&&e.altKey&&e.key.toLowerCase()==='l'&&bridge){e.preventDefault();bridge.lock();}});
root.addEventListener('change',e=>{
 if(e.target.dataset.workloadFilter&&!locked&&page==='Workloads'&&AnimaWorkloads.selectFilter(e.target.dataset.workloadFilter,e.target.value))draw();
 if(e.target.matches('#display-form [name=output]'))display.select(e.target.value);
 if(e.target.id==='motion-choice'){motionChoice=e.target.value;localStorage.setItem('motion',motionChoice);syncMotion();}
});
root.addEventListener('toggle',e=>{if(e.target.tagName==='DETAILS'&&e.target.open)for(const child of e.target.children)if(child.tagName!=='SUMMARY')animate(child,[{opacity:0},{opacity:1}]);},true);
lock();
new QWebChannel(qt.webChannelTransport,channel=>{
 bridge=channel.objects.console;
 bridge.telemetryChanged.connect(text=>{const sample=JSON.parse(text);updateTelemetry(sample,icon,motion);showConnectionStatus(sample?.remoteConnections);});
 bridge.telemetry();
 bridge.answer.connect(text=>{const answer=JSON.parse(text),task=waiting.get(answer.id);if(!task)return;waiting.delete(answer.id);if(!task.quiet)pending=Math.max(0,pending-1);answer.ok?task.resolve(answer.value):task.reject(Error(answer.error));});
 bridge.locked.connect(()=>{
  const heroMotion=!screenOffPending;screenOffPending=false;lockEpoch++;
  for(const task of waiting.values())task.reject(Error('The local session is locked.'));
  waiting.clear();pending=0;if(!locked)lock(heroMotion);
 });
 setInterval(()=>{
  if(locked||(pending&&pending!==networkScans)||!overlay.hidden)return;
  if(page==='Updates')void updates.refresh();
  refresh(['Overview','Network','Storage'].includes(page)).catch(()=>{if(!locked)root.querySelector('#node-label').textContent='Status unavailable · retrying';});
 },10000);
});

})();
