/* Only the authenticated session supplies activity. Keep it out of the lock screen. */
window.AnimaActivity=(()=>{
 let snapshot, loading=false, version=0;
 let attention, sawWaiting=false, checking=false, attentionVersion=0;
 function clear(){
  version++;
  snapshot=undefined;
  loading=false;
  attentionVersion++;
  attention=undefined;
  sawWaiting=false;
  checking=false;
 }
 async function refreshAttention(call,changed){
  if(checking)return;
  const request=++attentionVersion;
  checking=true;
  const next=await call('code-attention').catch(()=>null);
  if(request!==attentionVersion)return;
  checking=false;
  if(JSON.stringify(attention)===JSON.stringify(next))return;
  attention=next;
  if(next?.complete||next?.entries.length)sawWaiting=next.entries.length>0;
  changed();
 }
 function waitingLabel(count){
  return count===1?'1 coding session is waiting for a response':count+' coding sessions are waiting for a response';
 }
 function overview(pageLink){
  const total=attention?.entries.reduce((sum,entry)=>sum+entry.waitingThreads,0);
  if(!total&&!sawWaiting)return '';
  const detail=total
   ? waitingLabel(total)+(attention.complete?'':' · Some workspaces could not be checked')
   : 'A previous check found a waiting session · Current status unavailable';
  return '<div class="notice">'+pageLink('Activity','Needs you',detail)+'</div>';
 }
 function updateAttention(page,escape,pageLink){
  const summary=page.querySelector('.page-content [data-attention-overview]');
  const details=page.querySelector('.page-content [data-attention-details]');
  if(summary)summary.innerHTML=overview(pageLink);
  if(details)details.innerHTML=attentionDetails(escape);
 }
 function attentionDetails(escape){
  const heading='<h2>Needs you</h2>';
  if(attention===undefined)return heading+'<p role="status">Checking for coding sessions that need a response…</p>';
  if(attention===null)return heading+'<p class="notice warning" role="status">Coding session status is unavailable.</p>';
  const rows=attention.entries.map(entry=>`<div class="row"><div class="row-title"><div><strong title="${escape(entry.workspace)}">Workspace ${escape(entry.workspace.slice(0,8))}</strong><small>Generation ${escape(entry.generation)}</small></div></div><span class="value">${waitingLabel(entry.waitingThreads)}</span></div>`).join('');
  const instructions=rows?'<p>Open Code in an Anima app to review the requests.</p>':'';
  const availability=attention.complete
   ? (rows?'':'<p class="empty-state">No coding sessions are waiting for a response.</p>')
   : '<p class="notice warning" role="status">Some coding sessions could not be checked.</p>';
  return heading+instructions+rows+availability;
 }
 async function refresh(call){
  if(loading)return;
  const request=++version;
  loading=true;
  try{
   const result=await call('activity');
   if(request===version)snapshot=result;
  }catch{
   if(request===version)snapshot=null;
  }finally{
   if(request===version)loading=false;
  }
 }
 function timeLabel(atMs,escape){
  const date=new Date(atMs);
  return atMs>0&&Number.isFinite(date.getTime())?`<time class="value" datetime="${date.toISOString()}">${escape(date.toLocaleString())}</time>`:'';
 }
 function eventRow(event,escape){
  return `<div class="row"><div class="row-title"><div><strong>${escape(event.service)}</strong><small>${escape(event.transition)}</small></div></div>${timeLabel(event.atMs,escape)}</div>`;
 }
 function actionHistory(history,escape){
  if(!history)return '<p class="notice warning" role="status">Local action history is unavailable.</p>';
  const rows=history.entries.map(entry=>`<div class="row"><div class="row-title"><div><strong>${escape(entry.action)}</strong><small>${escape(entry.account)} · ${escape(entry.outcome)}</small></div></div>${timeLabel(entry.atMs,escape)}</div>`);
  const warning=history.complete?'':'<p class="notice warning" role="status">Some local actions could not be recorded.</p>';
  const previous=rows.length>5?'<details><summary>Older local actions</summary>'+rows.slice(5).join('')+'</details>':'';
  if(rows.length)return warning+rows.slice(0,5).join('')+previous;
  return warning+(history.complete?'<p class="empty-state">No recorded local actions.</p>':'');
 }
 function systemEvents(events,escape){
  if(!events)return '<p class="notice warning" role="status">System events are unavailable.</p>';
  if(!events.length)return '<p class="empty-state">No recorded service events since boot.</p>';
  return events.map(event=>eventRow(event,escape)).join('');
 }
 function render(escape,button,pageLink){
  const heading='<h1 tabindex="-1">Activity</h1>';
  const refreshButton=button('Refresh','activity-refresh');
  const diagnostics=pageLink('Recovery','Diagnostics & recovery','Run local checks or review maintenance');
  const top=heading+refreshButton+'<section class="section" data-attention-details>'+attentionDetails(escape)+'</section>';
  if(snapshot===undefined)return top+'<p role="status">Reading activity…</p>'+diagnostics;
  if(snapshot===null)return top+'<p class="notice warning" role="status">Activity is unavailable.</p>'+diagnostics;
  return top+'<section class="section" data-action-history><h2>Local console actions</h2>'
   +actionHistory(snapshot.actions,escape)+'</section><section class="section" data-service-events><h2>Service events since boot</h2>'
   +systemEvents(snapshot.system,escape)+'</section><section class="section">'
   +AnimaWorkloads.builds(snapshot.builds,escape)+'</section>'+diagnostics;
 }
 return {clear,refresh,refreshAttention,overview,updateAttention,render};
})();
