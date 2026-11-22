const callbacks={};
window.fixture={calls:[],pending:0,enrolled:false,signedIn:false,scan:[],download:false,failScan:false,radio:'enabled',devices:null,scanDelay:20,connections:[],enrollments:[],screenOff:0};
const machine=()=>({hostname:'anima-node-demo',enrolled:fixture.enrolled,daemon:'inactive',supplicant:'active',network:'70:full',radio:fixture.radio,devices:fixture.devices||[{device:'enp1s0',type:'ethernet',state:'connected',connection:'Wired connection'},{device:'wlan0',type:'wifi',state:'disconnected',connection:''}],storage:{used:18*1024**3,total:128*1024**3},drives:[{type:'disk',name:'nvme0n1',model:'Preview SSD',size:128*1024**3}]});
const consoleBridge={
 answer:{connect:f=>callbacks.answer=text=>{fixture.pending--;f(text);}},locked:{connect:f=>callbacks.locked=f},telemetryChanged:{connect:f=>callbacks.telemetry=f},
 telemetry(){callbacks.telemetry(JSON.stringify({batteries:[{name:'Battery',percent:82,state:'Discharging'}],cpu:7,memoryTotal:16*1024**3,memoryUsed:3*1024**3,uptime:3900}));},
 lock(){callbacks.locked();},
 screenOff(){fixture.screenOff++;callbacks.locked();},
 request(text){fixture.pending++;const r=JSON.parse(text);fixture.calls.push(r.op);let value={};
 if(r.op==='status')value=machine();
 if(r.op==='sign-in-poll')value={signedIn:fixture.signedIn,email:'preview@example.test'};
 if(r.op==='scan')value=fixture.scan;
 if(r.op==='radio-on')fixture.radio='enabled';
 if(r.op==='connect')fixture.connections.push(r.args);
 if(r.op==='enroll')fixture.enrolled=true;
 if(r.op==='enroll-selfhost'){fixture.enrollments.push(r.args);fixture.enrolled=true;}
 if(r.op==='displays')value=[{name:'Virtual-1',rotation:'normal'}];
 if(r.op==='update-status')value=fixture.download?{phase:'idle',operation:{phase:'downloading',bytes:120000000,totalBytes:900000000}}:{phase:'idle'};
 if(r.op==='update-check')value={available:true,release:{version:'preview-next',bytes:900000000},state:{phase:'idle'}};
 if(r.op==='update-download'){fixture.download=true;setTimeout(()=>{fixture.download=false;callbacks.answer(JSON.stringify({id:r.id,ok:true,value:{phase:'staged',version:'preview-next'}}));},2500);return;}
 if(r.op==='diagnostics')value={monitoring:'inactive',private_link:'unknown',network:'Ethernet connected'};
 setTimeout(()=>callbacks.answer(JSON.stringify({id:r.id,ok:!(r.op==='scan'&&fixture.failScan),value,error:'The scan could not finish.'})),r.op==='scan'?fixture.scanDelay:20);
 }};
window.qt={webChannelTransport:{}};
window.QWebChannel=function(transport,ready){ready({objects:{console:consoleBridge}});};
