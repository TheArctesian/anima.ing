window.AnimaRemoteAccess=(()=>{
 const descriptions={
  'legacy-preserved':'Your previous choices have been preserved. Enable VNC here when you want to add it.',
  'managed-defaults':'New installations allow SSH, RDP and VNC over the private link.',
  'pending-defaults':'Default setup is incomplete. Retry setup or save your own access choices here.',
 };
 const observations={
  'unavailable':'Service unavailable',
  'no-reply':'Service did not respond in time',
  'unexpected':'Unexpected protocol or security settings',
  'responding':'Local protocol check passed',
 };
 function protocolStatus(protocol,configured){
  if(!configured)return 'Private access setup pending';
  if(!protocol.enabled)return 'Private access off';
  return observations[protocol.localCheck]??(protocol.listening?'Service listening':'Service unavailable');
 }
 function render(access,connectionText,row,escape){
  const summary=descriptions[access.origin]??'These are your saved private access choices.';
  const protocols=access.protocols.map(protocol=>{
   const name=escape(protocol.name);
   const status=protocolStatus(protocol,access.configured);
   const label=protocol.enabled?'Disable':'Enable';
   const control=`<button data-action="remote-access-select" aria-label="${label} ${name}" data-protocol="${name}" data-port="${escape(protocol.port)}" data-enabled="${!protocol.enabled}">${label}</button>`;
   return row(name,status,control);
  }).join('');
  const retry=access.origin==='pending-defaults'?'<button data-action="remote-access-retry-defaults">Retry default setup</button>':'';
  return `<p>${escape(summary)}</p>${retry}${protocols}<p>${escape(connectionText)}</p>
   <div class="actions">
    <button data-action="remote-disconnect-confirm" data-protocol="RDP">Disconnect RDP clients</button>
    <button data-action="remote-disconnect-confirm" data-protocol="VNC">Disconnect VNC clients</button>
   </div>
   <p>Open a session from an Anima app to verify private-link access and sign-in. Disabling a protocol disconnects its private-link sessions and blocks new connections.</p>`;
 }
 function checks({root,call,dialog,button,row,escape,connectionText}){
  function panel(monitoring){
   const current=root.querySelector('.dialog [data-remote-access-results]')?.closest('.dialog');
   if(current)return current;
   const title=monitoring?'Connection details':'Private remote access';
   const enrollment=monitoring?row('Enrollment','Saved on this computer','Saved')
    +row('Monitoring','Local process state',escape(monitoring)):'';
   return dialog.modal(title,enrollment+'<p data-remote-check role="status"></p><div data-remote-access-results aria-live="polite"></div>',
    button('Check again','remote-access-status')+button('Done','close',true));
  }
  return async ({access,monitoring,retryDefaults=false}={})=>{
   const view=panel(monitoring),version=dialog.version;
   const results=view.querySelector('[data-remote-access-results]');
   const message=view.querySelector('[data-remote-check]');
   const check=view.querySelector('[data-action=remote-access-status]');
   if(check.getAttribute('aria-disabled')==='true')return;
   check.setAttribute('aria-disabled','true');
   message.textContent=retryDefaults?'Retrying private access setup…':'Checking local services…';
   try{
    const report=access??await (retryDefaults?call('remote-access-retry-defaults',{confirmed:true}):call('remote-access-status'));
    if(version!==dialog.version)return;
    const focus=results.contains(document.activeElement)?document.activeElement.dataset:null;
    results.innerHTML=render(report,connectionText(),row,escape);
    message.textContent='';
    if(focus){
     const replacement=[...results.querySelectorAll('button')].find(button=>
      button.dataset.action===focus.action&&button.dataset.protocol===focus.protocol);
     (replacement||check).focus({preventScroll:true});
    }
   }catch(error){
    if(version!==dialog.version)return;
    message.textContent=error.message+(results.firstElementChild?' Showing the previous observation.':'');
   }finally{
    check.removeAttribute('aria-disabled');
   }
  };
 }
 function connectionIndicator(root){
  let graphicalConnections=null;
  function remoteConnectionText(){
   if(!graphicalConnections)return 'Remote connection status unavailable';
   const {rdp,vnc}=graphicalConnections;
   return rdp+vnc?`${rdp} RDP · ${vnc} VNC connections`:'No remote connections observed';
  }
  function showConnectionStatus(value){
   graphicalConnections=value||null;
   for(const element of root.querySelectorAll('[data-remote-connections]')){
    element.textContent=remoteConnectionText();
    element.dataset.active=String(Boolean(value&&(value.rdp+value.vnc)));
   }
  }
  root.querySelector('.header-status').insertAdjacentHTML('beforeend','<span class="remote-connections" data-remote-connections role="status" title="Established graphical transport connections. This does not prove authentication or identify who controls the shared console.">Remote connection status unavailable</span>');
  return {remoteConnectionText,showConnectionStatus};
 }
 return {checks,connectionIndicator};
})();
