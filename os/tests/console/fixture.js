const callbacks={};
window.fixture={calls:[],pending:0,enrolled:false,signedIn:false,scan:[],download:false,failScan:false,radio:'enabled',devices:null,scanDelay:20,connections:[],enrollments:[],screenOff:0};
const machine=()=>({hostname:'anima-node-demo',enrolled:fixture.enrolled,daemon:'inactive',supplicant:'active',network:'70:full',radio:fixture.radio,devices:fixture.devices||[{device:'enp1s0',type:'ethernet',state:'connected',connection:'Wired connection'},{device:'wlan0',type:'wifi',state:'disconnected',connection:''}],storage:{used:18*1024**3,total:128*1024**3},drives:[{type:'disk',name:'nvme0n1',model:'Preview SSD',size:128*1024**3}]});
const consoleBridge={
 answer:{connect:f=>callbacks.answer=text=>{fixture.pending--;f(text);}},locked:{connect:f=>callbacks.locked=f},telemetryChanged:{connect:f=>callbacks.telemetry=f},
 telemetry(){
  const original={batteries:[{name:'Battery',percent:82,state:'Discharging'}],cpu:7,memoryTotal:16*1024**3,memoryUsed:3*1024**3,uptime:3900,remoteConnections:{rdp:0,vnc:0}};
  callbacks.telemetry(JSON.stringify(fixture.telemetrySnapshot===undefined?original:fixture.telemetrySnapshot));
 },
 lock(){callbacks.locked();},
 screenOff(){fixture.screenOff++;callbacks.locked();},
 terminal(){fixture.terminalOpened=true;},
 applicationTerminal(container,name,user){fixture.terminalTarget={container,name,user};},
 request(text){fixture.pending++;const r=JSON.parse(text);fixture.calls.push(r.op);let value={};
 if(r.op==='status')value=machine();
 if(r.op==='data-volumes')value=fixture.dataVolumes??[];
 if(r.op==='remote-disconnect')value={transportRestarted:true};
 if(['remote-access-status','remote-access-set','remote-access-retry-defaults'].includes(r.op)){
  if(r.op==='remote-access-retry-defaults'){
   if(fixture.remoteConfigured===false)fixture.remotePorts=[22,3389,5900];
   fixture.remoteConfigured=true;fixture.remoteOrigin='managed-defaults';
  }
  fixture.remotePorts??=[22,3389,5900];
  if(r.op==='remote-access-set'){fixture.remotePorts=fixture.remotePorts.filter(port=>port!==r.args.port);if(r.args.enabled)fixture.remotePorts.push(r.args.port);}
  value={schema:1,origin:fixture.remoteOrigin??'managed-defaults',configured:fixture.remoteConfigured??true,protocols:[['SSH',22],['RDP',3389],['VNC',5900]].map(([name,port])=>{
   const localCheck=fixture.remoteChecks?.[name]??'responding';
   return {name,port,enabled:fixture.remotePorts.includes(port),listening:localCheck!=='unavailable',localCheck};
  })};
 }
 if(r.op==='work-admission')value=fixture.workAdmission===undefined?(fixture.drain?'draining':'clear'):fixture.workAdmission;
 if(r.op==='workloads')value={runtime:'docker',scope:'system',dockerAvailable:true,hosting:fixture.hosting,workAdmission:fixture.workAdmission===undefined?(fixture.drain?'draining':'clear'):fixture.workAdmission,containers:fixture.containers||[],managed:{admissionReconciled:true,workspaces:{entries:fixture.workspaces||[{workspace:'00000000-0000-4000-8000-000000000001',generation:1,phase:'running',resources:{memory_bytes:1073741824,cpu_millis:500,tasks:128},storageBytes:2147483648,storageUsageBytes:134217728}],truncated:false},builds:{entries:fixture.builds||[],truncated:false}}};
 if(r.op==='activity')value={actions:fixture.actions===undefined?{entries:[{action:'Hold new work',account:'admin',outcome:'Accepted',atMs:1795518000001}],complete:true}:fixture.actions,system:[{service:'Private link',transition:'Started',atMs:1795518000000}],builds:{entries:fixture.builds||[],truncated:false}};
 if(r.op==='code-attention')value=fixture.attention??{entries:[],complete:true};
 if(r.op==='migration-accounts')value={accounts:[{uid:1000,user:'admin',defaultStorageDetected:true}],complete:false,note:'Synthetic standard-storage inventory.'};
 if(r.op==='migration-inventory')value=fixture.migration||{uid:1000,user:'admin',runtimeContext:{rootless:true,storage:{root:'/home/admin/.local/share/containers/storage',volumes:'/home/admin/.local/share/containers/storage/volumes',driver:'overlay'},idMappings:{uidmap:[{container_id:0,host_id:1000,size:1},{container_id:1,host_id:100000,size:65536}],gidmap:[{container_id:0,host_id:100,size:1},{container_id:1,host_id:100000,size:65536}]}},containers:[],volumes:[],networks:[{name:'example-internal',driver:'bridge',internal:true,dnsEnabled:true,subnets:[{subnet:'10.78.0.0/24',gateway:'10.78.0.1'}]}],startup:{quadlets:[{name:'workshop.container',path:'/home/admin/.config/containers/systemd/workshop.container',unit:'workshop.service'}],units:[{name:'workshop.service',enablement:'generated',evidence:'quadlet'}]},remaining:['Application-consistent backup','Owner-authorized cutover and health verification']};
 if(r.op==='maintenance-impact')value={drain:fixture.drain||null,updateWindowEndsAt:fixture.updateWindowEndsAt??null,reservations:0,sourceInProgress:fixture.sourceInProgress===true,runningDocker:fixture.runningDocker??1,legacyRunning:false,hostingClear:true,pendingTransition:false,ready:fixture.runningDocker===0&&fixture.sourceInProgress!==true};
 if(r.op==='maintenance-drain-begin')value=fixture.drain={schema:1,id:'0190a000-0000-7000-8002-000000000003'};
 if(r.op==='maintenance-drain-cancel'){fixture.cancelledDrain=r.args.id;fixture.drain=null;value={released:true};}
 if(r.op==='workload-terminal-target')value={container:r.args.container,name:'workshop-web',service:'web',user:'1010:1010'};
 if(r.op==='workload-details'){
  fixture.detailsTarget=r.args.container;
  const container=fixture.containers?.find(row=>row.id===r.args.container);
  value=fixture.containerDetails??{container:r.args.container,name:container?.name,state:container?.state,
   health:null,networks:[],mounts:[]};
 }
 if(r.op==='workload-logs'){
  fixture.logRequest=r.args;
  value={container:r.args.container,stdout:fixture.logOutput??'Synthetic preview output',
   stderr:fixture.logErrors??'',tail:r.args.lines??200};
 }
 if(r.op==='workload-build-logs'){
  fixture.buildLogRequest=r.args;
  value={attempt:r.args.attempt,program:'railpack',stage:'detecting',finished:true,
   stdout:'<em data-fake-log>detecting project</em>',stderr:'No supported provider found'};
 }
 if(r.op==='sign-in-poll')value={signedIn:fixture.signedIn,email:'preview@example.test'};
 if(r.op==='scan')value=fixture.scan;
 if(r.op==='radio-on')fixture.radio='enabled';
 if(r.op==='connect')fixture.connections.push(r.args);
 if(r.op==='enroll')fixture.enrolled=true;
 if(r.op==='enroll-selfhost'||r.op==='enroll-install'){fixture.enrollments.push(r.args);fixture.enrolled=true;}
 if(r.op==='displays')value=fixture.displays??[{name:'Virtual-1',rotation:'normal'}];
 if(r.op==='rotate')value={revertSeconds:15};
 if(r.op==='rotate'&&fixture.displays){
  fixture.rotationRequest=r.args;
  const display=fixture.displays.find(output=>output.name===r.args.output);
  fixture.preview={display,previous:display.rotation};
  display.rotation=r.args.rotation;
 }
 if(r.op==='keep-rotation')delete fixture.preview;
 if(r.op==='revert'&&fixture.preview){
  fixture.preview.display.rotation=fixture.preview.previous;
  delete fixture.preview;
 }
 if(r.op==='update-status')value=fixture.download?{phase:'idle',operation:{phase:'downloading',bytes:120000000,totalBytes:900000000}}:fixture.updateState??{phase:'idle'};
 if(r.op==='update-status'&&fixture.download&&fixture.holdUpdateStatus){
  fixture.holdUpdateStatus=false;
  fixture.deliverUpdateStatus=()=>callbacks.answer(JSON.stringify({id:r.id,ok:true,value}));
  return;
 }
 if(r.op==='update-activate'||r.op==='update-rollback')fixture.updateApproval={operation:r.op,...r.args};
 if(r.op==='update-schedule-activation'){
  fixture.windowApproval=r.args;
  value=fixture.updateState.activationWindow={id:'0190a000-0000-7000-8002-000000000007',
   version:fixture.updateState.version,startsAt:r.args.startsAt,endsAt:r.args.endsAt,phase:'scheduled',pausesNewWork:true};
 }
 if(r.op==='update-cancel-activation'){
  fixture.windowCancellation=r.args;
  value=fixture.updateState.activationWindow={...fixture.updateState.activationWindow,phase:'cancelled',pausesNewWork:false};
 }
 if(r.op==='update-check')value={available:true,release:{version:'preview-next',bytes:900000000},state:{phase:'idle'}};
 if(r.op==='update-download'){
  fixture.download=true;
  setTimeout(()=>{
   fixture.download=false;
   fixture.updateState={phase:'staged',version:'preview-next',review:{activate:'e'.repeat(64)}};
   callbacks.answer(JSON.stringify({id:r.id,ok:true,value:fixture.updateState}));
  },fixture.downloadDelay??2500);
  return;
 }
 if(r.op==='diagnostics')value={monitoring:'inactive',private_link:'unknown',network:'Ethernet connected'};
 const failed=r.op==='data-volumes'&&fixture.failDataVolumes||r.op==='scan'&&fixture.failScan||r.op==='work-admission'&&fixture.failWorkAdmission||r.op==='workloads'&&fixture.failWorkloads||r.op==='workload-logs'&&fixture.failLog||r.op==='remote-access-status'&&fixture.failRemoteCheck||r.op==='workload-build-logs'&&fixture.failBuildLog;
 const delay=r.op==='scan'?fixture.scanDelay:r.op==='status'?(fixture.statusDelay||20):r.op==='workloads'?(fixture.workloadDelay||20):r.op==='workload-logs'?(fixture.logDelay||20):20;
 setTimeout(()=>callbacks.answer(JSON.stringify({id:r.id,ok:!failed,value,error:'The local request could not finish.'})),delay);
 }};
window.qt={webChannelTransport:{}};
window.QWebChannel=function(transport,ready){ready({objects:{console:consoleBridge}});};
