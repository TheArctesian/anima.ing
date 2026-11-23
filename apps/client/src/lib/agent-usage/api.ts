import { graphql } from '$lib/hub/generated';
import type { AgentSource, AgentUsageFilter, AgentUsageState } from '$lib/hub/generated/graphql';
import type { Hub, HubResult } from '$lib/hub/hub';
import type { BatteryListing } from '$lib/metrics/battery/battery';

// One read (AIS-020). `$everything` drops the repository so the repository list stays whole; `$everyone` drops the
// device and program so the device choice keeps every device whose daemon sends counts.
const AGENT_USAGE = graphql(`
  query AgentUsage($filter: AgentUsageFilter, $everything: AgentUsageFilter, $everyone: AgentUsageFilter) {
    days: agentUsage(filter: $filter, by: [DAY, SOURCE, MODEL]) {
      firstDay
      lastDay
      pricesAsOf
      rows {
        ...AgentUsageSums
        source
        model
        day
      }
      total {
        ...AgentUsageSums
      }
    }
    models: agentUsage(filter: $filter, by: [SOURCE, MODEL]) {
      rows {
        ...AgentUsageSums
        source
        model
      }
    }
    devices: agentUsage(filter: $filter, by: [DEVICE]) {
      rows {
        ...AgentUsageSums
        device {
          id
          name
        }
      }
      devices {
        state
        device {
          id
          name
          batteries {
            name
            state
          }
        }
      }
    }
    repositories: agentUsage(filter: $everything, by: [REPOSITORY]) {
      rowsLeftOut
      rows {
        ...AgentUsageSums
        repository
      }
      total {
        ...AgentUsageSums
      }
    }
    newest: agentUsage(filter: $everything, by: [REPOSITORY, DAY]) {
      rows {
        repository
        day
      }
    }
    shares: agentUsage(filter: $filter, by: [DEVICE, SOURCE]) {
      rows {
        ...AgentUsageSums
        device {
          id
          name
        }
        source
      }
    }
    everyone: agentUsage(filter: $everyone, by: []) {
      accounts {
        key
        shortKey
        source
        plan
        reportedAt
        devices {
          id
          name
        }
        limits {
          window
          usedPercent
          resetsAt
          reached
        }
      }
      devices {
        state
        device {
          id
          name
          batteries {
            name
            state
          }
        }
      }
    }
    sessions: agentSessions(filter: $filter, first: 50) {
      sessionsLeftOut
      sessions {
        id
        device {
          id
          name
        }
        source
        model
        repository
        startedAt
        lastUseAt
        recent
        prompts
        toolCalls
        inputTokens
        outputTokens
        cacheReadTokens
        cacheWriteTokens
        estimatedCostUsd
        unpricedTokens
      }
    }
  }
`);

// The fragment is only read by codegen, which inlines it into the document that spreads it.
void graphql(`
  fragment AgentUsageSums on AgentUsageRow {
    sessions
    prompts
    toolCalls
    inputTokens
    outputTokens
    cacheReadTokens
    cacheWriteTokens
    estimatedCostUsd
    unpricedTokens
  }
`);

/** The cost is null when no model of the group has a known price. */
export interface UsageSums {
  readonly sessions: number;
  readonly prompts: number;
  readonly toolCalls: number;
  readonly inputTokens: number;
  readonly outputTokens: number;
  readonly cacheReadTokens: number;
  readonly cacheWriteTokens: number;
  readonly estimatedCostUsd: number | null;
  readonly unpricedTokens: number;
}

export interface NamedDevice {
  readonly id: string;
  readonly name: string;
}

/** A device whose daemon sent a usage state (AIS-024), with the batteries that power it (MET-032). */
export interface UsageDevice {
  readonly device: NamedDevice & { readonly batteries: readonly BatteryListing[] };
  readonly state: AgentUsageState;
}

export type DayRow = UsageSums & { readonly day: string; readonly source: AgentSource | null; readonly model: string | null };
export type ModelRow = UsageSums & { readonly source: AgentSource | null; readonly model: string | null };
export type DeviceRow = UsageSums & { readonly device: NamedDevice };
export type RepositoryRow = UsageSums & { readonly repository: string | null };

/** Sums over the days of the page (AIS-025); `recent` means used in the last 30 minutes (AIS-026). */
export type SessionRow = Omit<UsageSums, 'sessions'> & {
  readonly id: string;
  readonly device: NamedDevice;
  readonly source: AgentSource;
  readonly model: string | null;
  readonly repository: string | null;
  readonly startedAt: string;
  readonly lastUseAt: string;
  readonly recent: boolean;
};

export type DeviceSourceRow = UsageSums & { readonly device: NamedDevice; readonly source: AgentSource };

export interface AccountLimit {
  readonly window: string;
  /** 0 to 100; null when the program says only whether it is reached. */
  readonly usedPercent: number | null;
  readonly resetsAt: string | null;
  readonly reached: boolean;
}

/** Known to the hub only by its key; the name of the account never leaves a device (AIS-022, R-313). */
export interface AgentAccount {
  readonly key: string;
  readonly shortKey: string;
  readonly source: AgentSource;
  readonly plan: string | null;
  readonly reportedAt: string;
  readonly devices: readonly NamedDevice[];
  readonly limits: readonly AccountLimit[];
}

export interface AgentUsageView {
  readonly firstDay: string;
  readonly lastDay: string;
  readonly pricesAsOf: string;
  readonly total: UsageSums;
  readonly days: readonly DayRow[];
  readonly models: readonly ModelRow[];
  readonly devices: readonly DeviceRow[];
  readonly states: readonly UsageDevice[];
  readonly repositories: readonly RepositoryRow[];
  readonly everything: UsageSums;
  readonly newest: readonly { readonly repository: string | null; readonly day: string }[];
  readonly repositoriesLeftOut: number;
  readonly everyDevice: readonly UsageDevice[];
  readonly sessions: readonly SessionRow[];
  readonly sessionsLeftOut: number;
  readonly accounts: readonly AgentAccount[];
  readonly shares: readonly DeviceSourceRow[];
}

export type UsageRow = UsageSums & {
  readonly device: NamedDevice | null;
  readonly source: AgentSource | null;
  readonly model: string | null;
  readonly repository: string | null;
  readonly day: string | null;
};
export interface UsageAnswer {
  readonly pricesAsOf: string;
  readonly rowsLeftOut: number;
  readonly total: UsageSums;
  readonly devices: readonly UsageDevice[];
}

export interface UsageRequest {
  readonly filter: AgentUsageFilter;
}

function everyRepository(filter: AgentUsageFilter): AgentUsageFilter {
  return Object.fromEntries(Object.entries(filter).filter(([key]) => key !== 'repository'));
}

export function agentUsageApi(hub: Pick<Hub, 'request'>) {
  return {
    read: async ({ filter }: UsageRequest): Promise<HubResult<AgentUsageView>> => {
      const day = filter.lastDay ?? undefined;
      const everyone: AgentUsageFilter = day === undefined ? {} : { firstDay: day, lastDay: day };
      const result = await hub.request(AGENT_USAGE, { filter, everything: everyRepository(filter), everyone });
      if (!result.ok) return result;
      const { days, models, devices, repositories, newest, everyone: all, sessions, shares } = result.value;
      const withDay = days.rows.flatMap((row) => (row.day == null ? [] : [{ ...row, day: row.day }]));
      const withDevice = devices.rows.flatMap((row) => (row.device == null ? [] : [{ ...row, device: row.device }]));
      const shared = shares.rows.flatMap((row) => (row.device == null || row.source == null ? [] : [{ ...row, device: row.device, source: row.source }]));
      const lastUse = newest.rows.flatMap((row) => (row.day == null ? [] : [{ repository: row.repository ?? null, day: row.day }]));
      return {
        ok: true,
        value: {
          firstDay: days.firstDay,
          lastDay: days.lastDay,
          pricesAsOf: days.pricesAsOf,
          total: days.total,
          days: withDay.map((row) => ({ ...row, source: row.source ?? null, model: row.model ?? null })),
          models: models.rows.map((row) => ({ ...row, source: row.source ?? null, model: row.model ?? null })),
          devices: withDevice,
          states: devices.devices,
          repositories: repositories.rows.map((row) => ({ ...row, repository: row.repository ?? null })),
          everything: repositories.total,
          newest: lastUse,
          repositoriesLeftOut: repositories.rowsLeftOut,
          everyDevice: all.devices,
          sessions: sessions.sessions.map((row) => ({ ...row, model: row.model ?? null, repository: row.repository ?? null })),
          sessionsLeftOut: sessions.sessionsLeftOut,
          accounts: all.accounts.map((account) => ({
            ...account,
            plan: account.plan ?? null,
            limits: account.limits.map((limit) => ({ ...limit, usedPercent: limit.usedPercent ?? null, resetsAt: limit.resetsAt ?? null })),
          })),
          shares: shared,
        },
      };
    },
  };
}
