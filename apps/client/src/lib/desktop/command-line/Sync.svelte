<script lang="ts">
  import { page } from '$app/state';
  import { tokenFor } from '$lib/access/listener/check';
  import { showsSignedOutPage } from '$lib/access/navigation';
  import { useHub } from '$lib/hub/context';
  import { capabilities } from '$lib/platform/platform';
  import { startCommandLineSignIn } from './sign-in';

  const hub = useHub();
  let started = false;
  $effect(() => {
    if (!capabilities().commandLine || started || showsSignedOutPage(page.url)
      || hub.status.current.connection !== 'connected') return;
    started = true;
    void tokenFor(hub.address, true)(hub.address).then(async (token) => {
      if (token !== null) await startCommandLineSignIn(hub.address, token);
    }).catch(() => { console.warn('Command-line sign-in could not read the app session.'); });
  });
</script>
