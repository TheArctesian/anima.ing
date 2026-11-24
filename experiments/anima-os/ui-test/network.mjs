export async function checkNetwork({js,click,assert,until,shot}){
 const reopen=async()=>{
  await click('[data-action=lock]');
  await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
  await until('fixture.pending===0');await click('nav [data-page=Network]');
 };
 await js(`fixture.devices=[{device:'enp1s0',type:'ethernet',state:'connected',connection:'Wired connection'}];fixture.beforeScan=fixture.calls.filter(c=>c==='scan').length`);
 await reopen();
 await assert(`document.querySelector('main').textContent.includes('No Wi-Fi adapter')&&!document.querySelector('[data-action=scan]')&&!document.querySelector('[data-action^="wifi-"]')`,'Absent Wi-Fi adapter has an honest state without unusable actions');
 await assert(`fixture.calls.filter(c=>c==='scan').length===fixture.beforeScan`,'Absent Wi-Fi adapter never starts a scan');await shot('network-no-adapter');
 await js(`fixture.devices=null;fixture.radio='disabled'`);await reopen();
 await assert(`!!document.querySelector('[data-action=radio-on]')&&!document.querySelector('[data-action=scan]')`,'Wi-Fi off offers enable instead of scan');await shot('network-radio-off');
 await click('[data-action=radio-on]');await assert(`document.querySelector('main').textContent.includes('No networks found')`,'Enabling Wi-Fi automatically scans');
 await js('fixture.failScan=true');await click('[data-action=scan]');
 await assert(`document.querySelector('.overlay').hidden&&document.querySelector('main').textContent.includes('Couldn’t refresh networks')`,'Scan failure remains inline without blocking navigation');await shot('network-scan-failed');
 await js(`fixture.failScan=false;fixture.scanDelay=800;fixture.scan=[{device:'wlan0',bssid:'00:11:22:33:44:01',ssid:'Workshop',signal:'91',security:'WPA2'},{device:'wlan0',bssid:'00:11:22:33:44:02',ssid:'Workshop',signal:'40',security:'WPA2'},{device:'wlan0',bssid:'00:11:22:33:44:03',ssid:'Guest',signal:'64',security:'--'},{device:'wlan0',bssid:'00:11:22:33:44:04',ssid:'<img src=x onerror=alert(1)>',signal:'32',security:'WPA3'}];document.querySelector('[data-action=scan]').click()`);
 await assert(`!document.querySelector('nav').inert&&document.querySelector('.overlay').hidden&&document.querySelector('[data-action=scan]').disabled`,'Scanning keeps navigation available and prevents duplicate refresh');
 await shot('network-scanning');await js(`document.querySelector('nav [data-page=Storage]').click()`);
 await assert(`document.querySelector('h1').textContent==='Storage'`,'Can leave Network during a scan');await until('fixture.pending===0');
 await click('nav [data-page=Network]');
 await assert(`document.querySelectorAll('[data-action^="wifi-"]').length===3`,'Repeated access points consolidate to strongest network');
 await assert(`document.querySelector('main').textContent.includes('<img src=x onerror=alert(1)>')&&!document.querySelector('main img')`,'Network names remain inert text');await shot('network-available');
 await click('[data-action=wifi-0]');await assert(`!!document.querySelector('#wifi-form input[type=password]')`,'Secured network offers password input');
 await js(`document.querySelector('#wifi-form input').value='synthetic-wifi';document.querySelector('#wifi-form').requestSubmit()`);await until('fixture.pending===0');
 await assert(`fixture.connections.at(-1)?.bssid==='00:11:22:33:44:01'&&fixture.connections.at(-1)?.password==='synthetic-wifi'`,'Connect sends chosen strongest access point and entered password');
 await click('[data-action=wifi-1]');await assert(`!!document.querySelector('#wifi-form')&&!document.querySelector('#wifi-form input[type=password]')`,'Open network does not ask for a password');
 await js(`document.querySelector('#wifi-form').requestSubmit()`);await until('fixture.pending===0');
 await assert(`fixture.connections.at(-1)?.bssid==='00:11:22:33:44:03'&&fixture.connections.at(-1)?.password===''`,'Open network connects without a password');
 await js(`fixture.scanDelay=800;document.querySelector('[data-action=scan]').click();fixture.devices=[];document.querySelector('nav [data-page=Storage]').click()`);
 await assert(`document.querySelector('h1').textContent==='Storage'`,'Background scan does not trap navigation');
 await js(`document.querySelector('nav [data-page=Network]').click()`);
 await assert(`document.querySelector('main').textContent.includes('No Wi-Fi adapter')`,'Adapter removal clears scanned networks');await until('fixture.pending===0');
 await assert(`!document.querySelector('[data-action^="wifi-"]')`,'Late scan cannot restore networks from removed adapter');
 await js('fixture.devices=null;fixture.scanDelay=20');await click('nav [data-page=Storage]');await click('nav [data-page=Network]');
 await assert(`document.querySelectorAll('[data-action^="wifi-"]').length===3`,'Reattached adapter starts a fresh scan');
 await js(`fixture.scanDelay=800;document.querySelector('[data-action=scan]').click();document.querySelector('[data-action=lock]').click()`);
 await until('fixture.pending===0');await assert(`!!document.querySelector('#unlock-form')`,'Late scan cannot replace locked session');
 await js(`fixture.scanDelay=20;document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);await until('fixture.pending===0');
}
