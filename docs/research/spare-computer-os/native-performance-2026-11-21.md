# Actual Anima OS artifact comparison

Raw evidence: `/home/okita/Scripts/Work/Anima.ing/native-performance-20261121`. Each cell is the median of per-boot measurements, with minimum–maximum where values differ.

| Metric | Before | After |
| --- | ---: | ---: |
| ISO bytes | 3,934,945,280.00 | 3,925,868,544.00 |
| Live serial shell observed (s) | 20.18 (20.11–20.82) | 20.08 (20.07–20.16) |
| Live visible installer observed (s) | 21.03 (20.95–21.88) | 20.92 (20.92–21.01) |
| Live guest MemAvailable (MiB) | 3,253.57 (3,251.41–3,260.03) | 3,245.04 (3,244.09–3,256.74) |
| Live installer tree RSS sum (MiB) | 264.30 (264.29–264.72) | 264.53 (264.52–264.58) |
| Installed lock screen observed (s) | 15.19 (15.18–15.21) | 15.17 (15.15–15.21) |
| Installed guest MemAvailable (MiB) | 3,269.77 (3,248.88–3,276.06) | 3,252.80 (3,251.29–3,264.08) |
| Installed locked-console tree RSS sum (MiB) | 679.86 (675.57–683.19) | 679.00 (672.56–684.88) |
| Privileged session startup (ms) | 57.77 (57.21–59.73) | 3.74 (2.69–3.85) |
| Status round-trip median (ms) | 25.38 (25.37–25.96) | 26.52 (25.98–27.44) |

Per-boot memory values are medians of five idle samples. Status values are medians of ten successful protocol requests. Readiness is an observed upper bound including serial login, polling and (installed) OCR, not a paint timestamp. Live Calamares and installed-console measurements describe different software. Installed idle memory is measured with the graphical console locked; dashboard interaction and render latency are unmeasured.

Artifact-wide comparison cannot attribute a change to Rust or the UI. Host desktop and an unrelated Windows workload remained running; file checksums warm the ISO cache. Guest RAM and CPUs are fixed, but host scheduling/cache/thermal effects remain uncontrolled. RSS sums double-count shared pages. No live account, network, agent work or physical hardware was measured.

## Artifact identity

- before: `/home/okita/.local/share/anima-os-prototype/2026-11-20/delivery-hardware-fixes/anima-os-preview-x86_64.iso`; SHA-256 `e43c2a9521370232076618beefe3b2d286e483eafcf4d6be3a9d323ed75749f0`; 3 fresh offline VM samples.
- after: `/nix/store/jpw0gzanqfkgbdv0rspsp2v4898v9w5b-anima-os-preview-x86_64.iso/iso/anima-os-preview-x86_64.iso`; SHA-256 `ebe416621bccb1f158e2520286a9d305f22e85c3087871f796ab3b8d4e04b001`; 3 fresh offline VM samples.

Incomplete attempts excluded from statistics: `1-after-incomplete-101189923615040`. Their serial logs remain in the evidence directory.

All unselected sentinel disks unchanged: True.

Graphical usability acceptance remains a separate final operator check.

Host one-minute load at sample starts/ends ranged 0.42–2.83; host MemAvailable at starts ranged 29.93–30.57 GiB.

The excluded first native attempt hit a harness classification bug: the serial console emitted two carriage returns before a newline, so the harness selected the legacy installer path. It was interrupted before installation, preserved separately, and replaced with a fresh-disk sample after fixing the parser. The six completed measurements above exclude that attempt.
