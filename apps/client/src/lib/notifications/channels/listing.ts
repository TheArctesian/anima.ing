// NOT-002, NOT-004, NAV-005.
import type { Delivery, NotificationChannel } from '../api';

export function withDelivery(channels: readonly NotificationChannel[], id: string, delivery: Delivery): NotificationChannel[] {
  return channels.map((channel) => (channel.id === id ? { ...channel, lastDelivery: delivery } : channel));
}

export function withSaved(channels: readonly NotificationChannel[], saved: NotificationChannel): NotificationChannel[] {
  const known = channels.some((channel) => channel.id === saved.id);
  return known ? channels.map((channel) => (channel.id === saved.id ? saved : channel)) : [...channels, saved];
}

/** `channels` is the list without the deleted one; `null` sends focus to "Add channel". */
export function focusAfterDelete(channels: readonly Pick<NotificationChannel, 'id'>[], index: number): string | null {
  return (channels[index] ?? channels[index - 1])?.id ?? null;
}
