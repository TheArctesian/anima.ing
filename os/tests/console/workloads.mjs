export async function checkBuildProgress({js,click,assert}){
 await click('nav [data-page=Workloads]');
 await js(`{
  const workspace='00000000-0000-4000-8000-000000000001';
  const resources={memory_bytes:1073741824,cpu_millis:500,tasks:128};
  fixture.workspaces=[{workspace,generation:1,phase:'running',resources,storageBytes:2147483648,
    storageUsageBytes:134217728,usage:{cpuMicros:1250000,memoryBytes:25165824,tasks:4}}];
  fixture.builds=[{attempt:'00000000-0000-4000-8000-000000000002',workspace,phase:'running',
    strategy:'railpack',resources,usage:{cpuMicros:3250000,memoryBytes:67108864,tasks:11},
    workerProgress:{schema:1,stage:'detecting',failed:false}}];
 }`);
 await click('[data-action=workloads-refresh]');
 await assert(`document.querySelector('main').textContent.includes('Last worker stage: Detecting project')&&!document.querySelector('main').textContent.includes('Artifact verified')`,'Worker detection progress does not imply a verified artifact');
 await assert(`(()=>{
  const [workspace,build]=[...document.querySelectorAll('main h2')]
    .filter(heading=>['Managed workspaces','Builds'].includes(heading.textContent)).map(heading=>heading.parentElement.textContent);
  return ['CPU time 1.25 s','Memory 24 MiB','Tasks 4','Storage used 128 MiB','Storage quota 2 GiB'].every(text=>workspace.includes(text))&&
    ['CPU time 3.25 s','Memory 64 MiB','Tasks 11','Limits · 1024 MiB'].every(text=>build.includes(text));
 })()`, 'Workspace and build rows distinguish measured counters from the recorded limits');
 await js(`fixture.builds[0].workerProgress.failed=true`);await click('[data-action=workloads-refresh]');
 await assert(`document.querySelector('main').textContent.includes('Railpack could not prepare this project. Open Logs for the reason.')`,'Railpack preparation failure directs the operator to the reason');
 await checkBuildDiagnostics({js,click,assert});
 await js(`fixture.builds[0].workerProgressAvailable=false;fixture.builds[0].workerProgress=null;fixture.builds[0].usage=null`);await click('[data-action=workloads-refresh]');
 await assert(`document.querySelector('main').textContent.includes('Build progress unavailable')&&document.querySelector('main').textContent.includes('Build 00000000')`,'Unavailable worker progress does not erase its durable build receipt');
 await assert(`[...document.querySelectorAll('main h2')].find(heading=>heading.textContent==='Builds')
  .parentElement.textContent.includes('Live usage unavailable')&&!document.querySelector('main').textContent.includes('CPU time 3.25 s')`,
 'Missing live counters clear the old sample while retaining the build');
 await js('fixture.workspaces[0].storageUsageBytes=null');await click('[data-action=workloads-refresh]');
 await assert(`document.querySelector('main').textContent.includes('Storage use unavailable · Storage quota 2 GiB') &&
   !document.querySelector('main').textContent.includes('Storage used 128 MiB')`,
  'A missing Btrfs sample clears the measurement and keeps the recorded quota');
 await js('delete fixture.builds;delete fixture.workspaces');await click('[data-action=workloads-refresh]');
}

async function checkBuildDiagnostics({js,click,assert}){
 const action='main [data-action=workload-build-logs]';
 await assert("!fixture.calls.includes('workload-build-logs')",'Build inventory leaves diagnostic output unread');
 await click(action);
 await assert(`fixture.buildLogRequest.attempt==='00000000-0000-4000-8000-000000000002' &&
   document.querySelector('.dialog [role=status]').textContent.includes('Railpack · Detecting project') &&
   document.querySelector('[data-log-stdout]').textContent==='<em data-fake-log>detecting project</em>' &&
   document.querySelector('[data-log-stderr]').textContent==='No supported provider found' &&
   !document.querySelector('[data-fake-log]')`,'Build logs show the selected attempt and render its output as inert text');
 await js('fixture.failBuildLog=true');
 await click('[data-log-refresh]');
 await assert(`document.querySelector('[data-log-status]').textContent.includes('could not finish') &&
   !document.querySelector('[data-log-stderr]').textContent`,'An unsuccessful build refresh clears the previous logs');
 await click('.dialog [data-action=close]');
 await assert(`document.activeElement.matches(${JSON.stringify(action)})`,'Closing build logs returns focus to the selected build');
 await js('delete fixture.failBuildLog;delete fixture.buildLogRequest');
 await click('nav [data-page=Activity]');
 await click(action);
 await assert(`fixture.buildLogRequest.attempt==='00000000-0000-4000-8000-000000000002' &&
   document.querySelector('[data-log-stderr]').textContent==='No supported provider found'`,
  'Activity offers the same diagnostic read for its recorded build');
 await click('.dialog [data-action=close]');
 await click('nav [data-page=Workloads]');

}

export async function checkApplicationTerminal({js, click, assert}) {
  await js(`fixture.containers=[{id:'a'.repeat(64),name:'workshop-web',state:'running',status:'Up',
    image:'example/web',project:'anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e',application:'0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e'},
    {id:'b'.repeat(64),name:'unverified',state:'running',status:'Up',image:'example/web',project:'anima-'+('b'.repeat(32)),applicationRecordUnavailable:true},
    {id:'c'.repeat(64),name:'discovered',state:'running',status:'Up',image:'example/web',project:''}]`);
  await click('nav [data-page=Workloads]');
  await click('[data-action=workloads-refresh]');
  await assert(`document.querySelectorAll('[data-action=workload-terminal-target]').length===1`,
    'The terminal affordance requires a verified application record even with a matching project name format');
  await click('[data-action=workload-terminal-target]');
  await assert(`document.querySelector('.dialog').textContent.includes('workshop-web')
    && document.querySelector('.dialog').textContent.includes('1010:1010') && !fixture.terminalTarget`,
    'The exact account and target are visible before opening');
  await click('[data-action=application-terminal-open]');
  await assert(`fixture.terminalTarget?.container==='a'.repeat(64)
    && fixture.terminalTarget?.user==='1010:1010'`, 'The native bridge receives the reviewed target');
  await js('fixture.containers=[]');
  await click('[data-action=workloads-refresh]');
}

export async function checkWorkAdmission({js,click,assert}) {
  const panel="document.querySelector('.page-content [data-work-admission]')";
  const reads="fixture.calls.filter(op=>op==='work-admission').length";
  const start=await js(reads);
  await js("fixture.workAdmission='hosting-pressure'");
  await click('nav [data-page=Overview]');
  await assert(`${reads}>${start} && ${panel}?.textContent.includes('Hosting memory pressure')`,
    'Overview names the local hosting hold');
  await click('nav [data-page=Workloads]');
  await assert(`${panel}?.textContent.includes('Hosting memory pressure') &&
    document.querySelector('.page-content').textContent.includes('System Docker')`,
    'The managed-work hold appears alongside the independent Docker inventory');
  await js("fixture.workAdmission='draining'");
  await click('main [data-action=workloads-refresh]');
  await assert(`${panel}?.textContent.includes('Paused by the operator')`,
    'An operator drain has a distinct reason');
  await click('main [data-work-admission] [data-action=maintenance-review]');
  await assert("document.querySelector('.dialog h2').textContent==='Maintenance'","The drain links to the existing maintenance review");
  await click('.dialog [data-action=close]');
  await js("fixture.workAdmission='update-window'");
  await click('main [data-action=workloads-refresh]');
  await assert(`${panel}?.textContent.includes('Update window is holding new work')`,
    'Automatic admission hold names the scheduled update');
  await click('main [data-work-admission] [data-page=Updates]');
  await assert("document.querySelector('main h1').textContent==='OS updates'", 'The hold links to Updates');
  await click('nav [data-page=Workloads]');
  await js("fixture.workAdmission='pressure-unavailable'");
  await click('main [data-action=workloads-refresh]');
  await assert(`${panel}?.textContent.includes('Waiting for a fresh hosting pressure check')`,
    'Pressure observation loss is distinct from an active hosting stall');
  await js('fixture.failWorkloads=true');
  await click('main [data-action=workloads-refresh]');
  await assert(`${panel}?.textContent.includes('Work admission is unavailable') &&
    document.querySelector('.page-content').textContent.includes('Showing the last successful read')`,
    'Inventory can remain visible after a failed refresh while the prior admission reason clears');
  await js('fixture.failWorkloads=false;delete fixture.workAdmission');
  await click('main [data-action=workloads-refresh]');
  await assert(`${panel}?.textContent.includes('Local policy checks are clear') &&
    ${panel}?.textContent.includes('Anima checks capacity and access when assigning work')`,
    'Clear local checks leave capacity and authorization with Anima');
}

export async function checkWorkloadViews({js, click, assert, cmd}) {
  await js(`fixture.containers=[
    {id:'a'.repeat(64),name:'workshop-web',service:'web',state:'running',status:'Up 1 minute (unhealthy)',health:'unhealthy',image:'example/web',project:'anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e',application:'0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e'},
    {id:'b'.repeat(64),name:'workshop-db',service:'db',state:'exited',status:'Exited',image:'example/db',project:'anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e',application:'0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e'},
    {id:'c'.repeat(64),name:'standalone',state:'running',status:'Up',image:'example/worker',project:''},
    {id:'d'.repeat(64),name:'discovered',service:'web',state:'running',status:'Up',image:'example/web',project:'<workshop>'}
  ]`);
  await click('nav [data-page=Workloads]');
  await assert(`(()=>{const groups=[...document.querySelectorAll('main [data-compose-project]')];
    return groups.length===2&&groups[1].querySelector('h4').textContent==='<workshop>'&&!groups[1].querySelector('[data-application-id]')
      &&['workshop-web','workshop-db','1 of 2 running'].every(text=>groups[0].textContent.includes(text))
      &&!groups[0].textContent.includes('standalone')&&document.querySelector('main [data-standalone-containers]').textContent.includes('standalone')
      &&!document.querySelector('main workshop');})()`,
    'A Compose project groups its exact containers, escapes its name and keeps standalone containers separate');
  await assert(`document.querySelector('main [data-application-id] code')?.textContent==='0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e'`,
    'A project backed by application records shows the ID used in Hosting');
  await choose(js, 'workload-state', 'exited');
  await assert(`document.activeElement.id==='workload-state'&&document.querySelector('main [data-compose-project]').textContent.includes('1 of 2 running')
    &&document.querySelector('main').textContent.includes('workshop-db')&&!document.querySelector('main').textContent.includes('workshop-web')`,
    'The state filter narrows container rows and keeps the whole-project running count');
  await choose(js, 'workload-state', 'all');
  await checkContainerHealth({js,click,assert});
  await cmd('Emulation.setDeviceMetricsOverride',{width:320,height:640,deviceScaleFactor:1,mobile:false});
  await assert(`[...document.querySelectorAll('[data-workload-filter], [data-application-id], .workload-project .row, .workload-project .actions')]
    .every(element=>{const r=element.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth;})`,
    'The project rows, actions and view selector fit a narrow display');
  await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
  await checkHostingStatus({js,click,assert,cmd});
  await checkContainerDetails({js,click,assert});
  await checkContainerLogs({js,click,assert,cmd});
  await js(`delete fixture.containers[1].application;fixture.containers[1].applicationRecordUnavailable=true`);
  await click('main [data-action=workloads-refresh]');
  await assert(`!document.querySelector('main [data-application-id]')&&document.querySelector('main [data-compose-project]').textContent.includes('Application records unavailable')`,
    'A partially unobserved project keeps its rows and withholds the managed label');
  await choose(js, 'workload-management', 'managed');
  await assert(`document.querySelector('main').textContent.includes('workshop-web')
    &&!document.querySelector('main').textContent.includes('workshop-db')
    &&!document.querySelector('main [data-application-id]')`, 'Filtering out an unavailable member does not verify its whole project');
  await choose(js, 'workload-management', 'all');
  await js(`(()=>{const view=document.querySelector('#workload-view');
    view.focus();view.value='builds';view.dispatchEvent(new Event('change',{bubbles:true}));})()`);
  await assert(`document.querySelector('#workload-view').value==='builds'&&document.activeElement.id==='workload-view'
    &&document.querySelector('main').textContent.includes('No recorded builds.')
    &&!document.querySelector('main [data-compose-project]')&&!document.querySelector('main').textContent.includes('Workspace 00000000')`,
    'The view switcher narrows the page and retains keyboard focus');
  await click('main [data-action=workloads-refresh]');
  await assert(`document.querySelector('#workload-view').value==='builds'&&!document.querySelector('main [data-compose-project]')`,
    'Refreshing inventory keeps the selected view');
  await js(`(()=>{const view=document.querySelector('#workload-view');
    view.value='all';view.dispatchEvent(new Event('change',{bubbles:true}));delete fixture.containers;})()`);
  await click('main [data-action=workloads-refresh]');
  await assert(`fixture.pending===0&&document.querySelector('.overlay').hidden`, 'The workload fixture settles before the next journey');
}

async function checkContainerHealth({js,click,assert}) {
  const group="document.querySelector('main [data-compose-project]')";
  await assert(`${group}?.querySelector('[data-unhealthy-count]')?.textContent==='1 unhealthy' &&
    ${group}.querySelector('[data-container-health=unhealthy]').textContent.includes('Up 1 minute (unhealthy)')`,
    'Workloads identifies the unhealthy project and preserves the container’s status');
  await choose(js, 'workload-health', 'unhealthy');
  await assert(`${group}.textContent.includes('1 of 2 running') && ${group}.textContent.includes('workshop-web') &&
    !${group}.textContent.includes('workshop-db') && document.querySelectorAll('main [data-compose-project]').length===1 &&
    document.activeElement.id==='workload-health'`,
    'Filtering unhealthy containers leaves the whole-project counts and keeps focus');
  await js("fixture.containers[0].health='healthy';fixture.containers[0].status='Up 2 minutes (healthy)'");
  await click('main [data-action=workloads-refresh]');
  await assert(`document.querySelector('#workload-health').value==='unhealthy' &&
    document.querySelector('main').textContent.includes('No containers match these filters.')`,
    'A fresh healthy observation clears the warning and preserves the health filter');
  await choose(js, 'workload-health', 'all');
  await assert(`!${group}.querySelector('[data-unhealthy-count]') && ${group}.textContent.includes('workshop-web')`,
    'Clearing the health filter reveals the recovered service without the warning');
}

async function checkHostingStatus({js,click,assert,cmd}) {
  const section="document.querySelector('[data-hosting-status]')";
  const recovery='sudo systemctl start anima-applications.service';
  const enable='sudo anima-enable-hosting --enable';
  const cases=[
    [{setup:'not-configured',service:'inactive',socket:false},'Hosting setup required',enable],
    [{setup:'settings-saved',service:'failed',socket:false},'Hosting service failed',recovery],
    [{setup:'settings-saved',service:'active',socket:false},'Local applications socket unavailable',null],
    [{setup:'settings-saved',service:'active',socket:true},'Check deployment availability in Hosting in Anima',null],
    [{setup:'pairing-retained',service:'inactive',socket:false},'Restore Hosting settings',null],
    [{setup:null,service:'unknown',socket:null},'Local Hosting status unavailable',null],
  ];
  for(const [observation,message,command] of cases){
    await js(`fixture.hosting=${JSON.stringify(observation)}`);
    await click('main [data-action=workloads-refresh]');
    await assert(`(()=>{
      const panel=${section};
      return panel?.textContent.includes(${JSON.stringify(message)}) &&
        (panel.querySelector('code')?.textContent??null)===${JSON.stringify(command)} &&
        !!panel.querySelector('[data-action=terminal]')===${command!==null};
    })()`, `Workloads reports ${message} with the appropriate terminal action`);
    if(command===recovery){
      await cmd('Emulation.setDeviceMetricsOverride',{width:320,height:640,deviceScaleFactor:1,mobile:false});
      await assert(`(()=>{
        const panel=${section};
        return panel.scrollWidth<=panel.clientWidth;
      })()`,'Hosting recovery instructions fit a narrow display');
      await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
      const prior=await js('fixture.calls.length');
      await click('[data-hosting-status] [data-action=terminal]');
      await assert(`fixture.terminalOpened && fixture.calls.length===${prior}`,'Recovery opens the administration terminal directly');
      await js('delete fixture.terminalOpened');
    }
  }
  await js('delete fixture.hosting');
  await click('main [data-action=workloads-refresh]');
}

async function checkContainerLogs({js,click,assert,cmd}) {
  await js(`fixture.logOutput='Synthetic preview output\\n<em data-log-marker>needle</em>';
    fixture.logErrors='WARNING: needle';`);
  await click('main [data-compose-project] [data-action=workload-logs]');
  await assert(`document.querySelector('.dialog').textContent.includes('Synthetic preview output')&&
    document.querySelector('.dialog').textContent.includes('workshop-web')&&fixture.logRequest.lines===200`,
  'Logs opens a bounded default snapshot for the selected container');
  await cmd('Emulation.setDeviceMetricsOverride',{width:320,height:640,deviceScaleFactor:1,mobile:false});
  await assert(`[...document.querySelectorAll('.log-controls select,.log-controls button,.log-controls input')]
    .every(control=>{const box=control.getBoundingClientRect();return box.left>=0&&box.right<=innerWidth;})`,
  'Log controls fit a narrow viewport');
  await cmd('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
  const reads=await js(`fixture.calls.filter(op=>op==='workload-logs').length`);
  await js(`{
    const search=document.querySelector('[data-log-search]');
    search.focus();search.value='NEEDLE';search.dispatchEvent(new Event('input',{bubbles:true}));
  }`);
  await assert(`document.activeElement.matches('[data-log-search]')&&
    fixture.calls.filter(op=>op==='workload-logs').length===${reads}&&
    document.querySelector('[data-log-stdout]').textContent==='<em data-log-marker>needle</em>'&&
    document.querySelector('[data-log-stderr]').textContent==='WARNING: needle'&&
    !document.querySelector('[data-log-marker]')`, 'Search filters both streams locally and treats log text as plain text');
  await js(`document.querySelector('[data-log-lines]').value='50';fixture.logOutput='latest needle';`);
  await click('[data-log-refresh]');
  await assert(`fixture.logRequest.container==='a'.repeat(64)&&fixture.logRequest.lines===50&&
    document.querySelector('[data-log-search]').value==='NEEDLE'&&
    document.querySelector('[data-log-stdout]').textContent==='latest needle'`,
  'Refresh sends the selected bound for the same container and keeps the search');
  await js(`fixture.failLog=true;document.querySelector('[data-log-lines]').value='1000';`);
  await click('[data-log-refresh]');
  await assert(`document.querySelector('[data-log-status]').textContent.includes('previous snapshot')&&
    document.querySelector('[data-log-stdout]').textContent==='latest needle'`,
  'A failed read keeps the previous log snapshot in view');
  await click('.dialog [data-action=close]');
  await assert(`document.activeElement.dataset.action==='workload-logs'&&document.activeElement.dataset.container==='a'.repeat(64)`,
    'Closing logs returns focus to the exact row');
  await js(`delete fixture.failLog;fixture.logDelay=200;fixture.logOutput='pre-lock needle';`);
  await js(`document.querySelector('main [data-compose-project] [data-action=workload-logs]').click();consoleBridge.lock();`);
  await assert(`fixture.pending===0&&!!document.querySelector('#unlock-form')&&!document.querySelector('[data-log-stdout]')`,
  'Lock discards a pending log reply');
  await js(`delete fixture.logDelay;delete fixture.logOutput;delete fixture.logErrors;
    document.querySelector('#unlock-form [name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit();`);
  await assert(`fixture.pending===0&&document.querySelector('.overlay').hidden&&!!document.querySelector('[data-compose-project]')`,
  'Unlock returns to the Workloads inventory');
}

async function checkContainerDetails({js,click,assert}) {
  await js(`fixture.containerDetails={container:'c'.repeat(64),name:'standalone',state:'running',health:'unhealthy',
    networks:['workshop <img data-network-marker>'],
    mounts:[{kind:'volume',name:'workshop-data',destination:'/var/lib/workshop',writable:false}],
    resources:{usage:{cpuPercent:42.5,memory:'12 MiB',tasks:3},
      limits:{cpuMillis:1500,memoryBytes:67108864,tasks:16,cpuSet:'0,2'}}}`);
  await click('[data-standalone-containers] [data-action=workload-details]');
  await assert(`(()=>{
    const dialog=document.querySelector('.dialog');
    const expected=['standalone','Unhealthy','workshop <img data-network-marker>','workshop-data','/var/lib/workshop','Read-only'];
    return fixture.detailsTarget==='c'.repeat(64)&&expected.every(text=>dialog.textContent.includes(text))&&
      !dialog.querySelector('[data-network-marker]');
  })()`,'Details shows observed health, network and storage for the selected container');
  await assert(`(()=>{
    const dialog=document.querySelector('.dialog');
    return ['Current sample','CPU 42.5%','Memory 12 MiB','Tasks 3',
      'Docker configuration','CPU 1.5','CPU set 0,2','Memory 64 MiB','Tasks 16']
      .every(text=>dialog.textContent.includes(text));
  })()`,'Details distinguishes a current resource sample from configured Docker limits');
  await click('.dialog [data-action=close]');
  await assert(`document.activeElement.dataset.action==='workload-details'`,'Closing details returns focus to the selected container');
  await js('fixture.containerDetails.resources.usage=null');
  await click('[data-standalone-containers] [data-action=workload-details]');
  await assert(`(()=>{
    const text=document.querySelector('.dialog').textContent;
    return text.includes('Current sample unavailable')&&text.includes('Docker configuration')
      &&text.includes('Memory 64 MiB')&&text.includes('Storage mounts')&&!text.includes('42.5%');
  })()`,'An unavailable sample keeps the configured limits and the other details visible');
  await click('.dialog [data-action=close]');
  await js('delete fixture.containerDetails;delete fixture.detailsTarget');
}

function choose(js, id, value) {
  return js(`(()=>{const control=document.getElementById(${JSON.stringify(id)});
    if(!control)return false;control.focus();control.value=${JSON.stringify(value)};
    control.dispatchEvent(new Event('change',{bubbles:true}));return true;})()`);
}

export async function checkWorkloadRefresh({js, click, assert}) {
  await click('nav [data-page=Workloads]');
  await assert(`document.querySelector('main time[data-workloads-read]')?.dateTime`, 'Workloads shows when the snapshot was read');
  const observed=await js(`document.querySelector('main time[data-workloads-read]').dateTime`);
  await js(`fixture.failWorkloads=true;fixture.workloadDelay=500;
    document.querySelector('main [data-action=workloads-refresh]').click()`);
  await assert(`document.querySelector('main [data-action=workloads-refresh]')?.getAttribute('aria-disabled')==='true'
    &&document.querySelector('main').textContent.includes('Workspace 00000000')`, 'Refresh keeps the previous rows visible while reading');
  await assert(`document.querySelector('main').textContent.includes('Showing the last successful read')
    &&document.querySelector('main time[data-workloads-read]').dateTime===${JSON.stringify(observed)}
    &&document.querySelector('main').textContent.includes('Workspace 00000000')`, 'A failed refresh keeps the snapshot and its original timestamp');
  await js(`delete fixture.failWorkloads;delete fixture.workloadDelay`);
  await click('main [data-action=workloads-refresh]');
  await assert(`!document.querySelector('main').textContent.includes('Showing the last successful read')
    &&document.querySelector('main time[data-workloads-read]').dateTime!==${JSON.stringify(observed)}`, 'A successful refresh clears the stale notice');
  await js(`fixture.workloadDelay=250;fixture.containers=[{id:'a'.repeat(64),name:'pre-lock observation',
    state:'running',status:'Up',image:'example/web',project:''}];
    document.querySelector('main [data-action=workloads-refresh]').click();consoleBridge.lock();
    delete fixture.containers;delete fixture.workloadDelay`);
  await assert(`fixture.pending===0&&!!document.querySelector('#unlock-form')&&!document.querySelector('main').firstElementChild`,
    'A late workload reply cannot restore the snapshot after lock');
  await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
  await assert(`document.querySelector('.overlay').hidden&&document.querySelector('main time[data-workloads-read]')
    &&!document.querySelector('main').textContent.includes('pre-lock observation')`, 'Unlock reloads Workloads from a new read');
}
