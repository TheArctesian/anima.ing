# Anima.ing image assets

`logo.png` is the approved repository asset from `design/brand/logo.png`.
The Calamares and terminal palettes come from `design/tokens/palettes/nord.tokens.json`
and `design/tokens/terminal.tokens.json`. Body text uses Nord snow white after the
operator's explicit readability correction on 2026-11-20.

`hero.png` is a deterministic still of the existing anima-site particle hero,
rendered from `src/lib/story/field.ts` at site commit
`3d71012f754e7d76d677cbee66d6358f004a84d2`. Its background is the site's graphite base.
The source site is not modified. Regenerate with:

```sh
bun os/branding/render-hero.ts /path/to/anima-site > /tmp/anima-hero.svg
rsvg-convert /tmp/anima-hero.svg -o os/branding/hero.png
```

The fullscreen installer blends this still behind welcome/completion content with a slow drift; reduced motion disables that animation. Automatic screensaver, locking and
sleep are disabled during installation. This does not add a desktop to the installed server.

`wordmark.svg` is the operator-supplied vector wordmark. `settings.svg`, `back.svg`, `next.svg`, `power.svg` and `restart.svg` use IBM Carbon paths (Apache-2.0), matching `apps/client/src/lib/icons/shapes.ts`. The current installer uses Graphite #1B1D21 and frost #88C0D0 with IBM Plex Sans.


The installed lock-screen transitions use the same source geometry. Generate the
committed particle projection separately (no site runtime dependency):

```sh
bun os/branding/render-hero.ts /path/to/anima-site --particles > os/console/hero-points.js
```

`hero-motion.js` animates this projection for 620 ms on entry and 300 ms on exit,
then stops drawing. The static `hero.png` remains the idle and reduced-motion
fallback. No continuous particle loop runs behind hosted or agent workloads.
The source field, shapes and timeline used for this export match the hero commit
above. Runtime performance and visual continuity remain VM qualification items.
