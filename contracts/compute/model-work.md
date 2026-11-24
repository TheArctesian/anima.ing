# K11 — Model work: the work types of the model service, the copies and the runtime report

Status: **accepted** (R-344, operator, 2026-11-22: "all of it review after"; version 0.3 as written). The security review
SEC-C-012 still runs, after the acceptance and beside the build; SF-298 and SF-300 stay Open until code closes them or the
operator accepts them ([security-findings.md](../../docs/security-findings.md)). Version 0.7, 2026-11-23 (task L1-15, R-NEW-modelloading: Section 11.1, the loading, the memory and the pin,
after a design read and a security read by separate reviewers; the type `model_keep`; the members `reserveBytes`
and `keep` of `model_load`; the cause `memory_busy`; the members `models` and `memory` of the runtime report;
Sections 0, 3, 5, 10.1 and 13). Version 0.6,
2026-11-23 (version 0.6 takes R-443 and R-444, OD-C28 (b), PULL-012, task L1-22: the
models that Ollama, LM Studio and the Hugging Face cache keep on the machine are found and used where they are, with
no copy; the type `copy_adopt`; the check of each such file at each load; the causes `found_unknown`,
`copy_changed` and `copy_missing`; Sections 1, 2, 2.3, 3, 5, 8, 10.1, 10.2, 11 and 13). Version 0.5,
2026-11-23 (version 0.5 takes R-347, R-NEW-launcherkey and R-NEW-launcherlist: the root-owned model launcher of task
L1-6c, the cause `runtime_not_signed`, the details `launcher_missing` and `launcher_outdated`, Sections 5 and 9;
the launcher waits for its own security review before a merge). Version 0.4,
2026-11-23 (version 0.4 takes R-387: a relative symbolic link to a regular file of the same archive
is unpacked as a copy of that file, Section 6). Version 0.3,
2026-11-20 (version 0.3 takes the answers of the operator to OD-C22 to OD-C25: R-346,
R-339, R-342, R-326; each stricter form is a setting at the machine,
Section 10.1. Version 0.1, 2026-11-20, was the text of the first security read; version 0.2 has the corrections of
its 14 plain findings, and the safe default of its 4 findings that wait for the operator, marked "until the
operator decides"; [security-findings.md](../../docs/security-findings.md), the section of the model work).
R-344 gives this contract the number K11. The security review SEC-C-012 (a separate reviewer, 2026-11-22) tightened the text inside what R-344 permits (the operator's relayed direction: no new ruling); its findings are SF-357 to SF-365. R-379 (operator, 2026-11-22) sets no network block on a model server by default (Sections 4 and 9; SF-300 accepted). Task L1-3 of [compute-plan.md](../../docs/plans/compute-plan.md) Section 12.4. Code may be built from this text
(R-344; CON-001, AGT-004). Change it only through a programme ruling.

Requirements: CAG-002 to CAG-006, CAG-008, CAG-009, PULL-004 to PULL-011, RTC-002 to RTC-006, SEC-C-001,
SEC-C-003, SEC-C-007 to SEC-C-010 of the compute plan; USE-004 to USE-009 of [jobs.md](../../docs/proposals/jobs.md);
MET-002, MET-014, MET-020, LOG-007 of the design specification. Decisions: OD-C9 (R-165: the model work types
join the fixed list of the work route), OD-C8 (R-165: one switch), R-160 (the work route).

This file holds no secret, no token, no key and no example value of a secret.

## 0. What this contract uses, and what it changes

- **The work route exists.** Contract K10, [work-route.md](../work-route.md), is the contract of task W1 of
  [projects-and-builds-plan.md](../../docs/plans/projects-and-builds-plan.md). It is accepted (R-209, version 0.3;
  version 0.5 is in force by R-251). Its database rows are K2 range 0980–0999 (task W3). The socket of task W0
  (R-172) is in K8 Section 1. Thus this contract does not make a narrow route of its own, as OD-C9 option (a) and
  the pattern of `agent_install` (R-153) would need without K10. It adds the model work types to the fixed list of
  K10, with a new kind `model`. K10 Section 13, point W1-P5, says: "Each is one more type of the fixed list, added
  with a ruling." R-344 is that ruling.
- **Changes to K10 that R-344 permits.** The text of this contract is the change. The changes are: the list of work types of K10 Section 1 and Section 6 gets the twelve
  types of Section 2 below (`copy_adopt` by R-444); the kind `model` (Section 1 below); the purpose `hf_token` of K10 Section 8
  (Section 7 below); the causes of refusal of Section 5 below; the order of the claim (Section 3 below).
- **Changes to K8 that R-344 permits.** The optional member `runtimes` of the `Batch` of K8 Section 4
  (Section 8 below). It is an optional member, so it needs no `/daemon/v2/` (K8 Section 1).
- **A change of the design specification that R-344 permits.** MET-020 says that the daemon connects
  only to the hub and sends no request to a TCP port of its own computer. A pull (PULL-004), a runtime install
  (CAG-008) and a measure or a validation (PULL-011) need more. Section 4 gives the rule; MET-020 has the
  exception in its text (R-344).
- **What this contract does not hold.** The model request and its address (task L1-10, `model-request.md`); the
  wait policy, the pin and the readiness of a request (task L1-15); the device policy itself (task L1-5); the
  library and the choice of a variant (tasks L1-4 and L1-7); the model key (task L1-9).

## 1. Terms

| Term | Meaning |
| --- | --- |
| Model item | An item of K10 whose kind is `model`. |
| Kind `model` | The daemon does the steps itself, with its own user. No job code runs. Only the model work types have this kind. |
| Runtime | A model server program of a fixed list in the daemon: `llama_server`, `mlx_lm`, `vllm`. The list grows only with a new version of this contract. |
| Server | One running process of a runtime, with its own loopback port. A node can have more than one (RTC-002). |
| Variant | A set of model files in one format and one quantisation (LIBM-002). |
| Copy | The files of one variant on one node, in the model cache; or a found copy. |
| Found model | A model file, or the set of files of one model, that a different program keeps on the node: Ollama, LM Studio, the Hugging Face cache, or a folder that the owner of the machine names (Section 10.2). Anima did not write it and does not own it. |
| Found copy | A copy whose files are those of a found model, used where they are, with no second file (`copy_adopt`, Section 2.3; OD-C28 (b)). |
| Owned flag | A flag of a runtime that only the daemon sets (SEC-C-007). |

## 2. The work types (CAG-003)

The list is fixed in the daemon program. The hub cannot add a type. Each model item has the kind `model`, no
member `repository`, `project` null, and `secrets` empty, except `model_pull` (Section 7), and the member `actor` of Section 3. The daemon refuses an
item that breaks this (`member_unknown`).

Each `parameters` object has exactly the members that its row names. The daemon refuses each other member
(`member_unknown`), as K10 Section 5 says. Identifiers are UUIDs that the hub makes. A name of a runtime, a
format or a backend is a word of a fixed list in the daemon. An `Item` never holds a path, an address, a port,
a command, an argument, a flag or an environment variable.

| Type | `parameters` | What the daemon does |
| --- | --- | --- |
| `model_report` | `{}` | Sends the full model state of the node as the result: runtimes and versions, copies and their states, servers (Section 8). Changes nothing. |
| `runtime_install` | `{ "runtime": "llama_server", "version": "b8100", "backend": "metal" }` | Installs that runtime from its release source in the daemon configuration (Section 6). |
| `model_pull` | `{ "copy": "<uuid>", "variant": "<uuid>", "source": { "organisation": "unsloth", "repository": "Qwen3-8B-GGUF", "revision": "<40 hexadecimal characters>" }, "format": "gguf", "files": [ { "name": "Qwen3-8B-Q4_K_M.gguf", "sizeBytes": 5027783488, "sha256": "<64 hexadecimal characters>" } ], "gated": false }` | Downloads the files to the model cache and verifies them (Section 7). |
| `copy_delete` | `{ "copy": "<uuid>" }` | Unloads the copy if a server has it, then deletes its folder in the model cache (PULL-009). For a found copy it deletes only the record of Section 2.3; it never writes, moves or deletes a file of the found model. |
| `copy_adopt` | `{ "copy": "<uuid>", "found": "<64 hexadecimal characters>" }` | Makes the found model with that key a found copy: reads each of its files, records the size and the SHA-256 of each, and uses the files where they are (Section 2.3; PULL-012). |
| `server_start` | `{ "server": "<uuid>", "runtime": "llama_server", "version": "b8100", "backend": "metal", "settings": { … } }` | Starts one server of an installed runtime on loopback and checks the listener (Section 9). |
| `server_stop` | `{ "server": "<uuid>" }` | Stops the server: it asks it to end, waits at most 10 seconds, then ends the process. |
| `model_load` | `{ "server": "<uuid>", "copy": "<uuid>", "settings": { … }, "reserveBytes": 7516192768, "keep": { "idleSeconds": 900, "pinned": false } }` | Makes room by the rules of Section 11.1, then loads a copy on a server. `reserveBytes` and `keep` are optional (Section 11.1). For a runtime that cannot unload (vLLM), the daemon stops and starts the process with the new copy (RTC-004). llama-server runs in router mode and loads with no restart (RTC-005). |
| `model_unload` | `{ "server": "<uuid>", "copy": "<uuid>" }` | Unloads the copy from the server, or stops the process when the runtime cannot unload (RTC-004). |
| `model_measure` | `{ "server": "<uuid>", "copy": "<uuid>", "probe": "text_short" }` | Sends a fixed request of the daemon to the server and reports the result (PULL-011, CMP-006). |
| `model_keep` | `{ "copy": "<uuid>", "keep": { "idleSeconds": 900, "pinned": true } }` | Sets the idle time and the pin of a copy on this node (Section 11.1). It loads and unloads nothing by itself; the rules of Section 11.1 then apply to the copy. |
| `model_drain` | `{ "seconds": 60 }` | Sends no new request to each server, waits for the requests in progress for at most `seconds` (at most 60, CAP-013), then reports. It does not stop a server. |

- `version` is the version of a release of the runtime, by the pattern of that runtime in the daemon (for
  `llama_server`, `b` and digits). `backend` is `metal`, `cuda`, `rocm`, `vulkan` or `cpu`.
- `source.organisation` and `source.repository` have only the characters that Hugging Face permits for a name:
  letters, digits, `-`, `_` and `.`, at most 96 characters, not `.` or `..`. `revision` is a commit of the
  repository, never a branch name: a branch can change after the hub read its file list. It is exactly 40
  lower-case hexadecimal characters (`0-9`, `a-f`); each other value is refused (`member_unknown`)
  (SEC-C-012, SF-359).
- `files[].name` is a flat file name at the root of the repository, or one sub-folder and a flat file name
  joined by one `/` (R-408: the GGUF repositories of Unsloth and others keep one folder for each
  quantisation). The name has exactly 0 or 1 `/`. Each part has 1 or more characters, is neither `.` nor `..`,
  and is a flat name: only the characters above, no `/`, no `..`, no `%`, no control character, no character
  outside ASCII (SF-362), and no name that Windows reserves (Section 2.2); the file part is permitted by the
  list of Section 2.2 for the format; at most 256 characters in all (both parts and the `/`), at most 64 files
  (SF-380). The daemon flattens each file on write into
  `copies/<copy>/` under its file part. Two names whose flattened names are equal when they are in lower case are
  refused, never overwritten (as in the unpack of Section 6) (SEC-C-012, SF-359). `sizeBytes` is above 0. The daemon makes each
  address from a fixed pattern (Section 7).
- `found` is the key of a found model, as the daemon gave it in `found` of `model_report` (Section 8): exactly
  64 lower-case hexadecimal characters; each other value is refused (`member_unknown`). It is not a path, and the
  daemon never reads a path from it: it finds the model again with its own walk of the folders of Section 10.2.
- `probe` is a word of a fixed list in the daemon. Today: `text_short` (one fixed prompt, a fixed answer check)
  and `vision_short` (one fixed image in the daemon program). The hub sends no prompt.

### 2.1 `settings`: what the hub may set, and what the daemon owns (SEC-C-007, CAG-004)

`settings` is an object of typed members. It is never a list of flags. The daemon makes the command line of
the runtime from its own code, these members, and the paths that it makes itself.

| Member | Type and limit | Runtimes |
| --- | --- | --- |
| `contextTokens` | integer, 512 to 1,048,576 | all |
| `gpuLayers` | integer, 0 to 999, or the string `all` and no other string | `llama_server` |
| `memoryShare` | number, 0.05 to the memory share of the device policy (RTC-003) | `vllm` |
| `gpu` | integer, an index of a GPU in the inventory that the daemon reports (one vLLM process for each GPU, RTC-003); another index is refused (`gpu_unknown`) | `vllm` |
| `batchTokens` | integer, 16 to 65,536 | all |
| `kvCacheType` | `f16`, `q8_0` or `q4_0` | `llama_server` |

The daemon owns, and refuses from the hub and from each other source: the model path, the host, the port, the API
key, the parallel slots and remote code (SEC-C-007). The daemon also owns the log path, the chat template file,
each address, each file path and each environment variable of the process. A member of `settings` that is not in
the table, or that the table does not permit for the runtime, is refused (`flag_owned` when it names one of the
owned values, as `host`, `port`, `apiKey`, `parallel`, `modelPath`, `trustRemoteCode`; else `member_unknown`).

The values that the daemon sets:

- Host: `127.0.0.1` only (RTC-006, SEC-C-001). Never `0.0.0.0`, `::` or a name.
- Port: a free port that the daemon selects from the range in its configuration, default 41000 to 41999.
- API key: a random key of 32 bytes that the daemon makes for each start, gives to the process in its
  environment (never on its command line), and holds in memory only. It is not the model key of L1-9. The key protects a server only against other users of
  the machine and against browser pages; a process of the same user can read the environment of the runtime, so
  the key is no limit against such a process (SEC-C-012, SF-365).
- Parallel slots: from the memory share of the device policy and the fit estimate of the copy, by a pure function
  in `daemon-core`.
- Remote code: off. For vLLM the daemon passes no `--trust-remote-code` and sets the refusal of remote code in
  each form that the runtime has. A copy whose format needs remote code cannot be loaded (`remote_code`)
  (SEC-C-008). For vLLM the daemon also sets `--load-format safetensors` and `--tokenizer` to the folder of the
  copy, and sets no `--download-dir` and no `--allowed-local-media-path`. A media input is accepted only as a
  `data:` URL; the daemon sets the runtime so that it fetches no address (Section 4).
- Logs (SEC-C-010): llama-server with `--log-disable` and the lowest verbosity, `--no-slots`, and no slot save
  path; vLLM with `--disable-log-requests`. The standard output and the standard error of each runtime go to a
  file of mode 0600 in the state folder, a ring of at most 1 MiB, that the daemon never sends to the hub and
  never copies into a log line. A runtime that has no such flag in its version is refused
  (`runtime_not_configured`).
- Environment (SEC-C-001, SEC-C-007): the daemon starts each runtime with an empty environment and only these
  variables: `HOME` and `TMPDIR` set to the temporary folder of the process (Section 10), `PATH` set to the `bin`
  folder of the runtime only, `HF_HUB_OFFLINE=1`, `TRANSFORMERS_OFFLINE=1`, `VLLM_NO_USAGE_STATS=1`,
  `DO_NOT_TRACK=1`, and the variable that holds the API key. No proxy variable, no `HF_TOKEN`, no `DYLD_*`, no
  `LD_PRELOAD`, no `PYTHONPATH` reaches a runtime.

### 2.2 The files that a copy may have (SEC-C-008, SEC-C-009)

A model file can hold code: a pickle file runs code when it is read, a `.py` file of a repository runs through
`auto_map`, and a chat template is a program of Jinja. Thus each format has a fixed list of file names in the
daemon, and the daemon refuses each other file of `files` (`file_type`) before it downloads a byte.

| Format | Permitted files |
| --- | --- |
| `gguf` | `*.gguf` |
| `safetensors`, `mlx` | `*.safetensors`, `*.safetensors.index.json`, `config.json`, `generation_config.json`, `tokenizer.json`, `tokenizer_config.json`, `special_tokens_map.json`, `*.model` (a SentencePiece file), `chat_template.jinja`, `preprocessor_config.json` |

- Each `.bin`, `.pt`, `.pth`, `.pkl`, `.py`, `.npz`, `.ckpt` and `.h5` file is refused (`file_type`). A name in more than
  one sub-folder is refused (Section 2, R-408).
- A name that Windows reserves is refused in either part (`file_type`): `CON`, `PRN`, `AUX`, `NUL`, `COM1` to
  `COM9` and `LPT1` to `LPT9`, in any case, with or without an extension (`nul.gguf`, `Com1.json`); a part that
  ends in a dot or a space; a part with `:` (SF-382).
- Before a load, the daemon reads `config.json` and `tokenizer_config.json` of the copy with a JSON parser. It
  refuses the copy (`remote_code`) when one of them has the member `auto_map` or `custom_pipelines`, or names a
  class that is not in the list of the runtime. A file refused for another reason, such as a chat template over
  64 KiB, a JSON file that does not parse or is not an object, or a file over its size cap, is `file_rejected`
  (R-407).
- A chat template comes only from `tokenizer_config.json`, `chat_template.jinja` or the GGUF metadata of the
  copy, and is at most 64 KiB. The daemon gives no template of the hub and no template file from outside the
  copy. The runtime renders it in its own sandboxed Jinja form; a runtime version that renders a template with
  no such sandbox is not in the list of release sources. The runtime and its parsers still read files that the
  hub chose, so the isolation of Section 9 is the limit for a fault of a parser.

### 2.3 A file that Anima does not own: the found copy (PULL-012, R-444)

The operator decided that a model that is already on the machine is used where it is, as Unsloth Studio does
(OD-C28 (b)). The checks of Section 7 hold for a file that the daemon wrote; they do not hold for such a file,
because a different program, and each user who can write its folder, can change or delete it. Thus:

- **Formats.** A found model is GGUF only in this version: one `.gguf` file, or the parts of one split GGUF
  (`<name>-00001-of-0000N.gguf` to `-0000N-of-0000N`), and at most one projector file: the layer of Ollama
  (`application/vnd.ollama.image.projector`, also GGUF), or the file `mmproj*.gguf` of the same folder when that
  folder has exactly one. A projector file is never a found model alone; the one projector of a folder is the last
  file of each model of that folder. A found model of safetensors is listed with
  `usable` false and cannot be adopted (open point MW-P10). The file names follow Section 2.2 (`*.gguf`) and
  the length and character rules of Section 2, applied to the name of each file as the daemon reads it.
- **The key.** The daemon gives each found model a key: the SHA-256, in lower-case hexadecimal, of the tool word,
  a zero byte, and the location of the model in that folder (the host, namespace, model and tag of the Ollama
  manifest; the repository folder, the commit and the first file of the Hugging Face cache, since one commit can
  hold several quantisations; or the path of the first file below the folder). The key is stable while
  the model stays where it is. The hub only stores it and sends it back.
- **`copy_adopt`.** In `preparing` the daemon walks the folders of Section 10.2 again and finds the model with
  the key (else `found_unknown`). It checks the layout of each file (below), reads each file from byte 0, and
  computes its size and SHA-256. Where the program names a file by its SHA-256 (a blob of Ollama,
  `sha256-<hex>`, and a blob of the Hugging Face cache whose name has 64 hexadecimal characters), a different
  value is refused (`checksum`). It writes the record of the copy to `<model-state>/found/<copy>.json` (mode
  0600, with `openat` and `O_EXCL`; Section 10): the tool word, the key, the absolute folder of Section 10.2 (a
  load refuses it, `copy_missing`, when the walk no longer reads that folder), and for each file its location, its size,
  its SHA-256, its device and inode numbers and its modification time. The result is that of a pull
  (`copyState` `downloaded`, `bytes`, `files`), with `origin` `found`. The copy is `downloaded`, not
  `validated`: only `model_measure` validates it (PULL-011).
- **The check at each load.** Before each `model_load` and each `model_measure` of a found copy, the daemon
  opens each file of the record as the layout rules below say, and requires a regular file whose size and
  SHA-256, read again from byte 0, are those of the record. A file that is not there, or that is not a regular
  file, is `copy_missing`; a different size or SHA-256 is `copy_changed`. Each ends the item `failed` and sets
  the copy `failed` with that cause (Section 11). The daemon then gives the runtime the open descriptor that it
  checked (on Linux `/proc/self/fd/<n>` of the launcher, on macOS `/dev/fd/<n>`), never the path, so a rename or
  a replacement of the file after the check does not change what the runtime reads. A write into the same file
  after the check can still reach the runtime: that file belongs to a user of the machine, as the files of
  `partial/` do (Section 7, R-346), and the operator accepted it (R-444; Section 13).
- **The layout rules.** The daemon opens each folder and file of Section 10.2 relative to an open handle, with
  `O_NOFOLLOW`, `O_NONBLOCK` and no write access. It follows a symbolic link only in the Hugging Face cache, and
  only a link of `snapshots/<commit>/` whose text is `../../blobs/<name>` with a flat name, read with
  `readlinkat` and opened relative to the `blobs` folder of the same repository folder. When that blob is itself a
  link, it follows it only when its text is `../../blobs/<xx>/<name>`, with a name of 64 lower-case hexadecimal
  characters that starts with `<xx>`, and opens it relative to the folder of Section 10.2: the shared store
  `blobs/<xx>/` of the cache, where the downloader of Unsloth Studio keeps each large file once (R-443). It follows
  no other link. Each file and folder it uses must belong to the user of the daemon, to root, or to the owner of the
  folder of Section 10.2 it is in, and no other user may write it. Its group may write it only when that group is
  the private group of the user of the daemon, by the rule that `anima-daemon install` uses: the primary group of
  that user in `/etc/passwd`, with the name of the user, no other user and no other member in `/etc/group`, and each
  line of both files readable. A home folder
  made with the umask 002 of Ubuntu and Pop!_OS is such a case (R-NEW-privategroup). Else it is not listed. A walk reads at most
  6 levels, 4096 entries in one folder and 256 found models.
- **Never written.** The daemon never writes, renames, deletes, locks or changes the mode of a file or folder of
  Section 10.2, also not at `copy_delete`, at a `failed` state or at the end of a found model. It runs no file of
  such a folder: it gives a GGUF file to a runtime as data, as for a pulled copy.

## 3. How the daemon asks and reports (CAG-002, CAG-003)

- The transport, the token and the routes are those of K10 Section 2: `POST /work/v1/claim`, `/work/v1/report`
  and `/work/v1/secret`, with the daemon token of K8 Section 2. The daemon asks; the hub never opens a
  connection to a machine. The daemon listens on no port for work (MET-020, USE-005). The model servers listen on
  loopback for model requests; that is the model address of task L1-10, not a route for work.
- A daemon token alone permits no model work (CAG-002). The hub gives a model item only to a device that takes
  work (CMP-002, USE-004) and whose device policy permits it now (CAP-010 to CAP-012). The machine checks the
  switch and the policy again before each item (K10 Section 3: `switch_off`, `policy`). `server_stop`,
  `model_unload`, `model_drain`, `copy_delete` and `model_report` are never refused for `policy` or `switch_off`:
  they only reduce the work of the machine. A `model_keep` with `pinned` false is not refused for them either; a
  `model_keep` with `pinned` true is, because a pin can load a copy later by itself (Section 11.1).
- **Who may ask (R-326).** The hub makes each of the twelve types for an API key or an AI agent,
  with the MCP state of its tool, as R-153 does for `agent_install`. Each model item has the member `actor`:
  `User`, `Agent` or `Hub` (LOG-012). The setting `agentTypes` at the machine (Section 10.1) lists the types
  that the machine takes from an item whose `actor` is `Agent`; by default it has the twelve types. The daemon
  refuses another (`actor_not_permitted`). The hub writes `actor`, so the setting limits an honest hub only.
- The `Claim` gets `model` in `kinds` and the twelve types in `types` when the daemon has the model function. The
  model function is on when the switch is on; there is no second switch (OD-C8).
- Model items do not use the job slot of K10 (`free.slots` counts jobs), and they take no room of `free`: the daemon sends 0
  for `free.slots`, `free.memoryBytes` and `free.diskBytes` when it offers only model work, and the placement does not
  compare `free.memoryBytes` and `free.diskBytes` with the `limits` of a model item. The hub counts its own leases: one
  machine has at most one model item that changes the machine (`runtime_install`, `model_pull`, `copy_adopt`,
  `copy_delete`, `server_start`, `server_stop`, `model_load`, `model_unload`, `model_keep`) and one `model_measure` at a time. A `model_report`
  and a `model_drain` are not counted. One claim gets at most one item. The order: `model_drain`, `server_stop`,
  `model_unload`, then each other model item the oldest first, then a job.
- A model item has the states, the reports, the lease, the stop and the refusals of K10 Section 7 and Section 11.
  `preparing` is the check of the item and of the disk; `running` is the step; `finishing` is the clean-up of
  partial files. A `model_pull` holds its lease by its reports, each 15 seconds.
- `result` of a model item has the members of K10 Section 7 and one more optional member, `model`. Its members
  are fixed codes, numbers, identifiers and SHA-256 values, never text from a runtime or a file:

```json
{ "outcome": "succeeded", "cause": null, "seconds": 412,
  "model": { "copy": "<uuid>", "copyState": "downloaded", "bytes": 5027783488, "files": 1 } }
```

- The result of `copy_adopt` also has `"origin": "found"` in `model` and, for each file, its size and its SHA-256
  in the order of the record (`"fileHashes": [ { "sizeBytes": 5027783488, "sha256": "<64 hexadecimal characters>" } ]`),
  so that the hub knows the copy as the daemon checked it. It holds no file name and no path.

- The log of a model item holds only the lines of the daemon: the step, the sizes, the codes. It never holds the
  output of a runtime, a prompt, an answer or an image (SEC-C-010, LOG-007), and never a token (Section 7).
- Progress of a pull (PULL-007): each report of a running `model_pull` has `progress`:
  `{ "bytes": 1073741824, "totalBytes": 5027783488, "bytesPerSecond": 52428800 }`. The hub computes the time to
  completion. `progress` is an optional member of the `Report` of K10 for the kind `model` only.

## 4. The connections that model work makes (MET-020)

| Connection | Who makes it | Rule |
| --- | --- | --- |
| The hub | The daemon | K8 and K10 only. |
| Hugging Face | The fetch process (below) | HTTPS only, to `huggingface.co` and the file hosts that the daemon holds in a fixed list. It follows a redirect only to a host of that list. |
| Release source of a runtime | The fetch process | HTTPS only, to the address of the daemon configuration (Section 6). Redirects only to a host of the fixed list for that runtime. |
| A model server | The probe process (below) | `http://127.0.0.1:<port>` of a server that the daemon started, for `model_measure` and the check of a start only. Before each request it confirms that the listener of that port belongs to the process tree of that server (Section 9). |
| Each other address | A runtime | No block by default (R-379; SF-300 accepted). The daemon does not stop an outgoing connection of a runtime. What it does: the server listens on loopback only (Section 2.1; a LAN switch of the operator is not in this version); the environment of Section 2.1 is cleaned, holds no token, and sets the offline and no-telemetry variables, which a runtime can ignore; only GGUF and safetensors files load, with the `remote_code` refusal (Section 2.2); media only as a `data:` URL. The isolation of Section 9 (`sandbox`, off by default) refuses the network outside loopback where the system can do it. |

Rules of the fetch process (both sources):

- TLS with the root certificates of the system or of the daemon program only. No root certificate comes from
  the hub, from the configuration of a runtime source, or from an environment variable.
- No proxy. No `http`. No redirect to `http`, to an address written as an IP address, or to a host outside the
  list. At most 5 redirects.
- The hosts of Hugging Face are a fixed list in the daemon program: `huggingface.co`, `cdn-lfs.hf.co`,
  `cdn-lfs-us-1.hf.co`, `cas-bridge.xethub.hf.co`, `us.aws.cdn.hf.co`. A change of that set comes with a release of
  the daemon. (`us.aws.cdn.hf.co`: on 2026-11-23 a `/resolve/` request of each repository that was tried from `pop-os`
  went there, to `xet-bridge-us/`; the hosts of the other regions are not known yet.)
- It reads at most `sizeBytes` of a file, and at most the `sizeBytes` of a release source entry (Section 6).

The fetch process and the probe process are child processes of the daemon, made from the same program file with a
fixed mode. They get no daemon token, no hub origin and no proxy variable. They take their instruction on their
standard input as JSON from the daemon, and give back fixed codes and numbers. The part of the daemon that speaks
to the hub makes no other connection. This follows K10 point W1-P1. MET-020 has this exception in its text
(MW-P1, R-344).

The limit of this separation (SEC-C-012, SF-361): the fetch process and the probe process run as the user
of the daemon, with each file right of that user. "No daemon token" means that the daemon does not give them the
token; a fault in such a process can still read the daemon folder, the configuration and the token from disk.
The separation keeps the token out of their memory and their environment, not out of their reach. (The earlier
exception of R-336 for task L1-6 is removed: the child fetch process is built, task L1-6b; SF-364.)

## 5. What the daemon checks, and the causes of a refusal

The daemon checks each model item before `preparing`, with a pure function in `daemon-core` (CAG-007). It uses
the causes of K10 Section 5, and these:

| Cause | When |
| --- | --- |
| `type_unknown` | The type is not in the list of Section 2 (CAG-004) |
| `flag_owned` | `settings` names a value that the daemon owns (SEC-C-007) |
| `runtime_not_configured` | The daemon configuration has no release source for that runtime, version, operating system, architecture and backend (Section 6) |
| `runtime_not_installed` | `server_start` names a runtime and version that the node does not have |
| `runtime_advisory` | The version is in the list of versions with a published advisory that the daemon holds (SEC-C-005) |
| `source_not_permitted` | The organisation is not in the approved list of the machine (Section 7) |
| `copy_unknown` | The copy, the server or the variant is not on this node |
| `copy_not_ready` | `model_load` or `model_measure` names a copy that is not `downloaded` or `validated` |
| `remote_code` | The copy needs remote code to load: `auto_map`, `custom_pipelines`, or a class that is not on the list of the runtime (SEC-C-008) |
| `file_rejected` | A file of the copy is refused for a reason that is not remote code: a chat template over 64 KiB, a JSON file that does not parse or is not an object, a file that a content check finds too large (Section 2.2; R-407; a downloaded size that differs from `sizeBytes` is `size`, SF-383). Not retryable with the same copy; the operator and the actor see it |
| `disk_share` | The copy does not fit in the disk share of the device policy after the removal that the hub permitted (PULL-008) |
| `memory_share` | The load does not fit in the memory share of the device policy, also with no other copy loaded: the copy is too large for this runtime and this hardware (FIT-026, the second case) |
| `memory_busy` | The load fits in the memory share, but the reservations of the copies that cannot unload now (`loading`, `active` or `unloading`, or `ready` and idle for less than 30 seconds and not loaded by a return and unused since) fill it at this time (FIT-026, the first case). The daemon unloaded nothing for this load. Retryable |
| `file_type` | `files` names a file that the format does not permit (Section 2.2) |
| `gpu_unknown` | `gpu` is not a GPU of the inventory of the machine |
| `runtime_too_old` | The version is lower than the lowest version of that runtime that the daemon program holds |
| `actor_not_permitted` | `actor` is `Agent` and the setting `agentTypes` does not name the type |
| `revision_too_new` | The commit of the source is newer than the lowest age that the machine holds (Section 7) |
| `found_unknown` | `copy_adopt` names a key that the walk of Section 10.2 does not find now, or a found model that is not `usable` (Section 2.3) |

Causes after `preparing`, reported with the state `failed`: `checksum` (a SHA-256 is different), `size` (the downloaded size of a file
differs from the `sizeBytes` that the order gave; a file over the cap of a content check of Section 2.2 is
`file_rejected` instead; SF-383), `source_failed` (the source refused or did not answer after the retries), `fetch_failed` (the child fetch
process of Section 4 itself failed: a stall past its deadline, an end by a signal, an exit with a code that is not 0,
or a reply that is not one of its fixed words; it carries no detail of the child; retryable; R-406), `listener`
(Section 9), `isolation_failed` (the daemon cannot make the isolation of Section 9), `limit_memory` (Section 9),
`load_crash` (the server ended while it loaded a copy), `hash_mismatch` (Section 7), `start_failed` (the server did not become ready in 120 seconds), `probe_failed` (the fixed request got
no correct answer), `limit_disk`, `program_mismatch` (the SHA-256 of a runtime file), `archive_rejected` (a
`runtime_install` whose archive breaks a rule of the unpack of Section 6, or has no server program at the path of its
release source entry; R-328), `containment_failed` (Linux: the tree of the server cannot be contained,
Section 9; R-331), `runtime_not_signed` (Linux, only where the model launcher of Section 9 is used: the SHA-256 of the
runtime program is on no valid signed list of runtimes; R-347), `copy_changed` (a file of a found copy has a
different size or SHA-256 than its record, Section 2.3), `copy_missing` (a file of a found copy is not there or is
not a regular file, Section 2.3). A `containment_failed` result also has the member `detail`, one of `no_delegation`,
`no_nsdelegate`, `cgroup_join_failed`, `backstop_failed`, `parent_gone`, `userns_denied`, `exec_failed`,
`launcher_missing` (the model launcher is used and it, or its profile, is not installed), `launcher_outdated` (the
model launcher gives a different protocol number). The launcher
gives each of its steps its own exit code (120 to 124; 125 for `runtime_not_signed`) and writes that code to a failure file in the folder of the
server before it exits; the daemon takes an exit code as the launcher's only when that file holds it.

## 6. The install of a runtime (CAG-008)

- The daemon configuration holds the release sources: one entry for each runtime, version, operating system,
  architecture and backend, with the HTTPS address of the archive and its SHA-256. The operator of the machine
  writes it at the machine. The hub cannot write it. A daemon release has a default file of entries that the
  release signs (K8 Section 8); a value of the operator replaces a default entry.
- Each entry also has `sizeBytes`, the largest size of the archive. The fetch process reads no more.
- An entry with no SHA-256 is refused (`runtime_not_configured`), in the default file and in the value of the
  operator. The pinned SHA-256 is the control of the archive; the list of hosts of a release source (for
  GitHub, a broad list) is not (SEC-C-012, SF-363).
- The archive is a `.tar.gz` (or `.tgz`) or a `.zip`, by the end of the path of its address; another form is
  `runtime_not_configured` (R-329). Each rule of the unpack below applies to both forms in the same way;
  a `.zip` entry that is encrypted is refused, a `.zip` entry is a symbolic link when its Unix type bits say so
  (its bytes are the target, at most 4096 bytes), and the declared size of an entry counts in the limit before a
  byte is read. A break of a rule fails the item with `archive_rejected`.
- The daemon downloads the archive with the fetch process and verifies the SHA-256 before it opens the archive.
  It unpacks it into a new temporary folder, then renames that folder to `runtimes/<runtime>/<version>-<backend>/`
  of its runtime folder (Section 10). It accepts only regular files, folders and the symbolic links of the next
  item: it refuses an entry that is a hard link, a device, a FIFO, an absolute path or a path with `..`, an entry
  whose name has a character outside ASCII (two names can be equal after Unicode normalisation on APFS; SEC-C-012,
  SF-362), and two entries whose names are equal when they are in lower case. It sets the mode of each folder and of each program file to 0755 and of
  each other file to 0644; it clears each setuid, setgid and sticky bit. It stops at 8 GiB of unpacked bytes or at
  50,000 entries (`limit_disk`).
- A symbolic link is accepted only when its target is relative and names, from the folder of the link, a regular
  file of the same archive, directly or through other such links (at most 8 steps). `..` in a target is accepted
  only while the path stays in the archive. The daemon writes a copy of that file in the place of the link, with
  the mode of that file, after the last entry; the folder of the runtime holds no link. The bytes of each copy
  count in the limit of 8 GiB. A link that is absolute, that leaves the archive, that names a folder or nothing,
  or that is in a loop is refused (R-387). Reason: each upstream `llama.cpp` release for Linux holds
  such links (`libllama.so -> libllama.so.1`).
- The daemon program holds the lowest version of each runtime that it accepts, and a list of versions with a
  published advisory (SEC-C-005). It refuses a lower version (`runtime_too_old`) and a listed version
  (`runtime_advisory`). Only a signed release of the daemon (K8 Section 8) changes the two lists; the
  configuration of the machine and the hub cannot lower them.
- No shell runs at any step. The daemon starts a runtime by the absolute path of the program file in that folder,
  never by a name that the system looks up in `PATH`. It runs no install script of the archive, and no package
  manager (`pip`, `brew`, `apt`).
- A runtime that needs Python (`mlx_lm`, `vllm`) is installed only from an archive that holds its environment.
  How such an archive is made is open point MW-P4.
- The daemon reports each installed runtime and version in the runtime report (Section 8; CAP-003).

## 7. The pull (PULL-004 to PULL-010, SEC-C-009)

- **The source.** The daemon downloads each file directly from Hugging Face to its model cache. The files do not
  go through the hub (PULL-004). The address is
  `https://huggingface.co/<organisation>/<repository>/resolve/<revision>/<file>`, made by the fetch process from
  the parameters; each part is encoded as a path segment. For a name in a sub-folder, the folder and the file
  are each encoded as a path segment, with a literal `/` between them, never `%2F` (SF-381). `revision` and each file name are checked by the
  rules of Section 2 before the address is made. The daemon writes each file, flattened to its file part, with
  `openat` relative to the folder of the copy, a folder of mode 0700, with `O_NOFOLLOW` and `O_EXCL`, so a name
  never leaves that folder and never opens a file that is there (SEC-C-012, SF-359, R-408).
- **The approved organisations.** The hub holds the approved list (LIBM-003). The machine holds its own list too,
  and refuses a pull from an organisation that is not in it (`source_not_permitted`). By R-342, the default list of the machine (the setting `organisations`, Section 10.1) is `unsloth`,
  `mlx-community` and the publishers that the release of the daemon names; the owner of the machine changes it
  at the machine. The hub cannot write it. See open
  point MW-P2.
- **A second source of each hash (R-339; off by default).** The SHA-256 of an item comes from the hub,
  so it proves nothing against a hub that is taken over. The check against the hub value is always done. When
  the setting `secondHash` is on (Section 10.1), the fetch process also reads, from Hugging Face,
  the SHA-256 that the repository publishes for each LFS file at that `revision` (the file list of the commit),
  and refuses a file whose two values differ (`hash_mismatch`). A file that is not an LFS file is checked only
  against the hub value and must be at most 16 MiB. When the setting `revisionAgeDays` is above 0
  (default 0), the machine refuses a `revision` whose commit date at Hugging Face is newer (`revision_too_new`). The owner of the
  machine can change the age at the machine.
- **The checks.** The daemon writes each file to `<cache>/partial/<copy>/` and computes the SHA-256 as the bytes
  arrive. It moves the file to `<cache>/copies/<copy>/` only when the size and the SHA-256 are those of the item
  (PULL-005). A file that is larger than `sizeBytes` stops at once (`size`). A different SHA-256 deletes the
  file and fails the item (`checksum`).
- **Resume (PULL-006).** After a cut, the daemon continues each file from the last complete part with an HTTP
  `Range` request. A part is 16 MiB. The SHA-256 state is kept with each complete part, so the daemon does not
  read the file again. When the source answers 200 in place of 206, the daemon starts that file again from 0.
  The partial folder survives a restart of the daemon; the next `model_pull` of the same copy continues it.
  The files of `partial/` belong to the user of the daemon, and a runtime runs as that user (R-346), so their
  bytes can change between two parts. Thus, before the move to `copies/`, the daemon reads the whole file again
  and computes its SHA-256 from byte 0; only that value counts for the check. (A build that holds each partial
  file open through one descriptor for the whole life of the pull, and never opens it again by name, may skip
  the second read for a pull with no restart.) (SEC-C-012, SF-357.)
  The bytes of `partial/` count in the disk share and in `cacheBytes`. The daemon deletes a partial copy that no
  pull touched for 7 days, and at `copy_delete` of that copy. It holds at most 4 partial copies; a fifth pull
  is refused (`disk_share`).
- **The disk share (PULL-008).** Before the start, the daemon checks that the copy fits in the disk share of the
  device policy with what the cache holds. If it does not fit, it refuses (`disk_share`) and the hub asks the
  operator which copy to remove. The daemon never removes a copy by itself: only `copy_delete` removes one.
  While the pull runs, the daemon stops it (`limit_disk`) when the free disk drops below the reserve of the
  daemon configuration (default 10 GiB).
- **A found copy is not pulled.** `copy_adopt` (Section 2.3) makes a copy from files that are already on the
  machine. This section does not apply to it; the disk share does not count its bytes, because Anima did not
  write them.
- **The copy is downloaded, not validated (PULL-011).** A pull that succeeds makes the state `downloaded`. Only
  `model_measure` with a correct answer makes `validated` (Section 11).
- **The token of a gated repository (PULL-010, SEC-C-003).** When `gated` is true, `secrets` of the item is
  `["hf_token"]`, a new purpose of K10 Section 8 for `model_pull` only, with the life of the item. The hub reads
  the token from the key vault of E0-6 (K-E5) when the holder asks for it, and not before. The daemon asks for it
  in `preparing`, gives it to the fetch process on its standard input, and to no other process. It writes it to
  no file, no log, no command line and no environment variable. The operator must give a fine-grained token of
  Hugging Face with read access to gated repositories only; the hub says so where he enters it. The fetch
  process sends it only when `gated` is true, only in the `Authorization` header, only to the host
  `huggingface.co`, and only on a `/resolve/` request; it drops it at a redirect to each other host. A test shows
  that the header is not sent on the redirect from `huggingface.co` to `cas-bridge.xethub.hf.co`; the code strips
  it itself and does not depend on the default of an HTTP library (SF-303). In the order, `hf_token` is a type of
  its own: it wipes its bytes on drop, implements neither `Debug`, `Display` nor `Clone`, and is never serialised
  back into a message or a report. The fetch process is not dumpable: it sets `PR_SET_DUMPABLE` to 0 (Linux) and
  `RLIMIT_CORE` to 0 before it reads the token (SEC-C-012, SF-360). The hub writes
  each release to the audit log without the value (K10 Section 8). No command, MCP tool or query gives it
  (MCP-004).
- **The key vault on dev.** The operational keys of K-E5 Section 2 are built on `dev` and wait for the human
  check HT-035; the `credentials` domain of the hub holds them (R-147). The purpose `hf_token` reads one key of
  that store by its name. The token is not granted to the device as an operational key, because a grant puts it
  in the cache of the daemon for longer than one pull. Until the store has the Hugging Face token, or if the
  vault is not on the hub, the hub makes no `model_pull` item with `gated` true and tells the operator why;
  pulls from public repositories work.

## 8. The runtime report (CAG-006)

The runtime report is an optional member `runtimes` of the `Batch` of K8 Section 4. It is not a report of the
work route.

Reason: a server runs when no item is held, so the work route has no item to report on. K10 ends an attempt that
holds an item with no end (K10 Section 7, `expired`), and a report of K10 belongs to one item. The push batch is
already sent each 10 seconds by each enrolled daemon, is kept in the buffer while the hub cannot be reached
(MET-014, MET-022), and adds no new route. The daemon sends the member in one batch each 15 seconds at most
(CAG-006); because batches go each 10 seconds, a value is never older than 15 seconds, and the member is left out
of the batches between.

```json
"runtimes": {
  "at": "2026-11-20T10:00:10Z",
  "installed": [
    { "runtime": "llama_server", "version": "6040", "backend": "metal" },
    { "runtime": "mlx_lm", "state": "no_version", "backend": "metal" },
    { "runtime": "ollama", "version": "0.12.6", "backend": null }
  ],
  "servers": [
    { "server": "<uuid>", "runtime": "llama_server", "state": "ready", "restarts": 0,
      "loaded": ["<copy uuid>"], "inProgress": 1, "queued": 0, "tokensPerSecond": 41.5,
      "memoryBytes": 6442450944, "tempBytes": 0 }
  ],
  "copies": [ { "copy": "<uuid>", "state": "validated", "bytes": 5027783488 } ],
  "cacheBytes": 5027783488
}
```

- `state` of a server is `starting`, `ready`, `draining`, `restarting`, `failed` or `stopped`.
- `installed` is the list of the installed model runtimes (CAP-003, R-345). It is the only list of
  the runtimes; the tool inventory of K8 Section 4 does not hold them. It holds at most 16 entries with unique
  names. Each entry has a `runtime` and either a `version` of 1 to 64 bytes of `[0-9A-Za-z.+_-]` or a `state`,
  never both. The `state` is a fixed code, never text from the device: `timed_out`, `exited` or `no_version`,
  with the meanings of the tool inventory (CAP-024); the hub reads a `state` it does not know as `failed`. A
  runtime the daemon did not find is not listed.
- The runtimes the hub knows are `llama_server` (the program `llama-server`), `mlx_lm` (the program
  `mlx_lm.server`, macOS only), `vllm` (the program `vllm`) and `ollama` (the program `ollama`). Ollama is
  listed only so that a machine that has it shows up; it is not a runtime that the model service uses, and
  task L1-18 still replaces it (the Ollama form at the model address). The hub drops an entry of any other name, so a
  newer daemon can talk to an older hub.
- The daemon reads each runtime with the fixed-argument probe of the tool inventory: the program from the fixed
  tool folders, the one argument `--version`, 5 seconds at most, no shell, no model started, the first version
  on standard output (K8 Section 4, CAP-022, CAP-023). It reads them again at each probe of the inventory
  (each 15 minutes).
- Each entry has a `backend`: the backend the runtime uses on this machine, one of the backend names of the K8
  member `hardware` (`metal`, `cuda`, `rocm`, `vulkan`, `cpu`), or null when the daemon cannot find it. The
  member is always present. The daemon finds it with fixed rules, never from free text:
  - `mlx_lm`: `metal` (MLX runs only on Metal).
  - `llama_server`: a second fixed-argument probe, `llama-server --list-devices`, with the same limits. The first
    device line that starts with `Metal`, `CUDA`, `ROCm` or `Vulkan` gives `metal`, `cuda`, `rocm` or
    `vulkan`; output with no such line gives `cpu`; a probe that fails gives null.
  - `vllm`: the build tag after `+` in the version that it prints: `rocm…` gives `rocm`, `cpu` gives `cpu`,
    `cu…` gives `cuda`; no tag or any other tag gives null.
  - `ollama`: Ollama selects its backend when it loads a model, so the daemon takes it from the hardware
    verdicts: on macOS `metal`; on Linux the first usable of `cuda`, `rocm` and `cpu`; null when none is usable.
  - A runtime with a `state` (not usable) still has a `backend` when the rule finds one, else null.
- When `backend` and the hardware verdict disagree, the hardware verdict wins: a runtime whose `backend` is not
  usable in `hardware` is taken as not able to use that backend, and the hub and the placement read the
  verdict. The daemon still sends the `backend` that it found, so the operator sees the disagreement.
- Before a model server runs, the daemon sends `runtimes` with `installed` only (`servers` and `copies` empty,
  `cacheBytes` 0): after start, each time the list changes, and once every 24 hours when unchanged, as the
  tool inventory (CAP-027). When a server runs, the 15-second rule above applies.
- `tokensPerSecond` is the mean of output tokens over the last 15 seconds, or null. `memoryBytes` is the resident
  memory of the process and, for a GPU with its own memory, its use of that memory.
- The member holds no prompt, no output, no file name of a copy and no address (SEC-C-010). At most 16 servers
  and 64 copies.
- A hub that does not know the member ignores it (K8 Section 1).
- `model_report` gives the same object as its `result.model`, for the hub to read the whole state at once.
- Each entry of `copies` has `origin`: `pulled` or `found` (Section 2.3). A found copy does not count in
  `cacheBytes`.
- Only `result.model` of `model_report` has the list `found` (PULL-012), never the push batch: the walk of
  Section 10.2 reads many folders, so the daemon does it only when the hub asks. When the setting `findModels` is
  `false`, the list is empty.

```json
"found": [
  { "found": "<64 hexadecimal characters>", "tool": "ollama", "name": "qwen3:8b", "format": "gguf",
    "files": 1, "bytes": 5225376047, "usable": true }
]
```

- `tool` is `ollama`, `lm_studio`, `huggingface` or `folder` (a folder of `modelFolders`). `name` is the name that
  the tool shows: the Ollama name and tag (`qwen3:8b`, `hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M`), the organisation and
  repository of the Hugging Face cache with the file part (`unsloth/Qwen3-8B-GGUF/Qwen3-8B-Q4_K_M.gguf`), the
  publisher, repository and file of LM Studio, or the path below the folder of `modelFolders`. It is text from
  the device: at most 256 bytes of letters, digits, `-`, `_`, `.`, `:`, `/` and `+`; another name is not listed.
  The hub shows it as text only and never sends it back.
- At most 256 entries. `bytes` is the sum of the sizes of the files. A found model that is a found copy already
  is still listed, so that the hub can match the two by `found`.

## 9. Start, the isolation, the loopback check and the supervision (SEC-C-001, CAG-005)

- **The isolation (R-346; off by default).** A runtime reads files that the hub chose, and its parsers
  have faults (SEC-C-009). With no isolation, a malicious model file can take the account of the user of the
  daemon; the operator accepted that risk. When the setting `sandbox` is on (Section 10.1), the daemon starts
  each runtime in an isolation that denies each thing by default:
  on macOS a `sandbox-exec` profile of the daemon program; on Linux Landlock and seccomp, or `bubblewrap` where
  it is installed. The runtime can read its runtime folder and its copies, write only its temporary folder,
  and listen and connect only on loopback. It cannot read the daemon folder (the token and the configuration),
  the home folder of the user or the other folders of the daemon. When the daemon cannot make the isolation, the
  start fails (`isolation_failed`); there is no start with less isolation. With the setting off (the default),
  nothing blocks an outgoing connection of a runtime (R-379; SF-300 accepted). What stays required:
  the listener on loopback only (a LAN switch of the operator is not in this version), the cleaned environment
  with no token and with the offline and no-telemetry variables, only GGUF and safetensors with the
  `remote_code` refusal (Section 2.2), and the loopback check below for the listeners. The isolation stays a
  setting that the owner of the machine can set on.

- **The tree of a server, and what the loopback check binds (R-331).** Without the sandbox, the
  loopback check below constrains an honest runtime only: a runtime that is not honest can make outgoing
  connections and can hide a listener for the time between two checks.
  - On Linux the tree of a server is one cgroup v2 below the cgroup of the daemon, which its unit delegates. The
    server joins that cgroup before its program runs: the daemon starts the daemon program in the fixed mode
    `__model-launch`, which writes its own process id to `cgroup.procs`, sets the parent-death signal (`SIGKILL`) as a
    backstop, checks that the daemon is still its parent, makes a new cgroup namespace (in a user namespace of
    its own that maps the user and the group of the daemon to themselves; R-330), and then runs the
    runtime with `exec`. The root of the cgroups of the runtime is then the cgroup of its server: it cannot see
    the cgroup of the daemon or move a process out of its cgroup. When the namespaces cannot be made, the start
    fails (`containment_failed`), with no fallback. On a system that restricts user namespaces with AppArmor
    (`kernel.apparmor_restrict_unprivileged_userns=1`, the default of Ubuntu 24.04 and later and its family), each
    start of the daemon program as the launcher fails with the detail `userns_denied` (R-327). There the
    daemon uses the model launcher instead (Section 9.1). At each poll the main process, while it runs, must be in the
    cgroup of its server; when it is not, the server ends with `listener`. The loopback check
    reads the sockets of each process of `cgroup.procs` of the cgroup of the server and of each cgroup below it, in
    the network tables of each network namespace that a
    process is in, merged. A `cgroup.procs` that does not read, a process of the tree whose files do not read, or
    an empty tree while the server runs, counts as a listener that is not on loopback (it fails closed). A stop and
    an end use `cgroup.kill` and a signal to each process, then remove the cgroup; a cgroup that does not empty is
    logged and left, and each start has a cgroup of its own name. When the `pids` controller is delegated, the
    daemon sets `pids.max` of the cgroup to 512 (a placeholder value, to measure). The daemon sets
    `cgroup.max.descendants` of the cgroup to 0, so the runtime can make no cgroup below it. The cgroup v2 mount must
    have `nsdelegate` (`/proc/self/mountinfo`), because without it the cgroup namespace does not stop a move out of
    its root; when it does not, the start fails (`containment_failed`, `no_nsdelegate`). When the cgroup cannot be made
    (no cgroup v2, or no delegation to the user of the daemon), the start fails (`containment_failed`). There is no
    start with less containment.
  - On macOS the tree is the walk of the parent processes from the server (`/bin/ps`), and the daemon records each
    process of the tree at each poll, so a child that a double fork gives to `launchd` stays in the tree once it was
    seen. This is best effort unless the server runs under the sandbox.
  - Before each signal the daemon checks the identity of the process: its id and its start time. It never signals
    a process id that the system gave to a different program.
  - The daemon writes the identities and the cgroup of each server to a record in the folder of that server in the
    state folder at each poll. When the daemon starts, it ends the processes and the cgroups that each record names
    and removes the records (the sweep), because it starts no server by itself (MW-P5). The sweep ends only a
    cgroup named `anima-model-` directly below the cgroup of the daemon, because the runtime can write the record
    when the sandbox is off.
- **How the socket table is read (R-329).** On macOS the daemon runs `/usr/sbin/lsof` by its absolute
  path, with an empty environment, for the processes of the tree, with a limit of 5 seconds. A read that does not
  end in time or does not read counts as a listener that is not on loopback (it fails closed). `proc_pidinfo` would
  need code that the workspace forbids (`unsafe`).
- **Ready (R-329).** A server is `ready` when the socket table shows a TCP listener of its tree on
  loopback on its port. The daemon sends no request to the server for this. The HTTP probe, with the owner check
  of the listener right before it, comes with task L1-9 or `model_measure` (task L1-8).
- **The port.** The daemon starts the runtime with port 0 where the runtime permits it and reads the port from
  the socket table; else it selects a free port of its range. Before it sends the API key in a probe, it checks
  that the listener of that port belongs to the process tree of the server; if not, the cause is `listener`.
- **The loopback check.** When a server is ready, after each restart, after each `model_load`, and in each cycle
  of 15 seconds, the daemon reads the sockets of the process tree of the server from the system (on Linux
  `/proc/<pid>/net/tcp`, `tcp6`, `udp` and `udp6` with the socket inodes of each process of the tree; on macOS
  `proc_listchildpids` and `proc_pidinfo` with `PROC_PIDFDSOCKETINFO`). It does not connect to find them. Each
  listening TCP socket and each bound UDP socket must be on `127.0.0.1`, `::1` or `::ffff:127.0.0.1`; no other
  address that maps IPv4 counts as loopback. If one is not, the daemon ends the process tree at once, reports
  `failed` with the cause `listener`, and does not start that server again until a new `server_start`. Between
  two checks a listener can live for at most 15 seconds; only the isolation closes that time.
- **Memory and disk.** The daemon ends a server whose memory is above its share of the device policy and 10 %
  (`limit_memory`). On Linux it also sets `memory.max` of a cgroup of the server, or `RLIMIT_AS` where no cgroup
  is available. The temporary folder of a server has a limit of 2 GiB; its bytes count in the report
  (`tempBytes`), and a server above the limit is ended (`limit_disk`). The disk reserve of Section 7 is checked
  for each write of model work, also in the runtime and temporary folders.
- **The supervision (CAG-005).** When a server process ends and no item asked for it, the daemon starts it again
  with the same settings after a wait of 1, 2, 4, 8, 16, 32, 64, 128, 256 seconds, then each 300 seconds. The
  daemon counts each end that no item asked for, also of a server that was `ready` for a long time. After 5 in one
  hour or 20 in 24 hours, the server stays `failed` until a new `server_start`; only a new `server_start` that
  then stays `ready` for 10 minutes sets the counts to 0. When a server ends during a `model_load` or in the 60
  seconds after it, the copy of that load is `failed` with the cause `load_crash`, and the daemon does not load
  it again by itself. Each restart and each `failed` is an event of the
  runtime report (`restarts`) and an entry in the log of the machine. The function of the waits is pure, in
  `daemon-core`, with a test clock (CAG-007).
- On a stop of the daemon, the daemon stops each server. At its start it starts no server by itself; the hub
  sends `server_start` again from its record. See open point MW-P5.

## 9.1 The model launcher (task L1-6c; R-327, R-347)

- **Where it is used.** On Linux, when AppArmor is on (`/sys/module/apparmor/parameters/enabled` is `Y`) and
  `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` is `1`. Elsewhere the daemon program is the launcher, as
  Section 9 says, and nothing of this section applies.
- **The program.** `/usr/local/libexec/anima/anima-model-launcher`: owner root, mode 0755, in folders that root owns
  and that no other user can write. The AppArmor profile `anima-model-launcher` attaches to that path and allows
  `userns`; each program that the launcher runs goes to the profile `anima-model-runtime`, which does not allow it.
  The launcher takes the path of the signed list and of its signature, then the arguments of `__model-launch`. It
  does the steps of Section 9 with the same exit codes. Before it makes the namespaces it opens the runtime program
  (no symbolic link, a regular file only), checks the signature of the list with the updater public key that is
  built into it, requires an entry of the list with the SHA-256 of the open file, and else exits with 125. It runs the
  open file, not its path. `--protocol` writes `anima-model-launcher <n>`.
- **The checks of the daemon at a start.** The launcher path is a regular file of owner root that no group or other
  user can write, its folders follow the same rule, and the profile is loaded; else `containment_failed` with
  `launcher_missing`. `--protocol` gives the number of the daemon; else `launcher_outdated`. The program is on the
  kept list; else `runtime_not_signed`.
- **The signed list.** `runtime-list.json`: `sequence` (an integer) and `runtimes`, each entry with `runtime`,
  `version`, `backend`, `os`, `architecture`, `program` and `sha256` (of the program file after the unpack). The
  signature is a minisign signature of the updater key whose trusted comment names `file:runtime-list.json` and
  `version:<sequence>`. The daemon downloads the pair from `https://releases.anima.ing/runtimes/` when it starts and
  each 24 hours, keeps a pair only when the signature holds and the sequence is not lower than the kept one
  (`<model-state>/runtime-list.json` and `.sig`, mode 0600), and uses the kept pair offline. With no kept pair, each
  start where the launcher is used fails with `runtime_not_signed`.

## 10. The folders that model work writes (CAG-009, MET-002)

Model work writes only in these folders, and below them. It changes nothing else on the machine: no system
setting, no file of a user, no package, no service unit.

| Installation | Runtimes | Model cache | State |
| --- | --- | --- | --- |
| macOS, user (launchd agent) | `~/Library/Application Support/anima-daemon/runtimes/` | `~/Library/Application Support/anima-daemon/models/` | `~/Library/Application Support/anima-daemon/model-state/` |
| Linux, user (systemd user unit) | `$XDG_DATA_HOME/anima-daemon/runtimes/` (default `~/.local/share/anima-daemon/runtimes/`) | `$XDG_DATA_HOME/anima-daemon/models/` | `$XDG_STATE_HOME/anima-daemon/model-state/` (default `~/.local/state/anima-daemon/model-state/`) |
| Linux, system unit | `/var/lib/anima-daemon/runtimes/` | `/var/lib/anima-daemon/models/` | `/var/lib/anima-daemon/model-state/` |
| `ANIMA_DAEMON_HOME` set | `<home>/runtimes/` | `<home>/models/` | `<home>/model-state/` |

- The configuration of the daemon (release sources, the approved list of the machine, the port range, the disk
  reserve) stays in the daemon folder of K8 Section 2 (`config/home.rs`). The hub cannot write it.
- The daemon makes each folder with mode 0700 and refuses to use one that a different user owns, or that a link
  replaces. The system unit adds the three folders to its `ReadWritePaths` and nothing else.
- On Linux the user folders are below `XDG_DATA_HOME`, not `XDG_CONFIG_HOME`, because a model cache is large and
  is not configuration. A macOS folder below `~/Library/Caches` is not used, because the system can delete it.
- A runtime process writes only to a temporary folder below the state folder, which the daemon empties at each
  start of that process (limit: Section 9).
- The daemon makes, moves and deletes each file and folder of model work relative to an open handle of its
  folder (`openat`, `renameat`, `unlinkat`), with `O_NOFOLLOW`. `copy_delete` and each clean-up follow no
  symbolic link.
- MET-002 gets one more exception, which R-165 decided (OD-C9): "the daemon does the model work of the fixed list
  of CAG-003, and with it changes only its own folders (CAG-009)". The text of MET-002 has it (R-344).

### 10.1 Settings at the machine

The file `model-work.json` in the daemon folder of K8 Section 2. Only the owner of the machine writes it; the hub
cannot write it, and an update of the daemon does not change it (as the switch, K10 Section 3, SF-007). Each
stricter value is permitted; the defaults are the answers of the operator.

| Setting | Default | Stricter form |
| --- | --- | --- |
| `sandbox` | `false` (R-346) | `true`: Section 9, the isolation |
| `secondHash` | `false` (R-339) | `true`: Section 7, the hash of Hugging Face |
| `revisionAgeDays` | `0` (R-339) | Days, for example 7: Section 7 |
| `organisations` | `unsloth`, `mlx-community`, the publishers of the release (R-342) | A shorter list |
| `agentTypes` | The twelve types (R-326) | A shorter list, for example `model_report`, `server_stop`, `model_unload`, `model_drain` |
| `findModels` | `true` (R-443) | `false`: no folder of Section 10.2 is read, `found` is empty, and `copy_adopt` is refused (`found_unknown`) |
| `modelFolders` | Empty | At most 16 absolute folders that the owner of the machine adds (R-444: "the operator can add other folders"); Section 10.2. This is the one setting whose change permits more, and only the owner of the machine makes it |

The daemon reports the values in `model_report`, so the hub can show them. The plain controls (Section 2.2, the
hash of the hub, Sections 4, 6 and 10) are not settings.

### 10.2 The folders that model work reads and never writes (PULL-012, R-444)

Model work reads these folders, to find the models of Section 2.3, when the setting `findModels` is `true`. It
never writes in them (Section 2.3). The folders below a home folder are those of the user of the daemon; a
daemon of the system unit reads only the Ollama folder of the system and `modelFolders`.

| Tool | Folders (each that exists) | What a found model is |
| --- | --- | --- |
| `ollama` | `~/.ollama/models/`; on Linux also `/usr/share/ollama/.ollama/models/` (the service of the Ollama installer) | A manifest below `manifests/` (at most 4 levels: host, namespace, model, tag) that names one layer of the type `application/vnd.ollama.image.model`, a file `blobs/sha256-<hex>` |
| `huggingface` | `~/.cache/huggingface/hub/` (the cache of `huggingface_hub`, which Unsloth Studio and the Hugging Face tools use) | A folder `models--<organisation>--<repository>/snapshots/<commit>/` with GGUF files, each a link to `../../blobs/<name>`, which can link once more to the shared store `blobs/<xx>/<name>` of the cache (Section 2.3) |
| `lm_studio` | `~/.lmstudio/models/` and `~/.cache/lm-studio/models/` | A GGUF file at `<publisher>/<repository>/<file>` |
| `folder` | Each folder of `modelFolders` (Section 10.1) | A GGUF file at most 3 levels below the folder |

- The daemon reads no environment variable for these folders (`OLLAMA_MODELS`, `HF_HOME`, `HF_HUB_CACHE`): a
  service does not have the environment of a terminal. The owner of the machine names such a folder in
  `modelFolders`; the human check of task L1-22 measures where each tool keeps its models on `pop-os`.
- A model that two folders list is two found models, with two keys.

## 11. The states of a copy (CMP-006, PULL-011)

The daemon keeps the state of each copy in its state folder. The hub keeps the record (CMP-006).

```text
            model_pull                       model_measure (correct)
(none) ──────────────► downloading ──► downloaded ──► validating ──► validated
                           │                ▲              │
                           │                └──────────────┤ (probe_failed, start_failed)
                           ▼                               ▼
                         failed ◄──────────────────────── failed
            copy_delete from each state ──► (none)

            copy_adopt (Section 2.3)
(none) ─────────────────────────────► downloaded
            a check at a load: copy_changed or copy_missing ──► failed
```

- `downloading`: the files are in `partial/`. `downloaded`: each file is verified and the copy was not validated
  on this node. `validating`: a `model_measure` runs. `validated`: the fixed request got a correct answer on this
  runtime and this hardware. `failed`: the copy cannot be used; `cause` says why. A `model_pull` of a `failed`
  copy starts it again.
- A `validated` copy goes back to `downloaded` when the runtime version or the backend that validated it changes.
- A found copy has the same states. It has no `downloading`: `copy_adopt` makes it `downloaded` when each file is
  read and recorded. A found copy that is `failed` with `copy_changed` or `copy_missing` is never checked again by
  itself; a new `copy_adopt` of the same key, with a new copy, records the files as they are now. Its record in
  `<model-state>/found/` is the state that the daemon keeps.
- The states of a copy on a server (`loading`, `ready`, `active`) are not states of this diagram. Section 11.1
  gives them, as members of `servers[]` (task L1-15, FIT-025).

### 11.1 Loading, memory and the pin (L1-15, FIT-020 to FIT-025)

This section is the part of task L1-15 in this contract. The wait policy of a request, the status of a request,
the queue limit, the timeout and the cancellation at the model address (FIT-026, the choice of the caller;
RTR-031, RTR-039) are the part of L1-15 in `model-request.md`, which task L1-10 makes. This section gives what the
node does; the model address uses it.

**Which hub sends what.** The hub sends `reserveBytes`, `keep` and the type `model_keep` only to a daemon whose
`Claim` lists `model_keep` in `types`. An older daemon refuses each member that its row does not name (Section 2),
so it gets the `model_load` of version 0.6.

**The states of a copy on a server (FIT-025).** A copy that is `downloaded` or `validated` (Section 11) can be on
a server. On a server it has one of these states:

- `loading`: a `model_load` or a return runs for it.
- `ready`: it is loaded, and no request is in progress on it.
- `active`: it is loaded, and one or more requests are in progress on it.
- `unloading`: the daemon selected it to unload. No request starts on it; a request for it waits for a new load.

A copy goes from `ready` to `unloading` at the moment the daemon selects it, under the lock of its memories
(below), so that a request cannot start between the choice and the unload. A copy that is on no server has no
state of this list. The hub shows the copy state of Section 11 and, for each server, this state; it never joins
them into one word.

**The reservation of a copy (FIT-024).** Each copy that is `loading`, `ready`, `active` or `unloading` holds a
reservation of memory, in bytes. The daemon's own lower estimate is the sum of the `sizeBytes` of the files of the
copy and 25 % of that sum, for the cache and the runtime; for a runtime that takes a share of a GPU at its start
(vLLM), it is at least `memoryShare` times the size of the memory of that GPU. The reservation is the larger of that
estimate and `reserveBytes`. The hub gives `reserveBytes` from its fit estimate (task L1-2) for the settings of the
load (the context, the cache type, the parallel slots). Thus a wrong hub can make the daemon too careful, never
too bold. The reservation of an active request is in the reservation of its copy: the parallel slots of Section
2.1 are counted in the estimate, so a request adds no reservation.

**The memories of a load.** The node counts its reservations for each memory apart: `main` (the main memory) and
`gpu` with the index of each GPU of its inventory that has its own memory (`gpu0`, `gpu1`). The usable memory of
each is its size times the memory share of the device policy (RTC-003). A load puts a part of its reservation on
each memory that it uses:

- `vllm`: all of it on the GPU of `gpu`.
- `llama_server` with layers on a GPU of its own memory: the part on the GPUs is the reservation times
  `gpuLayers` divided by the layers of the copy (all of it for `all`), split in equal parts over the GPUs that
  the runtime can see; the rest is on `main`. When the daemon does not know the layers of the copy, the whole
  reservation counts against each of those GPUs and against `main` (too careful, never too bold).
- Each other load, and each load on a machine whose GPU shares the main memory (Metal): all of it on `main`.

**Room for a load (FIT-020, FIT-021, FIT-024).** Before the daemon loads a copy, it takes the lock of each memory
of the load, then:

1. If a part of the reservation is larger than the usable memory of its memory, it refuses with
   `memory_share` and changes nothing. No unload can make room.
2. If, on each of its memories, the parts that count against it with the new part are not more than the usable
   memory, it loads.
3. Else it selects copies to unload, from the copies that count against a memory that is too full: only copies
   that are `ready`, and that are idle for at least 30 seconds or were loaded by a return and not used since. It
   never selects a copy that is `loading`, `active` or `unloading`, and it never stops a request. It takes copies
   that are not pinned first, then pinned copies (FIT-023: a pin is not a reservation); in each group, the copy
   that was used longest ago first. It takes copies in that order until there is room on each memory.
4. If all those copies do not make room, it refuses with `memory_busy` and unloads nothing. Else it sets them to
   `unloading`, unloads them, then loads. An unload frees the memory of the runtime and keeps the files of the copy
   (FIT-021); it is never a `copy_delete`.

For a runtime that cannot unload (vLLM), a `model_load` on a server that has a copy replaces that copy: that copy
is selected as in step 3, so a load on a server whose copy is not `ready` and idle is refused with `memory_busy`.

The daemon takes the locks of the memories in one order: `main`, then the GPUs by their index. The locks make
one load at a time on each memory, so that two loads cannot count the same free memory. A second
`model_load` for the same memory waits for the first, at most 120 seconds (the limit of `start_failed`), then is
refused with `memory_busy`. A `model_load` that arrives while a return loads on one of its memories ends that
return (the copy stays `displaced`) and goes first.

**The idle time (FIT-022).** `keep.idleSeconds` is an integer from 60 to 86,400. A copy that is `ready`, not
pinned, and that no request used for `idleSeconds`, is unloaded by the daemon, with the states and the lock of
step 4. "Used" is the end of the last request on the copy, or the end of its load. The daemon checks each 15
seconds, so an unload comes at most 15 seconds late.

**The pin and its return (FIT-023).** `keep.pinned` is `true` or `false`. A pinned copy is never unloaded because
it is idle. It can be unloaded only by step 3 above. Then the node marks it `displaced` and loads it again by
itself when all of these are true:

- the switch is on and the device policy permits model work now (Section 3);
- the copy is still pinned, and still `downloaded` or `validated`, and each check that Sections 2.2, 2.3 and 5 make
  for a `model_load` passes (a check that fails ends the mark);
- no load runs on its memories, and no `model_load` was refused with `memory_busy` on them in the last 60
  seconds (a return does not hold back a request that waits);
- each part of its reservation fits on its memory with no unload (a return never unloads a copy);
- the server that it was displaced from is `ready`; for a runtime that cannot unload, that server has no copy;
- the interval since its last return has passed. The interval is 10 minutes. It doubles, to at most 4 hours,
  each time a copy is displaced again less than 10 minutes after its return (no loop of loads). It goes back to
  10 minutes after a return that stays loaded for 10 minutes.

A return loads the copy with the settings of its last load. A `model_unload`, a `copy_delete`, a `server_stop`, or
a `model_keep` with `pinned` false ends the mark.

**`keep` and `model_keep`.** `keep` has exactly the members `idleSeconds` and `pinned`, and both are required. A
different member, a value of the wrong type, or a value out of range is refused (`member_unknown`). `reserveBytes`
is an integer from 1 to 2^53 − 1; else `member_unknown`. With no `keep`, a `model_load` keeps the values that the
node holds for the copy, else `idleSeconds` 900 (15 minutes) and `pinned` false. A `model_keep` for a copy that is
not on the node (not in the model cache, not a found copy) is refused (`copy_unknown`). Section 3 gives the lease
and the switch rules of `model_keep`.

**What the node keeps.** The daemon keeps one file for each copy that has a `keep`:
`<model-state>/keep/<copy>.json`, mode 0600, at most 4 KiB, written as a temporary file in the same folder and
then moved into place with `renameat` (Section 10). It holds `keep`, the mark `displaced`, the server and the
settings of the last load, `reserveBytes`, the time of the last return and the interval. A `copy_delete` deletes
the file. When the daemon reads a file, it checks each member by Sections 2.1 and 11.1; it deletes a file that
fails a check and uses the defaults. There is at most one file for each copy, and at most 64 files: when the node holds 64, a `keep` for a
different copy is refused (`member_unknown`).
After a restart of the daemon, it loads nothing by itself: the hub sends `server_start` again (MW-P5). The daemon
marks each pinned copy that was loaded before the restart as `displaced`, so that it returns by the rules above.

**The report.** Each entry of `servers[]` of Section 8 gets the member `models`, and the `runtimes` object gets the
member `memory`:

```json
"servers": [
  { "server": "<uuid>", "runtime": "llama_server", "state": "ready", "restarts": 0,
    "loaded": ["<copy uuid>"],
    "models": [
      { "copy": "<copy uuid>", "state": "active", "reservedBytes": 7516192768,
        "idleSeconds": 900, "pinned": true, "usedSecondsAgo": 0 }
    ],
    "inProgress": 1, "queued": 0, "tokensPerSecond": 41.5, "memoryBytes": 6442450944, "tempBytes": 0 }
],
"memory": [
  { "memory": "main", "usableBytes": 25769803776, "reservedBytes": 7516192768, "displaced": [] },
  { "memory": "gpu0", "usableBytes": 12884901888, "reservedBytes": 0, "displaced": ["<copy uuid>"] }
]
```

- `models[].state` is `loading`, `ready`, `active` or `unloading`. `usedSecondsAgo` is the time since the copy was
  last used, rounded down to a multiple of 60 seconds, or 0 while it is `active` or `loading`. At most 16 entries
  for each server.
- `loaded` stays, for a hub that does not know `models`: it lists the copies that are `ready` or `active`.
- `memory` has one entry for each memory of the node, at most 17. `displaced` lists the pinned copies that wait
  for their return and count against that memory, at most 16.
- A hub that does not know `models` or `memory` ignores them (K8 Section 1).

**How the daemon loads a copy on `llama_server` (RTC-005).** This part tells how this version does the steps above;
it adds no rule. The daemon starts `llama-server` in router mode with `--models-preset <server folder>/preset.ini`,
`--no-models-autoload`, `--models-max 0` and `--metrics`, so the router loads a copy only when the daemon asks, and the
daemon is the only judge of the memory. The preset has one section for each copy that the server holds, named by the
copy id, with its model file, its projector and its settings; the daemon writes it as a private file (Section 10) before
each load and after each unload, and the router reads it again at `GET /models?reload=1`. A load is
`POST /models/load`; an unload is `POST /models/unload`. The router answers both at once and then works, so the daemon
reads `GET /models` each half second until the copy is loaded or unloaded, for at most 120 seconds (`start_failed`;
a copy that the router marks failed is `load_crash`). Each 15 seconds the daemon reads `GET /metrics?model=<copy>`: a
request in progress makes a copy `active`, and a change of its count of decoded tokens is a use. A copy that the router
no longer has loaded frees its reservation, and a pinned one is marked `displaced`. One lock holds the loads of the whole
node, so the node does one load at a time on all its memories, which is stricter than step 4. A found copy (Section 2.3)
is not loaded in this version (`copy_not_ready`): its file reaches the router only by its path, which Section 2.3 does not
permit in a preset (MW-P15). Until the inventory of L1-5, each load counts on main memory with a share of 1.0.

## 12. Open points of this text

| Point | Question | Proposal |
| --- | --- | --- |
| MW-P1 | MET-020 permits the daemon only the hub. A pull, an install and a measure need Hugging Face, a release host and loopback | A child fetch process and a child probe process, with no token and fixed hosts (Section 4); Decided (R-344): MET-020 has that exception in its text |
| MW-P7 | The SHA-256 comes from the hub (F4) | Decided (R-339): the hub hash only; `secondHash` and `revisionAgeDays` are settings at the machine, off |
| MW-P8 | Is `mlx-community` in the default list? (F5) | Decided (R-342): yes; `organisations` is a setting at the machine |
| MW-P9 | A runtime runs with the rights of the user of the daemon (F2) | Decided (R-346): no required isolation; `sandbox` is a setting at the machine, off |
| MW-P2 | Does the machine hold its own list of approved organisations, or does it trust the list of the hub? | The machine holds its own list (Section 7), as the list of repositories of R-185 |
| MW-P3 | Is the runtime report a member of the push batch or a report of the work route? | The push batch (Section 8) |
| MW-P4 | How is an archive of `mlx_lm` or `vllm` with its Python environment made and signed? | Anima builds it with the release (task L1-6 decides the recipe); until then only `llama_server` from its upstream releases |
| MW-P5 | Does the daemon start its servers again by itself after its own restart? | No: the hub sends `server_start` again, so that the machine runs nothing that the hub does not know |
| MW-P10 | A found model of safetensors (a folder of the Hugging Face cache for vLLM or MLX) | Not in this version: it is listed with `usable` false. A runtime reads a folder, not one descriptor, so the check at each load of Section 2.3 does not bind what it reads. A later version can make a folder of links in the model cache, checked at each load |
| MW-P6 | Can an AI agent through an API key make a model item? (F15) | Decided (R-326): each type; `agentTypes` is a setting at the machine |
| MW-P12 | Is a pin for one model on all nodes, or for one copy on one node? (L1-15) | For one copy on one node: the node is the place of the memory. The hub can give the operator one control that sends `model_keep` to each node that has a copy |
| MW-P13 | Who gives the reservation of a load? (L1-15) | The hub, from its fit estimate (L1-2), as `reserveBytes`; the daemon never takes less than the files (Section 11.1). When the daemon later measures the memory of a loaded copy, task L1-8 can correct the estimate |
| MW-P14 | May a return unload a copy that is not pinned, so that a pin comes back before the copy that displaced it reaches its idle time? (L1-15, FIT-023) | Not in this version: a return never unloads (Section 11.1). FIT-023 says "after the other work is done"; the idle time of the other copy is the end of that work |
| MW-P15 | How does a router of `llama_server` load a found copy (Section 2.3)? (L1-16) | Not in this version: a `model_load` of a found copy is refused with `copy_not_ready`. The preset of the router names a file by its path, and the check at each load of Section 2.3 binds an open descriptor, not a path. A later version can pass the checked descriptor as a `/proc/self/fd` path to a child that the daemon starts, or make a link in the model cache that the daemon checks at each load |

## 13. What a person who controls the hub can do with this contract

On a machine that takes work, a person who controls the hub can: install a runtime from a release source that the
machine holds; download files of the permitted types from an approved organisation of the machine (by default
also `mlx-community`), at a commit of their choice and with their own hash (R-339), and fill the disk
share; load such a file in a runtime, whose parser can have faults (SEC-C-009), with the rights of the user of
the daemon when `sandbox` is off (R-346), and so take that account; use memory and power
within the device policy; stop each model server; make a found model a found copy and load it, which reads a
file that a different program or a user of the machine put there and can change (Section 2.3; the operator
accepted this, R-444); learn the names and sizes of the found models; set the reservation, the idle time and the pin of a copy, so that
the node keeps a pinned copy loaded and loads it again by itself after it was displaced, while the switch and the
device policy permit it (at most once each 10 minutes, with longer waits after a loop; Section 11.1); learn, to
the minute, when each copy was last used. The person cannot name a command, a flag that the daemon owns,
an address, a path or a host outside the lists of the machine, and cannot make the daemon read a folder that is
not in Section 10.2 or write in one that is. The security review (SEC-C-012) reads this
section.
