const AnimaUpdates = (() => {
  const phases = {
    idle: 'Not checked', current: 'Up to date', staged: 'Ready to restart', booting: 'Awaiting boot checks',
    'rolled-back': 'Previous system restored', checking: 'Checking release', downloading: 'Downloading',
    verifying: 'Verifying download', importing: 'Importing system', staging: 'Preparing next boot', interrupted: 'Needs review',
  };
  const phaseLabel = phase => Object.hasOwn(phases, phase) ? phases[phase] : 'Not yet checked';
  const windowPhases = {
    scheduled: 'Scheduled', deferred: 'Waiting for work to finish', expired: 'Window ended',
    changed: 'Update selection changed', cancelled: 'Cancelled', dispatching: 'Restart request recorded',
    'needs-review': 'Needs review',
  };
  const windowTime = new Intl.DateTimeFormat([], {
    dateStyle: 'medium', timeStyle: 'short', timeZone: 'UTC',
  });

  function create({call, redraw, view, escape, size, dialog}) {
    const {button, pageLink, row, subhead} = AnimaPresentation;
    let status = null;
    let offer = null;
    let unavailable = false;
    let reading = null;
    let downloading = null;
    let timer = null;
    let revision = 0;
    let reviewedWindow = null;

    const working = () => downloading !== null || Boolean(status?.operation && status.operation.phase !== 'interrupted');
    const summary = () => JSON.stringify([status?.phase, status?.version, status?.previous, status?.activationWindow, working(), unavailable]);
    const eligibleWindow = () => !working() && !unavailable && status?.phase === 'staged'
      && /^[a-f0-9]{64}$/.test(status?.review?.activate);
    const pendingWindow = () => ['scheduled', 'deferred'].includes(status?.activationWindow?.phase);

    function invalidateRead() {
      revision++;
      reading = null;
    }

    function schedule() {
      if (working()) {
        timer ??= setInterval(refresh, 1000);
      } else {
        clearInterval(timer);
        timer = null;
      }
    }

    function observe(next, forceRedraw = false) {
      const before = summary();
      status = next;
      unavailable = false;
      schedule();
      if (forceRedraw || before !== summary()) redraw();
      else renderProgress();
    }

    function refresh() {
      if (reading) return reading;
      const requested = revision;
      reading = call('update-status').then(next => {
        if (requested === revision) observe(next);
      }, () => {
        if (requested !== revision || unavailable) return;
        unavailable = true;
        redraw();
      }).finally(() => {
        if (requested === revision) reading = null;
      });
      return reading;
    }

    async function check(request) {
      if (working()) return;
      const requested = revision;
      const result = await request();
      if (requested !== revision) return;
      offer = result;
      redraw();
      await refresh();
    }

    async function download() {
      if (working()) return;
      const transfer = Symbol();
      invalidateRead();
      downloading = transfer;
      observe(status ? {...status, operation: null} : null, true);
      const request = call('update-download');
      void refresh();
      let result;
      try {
        result = await request;
      } catch (error) {
        if (downloading !== transfer) return;
        downloading = null;
        invalidateRead();
        schedule();
        redraw();
        void refresh();
        throw error;
      }
      if (downloading !== transfer) return;
      downloading = null;
      invalidateRead();
      offer = null;
      observe(result, true);
    }

    function clear() {
      invalidateRead();
      status = null;
      offer = null;
      unavailable = false;
      downloading = null;
      reviewedWindow = null;
      schedule();
    }

    function scheduleDialog() {
      if (!eligibleWindow() || pendingWindow()) return;
      dialog.modal('Schedule update restart',
        '<p>Release ' + escape(status.version) + ' can restart this computer during the selected window if workloads are clear.</p>'
        + '<p>New agent and build work pauses during the window. Existing work can finish. Hosted applications keep running until you stop them through Hosting.</p>'
        + '<form id="activation-window-form"><label>Starts (UTC)<input type="datetime-local" name="starts" required></label>'
        + '<label>Ends (UTC)<input type="datetime-local" name="ends" required></label>'
        + '<button type="submit" class="primary">Schedule restart</button></form>',
        button('Cancel', 'close'));
      reviewedWindow = {version: dialog.version, review: status.review.activate};
    }

    function cancelDialog() {
      if (unavailable || !pendingWindow()) return;
      dialog.modal('Cancel scheduled restart?', '<p>The verified update will stay ready to install.</p>',
        button('Keep window', 'close') + button('Cancel window', 'update-window-cancel-confirm', true));
      reviewedWindow = {version: dialog.version, id: status.activationWindow.id};
    }

    async function saveWindow(form) {
      if (reviewedWindow?.version !== dialog.version || !reviewedWindow.review) return;
      const startsAt = Date.parse(form.elements.starts.value + 'Z') / 1000;
      const endsAt = Date.parse(form.elements.ends.value + 'Z') / 1000;
      if (!Number.isSafeInteger(startsAt) || !Number.isSafeInteger(endsAt)) throw Error('Choose valid window times.');
      await changeWindow('update-schedule-activation', {startsAt, endsAt, reviewRevision: reviewedWindow.review});
    }

    async function cancelWindow() {
      if (reviewedWindow?.version !== dialog.version || !reviewedWindow.id) return;
      await changeWindow('update-cancel-activation', {id: reviewedWindow.id});
    }

    async function changeWindow(action, args) {
      const version = dialog.version, requested = revision;
      await call(action, {...args, confirmed: true});
      if (requested !== revision) return;
      if (version === dialog.version) dialog.close(true);
      invalidateRead();
      await refresh();
    }

    function renderWindow() {
      const window = status?.activationWindow;
      if (window?.phase === 'unavailable') {
        return row('Activation window', 'Scheduled restart', 'Unavailable');
      }
      const configure = eligibleWindow() && !pendingWindow() ? button('Schedule restart', 'update-window-open') : '';
      if (!window) return configure;
      const period = t => windowTime.format(t * 1000);
      const detail = escape(period(window.startsAt) + ' – ' + period(window.endsAt) + ' UTC');
      const label = unavailable ? 'Last observed' : windowPhases[window.phase] || 'Unavailable';
      const cancel = !unavailable && pendingWindow() ? button('Cancel window', 'update-window-cancel') : '';
      return '<section class="section"><h2>Activation window</h2>'
        + row(escape(window.version), detail, label)
        + (window.pausesNewWork ? '<p>New agent and build work pauses during this window.</p>' : '')
        + (window.reason ? '<p>' + escape(window.reason) + '</p>' : '')
        + '<div class="actions">' + cancel + configure + '</div></section>';
    }

    function renderProgress() {
      const {panel, onUpdates} = view();
      if (!panel) return;
      const operation = status?.operation;
      panel.hidden = !working() && !operation;
      if (panel.hidden) return;
      const measured = !unavailable && operation?.phase !== 'interrupted'
        && Number.isFinite(operation?.bytes) && Number.isFinite(operation?.totalBytes) && operation.totalBytes > 0;
      const note = operation?.phase === 'interrupted' ? 'The update did not finish. Review its status before trying again.'
        : unavailable ? 'Progress unavailable · retrying.' : 'Working. You can keep using local controls.';
      const detail = measured ? `<span>${size(operation.bytes)} of ${size(operation.totalBytes)}</span>
        <progress max="${operation.totalBytes}" value="${operation.bytes}" aria-label="Update transfer"></progress>` : `<span>${note}</span>`;
      const link = onUpdates ? '' : pageLink('Updates', 'View update', 'Progress and restart options');
      panel.innerHTML = `<strong>OS update · ${phaseLabel(operation?.phase || 'downloading')}</strong>${detail}${link}`;
    }

    function render() {
      const actions = working() ? '' : button('Check for updates', 'update-check')
        + (!unavailable && status?.phase === 'staged' ? button('Restart to update', 'update-restart', true) : '')
        + (!unavailable && status?.previous ? button('Roll back OS', 'update-rollback-prompt') : '');
      const available = offer?.available && !working()
        ? `<div class="section"><h2>Update ${escape(offer.release.version)}</h2>
          <p>${size(offer.release.bytes)} download. The current system is retained for recovery.</p>${button('Download update', 'update-download', true)}</div>`
        : offer?.available === false ? '<p class="empty-state">No newer system is available.</p>' : '';
      return subhead('OS updates', 'Verified system updates retain your persistent data.')
        + row('Update status', 'This computer', unavailable ? 'Unavailable' : phaseLabel(status?.phase))
        + (status?.version ? row('Release', escape(status.version), unavailable ? 'Last observed' : phaseLabel(status.phase)) : '')
        + `<div class="actions">${actions}</div>` + renderWindow() + available
        + '<div class="notice">Downloading keeps services running. Restarting interrupts work; finish it first. OS rollback does not rewind your data.</div>';
    }

    return {
      check, clear, download, refresh, render, renderProgress, working,
      scheduleDialog, cancelDialog, saveWindow, cancelWindow,
      get ready() { return status?.phase === 'staged' && !unavailable; },
    };
  }

  return {create};
})();
