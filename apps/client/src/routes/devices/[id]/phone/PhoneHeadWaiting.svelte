<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/icons/Icon.svelte';
  import { press } from '$lib/motion/roles/press';
</script>

<!-- The phone head while the device loads: the same rows and heights as PhoneHead, so the tabs and the panel under it
     do not move when the device comes (MOT-007, MA-4 of the site-language plan). -->
<header class="phone-device-head" aria-busy="true">
  <div class="navrow">
    <a class="back" href={resolve('/devices')} {@attach press()}><Icon name="carbon:chevron-left" />Devices</a>
  </div>
  <div class="title-row">
    <div class="identity">
      <p class="name"><span class="bone" aria-hidden="true"></span><span class="hidden" role="status">Loading the device</span></p>
      <p class="host"><span class="bone short" aria-hidden="true"></span></p>
    </div>
  </div>
</header>

<style>
  /* One line of each text, with a quiet block where the words will stand. */
  .name,
  .host {
    position: relative;
  }

  .name::after,
  .host::after {
    content: "\200b";
  }

  /* The loaded host line holds the copy button, one touch target high. */
  .host {
    min-block-size: var(--target-min-touch);
  }

  .bone {
    position: absolute;
    inset-block: 15%;
    inset-inline-start: 0;
    inline-size: calc(var(--space-16) * 3);
    border-radius: var(--radius-sm);
    background: var(--surface-raised);
  }

  .bone.short {
    inline-size: calc(var(--space-16) * 2);
  }

  .hidden {
    position: absolute;
    overflow: hidden;
    clip-path: inset(50%);
    inline-size: 1px;
    block-size: 1px;
    white-space: nowrap;
  }
</style>
