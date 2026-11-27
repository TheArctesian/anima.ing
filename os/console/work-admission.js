const AnimaWorkAdmission = (() => {
  const labels = {
    maintenance: ['Maintenance is holding new work', 'Review the pending operation to reopen admission.'],
    reconciling: ['Recovering previous work', 'New work waits for this boot’s recovery checks.'],
    draining: ['Paused by the operator', 'Review maintenance to resume new agent and build work.'],
    'update-window': ['Update window is holding new work', 'Existing work can finish. Review the scheduled restart in Updates.'],
    'hosting-pressure': ['Hosting memory pressure', 'New agent and build work waits while the node protects hosted applications.'],
    'pressure-unavailable': ['Waiting for a fresh hosting pressure check', 'New work waits for a current sample.'],
    clear: ['Local policy checks are clear', 'Anima checks capacity and access when assigning work.'],
  };

  function render(state) {
    const [title, detail] = Object.hasOwn(labels, state) ? labels[state]
      : state === undefined ? ['Checking local work policy…', '']
      : ['Work admission is unavailable', ''];
    const action = state === 'update-window' ? AnimaPresentation.pageLink('Updates', 'View activation window', '')
      : state === 'maintenance' || state === 'draining'
        ? AnimaPresentation.button('Review maintenance', 'maintenance-review') : '';
    return `<section class="notice${state==='clear'||state===undefined?'':' warning'}" data-work-admission aria-label="New agent and build work">
      <div role="status"><small class="overline">New agent and build work</small><br><strong>${title}</strong>${detail ? `<p>${detail}</p>` : ''}</div>${action}</section>`;
  }

  function create(call, changed) {
    let state, generation = 0, reading = false;
    function clear() {
      generation++;
      state = undefined;
      reading = false;
    }
    async function refresh() {
      if (reading) return;
      const request = ++generation;
      reading = true;
      const next = await call('work-admission').catch(() => null);
      if (request !== generation) return;
      reading = false;
      if (state === next) return;
      state = next;
      changed();
    }
    return {clear, refresh, render: () => render(state)};
  }

  return {create, render};
})();
