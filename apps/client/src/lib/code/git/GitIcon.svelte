<script lang="ts" module>
  import type { IconName } from '$lib/icons/icons';

  /**
   * T3 Code's Lucide name for each icon of the Git parts, drawn with the app's own icon until the Lucide list of the
   * Code section (`$lib/code/icons/icons.ts`, stage 1) is in `dev`. Then this file goes and each use becomes `CodeIcon`.
   */
  const STAND_INS = {
    check: 'carbon:checkmark',
    'chevron-down': 'carbon:chevron-down',
    'chevron-right': 'carbon:chevron-right',
    circle: 'carbon:circle-dash',
    'circle-alert': 'carbon:warning',
    'cloud-upload': 'carbon:cloud-upload',
    'file-diff': 'carbon:document',
    folder: 'carbon:folder',
    'folder-git-2': 'carbon:folder-shared',
    'git-branch': 'carbon:branch',
    'git-commit': 'carbon:checkmark-outline',
    'git-pull-request': 'carbon:merge',
    info: 'carbon:information',
    laptop: 'carbon:screen',
    'loader-circle': 'carbon:renew',
    minus: 'carbon:subtract',
    pencil: 'carbon:edit',
    terminal: 'carbon:terminal',
    'triangle-alert': 'carbon:warning-alt',
    x: 'carbon:close'
  } as const satisfies Record<string, IconName>;

  export type GitIconName = keyof typeof STAND_INS;
</script>

<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte';

  interface Props {
    name: GitIconName;
  }

  const { name }: Props = $props();
</script>

<Icon name={STAND_INS[name]} size="sm" />
