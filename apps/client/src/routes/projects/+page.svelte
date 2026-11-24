<script lang="ts">
  import { onMount } from 'svelte';
  import { projectsApi } from '$lib/projects/api';
  import ProjectsScreen from '$lib/projects/ProjectsScreen.svelte';
  import { SharedProjectGroups } from '$lib/projects/groups/shared.svelte';
  import { ProjectsStore } from '$lib/projects/store.svelte';
  import { useHub } from '$lib/hub/context';
  import { isOnThisComputer } from '$lib/hub/hub';

  const hub = useHub();
  const store = new ProjectsStore(projectsApi(hub));
  const projectGroups = new SharedProjectGroups(hub, localStorage);
  onMount(() => {
    void store.load();
    void projectGroups.refresh();
    const refresh = () => {
      if (document.visibilityState === 'visible' && !projectGroups.busy) void projectGroups.refresh();
    };
    const timer = window.setInterval(refresh, 30_000);
    document.addEventListener('visibilitychange', refresh);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', refresh);
    };
  });
</script>

<ProjectsScreen {store} {projectGroups} keyFileOnly={isOnThisComputer(hub.address)} />
