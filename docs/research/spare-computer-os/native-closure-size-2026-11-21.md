# Native preview closure size — 2026-11-21

Read-only attribution of the final preview built from `566f5e7ca` (ISO SHA-256 `ebe416621bccb1f158e2520286a9d305f22e85c3087871f796ab3b8d4e04b001`). No package, image, source projection or runtime setting changed during this investigation. Nix database metadata queries do not measure startup, responsiveness or resident memory.

## Exact artifacts and measurements

| Artifact | Store output / derivation | Measured size |
| --- | --- | --- |
| Compressed ISO filesystem image | `/nix/store/jpw0gzanqfkgbdv0rspsp2v4898v9w5b-anima-os-preview-x86_64.iso/iso/anima-os-preview-x86_64.iso`; derivation `/nix/store/98xrf7b9pvw51g0ps664q9g6nd64z0m0-anima-os-preview-x86_64.iso.drv` | **3,925,868,544 file bytes**, 3.926 decimal GB / 3.656 GiB |
| Installed appliance closure | `/nix/store/v185bydi6n7xz96d9fpb16sicwspm8hj-nixos-system-anima-Preview`; derivation `/nix/store/d1sagfg4ig73i7qf7xy4iyjh1645nbda-nixos-system-anima-Preview.drv` | **7,963,321,616 NAR bytes**, 7.416 GiB, 1,227 unique paths |
| Live installer including offline appliance closure | `/nix/store/hx87nccqsd8zp3kqlfzk3y0js654147i-nixos-system-anima-live-Preview`; derivation `/nix/store/qfrigx427n1axcx6cmn6h19a2ixvxi8p-nixos-system-anima-live-Preview.drv` | **9,859,909,184 NAR bytes**, 9.183 GiB, 1,658 unique paths |
| Live-only difference | Set difference between the two closures above | **1,896,587,568 NAR bytes**, 1.766 GiB |

NAR sizes describe serialized uncompressed store paths, including NAR metadata; they are not filesystem allocation, compressed download sizes or RAM consumption. Shared paths are counted once per closure. The ISO stores the combined closure in SquashFS using `zstd -Xcompression-level 6`, plus boot assets. A NAR saving cannot be converted directly into an ISO saving without rebuilding and measuring. The ISO output deliberately discards references, so querying its own closure alone misses the embedded systems.

## Dominant individual store paths

Values below are each output's own NAR size, not its transitive closure. These rows are disjoint; transitive family totals elsewhere must not be added to them.

| Output | NAR MiB | Reason present |
| --- | ---: | --- |
| `kac2f1x29yfx4y2fp4y8kngwp6pc21zj-linux-firmware-20261019-zstd` | 825.24 | Broad hardware firmware support |
| `11czk50zsr8l8r8kp4wj39y4s198ivf5-clang-21.1.8-lib` | 812.98 | Console → PySide environment → shiboken6 → shiboken6-generator → clang |
| `h5dv87p03wnxwgih9s1l9mykhibjvkfc-mbrola-voices-0-unstable-2020-03-30` | 644.65 | Installed speech-dispatcher → mbrola voices |
| `z54hii9gy0g68isj8zi1j5mg2cj9c85z-llvm-21.1.8-lib` | 540.26 | Shared dependencies including Mesa graphics; not all compiler-sized paths are build-only |
| `s04x6yc5bxl4gjqs3klhik80l46a99wz-qtwebengine-6.11.3` | 428.07 | Existing graphical console and hosted sign-in browser |
| `bxbww9by0n259mlp1pps8ds46i7qla41-firefox-unwrapped-160.0.1` | 391.29 | Live graphical installation base's Firefox package |
| `m21dvmfq8fqgsg5s3fvlwzfx3avhfnli-ibm-plex-1.1.0` | 286.11 | Full font family registered by console configuration |
| `pkb2bibhgwnnnjcngk6h95db9mjfy7k0-gcc-15.3.0` | 264.40 | Compiler chain reached through binding generator |
| `smskpmw6v5nxd25kqdhi5sjjmxgs8qaz-mesa-26.1.8` | 264.02 | Graphics drivers |
| `xcff261y078ig5m7zqyfyrk70s26rhc5-mbrola-voices-0-unstable-2020-03-30` | 258.66 | Separate live speech-dispatcher/mbrola voice output |
| `xnwz9z22pazgfn3s3y7pkysh130180dw-nixos-26.05.20261118.3457b69` | 197.45 | Live `/etc/nix/registry.json` references pinned Nixpkgs |
| `1d7gcx83cpb9mvsglqpz358cz20dpwav-source` | 197.45 | Appliance `/etc/nix/registry.json` references pinned Nixpkgs |
| `j7qrvgg3mprhw8nyi2z1dl5p62g92pja-qtdeclarative-6.11.3` | 177.24 | Qt interface dependencies |
| `0w4ig1wyvy9y62n37jp5mxg4y46cl9jl-linux-6.18.64-modules` | 143.94 | Kernel drivers |
| `ljp572c0l1f51rf00mbwzh1wfgxchi0a-papirus-icon-theme-20250501` | 139.41 | Desktop icon theme |
| `9qkpw9669q6w9nl6bmh2k35nq76i52nf-python3-3.13.18` | 128.01 | Current Qt/PySide presentation, installation/boot tooling and other runtime consumers |

All Qt 6.11 outputs together occupy **863.75 MiB** of their own NAR bytes, before shared graphics/media dependencies. The four native controller outputs together occupy **14,897,256 bytes / 14.21 MiB**: `anima-osd` 2,375,536; installer 1,548,048; updater 5,284,752; session 5,688,920. Their shared libc/libgcc dependencies already serve other software. Even deleting all four controllers would remove only 0.15% of the embedded NAR bytes. Rust improves control-boundary design and can improve runtime behavior, but these controllers were never the main ISO-size cost.

## Prioritized next packaging slice

1. **Separate the PySide runtime from its binding generator.** A metadata graph cut at `shiboken6-generator-6.11.0` leaves 14 paths unreachable: **1,267,253,288 NAR bytes / 1,208.55 MiB**. This is the strongest appearance-preserving target. It is a modeled upper bound, not an implemented or validated saving: inspect why shiboken references the generator, package the runtime correctly, then prove PySide imports, Qt lifecycle, hosted sign-in and installed graphical boot. Do not delete referenced libraries from a store output. LLVM remains reachable independently through graphics.
2. **Review live-only Firefox and pinned source payloads.** The live installer inherits Firefox from `installation-cd-graphical-base.nix`; the product now starts Calamares and uses QtWebEngine for its console. Cutting the Firefox wrapper in the graph makes **493,987,480 NAR bytes / 471.10 MiB** unreachable. Prove no recovery/documentation/browser handoff depends on it before changing that default. The two registry source trees add **414,090,368 bytes / 394.91 MiB**; unpinning/removing shipped source references needs an explicit decision about offline recovery and rebuild behavior. Neither candidate was removed here.
3. **Select fonts and voice data deliberately.** Both voice outputs total **947,196,552 bytes / 903.32 MiB**; the full Plex output is **300,005,704 bytes / 286.11 MiB**. Preserve accessible speech, required languages, glyph fallback, weights and the current visual appearance. Establish the supported set and test it before making a subset. These entire totals are ceilings, not promised savings.
4. **Retain hardware and renderer coverage until separately qualified.** Firmware, Mesa/LLVM, kernel drivers and QtWebEngine have direct product roles. A browser or renderer rewrite is a larger compatibility project and is unnecessary to investigate the first targets. No evidence here supports stripping firmware for unknown machines or promising a particular startup improvement.

The next bounded goal should be a validated runtime-only binding package plus a measured review of the live browser/source payloads. Report actual rebuilt ISO bytes and repeat graphical acceptance after each packaging change. Do not promise a sub-3-GB ISO from this metadata alone. The retired privileged Python update/session implementations are absent from their installed runtime packages; the remaining Python interpreter has current consumers, so its presence is not evidence that the Rust migration failed.

## Reproduction

Run from the repository root; all queries are read-only. The final ISO derivation's `structuredAttrs.squashfsCommand` identifies the closure manifest used below.

```sh
stat -c '%s %n' /tmp/anima-os-native-iso/iso/anima-os-preview-x86_64.iso
nix-store --query --deriver /tmp/anima-os-native-iso
nix --extra-experimental-features 'nix-command flakes' derivation show \
  /nix/store/98xrf7b9pvw51g0ps664q9g6nd64z0m0-anima-os-preview-x86_64.iso.drv

python3 - <<'PY'
import json, pathlib, subprocess
manifest = pathlib.Path('/nix/store/r2c2shlwpjdjp5px08y3fd3bcrylgzgy-closure-info/store-paths')
data = json.loads(subprocess.check_output([
    'nix', '--extra-experimental-features', 'nix-command flakes',
    'path-info', '--json', '--json-format', '1', *manifest.read_text().splitlines(),
]))
def reachable(root, excluded=frozenset()):
    seen, pending = set(), [root]
    while pending:
        path = pending.pop()
        if path in seen or path in excluded:
            continue
        seen.add(path)
        pending.extend(data[path]['references'])
    return seen
for root in (p for p in data if 'nixos-system-' in p):
    paths = reachable(root)
    print(root, len(paths), sum(data[p]['narSize'] for p in paths))
    for term in ['shiboken6-generator-', 'firefox-160.0.1', 'mbrola-voices-']:
        cut = {p for p in data if term in p}
        lost = paths - reachable(root, cut)
        print('modeled cut', term, len(lost), sum(data[p]['narSize'] for p in lost))
for path, info in sorted(data.items(), key=lambda item: item[1]['narSize'], reverse=True)[:30]:
    print(info['narSize'], path)
PY
```

For a dependency chain, `nix why-depends APPLIANCE_OR_LIVE_STORE_PATH TARGET_STORE_PATH` reproduces the reference graph explanation. Graph cuts describe reachability only; they are not runtime compatibility tests or compressed-size predictions.
