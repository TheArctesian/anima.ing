let localReadings;
function updateTelemetry(value,icon,motion){
 localReadings=value;
 renderBatteries(value?.batteries??[]);
 renderTelemetry(icon,motion);
}
function renderBatteries(observed){
 const batteries=document.querySelector('#battery-status');
 batteries.replaceChildren();
 for(const battery of observed){
  const badge=document.createElement('span');
  const percentage=Number.isFinite(battery.percent)?battery.percent:null;
  const label=`${battery.name}: ${percentage===null?'charge unknown':Math.round(percentage)+'%'} · ${battery.state}`;
  badge.className='battery';badge.title=label;badge.setAttribute('aria-label',label);badge.setAttribute('role','img');
  badge.dataset.low=percentage!==null&&percentage<=15&&battery.state==='Discharging';
  badge.innerHTML=`<svg viewBox="0 0 32 20" aria-hidden="true"><rect x="1" y="3" width="26" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M29 7v6" stroke="currentColor" stroke-width="2"/>${percentage===null?'<text x="10" y="14" fill="currentColor" font-size="12">?</text>':`<rect x="4" y="6" width="${20*percentage/100}" height="8" rx="1" fill="currentColor"/>`}${battery.state==='Charging'?'<path d="m17 1-7 10h5l-1 8 8-11h-6z" fill="#1B1D21" stroke="currentColor"/>':''}</svg>`;
  batteries.append(badge);
 }
}

function changeMetric(metric,value,missing,{animate,reduced,role}){
 const current=metric.querySelector('strong'),meter=metric.querySelector('meter');
 const reading=value===null?missing:Math.round(value)+'%';
 if(current.textContent!==reading){
  metric.querySelector('.metric-previous')?.remove();
  const opacity=getComputedStyle(current).opacity;
  current.getAnimations().forEach(animation=>animation.cancel());
  if(current.textContent&&!reduced()){
   const old=document.createElement('span');
   old.className='metric-previous';old.setAttribute('aria-hidden','true');old.textContent=current.textContent;
   current.before(old);
   animate(old,[{opacity},{opacity:0}],role.fast).then(()=>old.remove());
   animate(current,[{opacity:0},{opacity:1}],role.fast);
  }
  current.textContent=reading;
 }
 meter.hidden=value===null;
 if(value===null)meter.removeAttribute('value');
 else meter.value=value;
}

function renderTelemetry(icon,motion){
 const panel=document.querySelector('#telemetry-readings');
 if(!panel)return;
 if(!localReadings){
  panel.textContent=localReadings===null?'Local readings unavailable.':'Collecting local readings…';
  return;
 }
 const r=localReadings;
 const memory=Number.isFinite(r.memoryTotal)&&r.memoryTotal>0&&Number.isFinite(r.memoryUsed)?100*r.memoryUsed/r.memoryTotal:null;
 const metrics=[['CPU','chip',Number.isFinite(r.cpu)?r.cpu:null,r.cpuCollecting?'Collecting…':'Unavailable'],
  ['Memory','sim-card',memory,'Unavailable']];
 if(!panel.querySelector('.metric')){
  panel.innerHTML=metrics.map(([label,glyph])=>`<div class="metric"><span class="metric-label">${icon(glyph)}${label}</span><span class="metric-value"><strong></strong></span><meter min="0" max="100" aria-label="${label} usage"></meter></div>`).join('')+'<p class="hint"></p>';
 }
 panel.querySelectorAll('.metric').forEach((metric,index)=>{
  const [,,value,missing]=metrics[index];
  changeMetric(metric,value,missing,motion);
 });
 panel.querySelector('.hint').textContent=Number.isFinite(r.uptime)?`Uptime · ${Math.floor(r.uptime/3600)}h ${Math.floor(r.uptime%3600/60)}m · Local readings, refreshed every 5 seconds`:'Uptime unavailable';
}
