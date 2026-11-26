async function startDownload(click) {
  await click('nav [data-page=Settings]');
  await click('main [data-page=Updates]');
  await click('main [data-action=update-check]');
  await click('main [data-action=update-download]');
  await click('nav [data-page=Network]');
}

export async function checkUpdateProgress({js, click, assert}) {
  const progress = 'main #operation-status:not([hidden]) progress';
  const reading = `document.querySelector(${JSON.stringify(progress)})?.value===120000000`;
  await js('fixture.downloadDelay=4000');
  await startDownload(click);
  await assert(reading, 'Measured update progress remains visible across navigation');
  await js('fixture.holdUpdateStatus=true');
  await assert('!!fixture.deliverUpdateStatus', 'An older progress read is in flight');
  await assert('!fixture.download', 'The update was staged while the read was in flight');
  await js('fixture.deliverUpdateStatus();delete fixture.deliverUpdateStatus');
  await click('nav [data-page=Overview]');
  await assert(`document.querySelector('main').textContent.includes('An OS update is ready.') && !(${reading})`,
    'A late progress reply cannot hide the completed update or resurrect its transfer');

  await startDownload(click);
  await assert(reading, 'The second download has published progress');
  await js('consoleBridge.lock()');
  await assert(`document.querySelector('#unlock-form') && !document.querySelector(${JSON.stringify(progress)})`,
    'Lock clears update details while the native transfer is still active');
  await js(`document.querySelector('[name=password]').value='synthetic';document.querySelector('#unlock-form').requestSubmit()`);
  await assert(reading, 'Unlock reattaches to the existing transfer');
  await assert('!fixture.download', 'The native transfer finished after unlock');
  await click('nav [data-page=Overview]');
  await assert(`document.querySelector('main').textContent.includes('An OS update is ready.') &&
    fixture.calls.filter(op=>op==='update-download').length===2 && !(${reading})`,
    'The new session observes staging without submitting the transfer again');
  await js('delete fixture.downloadDelay;delete fixture.updateState');
}

export async function checkUpdateWindow({js, click, assert, cmd, shot}) {
  await cmd('Emulation.setTimezoneOverride', {timezoneId:'America/New_York'});
  await js(`fixture.updateState={phase:'staged',version:'20261126000000',review:{activate:'e'.repeat(64)}}`);
  await click('nav [data-page=Settings]');
  await click('main [data-page=Updates]');
  await click('main [data-action=update-window-open]');
  await assert(`document.querySelector('.dialog').textContent.includes('20261126000000')
    &&document.querySelector('.dialog').textContent.includes('Starts (UTC)')
    &&document.querySelector('.dialog').textContent.includes('New agent and build work pauses')`,
    'The scheduling dialog identifies the exact staged release');
  await js(`(()=>{
    const form=document.querySelector('#activation-window-form');
    form.elements.starts.value='2026-12-01T03:00';
    form.elements.ends.value='2026-12-01T04:30';
    form.requestSubmit();
  })()`);
  await assert(`fixture.windowApproval?.confirmed&&fixture.windowApproval.reviewRevision==='e'.repeat(64)
    &&fixture.windowApproval.startsAt===Date.parse('2026-12-01T03:00:00Z')/1000
    &&fixture.windowApproval.endsAt-fixture.windowApproval.startsAt===5400
    &&document.querySelector('main').textContent.includes('Scheduled')`,
    'Explicit UTC times approve the staged revision and display the saved window');
  await shot('update-window-scheduled');
  await click('main [data-action=update-window-cancel]');
  await click('[data-action=update-window-cancel-confirm]');
  await assert(`fixture.windowCancellation?.confirmed&&fixture.windowCancellation.id==='0190a000-0000-7000-8002-000000000007'
    &&document.querySelector('main').textContent.includes('Cancelled')
    &&document.querySelector('[data-action=update-restart]')`,
    'Cancellation names the observed window and the staged release remains ready');
  await js('delete fixture.updateState');
  await cmd('Emulation.setTimezoneOverride', {timezoneId:''});
}
