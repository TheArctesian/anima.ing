const AnimaLogs = {
  buildStage(stage) {
    const labels = {
      'waiting-for-builder': 'Waiting for builder',
      detecting: 'Detecting project',
      building: 'Building image',
      'checking-export': 'Checking exported image',
    };
    return Object.hasOwn(labels, stage) ? labels[stage] : '';
  },

  container({container, name, call, dialog, button}) {
    AnimaLogs.open({
      title: 'Container logs', name, dialog, button, recentLines: true,
      async snapshot(lines) {
        const result = await call('workload-logs', {container, lines});
        return {...result, summary: `Showing up to ${result.tail} recent lines.`};
      },
    });
  },

  build({attempt, call, dialog, button}) {
    AnimaLogs.open({
      title: 'Build logs', name: `Attempt ${attempt}`, dialog, button, recentLines: false,
      async snapshot() {
        const result = await call('workload-build-logs', {attempt});
        const program = result.program === 'railpack' ? 'Railpack' : 'BuildKit';
        const stage = AnimaLogs.buildStage(result.stage) || 'Stage unavailable';
        const incomplete = result.finished ? '' : ' · Stage did not finish';
        return {...result, summary: `${program} · ${stage}${incomplete}`};
      },
    });
  },

  open({title, name, dialog, button, recentLines, snapshot: load}) {
    const panel = dialog.modal(title, `
      <p class="log-target" data-log-name></p>
      <div class="log-controls">
        ${recentLines ? `<label>Recent lines<select data-log-lines>
          <option value="50">50</option>
          <option value="200" selected>200</option>
          <option value="500">500</option>
          <option value="1000">1000</option>
        </select></label>` : ''}
        <button type="button" data-log-refresh>${AnimaPresentation.icon('renew')}<span>Refresh</span></button>
        <label class="log-search">Search loaded logs<input type="search" data-log-search></label>
      </div>
      <p data-log-status role="status"></p>
      <h3>Standard output</h3><pre class="container-log" tabindex="0" aria-label="Standard output" data-log-stdout></pre>
      <h3>Standard error</h3><pre class="container-log" tabindex="0" aria-label="Standard error" data-log-stderr></pre>
    `, button('Close', 'close'));
    const version = dialog.version;
    const find = name => panel.querySelector(`[data-log-${name}]`);
    const lines = find('lines'), search = find('search'), refresh = find('refresh');
    const status = find('status'), stdout = find('stdout'), stderr = find('stderr');
    find('name').textContent = name;
    let snapshot;
    let reading = false;

    function displayed(entries, query, empty) {
      if (!entries.length) return empty;
      if (!query) return entries.join('\n');
      return entries.filter(line => line.toLowerCase().includes(query)).join('\n') || 'No matching lines';
    }

    function render() {
      if (!snapshot) return;
      const query = search.value.trim().toLowerCase();
      stdout.textContent = displayed(snapshot.stdout, query, 'No output');
      stderr.textContent = displayed(snapshot.stderr, query, 'No errors');
    }

    async function read() {
      if (reading || dialog.version !== version) return;
      reading = true;
      refresh.setAttribute('aria-disabled', 'true');
      status.textContent = 'Reading logs…';
      try {
        const result = await load(recentLines ? Number(lines.value) : undefined);
        if (dialog.version !== version) return;
        snapshot = {stdout: result.stdout ? result.stdout.split('\n') : [],
          stderr: result.stderr ? result.stderr.split('\n') : []};
        status.textContent = result.summary;
        render();
      } catch (error) {
        if (dialog.version !== version) return;
        if (!recentLines) {
          snapshot = undefined;
          stdout.textContent = stderr.textContent = '';
        }
        status.textContent = error.message + (snapshot ? ' Showing the previous snapshot.' : '');
      } finally {
        reading = false;
        refresh.removeAttribute('aria-disabled');
      }
    }

    search.addEventListener('input', render);
    refresh.addEventListener('click', read);
    void read();
  },
};
