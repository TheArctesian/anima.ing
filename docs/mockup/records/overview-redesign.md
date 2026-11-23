# The record of the Device Overview redesign

- Mockup: [device-overview-v2/overview-redesign.html](../device-overview-v2/overview-redesign.html)
- Owner: M2-7 in [mobile-plan.md](../../plans/mobile-plan.md)
- Screens: The Overview of [devices](../../../apps/client/src/routes/devices) and the [metrics display](../../../apps/client/src/lib/metrics)
- State: Approved by the operator in chat on 2026-11-23. Physical acceptance is part of M2-7.
- Compared: `b8b3d9f44` (2026-11-23). Compared rendered phone and tablet views in WebKit. The health line, selected resource trend, All resource charts, and expanded Storage follow the approved frame.
- Differences: Readings show the reported sample time. The application lays out real drives and volumes using the storage table and preserves drive activity and unmounted drives. The Settings tab takes a place in the phone tab row when selected, with Sessions then reachable through More. The prototype uses synthetic Pop!_OS readings. The application also retains Hub plan usage, connection setup, and the independent load and error states of Checks and Metrics.
