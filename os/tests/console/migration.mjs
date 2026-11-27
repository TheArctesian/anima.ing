export async function checkMigration({js,click,assert,shot}){
 await click('nav [data-page=Workloads]');
 const inspect=async()=>{
  await click('[data-action=migration-accounts]');
  await click('[data-action=migration-inventory]');
 };
 await inspect();
 await assert(`(()=>{const text=document.querySelector('.dialog').textContent;return text.includes('example-internal')&&text.includes('Internal')&&text.includes('DNS enabled')&&text.includes('10.78.0.0/24')&&text.includes('10.78.0.1')})()`,'Legacy report preserves network topology');
 await click('.dialog summary');
 await assert(`(()=>{const text=document.querySelector('.dialog').textContent;return text.includes('/home/admin/.local/share/containers/storage')&&text.includes('Runtime 0 → host 1000')&&text.includes('Runtime 0 → host 100')&&text.includes('65536 IDs')})()`,'Legacy report exposes actual storage and outer user/group mappings for ownership review');
 await shot('migration-networks');await click('[data-action=close]');
 await js(`fixture.migration={uid:1000,user:'admin',containers:[],volumes:[],networks:[{name:'<img src=x onerror=alert(1)>',driver:'bridge',internal:true,dnsEnabled:false,subnets:[]}],remaining:[]}`);
 await inspect();
 await assert(`document.querySelector('.dialog').textContent.includes('<img src=x onerror=alert(1)>')&&!document.querySelector('.dialog img')`,'Legacy network names are text, not executable markup');
 await assert(`document.querySelector('.dialog').textContent.includes('Runtime storage and ownership mappings are unavailable. Do not copy workload data yet.')`,'An older report cannot imply that ownership mapping is known');
 await assert(`document.querySelector('.dialog').textContent.includes('Quadlet inventory is unavailable.')&&document.querySelector('.dialog').textContent.includes('Systemd startup inventory is unavailable.')`,'An older report keeps missing startup evidence visible');
 await click('[data-action=close]');
 await js(`delete fixture.migration.networks;
  fixture.migration.startup={quadlets:[{name:'<img src=x>.container',path:'/legacy/<img src=x>.container',unit:'custom-app.service'}],
  units:[{name:'custom-app.service',enablement:'generated',evidence:'quadlet'},{name:'container-db.service',enablement:'enabled',evidence:'container_label'}]};
  fixture.migration.containers=[{name:'db',mounts:[],autoRemove:true,systemdUnitHint:'container-db.service'}]`);
 await inspect();
 await assert(`document.querySelector('.dialog').textContent.includes('Network inventory is unavailable.')&&!document.querySelector('.dialog').textContent.includes('No networks reported.')`,'An older or unavailable network report is not presented as empty');
 await assert(`(()=>{const dialog=document.querySelector('.dialog'),text=dialog.textContent;return !dialog.querySelector('img')&&text.includes('/legacy/<img src=x>.container')&&text.includes('Quadlet definition')&&text.includes('Container label')&&text.includes('Reported unit: container-db.service')&&text.includes('generated')})()`,'Startup evidence is shown in context and metadata stays text');
 await assert(`document.querySelector('.dialog').textContent.includes('anima-applications capture-source')`,'Auto-remove sources show pre-stop capture before migration');
 await click('[data-action=close]');
 await js('fixture.migration.networks=[]; fixture.migration.startup={quadlets:[],units:null}');await inspect();
 await assert(`document.querySelector('.dialog').textContent.includes('No networks reported.')&&!document.querySelector('.dialog').textContent.includes('Network inventory is unavailable.')`,'An observed empty inventory is distinct from unavailable data');
 await assert(`document.querySelector('.dialog').textContent.includes('No Quadlet definitions reported.')&&document.querySelector('.dialog').textContent.includes('Systemd startup inventory is unavailable.')`,'Empty and unavailable startup sources stay distinct');
 await click('[data-action=close]');await js('delete fixture.migration');
}
