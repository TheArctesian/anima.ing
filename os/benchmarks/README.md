# Actual artifact comparison

Run sequentially after compiler and other VM workloads finish:

```sh
python3 os/benchmarks/run.py /absolute/before.iso /absolute/after.iso /disk-backed/new-results-directory --rounds 3
```

The harness uses the existing `vm.Machine`: KVM, UEFI, two virtual CPUs, 4096 MiB memory, no NIC or host shares. Every sample gets fresh disposable regular files: a 64 GiB virtual installation disk and a 1 GiB unselected sentinel whose hash is checked. It installs only `/dev/vda` inside that VM. Before/after order alternates each round. The old interactive installer and new JSON installer are supported; installation duration is not compared.

Each sample records ISO byte size/hash, host load, boot-to-serial-shell time, observed graphical readiness, five guest memory samples after a 20-second settling period, and screenshots. Live readiness means a mapped Calamares window, not a rendered-content or user-usability claim. Installed readiness requires the rendered “Local session locked” label. These observations follow serial login and include polling, serial authentication and OCR overhead: they are upper bounds, not precise paint timestamps.

Live installer memory and installed-console memory are separate results. Installed idle memory is sampled while the graphical console remains locked; dashboard interaction and render latency are not measured. `MemAvailable` estimates guest-wide reclaimable capacity; shell-tree RSS sums process resident sizes and double-counts shared pages. It is not proportional set size or an allocation profile. The guest's one-shot read-only sampler adds Python overhead equally to both images.

Installed session measurements time native/legacy privileged process startup and ten successful `status` protocol round trips inside the guest. They exclude local-password authentication and rendering. A missing or incompatible protocol is recorded as unavailable rather than replaced with a proxy. No account, daemon work, network transfer, or sustained agent workload is benchmarked.

Do not infer causality from artifact-wide changes or a single run. Host caches, storage state, VM scheduling and thermal/load variation remain uncontrolled; repeated values must be reported with spread. The harness preserves raw results and serial logs. Secrets are generated only for each disposable VM. This measurement is separate from final human graphical acceptance.

`--resume` retains completed JSON records only after checking artifact paths. An incomplete attempt is moved aside and excluded; its replacement always gets fresh disks. `report.py RESULTS_DIRECTORY --output REPORT.md` produces medians and ranges from completed records.
