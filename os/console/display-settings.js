const AnimaDisplaySettings = {
  create({call, draw, active, escape}) {
    const {button, icon, subhead} = AnimaPresentation;
    const directions = {
      normal: 'Landscape',
      left: 'Portrait left',
      right: 'Portrait right',
      inverted: 'Landscape flipped',
    };
    let outputs;
    let selected;
    let revision = 0;

    function clear() {
      revision++;
      outputs = selected = undefined;
    }

    function reset() {
      document.getElementById('display-form')?.reset();
    }

    async function refresh() {
      const reading = ++revision;
      let observed;
      try {
        observed = await call('displays');
      } catch {
        observed = null;
      }
      if (reading !== revision) return;
      outputs = observed;
      if (!outputs?.some(output => output.name === selected)) selected = outputs?.[0]?.name;
      if (active()) {
        draw();
        reset();
      }
    }

    function select(name) {
      if (!active() || !outputs?.some(output => output.name === name)) return;
      selected = name;
      draw();
    }

    async function preview(form, dialog) {
      const reading = revision;
      const {output, rotation} = form.elements;
      const {revertSeconds} = await call('rotate', {output: output.value, rotation: rotation.value});
      if (reading !== revision || !active()) return;
      dialog.modal('Keep these display settings?',
        `<p id="revert-count">Reverting in ${escape(revertSeconds)} seconds.</p>`,
        button('Revert', 'revert') + button('Keep settings', 'keep-rotation', true));
      dialog.revertCountdown(Date.now() + revertSeconds * 1000, reset);
    }

    function orientationForm() {
      if (outputs === undefined) return '<p role="status">Reading displays…</p>';
      if (outputs === null) return '<p class="notice warning" role="status">Display information is unavailable. Return to Settings and try again.</p>';
      if (!outputs.length) return '<p role="status">No configurable display is available.</p>';
      const current = outputs.find(output => output.name === selected);
      const monitors = outputs.map(({name}) =>
        `<option value="${escape(name)}" ${name === selected ? 'selected' : ''}>${escape(name)}</option>`).join('');
      const rotations = Object.entries(directions).map(([value, label]) =>
        `<option value="${value}" ${value === current.rotation ? 'selected' : ''}>${label}</option>`).join('');
      return `<form id="display-form">
        <label>Display<select name="output">${monitors}</select></label>
        <label>Orientation<select name="rotation">${rotations}</select></label>
        <button class="primary">${icon('checkmark')}Apply display settings</button>
      </form><div class="notice">Unconfirmed changes revert after 15 seconds. Confirmed orientation is saved for future boots.</div>`;
    }

    function render(motionChoice) {
      return subhead('Display & accessibility', 'Keep this screen readable without interrupting background services.')
        + orientationForm()
        + `<label>Interface motion<select id="motion-choice">
          <option value="system" ${motionChoice === 'system' ? 'selected' : ''}>Follow system</option>
          <option value="reduced" ${motionChoice === 'reduced' ? 'selected' : ''}>Reduced motion</option>
        </select></label>
        <section class="section"><h2>Screen power</h2>
          <p>Turn the screen off and lock this console. Press a key, move the mouse or touch the screen to wake it. Background services keep running.</p>
          ${button('Turn screen off', 'screen-off')}
        </section>`;
    }

    return {clear, preview, refresh, render, select};
  },
};
