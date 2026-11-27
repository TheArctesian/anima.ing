export async function checkMaintenance({js, click, assert}) {
  await click('footer [data-action=restart]');
  await assert(`document.querySelector('.dialog').textContent.includes('Power changes are held')
    && !document.querySelector('[data-action=power-confirm]')`, 'Running workloads hold restart');
  await click('[data-action=maintenance-drain-begin]');
  await assert(`document.querySelector('.dialog').textContent.includes('New workspace and build reservations are paused')
    && !fixture.calls.includes('restart')`, 'Draining pauses new work without requesting restart');
  await click('[data-action=close]');
  await assert("document.querySelector('[data-work-admission]')?.textContent.includes('Paused by the operator')",
    'A successful drain immediately refreshes the visible local gate');
  await click('footer [data-action=restart]');
  await assert(`document.querySelector('[data-action=maintenance-drain-cancel]')?.dataset.receipt===fixture.drain.id`,
    'Reopening the review uses the observed drain identity');
  await js('fixture.runningDocker=0;fixture.sourceInProgress=true');
  await click('[data-action=maintenance-review][data-target=restart]');
  await assert(`[...document.querySelectorAll('.dialog .row')].some(row =>
    row.textContent.includes('Source build pipeline') && row.textContent.includes('In progress'))
    && !document.querySelector('[data-action=power-confirm]')`, 'A source pipeline remains visible between reservations');
  await js('fixture.sourceInProgress=false');
  await click('[data-action=maintenance-review][data-target=restart]');
  await assert(`document.querySelector('[data-action=power-confirm]').dataset.target==='restart'`,
    'A clear observation restores explicit restart confirmation');
  await js('fixture.updateWindowEndsAt=1795665600');
  await click('[data-action=maintenance-review][data-target=restart]');
  await assert(`document.querySelector('[data-action=maintenance-drain-cancel]').textContent==='Clear manual pause'
    &&document.querySelector('.dialog [data-page=Updates]')`,
    'The maintenance review gives the manual pause and the scheduled window separate controls');
  await click('[data-action=maintenance-drain-cancel]');
  await assert(`fixture.cancelledDrain==='0190a000-0000-7000-8002-000000000003'
    && !fixture.drain && !fixture.calls.includes('restart')
    && document.querySelector('.dialog').textContent.includes('The update window is holding new agent and build work')`,
    'Clearing the manual pause leaves the scheduled hold visible');
  await js('delete fixture.updateWindowEndsAt');
  await click('[data-action=close]');
}

export async function checkUpdateReview({js, click, assert}) {
  await js(`fixture.updateState={phase:'staged',version:'20261120000000',review:{activate:'a'.repeat(64),rollback:'b'.repeat(64)},
    previous:'/nix/store/'+'0'.repeat(32)+'-nixos-system-before-update'}`);
  await click('nav [data-page=Settings]');
  await click('main [data-page=Updates]');
  await click('main [data-action=update-restart]');
  await assert(`document.querySelector('.dialog').textContent.includes('20261120000000')`,
    'Update review identifies the release the operator is approving');
  await js(`fixture.updateState={...fixture.updateState,version:'20261121000000',review:{activate:'c'.repeat(64),rollback:'d'.repeat(64)}}`);
  await click('[data-action=power-confirm]');
  await assert(`fixture.updateApproval?.operation==='update-activate'&&fixture.updateApproval?.confirmed
    &&fixture.updateApproval?.reviewRevision==='a'.repeat(64)`, 'Confirmation keeps the reviewed revision after the staged release changes');
  await assert(`document.querySelector('.overlay').hidden`, 'The activation request settles before another review');
  await click('main [data-action=update-rollback-prompt]');
  await assert(`document.querySelector('.dialog').textContent.includes('nixos-system-before-update')`,
    'Rollback review identifies the exact previous system');
  await click('[data-action=power-confirm]');
  await assert(`fixture.updateApproval?.operation==='update-rollback'&&fixture.updateApproval?.confirmed
    &&fixture.updateApproval?.reviewRevision==='d'.repeat(64)`, 'Rollback carries its own current review revision');
  await js('delete fixture.updateState');
}
