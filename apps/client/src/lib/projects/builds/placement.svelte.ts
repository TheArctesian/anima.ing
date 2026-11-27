import type { BuildPlacement, SourceBuildApi } from './api';

export class BuildNodePin {
  placement = $state<BuildPlacement | null>(null);
  selected = $state('');
  reading = $state(false);
  saving = $state(false);
  error = $state('');

  constructor(private api: SourceBuildApi, private project: string) {}

  get hasChanges(): boolean {
    return this.selected !== (this.placement?.pinned?.id ?? '');
  }

  private accept(placement: BuildPlacement): void {
    this.placement = placement;
    this.selected = placement.pinned?.id ?? '';
  }

  async load(): Promise<void> {
    if (this.reading || this.saving) return;
    this.reading = true;
    this.error = '';
    try {
      const reply = await this.api.placement(this.project);
      if (reply.ok) this.accept(reply.value.projectBuildPlacement);
      else this.error = reply.error.message;
    } finally { this.reading = false; }
  }

  async save(): Promise<void> {
    if (this.reading || this.saving || this.placement === null) return;
    this.saving = true;
    this.error = '';
    try {
      const reply = await this.api.pin(this.project, this.selected || null);
      if (reply.ok) this.accept(reply.value.setProjectBuildPin);
      else this.error = reply.error.message;
    } finally { this.saving = false; }
  }
}
