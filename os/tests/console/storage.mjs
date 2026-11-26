export async function checkStorage({js,click,assert}) {
  await js(`fixture.dataVolumes=[
    {uuid:'6318ea8b-0bfa-44d1-8d7a-92408bbbf178',model:'Workshop <disk>',mountpoint:'/persist/volumes/6318ea8b-0bfa-44d1-8d7a-92408bbbf178',state:'mounted',capacity:{total:42949672960,available:25769803776,used:17179869184}},
    {uuid:'7bc8aa8a-34f1-435e-8338-09c37770bac1',model:'Archive',mountpoint:'/persist/volumes/7bc8aa8a-34f1-435e-8338-09c37770bac1',state:'not-mounted'},
    {uuid:'e7b4c51e-d8bc-4306-9b32-2ec9b8027faa',model:'Spare disk',mountpoint:'/persist/volumes/e7b4c51e-d8bc-4306-9b32-2ec9b8027faa',state:'unexpected-mount'}
  ]`);
  await click('nav [data-page=Storage]');
  await assert(`(()=>{
    const panels=[...document.querySelectorAll('[data-volume-state]')];
    return panels.length===3 && panels[0].textContent.includes('Workshop <disk>') &&
      panels[0].textContent.includes('24.0 GiB available of 40.0 GiB') &&
      panels[0].textContent.includes('16.0 GiB used') &&
      panels[1].textContent.includes('Recorded filesystem is unmounted') &&
      panels[2].textContent.includes('Unexpected filesystem mounted here') &&
      !panels[1].textContent.includes('GiB') && !panels[0].querySelector('disk');
  })()`,'The exact data volume supplies the capacity and unresolved volumes have a distinct state');
  await js('fixture.failDataVolumes=true');
  await click('main [data-action=storage-refresh]');
  await assert(`document.querySelector('[data-data-volumes]').textContent.includes('Data volume status is unavailable') &&
    !document.querySelector('[data-volume-state]') && document.querySelector('main').textContent.includes('/dev/nvme0n1')`,
    'A volume read failure clears its capacity and leaves the physical disk inventory visible');
  await js('delete fixture.failDataVolumes;delete fixture.dataVolumes');
  await click('main [data-action=storage-refresh]');
  await assert(`document.querySelector('[data-data-volumes]').textContent.includes('No data volumes configured')`,
    'A successful empty volume inventory has its own state');
}
