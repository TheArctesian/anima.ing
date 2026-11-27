const AnimaMaintenance = {
  create({call, dialog, unlocked, row, button, esc, changed}) {
    const power = new Map([
      ['restart', {title: 'Restart this computer?', label: 'Restart'}],
      ['shutdown', {title: 'Shut down this computer?', label: 'Shut down'}],
      ['update-activate', {title: 'Restart to apply the update?', label: 'Restart and update', update: 'activate'}],
      ['update-rollback', {title: 'Restart into the previous system?', label: 'Restart and roll back', update: 'rollback'}],
    ]);
    let reviewed = null;

    function control(label, action, target = '', primary = false) {
      return `<button class="${primary ? 'primary' : 'quiet'}" data-action="${action}" data-target="${esc(target)}">${label}</button>`;
    }

    function readableCount(value) {
      return Number.isSafeInteger(value) && value >= 0 ? String(value) : 'Unavailable';
    }

    function observations(impact) {
      const legacy = impact.legacyRunning;
      return row('Active reservations', 'Agent workspaces and builds', readableCount(impact.reservations))
        + row('Source build pipeline', 'Covers the handoff from source checkout to build',
          impact.sourceInProgress === true ? 'In progress' : impact.sourceInProgress === false ? 'Clear' : 'Unavailable')
        + row('Running Docker containers', 'Includes managed workspaces', readableCount(impact.runningDocker))
        + row('Legacy Podman workloads', 'Observed across local accounts', legacy === true ? 'Running' : legacy === false ? 'None observed' : 'Unavailable')
        + row('Hosting dispatch', 'Managed application operations', impact.hostingClear ? 'Clear' : 'In progress or needs review');
    }

    function drainActions(impact, target) {
      if (impact.pendingTransition) {
        return '<p>A previous maintenance request holds admission.</p>'
          + control('Review previous request', 'maintenance-status');
      }
      const scheduled = Number.isSafeInteger(impact.updateWindowEndsAt)
        ? '<p>The update window is holding new agent and build work. Hosted applications keep running until their owners stop them.</p>'
          + AnimaPresentation.pageLink('Updates', 'View activation window', '')
        : '';
      if (impact.drain) {
        return scheduled + '<p>New workspace and build reservations are paused. Accepted source builds and work with an active reservation can finish. Stop long-lived workspaces and applications through their owners before continuing.</p>'
          + `<button class="quiet" data-action="maintenance-drain-cancel" data-target="${esc(target)}" data-receipt="${esc(impact.drain.id)}">${scheduled ? 'Clear manual pause' : 'Resume new work'}</button>`;
      }
      if (scheduled) return scheduled;
      return '<p>Pause new workspace and build reservations while reserved work finishes. Hosted applications stay running until their owners stop them.</p>'
        + control('Pause new work', 'maintenance-drain-begin', target);
    }

    function updateSelection(action, status) {
      const revision = status?.review?.[action];
      if (!/^[a-f0-9]{64}$/.test(revision)) return null;
      if (action === 'activate' && status.phase === 'staged' && status.version) {
        return {revision, summary: row('Release', 'Verified update ready to install', esc(status.version))};
      }
      if (action === 'rollback' && typeof status.previous === 'string' && status.previous.startsWith('/nix/store/')) {
        return {revision, summary: row('Previous system', 'Rollback target', esc(status.previous.slice('/nix/store/'.length)))};
      }
      return null;
    }

    async function review(target = '') {
      const transition = power.get(target);
      const title = transition?.title ?? 'Maintenance';
      dialog.modal(title, '<p role="status">Checking running work…</p>', '');
      const version = dialog.version;
      const [impact, updateStatus] = await Promise.all([
        call('maintenance-impact'),
        transition?.update ? call('update-status') : null,
      ]);
      if (!unlocked() || version !== dialog.version) return;
      const selection = updateSelection(transition?.update, updateStatus);
      const awaitingUpdate = transition?.update && !selection;
      const note = awaitingUpdate ? '<p>The update selection is unavailable. Check OS updates again before restarting.</p>' : transition
        ? (impact.ready ? '<p>Workload checks passed. Remote access disconnects while this computer is offline.</p>'
          : '<p>Power changes are held until these checks clear.</p>')
        : '';
      const confirm = transition && impact.ready && !awaitingUpdate ? control(transition.label, 'power-confirm', target, true) : '';
      dialog.modal(title, (selection?.summary || '') + note + observations(impact) + drainActions(impact, target),
        button(transition ? 'Cancel' : 'Close', 'close') + control('Refresh status', 'maintenance-review', target) + confirm);
      reviewed = confirm ? {target, version: dialog.version, revision: selection?.revision} : null;
    }

    async function previous() {
      const version = dialog.version;
      const receipt = await call('maintenance-status');
      if (!unlocked() || version !== dialog.version) return;
      if (!receipt) {
        await review();
        return;
      }
      const body = '<p>A previous ' + esc(receipt.operation)
        + ' request has closed admission. Confirm the requested action did not take effect before releasing the receipt.</p>'
        + `<button class="quiet" data-action="maintenance-release" data-receipt="${esc(receipt.id)}">Reconcile and release</button>`;
      dialog.modal('Previous maintenance request', body, button('Close', 'close'));
    }

    async function update(action, target, id) {
      const version = dialog.version;
      await call(action, {id, confirmed: true});
      changed();
      if (unlocked() && version === dialog.version) await review(target);
    }

    async function dispatch(target) {
      const transition = power.get(target);
      if (!transition || reviewed?.target !== target || reviewed.version !== dialog.version) return;
      const args = {confirmed: true};
      if (reviewed.revision) args.reviewRevision = reviewed.revision;
      dialog.modal(transition.title, '<p>Checking workload state and requesting the power change…</p>', '');
      const version = dialog.version;
      await call(target, args);
      if (unlocked() && version === dialog.version) dialog.close(true);
    }

    return {
      handle(action, {target, receipt}) {
        switch (action) {
          case 'restart':
          case 'shutdown': return review(action);
          case 'update-restart': return review('update-activate');
          case 'update-rollback-prompt': return review('update-rollback');
          case 'maintenance-review': return review(target);
          case 'maintenance-status': return previous();
          case 'maintenance-release':
          case 'maintenance-drain-begin':
          case 'maintenance-drain-cancel': return update(action, target, receipt);
          case 'power-confirm': return dispatch(target);
          default: return null;
        }
      },
    };
  },
};
