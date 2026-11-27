const AnimaStorage = {
  create({call, redraw, escape, row, head, size}) {
    let volumes, reading = false, generation = 0;
    const states = {
      'not-mounted': 'Recorded filesystem is unmounted',
      'unexpected-mount': 'Unexpected filesystem mounted here',
      unavailable: 'Mount or capacity is unavailable',
    };

    function clear() {
      generation++;
      reading = false;
      volumes = undefined;
    }

    async function refresh() {
      if (reading) return;
      const request = ++generation;
      reading = true;
      redraw();
      const result = await call('data-volumes').catch(() => null);
      if (request !== generation) return;
      volumes = Array.isArray(result) ? result : null;
      reading = false;
      redraw();
    }

    function volumeRow(volume) {
      const name = volume.model || `Data volume ${volume.uuid.slice(0,8)}`;
      const path = `<code>${escape(volume.mountpoint)}</code>`;
      const capacity = volume.state === 'mounted' ? volume.capacity : null;
      const status = capacity
        ? `${size(capacity.available)} available of ${size(capacity.total)}<br>${size(capacity.used)} used`
        : (Object.hasOwn(states, volume.state) ? states[volume.state] : states.unavailable);
      return `<div data-volume-state="${escape(volume.state)}">${row(escape(name), path, status)}</div>`;
    }

    function dataVolumes() {
      const refresh = `<button type="button" data-action="storage-refresh" aria-disabled="${reading}">${AnimaPresentation.icon('renew')}<span>Refresh data volumes</span></button>`;
      const body = volumes === undefined ? '<p role="status">Reading data volumes…</p>'
        : volumes === null ? '<p class="notice warning" role="status">Data volume status is unavailable.</p>'
        : volumes.length ? volumes.map(volumeRow).join('') : '<p class="empty-state">No data volumes configured.</p>';
      return `<section class="section" data-data-volumes><h2>Data volumes</h2>${body}${refresh}</section>`;
    }

    function persistent(storage) {
      if (!storage) return '<p>System storage is unavailable.</p>';
      const values = [
        ['Used', storage.used],
        ['Available', storage.available ?? storage.total - storage.used],
        ['Total', storage.total],
      ];
      return '<div class="numbers">' + values.map(([label, bytes]) =>
        `<div><div class="overline">${label}</div><div class="number">${size(bytes)}</div></div>`).join('') + '</div>';
    }

    function render(state) {
      const drives = (state?.drives || []).filter(drive => drive.type === 'disk')
        .map(drive => row(escape(drive.model || drive.name), escape('/dev/' + drive.name), size(drive.size))).join('');
      return head('Storage', 'Disks and persistent capacity on this computer.') +
        '<section class="section"><h2>System storage</h2>' + persistent(state?.storage) + '</section>' +
        dataVolumes() + (drives ? '<section class="section"><h2>Physical disks</h2>' + drives + '</section>' : '') +
        '<div class="notice">OS rollback keeps the current workload data.</div>';
    }

    return {clear, refresh, render};
  },
};
