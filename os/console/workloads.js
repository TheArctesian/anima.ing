/* Presentation only: authenticated native requests own runtime access. */
window.AnimaWorkloads=(()=>{
 const views=[['all','All workloads'],['containers','Docker containers'],['workspaces','Managed workspaces'],['builds','Builds']];
 const management=[['all','All containers'],['managed','Managed by Anima'],['discovered','Discovered'],['unavailable','Records unavailable']];
 const states={running:'Running',preparing:'Preparing',prepared:'Prepared',starting:'Starting',stopping:'Stopping',stopped:'Stopped',created:'Created',restarting:'Restarting',paused:'Paused',exited:'Exited',dead:'Dead',removing:'Removing',launching:'Launching',cancelling:'Cancelling',cancelled:'Cancelled',failed:'Failed',verified:'Artifact verified',deleting:'Removing storage',unknown:'Unknown or needs reconciliation'};
 const healthLabels={unhealthy:'Unhealthy',starting:'Health check starting',healthy:'Healthy'};
 const healthChoices=[['all','All health states'],...Object.entries(healthLabels)];
 const choices={view:views,state:[['all','All states'],...Object.entries(states)],health:healthChoices,management};
 const initialFilters=()=>({view:'all',state:'all',health:'all',management:'all'});
 let inventory=null,observedAt=null,failed=false,loading=false,generation=0,filters=initialFilters();
 const clear=()=>{generation++;inventory=null;observedAt=null;failed=false;loading=false;filters=initialFilters();};
 function selectFilter(filter,value){
  if(!Object.hasOwn(choices,filter)||!choices[filter].some(([key])=>key===value))return false;
  filters[filter]=value;return true;
 }
 async function refresh(call,redraw){
  if(loading)return;
  const version=++generation;loading=true;
  redraw();
  try{const result=await call('workloads');if(version===generation){inventory=result;observedAt=new Date();failed=false;}}
  catch{if(version===generation)failed=true;}
  finally{if(version===generation){loading=false;redraw();}}
 }
 const stateKey=state=>Object.hasOwn(states,state)?state:'unknown';
 const includes=kind=>filters.view==='all'||filters.view===kind;
 const matchesState=state=>filters.state==='all'||stateKey(state)===filters.state;
 const containerManagement=row=>row.applicationRecordUnavailable?'unavailable':row.application?'managed':'discovered';
 function visibleContainer(row){
  return matchesState(row.state)&&(filters.health==='all'||row.health===filters.health)
   &&(filters.management==='all'||containerManagement(row)===filters.management);
 }
 function filteredReceipts(section){
  if(!section||filters.state==='all')return section;
  return {...section,entries:section.entries.filter(entry=>matchesState(entry.phase)),filtered:true};
 }
 function limits(resources){
  if(!resources)return 'Resource limits unavailable';
  return `Limits · ${Math.ceil(resources.memory_bytes/1048576)} MiB · ${resources.cpu_millis/1000} CPU · ${resources.tasks} tasks`;
 }
 function managedCounters(entry){
  if(entry.usage){
   const {cpuMicros,memoryBytes,tasks}=entry.usage;
   return `Usage · CPU time ${(cpuMicros/1e6).toFixed(2)} s · Memory ${(memoryBytes/1048576).toLocaleString(undefined,{maximumFractionDigits:2})} MiB · Tasks ${tasks}`;
  }
  return entry.usage===null&&['running','starting','stopping','launching','cancelling','unknown'].includes(entry.phase)
   ? 'Live usage unavailable' : '';
 }
 function workspaceStorage(entry){
  if(!(entry.storageBytes>0))return '';
  const format=amount=>amount.toLocaleString(undefined,{maximumFractionDigits:2});
  const quota=`Storage quota ${format(entry.storageBytes/1073741824)} GiB`;
  if(entry.storageUsageBytes===undefined)return quota;
  if(entry.storageUsageBytes===null)return `Storage use unavailable · ${quota}`;
  const bytes=entry.storageUsageBytes;
  const value=bytes>0&&bytes<1048576?`${format(Math.ceil(bytes/1024))} KiB`:`${format(bytes/1048576)} MiB`;
  return `Storage used ${value} · ${quota}`;
 }
 function recorded(title,section,kind,escape){
  const heading=`<h2>${title}</h2>`;
  if(!section)return heading+'<p class="notice warning">Recorded state is unavailable. This does not mean the node is idle.</p>';
  const rows=section.entries.map(entry=>{
   const id=kind==='Build'?entry.attempt:entry.workspace;
   const detail=kind==='Build'?`${entry.strategy==='dockerfile'?'Dockerfile':entry.strategy==='railpack'?'Railpack':'Strategy pending'} · workspace ${entry.workspace.slice(0,8)}`:`Generation ${entry.generation}`;
   const progress=kind==='Build'?buildProgress(entry):'';
   const usage=managedCounters(entry);
   const storage=kind==='Workspace'?workspaceStorage(entry):'';
   const facts=[detail,usage,limits(entry.resources),storage,progress].filter(Boolean)
    .map(value=>`<small>${escape(value)}</small>`).join('');
   const logs=kind==='Build'?`<button data-action="workload-build-logs" data-attempt="${escape(id)}" aria-label="Logs for build ${escape(id.slice(0,8))}">Logs</button>`:'';
   return `<div class="row"><div class="row-title"><div><strong title="${escape(id)}">${kind} ${escape(id.slice(0,8))}</strong>${facts}</div></div><div class="actions"><span class="value">${escape(states[stateKey(entry.phase)])}</span>${logs}</div></div>`;
  }).join('');
  return heading+(rows||`<p class="empty-state">No ${section.filtered?'matching':'recorded'} ${kind==='Build'?'builds':'workspaces'}.</p>`)+(section.truncated?'<p class="notice">Showing a limited inventory. Additional receipts are not shown.</p>':'');
 }
 function buildProgress(entry){
  if(entry.workerProgressAvailable===false)return 'Build progress unavailable';
  const progress=entry.workerProgress;
  if(entry.strategy==='railpack'&&progress?.failed===true&&progress.stage==='detecting')
   return 'Railpack could not prepare this project. Open Logs for the reason.';
  const stage=AnimaLogs.buildStage(progress?.stage);
  if(!stage)return '';
  return `${progress.failed?'Worker reported failure':'Last worker stage'}: ${stage}`;
 }
 const builds=(section,escape)=>recorded('Builds',section,'Build',escape);
 function containerRow(container,escape){
  const {id,name,state,status,service,image,ports}=container;
  const health=Object.hasOwn(healthLabels,container.health)?container.health:'';
  const terminal=state==='running'&&containerManagement(container)==='managed'
   ? `<button data-action="workload-terminal-target" data-container="${escape(id)}" aria-label="Terminal for ${escape(name)}">Terminal</button>` : '';
  const details=`<button data-action="workload-details" data-container="${escape(id)}" aria-label="Details for ${escape(name)}">Details</button>`;
  const detail=service?`${escape(service)} · ${escape(image)}`:escape(image);
  return `<div class="row"><div class="row-title"><div><strong>${escape(name)}</strong><small>${detail}</small><small>${escape(ports||'No published ports')}</small></div></div><div class="actions"><span class="value" data-container-health="${health}">${escape(status)}</span>${details}<button data-action="workload-logs" data-container="${escape(id)}" data-name="${escape(name)}" aria-label="Logs for ${escape(name)}">Logs</button>${terminal}</div></div>`;
 }
 function hostingStatus(observation){
  if(!observation)return '';
  const {setup,service,socket}=observation;
  const retry='sudo systemctl start anima-applications.service';
  let title='Local Hosting status unavailable', detail='Refresh Workloads to check again.', command;
  if(setup==='not-configured'){
   title='Hosting setup required';
   detail='Enable managed deployments from the administration terminal.';
   command='sudo anima-enable-hosting --enable';
  }else if(setup==='pairing-retained'){
   title='Restore Hosting settings';
   detail='The node still has its saved pairing. Restore the local settings before starting the service.';
  }else if(setup==='settings-saved'){
   switch(service){
    case 'failed': case 'inactive':
     title=service==='failed'?'Hosting service failed':'Hosting service stopped';
     detail='The settings are saved. Resolve any service error, then start the service.';
     command=retry;
     break;
    case 'active':
     title=socket===true?'Local applications service running':'Local applications socket unavailable';
     detail=socket===true?'Check deployment availability in Hosting in Anima.':'Refresh Workloads. If the socket is still unavailable, check the service journal.';
     break;
    case 'activating': case 'deactivating':
     title=service==='activating'?'Hosting service is starting':'Hosting service is stopping';
     break;
   }
  }
  const action=command?`<p><code>${command}</code></p>${AnimaPresentation.button('Open administration terminal','terminal')}`:'';
  return `<section class="section" data-hosting-status><h2>Hosting</h2><p><strong>${title}</strong></p><p>${detail}</p>${action}</section>`;
 }
 function dockerContainers(escape){
  const heading='<h2>System Docker</h2>';
  if(inventory.dockerAvailable===false)return heading+'<p class="notice warning">Docker is unavailable. Container state is unknown.</p>';
  if(!inventory.containers.length)return heading+'<p class="empty-state">No system Docker containers yet.</p>';
  const projects=new Map(),standalone=[];
  for(const container of inventory.containers){
   if(!container.project){standalone.push(container);continue;}
   if(!projects.has(container.project))projects.set(container.project,[]);
   projects.get(container.project).push(container);
  }
  const rows=containers=>containers.filter(visibleContainer).map(container=>containerRow(container,escape)).join('');
  const grouped=[...projects].map(([project,containers])=>{
   const visible=rows(containers);
   if(!visible)return '';
   const running=containers.filter(container=>container.state==='running').length;
   const unhealthy=containers.filter(container=>container.health==='unhealthy').length;
   const warning=unhealthy?` · <strong data-unhealthy-count>${unhealthy} unhealthy</strong>`:'';
   return `<section class="workload-project" data-compose-project><h4>${escape(project)}</h4><p>${running} of ${containers.length} running${warning}</p>${applicationLabel(containers,escape)}${visible}</section>`;
  }).join('');
  const projectList=grouped?'<section class="workload-groups"><h3>Compose projects</h3>'+grouped+'</section>':'';
  const standaloneRows=rows(standalone);
  const other=standaloneRows?`<section class="workload-groups" data-standalone-containers><h3>Standalone containers</h3>${standaloneRows}</section>`:'';
  return heading+(projectList+other||'<p class="empty-state">No containers match these filters.</p>');
 }
 function applicationLabel(containers,escape){
  const application=containers[0].application;
  if(application&&containers.every(container=>container.application===application)){
   return `<p data-application-id>Managed by Anima · Application ID <code>${escape(application)}</code></p>`;
  }
  if(containers.some(container=>container.applicationRecordUnavailable)){
   return '<p>Application records unavailable</p>';
  }
  if(containers.some(container=>container.application)){
   return '<p>Some containers match the Anima deployment</p>';
  }
  return '';
 }
 function stateChoices(){
  const seen=new Set(['all',filters.state]);
  if(includes('containers')&&inventory.dockerAvailable!==false)for(const row of inventory.containers)seen.add(stateKey(row.state));
  for(const kind of ['workspaces','builds']){
   if(includes(kind))for(const row of inventory.managed?.[kind]?.entries||[])seen.add(stateKey(row.phase));
  }
  return choices.state.filter(([key])=>seen.has(key));
 }
 function select(filter,label,options){
  const id='workload-'+filter;
  const entries=options.map(([key,title])=>`<option value="${key}" ${filters[filter]===key?'selected':''}>${title}</option>`).join('');
  return `<div class="workload-filter"><label for="${id}">${label}</label><select id="${id}" data-workload-filter="${filter}">${entries}</select></div>`;
 }
 function freshness(escape){
  const warning=failed?'<p class="notice warning" role="status">Refresh failed. Showing the last successful read.</p>':'';
  const time=observedAt.toLocaleTimeString([], {hour:'numeric',minute:'2-digit',second:'2-digit'});
  return warning+`<p class="workloads-read">Last read <time data-workloads-read datetime="${observedAt.toISOString()}">${escape(time)}</time></p>`;
 }
 function render(escape){
  const heading='<h1 tabindex="-1">Workloads</h1><p>Docker containers, managed workspaces and builds on this node.</p>';
  const refresh=`<button data-action="workloads-refresh" aria-disabled="${loading}">${loading?'Refreshing…':'Refresh'}</button>`;
  if(!inventory)return heading+(failed?'<p class="notice warning" role="status">Workload information is unavailable.</p>'+refresh:'<p role="status">Reading workloads…</p>');
  const dockerFilters=includes('containers')?select('health','Health',healthChoices)+select('management','Docker ownership',management):'';
  const toolbar=`<div class="workloads-toolbar actions">${select('view','View',views)}${select('state','State',stateChoices())}${dockerFilters}${refresh}<button data-action="migration-accounts">Legacy Podman inventory</button></div>`;
  const previous=inventory.managed?.admissionReconciled===false?'reconciling':null;
  const observed=Object.hasOwn(inventory,'workAdmission')?inventory.workAdmission:previous;
  const admission=filters.view==='containers'?'':AnimaWorkAdmission.render(failed?null:observed);
  return heading+toolbar+freshness(escape)+admission+
   (includes('containers')?hostingStatus(inventory.hosting)+`<section class="section">${dockerContainers(escape)}</section>`:'')+
   (includes('workspaces')?`<section class="section">${recorded('Managed workspaces',filteredReceipts(inventory.managed?.workspaces),'Workspace',escape)}</section>`:'')+
   (includes('builds')?`<section class="section">${builds(filteredReceipts(inventory.managed?.builds),escape)}</section>`:'')+
   '<p class="notice">Open Code for coding sessions and Hosting for deployments. Workspace and build states come from their most recent local records. CPU time is cumulative for the running group.</p>';
 }
 function legacyContext(context,escape){
  if(!context)return '<p class="notice warning">Runtime storage and ownership mappings are unavailable. Do not copy workload data yet.</p>';
  const storage=context.storage;
  const location=storage?`<h3>Runtime storage</h3><p>${escape(storage.driver)} · ${escape(storage.root)}</p><p>Volumes: ${escape(storage.volumes)}</p>`:'<p class="notice warning">Runtime storage is unavailable.</p>';
  if(context.rootless===false)return location+'<p>No outer rootless namespace. Review each container’s ownership mappings before copying data.</p>';
  const mappings=['uidmap','gidmap'].map(kind=>{
   const ranges=context.idMappings?.[kind];
   if(!ranges?.length)return `<p class="notice warning">${kind==='uidmap'?'User':'Group'} ownership mappings are unavailable. Do not copy workload data yet.</p>`;
   return `<h4>${kind==='uidmap'?'User':'Group'} IDs</h4>`+ranges.map(range=>`<p>Runtime ${escape(range.container_id)} → host ${escape(range.host_id)} · ${escape(range.size)} ${range.size===1?'ID':'IDs'}</p>`).join('');
  }).join('');
  return location+'<details><summary>Runtime ownership mappings</summary>'+mappings+'<p>These describe the outer runtime namespace. Container mappings must also be applied before restoring file ownership.</p></details>';
 }
 function legacyStartup(startup,escape){
  const evidence={quadlet:'Quadlet definition',container_label:'Container label',name_pattern:'Podman naming pattern'};
  const quadlets=startup?.quadlets?.map(entry=>'<div class="row"><div><strong>'+escape(entry.name)+'</strong><small>'+escape(entry.path)+'</small><small>'+escape(entry.unit||'Generated service unknown')+'</small></div></div>').join('');
  const units=startup?.units?.map(entry=>'<div class="row"><div><strong>'+escape(entry.name)+'</strong><small>'+escape(evidence[entry.evidence]||'Startup candidate')+'</small></div><span class="value">'+escape(entry.enablement)+'</span></div>').join('');
  return '<h3>Startup candidates</h3><p>Verify which units start this workload before cutover. Check external automation separately.</p><h4>Quadlet definitions</h4>'+
   (quadlets||(startup?.quadlets?'<p>No Quadlet definitions reported.</p>':'<p class="notice warning">Quadlet inventory is unavailable.</p>'))+'<h4>systemd units</h4>'+
   (units||(startup?.units?'<p>No startup candidates reported by systemd.</p>':'<p class="notice warning">Systemd startup inventory is unavailable.</p>'));
 }
 function legacyContainer(container,escape){
  const unit=container.systemdUnitHint?`<small>Reported unit: ${escape(container.systemdUnitHint)}</small>`:'';
  const removal=container.autoRemove===true?'<small>Removes on stop. Use <code>anima-applications capture-source</code> before stopping.</small>':'';
  const mounts=container.mounts.map(mount=>`<small>${escape(mount.type)}: ${escape(mount.source)} → ${escape(mount.destination)} · ${mount.writable?'writable':'read only'}</small>`).join('');
  return `<div class="row"><div><strong>${escape(container.name)}</strong><small>${escape(container.imageName)} · ${escape(container.status)}</small>${unit}${removal}${mounts}</div></div>`;
 }
 function legacy(report,escape){
  const containers=report.containers.map(container=>legacyContainer(container,escape)).join('');
  const volumes=report.volumes.map(volume=>`<div class="row"><div><strong>${escape(volume.name)}</strong><small>${escape(volume.driver)} · ${escape(volume.mountpoint)}</small></div></div>`).join('');
  const networks=report.networks?.map(network=>`<div class="row"><div><strong>${escape(network.name)}</strong><small>${escape(network.driver)} · ${network.internal?'Internal':'External routing'} · DNS ${network.dnsEnabled?'enabled':'disabled'}</small>${(network.subnets||[]).map(subnet=>`<small>${escape(subnet.subnet)}${subnet.gateway?` · gateway ${escape(subnet.gateway)}`:''}</small>`).join('')}</div></div>`).join('');
  return `<p>Account ${escape(report.user)} · UID ${report.uid}. Read-only inventory. Originals remain in Podman.</p>${legacyContext(report.runtimeContext,escape)}<h3>Containers</h3>${containers||'<p>No containers reported.</p>'}<h3>Named volumes</h3>${volumes||'<p>No volumes reported.</p>'}<h3>Networks</h3>${networks||(report.networks?'<p>No networks reported.</p>':'<p>Network inventory is unavailable.</p>')}${legacyStartup(report.startup,escape)}<h3>Before cutover</h3><ul>${report.remaining.map(item=>`<li>${escape(item)}</li>`).join('')}</ul><p class="notice">Review the workload before stopping it or copying live application data.</p>`;
 }
 function containerResources(resources,escape){
  const {usage,limits}=resources??{};
  const sample=usage
   ? ` · CPU ${escape(usage.cpuPercent)}% · Memory ${escape(usage.memory)} · Tasks ${escape(usage.tasks)}`
   : ' unavailable';
  if(!limits)return `<h3>Resources</h3><p><strong>Current sample</strong>${sample}</p><p>Docker configuration unavailable.</p>`;
  const cpu=limits.cpuMillis===null?'CPU quota unset':`CPU ${limits.cpuMillis/1000}`;
  const cpuSet=limits.cpuSet?` · CPU set ${escape(limits.cpuSet)}`:'';
  const memory=limits.memoryBytes===null?'Memory limit unset'
   :`Memory ${(limits.memoryBytes/1048576).toLocaleString(undefined,{maximumFractionDigits:2})} MiB`;
  const tasks=limits.tasks===null?'Task limit unset':`Tasks ${escape(limits.tasks)}`;
  return `<h3>Resources</h3><p><strong>Current sample</strong>${sample}</p>
   <p><strong>Docker configuration</strong> · ${cpu}${cpuSet} · ${memory} · ${tasks}</p>`;
 }
 function detailsBody(result,escape){
  const health=Object.hasOwn(healthLabels,result.health)?healthLabels[result.health]:null;
  const networks=result.networks.map(name=>`<li>${escape(name)}</li>`).join('');
  const mounts=result.mounts.map(({name,kind,destination,writable})=>{
   const label=name||({bind:'Bind mount',tmpfs:'Temporary filesystem',volume:'Volume'}[kind]??kind);
   return `<li>${escape(label)} → <code>${escape(destination)}</code> · ${writable?'Read/write':'Read-only'}</li>`;
  }).join('');
  return `<div class="container-details">
   <p><strong>${escape(result.name)}</strong></p>
   <p>${escape(states[stateKey(result.state)])} · ${escape(health??(result.health===null?'No health check configured':'Health status unavailable'))}</p>
   ${containerResources(result.resources,escape)}
   <h3>Attached networks</h3>${networks?`<ul>${networks}</ul>`:'<p>No attached networks reported.</p>'}
   <h3>Storage mounts</h3>${mounts?`<ul>${mounts}</ul>`:'<p>No storage mounts reported.</p>'}
  </div>`;
 }
 function dialogs({call,dialog,button,escape,active,open}){
  async function review(container){
   const version=dialog.version;
   const target=await call('workload-terminal-target',{container});
   if(!active()||version!==dialog.version)return;
   const account=`<p>${escape(target.name)} · ${escape(target.service)} · account ${escape(target.user)}</p>`;
   const confirm=`<button class="primary" data-action="application-terminal-open" data-container="${escape(target.container)}" data-name="${escape(target.name)}" data-user="${escape(target.user)}">Open terminal</button>`;
   dialog.modal('Open application terminal?',account+'<p>Enter the local administrator password in the terminal to continue.</p>',button('Cancel','close')+confirm);
  }
  async function details(container){
   const version=dialog.version;
   const result=await call('workload-details',{container});
   if(!active()||version!==dialog.version)return;
   dialog.modal('Container details',detailsBody(result,escape),button('Close','close'));
  }
  return (action,{container,name,user,attempt})=>{
   if(action==='workload-terminal-target')return review(container);
   if(action==='workload-logs'&&active()){AnimaLogs.container({container,name,call,dialog,button});return true;}
   if(action==='workload-build-logs'&&active()){AnimaLogs.build({attempt,call,dialog,button});return true;}
   if(action==='workload-details')return details(container);
   if(action==='application-terminal-open'&&active()){open(container,name,user);dialog.close(true);return true;}
   return null;
  };
 }
 return {clear,refresh,render,selectFilter,builds,legacy,dialogs};
})();
