const AnimaPages = {
  create({main, animate, reduced, role}) {
    let page;
    let markup;

    function clear() {
      main.getAnimations({subtree: true}).forEach(animation => animation.cancel());
      main.replaceChildren();
      page = markup = undefined;
    }

    function restoreFocus(content, previous) {
      const attributes = ['id', 'name', 'data-action', 'data-page', 'data-container', 'data-attempt'];
      const identity = attributes.flatMap(attribute => previous.hasAttribute(attribute)
        ? [`[${attribute}="${CSS.escape(previous.getAttribute(attribute))}"]`] : []).join('');
      if (!identity && !previous.matches('h1,summary')) return;
      const target = content.querySelector(previous.localName + identity);
      target?.focus({preventScroll: true});
    }

    function show(destination, html) {
      const navigating = destination !== page;
      if (!navigating && html === markup) return;
      const previous = main.querySelector('.page-content');
      const focused = previous?.contains(document.activeElement) ? document.activeElement : null;
      const scroll = navigating ? 0 : main.scrollTop;
      const expanded = !navigating && previous?.querySelector('details')?.open;
      page = destination;
      markup = html;

      const content = document.createElement('div');
      content.className = 'page-content';
      content.innerHTML = html;
      if (navigating) main.querySelector('.page-outgoing')?.remove();
      if (previous && navigating && !reduced()) {
        const opacity = getComputedStyle(previous).opacity;
        previous.getAnimations().forEach(animation => animation.cancel());
        previous.className = 'page-outgoing';
        previous.inert = true;
        previous.setAttribute('aria-hidden', 'true');
        previous.querySelectorAll('[id]').forEach(element => element.removeAttribute('id'));
        animate(previous, [{opacity}, {opacity: 0}], role.fast).then(() => previous.remove());
        animate(content, [{opacity: 0}, {opacity: 1}]);
        main.append(content);
      } else if (previous) {
        previous.replaceWith(content);
      } else {
        main.append(content);
      }

      const details = content.querySelector('details');
      if (expanded && details) details.open = true;
      main.scrollTop = scroll;
      if (!navigating && focused) restoreFocus(content, focused);
    }

    return {clear, show};
  },
};
