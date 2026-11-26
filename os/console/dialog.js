const AnimaDialog = {
  create({root, icon, animate, reduced, role, canDismiss}) {
    const overlay = root.querySelector('.overlay');
    overlay.tabIndex = -1;
    const shell = root.querySelector('.shell');
    const main = root.querySelector('main');
    let version = 0;
    let returnFocus = null;
    let returnSelector = null;
    let closing = false;
    let timer = null;

    function backgroundInert(value) {
      for (const element of root.querySelectorAll('header,nav,main,footer')) element.inert = value;
    }

    function cancelCountdown() {
      clearInterval(timer);
      timer = null;
    }

    function close(immediate = false) {
      cancelCountdown();
      if (overlay.hidden || (closing && !immediate)) return;
      closing = true;
      const panel = overlay.querySelector('.dialog');
      panel.inert = true;
      panel.querySelectorAll('input[type=password]').forEach(input => { input.value = ''; });
      overlay.focus({preventScroll: true});
      const transition = ++version;
      const finish = () => {
        if (transition !== version) return;
        AnimaHero.stop();
        overlay.getAnimations({subtree: true}).forEach(animation => animation.cancel());
        overlay.hidden = true;
        overlay.innerHTML = '';
        overlay.classList.remove('lock-screen');
        closing = false;
        shell.removeAttribute('data-locked');
        backgroundInert(false);
        if (returnFocus?.isConnected && returnFocus !== document.body) {
          returnFocus.focus();
        } else {
          const fallback = root.querySelector(returnSelector || '.page-content h1') || main.querySelector('h1');
          fallback?.focus({preventScroll: true});
        }
      };
      if (immediate || reduced()) {
        finish();
      } else if (overlay.classList.contains('lock-screen')) {
        panel.style.opacity = '0';
        animate(panel, [{opacity: 1}, {opacity: 0}], role.fast);
        AnimaHero.play(overlay.querySelector('.lock-art'), 'out', reduced).then(async () => {
          if (transition !== version) return;
          await animate(overlay, [{opacity: 1}, {opacity: 0}], role.press);
          finish();
        });
      } else {
        const opacity = getComputedStyle(overlay).opacity;
        panel.style.opacity = getComputedStyle(panel).opacity;
        panel.style.transform = getComputedStyle(panel).transform;
        overlay.getAnimations({subtree: true}).forEach(animation => animation.cancel());
        animate(overlay, [{opacity}, {opacity: 0}], role.fast).then(finish);
      }
    }

    function modal(title, body, actions, heroMotion = true) {
      const locked = title === 'Local session locked';
      const art = locked ? overlay.querySelector('.lock-art') : null;
      if (!art) AnimaHero.stop();
      ++version;
      closing = false;
      if (overlay.hidden) {
        returnFocus = document.activeElement;
        const {action, page, container, attempt} = returnFocus?.dataset || {};
        const target = Object.entries({container, attempt}).flatMap(([name, value]) =>
          value ? [`[data-${name}="${CSS.escape(value)}"]`] : []).join('');
        returnSelector = action ? `[data-action="${CSS.escape(action)}"]${target}`
          : page ? `[data-page="${CSS.escape(page)}"]` : null;
      }
      overlay.getAnimations({subtree: true}).forEach(animation => animation.cancel());
      cancelCountdown();
      const glyph = /locked|sign-in/i.test(title) ? 'locked'
        : /Restart|Shut down|Power/.test(title) ? 'power'
        : /Wi-Fi|network/i.test(title) ? 'connection-signal' : 'settings';
      overlay.innerHTML = `<section class="dialog" role="dialog" aria-modal="true" aria-label="${title}"><h2>${icon(glyph)}${title}</h2>${body}<div class="actions">${actions}</div></section>`;
      overlay.hidden = false;
      backgroundInert(true);
      overlay.classList.toggle('lock-screen', locked);
      if (locked) {
        if (art) overlay.prepend(art);
        else overlay.insertAdjacentHTML('afterbegin', '<div class="lock-art" aria-hidden="true"><img src="hero.png" alt="" draggable="false"></div>');
        if (heroMotion) void AnimaHero.play(overlay.querySelector('.lock-art'), 'in', reduced);
        else AnimaHero.stop();
      }
      shell.toggleAttribute('data-locked', locked);
      const panel = overlay.querySelector('.dialog');
      if (!locked) {
        animate(overlay, [{opacity: 0}, {opacity: 1}], role.fast);
        animate(panel, [{transform: 'translateY(6px)', opacity: 0}, {transform: 'translateY(0)', opacity: 1}]);
      }
      (tabStops()[0] || overlay).focus({preventScroll: true});
      return panel;
    }

    function revertCountdown(deadline, expired) {
      const count = overlay.querySelector('#revert-count');
      timer = setInterval(() => {
        const seconds = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
        if (seconds === 0) {
          close();
          expired();
        }
        else count.textContent = `Reverting in ${seconds} seconds.`;
      }, 200);
    }

    function tabStops() {
      return [...overlay.querySelectorAll('button,input,select,textarea,a[href],[tabindex]')]
        .filter(element => element.tabIndex >= 0 && !element.matches(':disabled') && element.checkVisibility());
    }

    overlay.addEventListener('keydown', event => {
      if (event.key === 'Escape' && !closing && canDismiss() && !overlay.querySelector('#revert-count')) close();
      if (event.key !== 'Tab') return;
      const controls = closing ? [] : tabStops();
      const first = controls[0], last = controls.at(-1);
      const from = event.shiftKey ? first : last;
      const to = event.shiftKey ? last : first;
      if (!from || document.activeElement === overlay || document.activeElement === from) {
        event.preventDefault();
        to?.focus();
      }
    });

    return {close, modal, revertCountdown, get version() { return version; }};
  },
};
