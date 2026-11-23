---
kind: Proposal
state: Draft
evidence: Version 1 (2026-11-22). R-NEW-shortener (operator, 2026-11-22, relayed) names the port and the tasks SA1 to SA4 of Part 1. SA0 (this proposal) is done in this commit. Nothing of Part 1 is built. Part 2 (the proxy) has no ruling that names a build: task SA5 is a design note only. SHA-001 to SHA-024 are not approved one by one.
---

# Proposal: shortened answers (the port of the Headroom shorteners)

Progress: the plan register in [work-plan.md](../work-plan.md) Section 9 holds the state of this proposal (PLN-005).

From: a side session of the orchestrating session, 2026-11-22.
Version: 1. Date: 2026-11-22.
Status: the header of this document holds the state (PLN-020). History: Draft.
Map: [work-plan.md](../work-plan.md). This proposal serves row 4.24 of [priorities.md](../plan/priorities.md) (agent threads, the page Code): the answers that an agent reads cost less of its context. Part 1 changes the MCP server of [cli-mcp-plan.md](../plans/cli-mcp-plan.md) (MCP-008 stays as it is). Part 2 touches the thread runner of [agent-threads.md](agent-threads.md); this proposal names what it needs from the runner and changes no file of it.
Identifiers: SHA-001 to SHA-024. Tasks: SA0 to SA5. Open decisions: OD-SHA1 to OD-SHA6.

## 1. Goal

A coding agent reads long JSON answers and long logs. Most of the lines say the same thing. The
operator read Headroom (`github.com/headroomlabs-ai/headroom`, Apache-2.0) and ruled
(R-NEW-shortener): copy its shorteners into Anima.

- **Part 1.** The JSON shortener and the log shortener become a crate of Anima. The MCP server
  `anima mcp` uses it on its tool answers.
- **Part 2.** A shortening proxy inside Anima, on the path from Claude Code and Codex to the
  model provider, for the threads that Anima starts. This proposal gives only the direction
  (Section 7). Task SA5 writes the design note. No code of Part 2 before a ruling.

Decided with the operator before this proposal (relayed): no Python in the product; no beacon
and no upload of any kind; no ML model for prose; the copyright and licence notices of Headroom
stay, and each changed file says that Anima changed it.

## 2. What Headroom is, measured

The source was read at commit `ce0460cfa8598dc939243dbc7aec4b2d35f4e44a` (2026-11-21). Nothing
of it was run.

- The JSON shortener (SmartCrusher) and the log shortener are **Rust** now
  (`crates/headroom-core`). The Python files of the same names are wrappers. Python still holds
  some rules that Rust does not have: the gate "at least 200 tokens", the list of tools that are
  never shortened, the learning store (TOIN), and the tool `headroom_retrieve`.
- The proxy that Headroom ships is **Python** (`headroom/proxy`, about 20,000 lines for the two
  providers). The Rust proxy (`crates/headroom-proxy`) is a front for it; its shortening is off
  by default and it has no store for originals.
- CacheAligner no longer changes a request. It only writes a warning when the system prompt
  holds text that changes (a date, an identifier).
- The files have no copyright header. The licence is in `LICENSE` (Apache-2.0) and `NOTICE`
  ("Headroom. Copyright 2025 Headroom Contributors").
- The log shortener of Headroom does **not** keep each error: it keeps at most 10 error lines
  and 10 failure lines, and a last limit can remove more. The JSON shortener keeps each row
  that holds an error word, each rare row and each row with a number far from the mean.

| Part of Headroom | Language | Lines (with tests) | Needs |
| --- | --- | --- | --- |
| `transforms/smart_crusher/` (20 files) | Rust | 10,075 | sha2, md-5, regex |
| `transforms/smart_crusher/compaction/` (lossless tables) | Rust | 3,153 | sha2 |
| `transforms/log_compressor.rs` | Rust | 2,769 (1,696 without tests) | aho-corasick, regex, md-5 |
| `transforms/pipeline/reformats/log_template.rs` | Rust | 597 | toml |
| `transforms/adaptive_sizer.rs`, `anchor_selector.rs` | Rust | 1,835 | md-5, flate2 |
| `relevance/` (BM25; the embedding model is the ML part) | Rust | 606 without ML | fastembed, ort (ML) |
| `ccr/` (the store of originals) | Rust | 1,332 | blake3, dashmap, rusqlite |
| `transforms/live_zone.rs`, `cache_control.rs`, `auth_mode.rs` | Rust | 4,008 | serde_json raw values |
| `crates/headroom-proxy` | Rust | 26,066 | axum, hyper, AWS and Google parts |
| `headroom/proxy`, `headroom/ccr`, `headroom/telemetry` | Python | more than 25,000 | FastAPI, LiteLLM |

## 3. What Anima takes

Anima's limits (300 code lines for a file, 60 for a function, Clippy `pedantic`, names from the
domain) do not accept the files of Headroom as they are: `crusher.rs` alone has 1,350 code
lines. So "copy" means: the rules, the numbers and the lists come from Headroom, file by file;
the code is cut into small files of Anima. Each file that holds copied code starts with a
notice (SHA-020).

### 3.1 Ported (the rules and the numbers of Headroom)

| From Headroom (`crates/headroom-core/src/transforms/`) | What Anima takes | To (new crate `crates/shortening/src/`) | Size in Anima (estimate, code lines) |
| --- | --- | --- | --- |
| `smart_crusher/error_keywords.rs` | The 12 error words | `rows/error_words.rs` | 30 |
| `smart_crusher/classifier.rs` | The kind of a list: objects, strings, numbers, mixed | `rows/kind.rs` | 120 |
| `smart_crusher/outliers.rs` | A row with an error word; a row with a rare field (in fewer than 20 % of rows); a row with a rare value of a common field | `rows/rare.rs` | 280 |
| `smart_crusher/stats_math.rs`, parts of `analyzer.rs` | Mean, deviation; a number more than 2 deviations from the mean; a change point | `rows/numbers.rs` | 220 |
| `smart_crusher/crushers.rs` (`compute_k_split`), `orchestration.rs` | The first rows and the last rows (30 % and 15 % of the kept number), equal rows removed, the rest by an even step; each error, rare and far row is kept above the limit | `rows/choice.rs` | 280 |
| `smart_crusher/config.rs` | The numbers: lists from 5 rows, 15 rows kept, the fractions | `rows/limits.rs` | 60 |
| `smart_crusher/crusher.rs` (the walk) | The walk through objects to the lists inside, depth 50; an answer that is not changed comes back byte for byte | `rows.rs` | 200 |
| `log_compressor.rs` (`LevelClassifier`, `normalize_for_dedupe`, the stack trace rules) | The level of a line by whole words; equal lines after numbers, addresses and paths are replaced; the lines of a stack trace stay with their error | `lines/level.rs`, `lines/same.rs`, `lines/trace.rs` | 450 |
| `pipeline/reformats/log_template.rs` | A run of lines of one shape becomes one line with a count | `lines/shape.rs` | 220 |

Total: about 1,900 code lines and their tests, against about 16,000 lines of the same parts in
Headroom. The crate needs `serde_json`, `sha2` (both in the workspace) and `regex`.

### 3.2 Written new (Headroom has it in Python, or in a form Anima cannot use)

| Part | Why new | Size (estimate) |
| --- | --- | --- |
| The mark in a shortened answer (SHA-004) | Headroom writes `<<ccr:hash 40_rows_offloaded>>` in a row. Anima's answers are JSON objects for an agent: the mark is a field with plain words. | 80 |
| The store of originals (SHA-010 to SHA-014) | Headroom's stores are a map in memory and SQLite, with a time limit that each read starts again and no size limit on disk. Anima needs files of the user with both limits. | 250 |
| The command and the tool for an original (SHA-015, SHA-016) | Headroom's `headroom_retrieve` is Python. | 200 |
| The measure (SHA-018) | Headroom counts with its tokenizers and sends the numbers to its beacon. Anima keeps a small local record. | 150 |
| The gate "too small to shorten" | Python only in Headroom (200 tokens). | 20 |

### 3.3 Left out

| Part | Reason |
| --- | --- |
| Kompress, the embedding model, Magika (ML) | Decided: heavy, and a small gain. |
| The beacon, the usage poll with the user's sign-in, the dashboard | Decided: nothing is sent. |
| LiteLLM, Bedrock, Vertex, the Python proxy | No Python; Anima does not call a model. |
| TOIN (the learning store), the relevance score (BM25) | They need the question of the person. An MCP tool answer has none. Part 2 can add BM25 later. |
| `adaptive_sizer.rs` (how many rows by a zlib and simhash test) | The fixed number 15 is predictable. Measure first (OD-SHA5). |
| The lossless table form (`compaction/`, 3,153 lines) | It writes a CSV text in place of JSON. A later task can add a table of columns, which loses nothing. |
| The code, diff and search shorteners (tree-sitter) | Not asked. No MCP tool of Anima returns source code. |
| CacheAligner | It changes nothing in Headroom now. |

## 4. Requirements

The rules that keep it safe:

- **SHA-001.** A row or a line that holds an error word, a rare field, a rare value or a number
  far from the mean is never left out. If such rows are more than the limit, the answer is
  longer than the limit. This is stricter than Headroom for logs (Section 2).
- **SHA-002.** The shortener never changes the message of a person, a system prompt, the name
  or the arguments of a tool call, or an error result (`isError`). In Part 1 it sees only the
  successful answers of Anima's own tools.
- **SHA-003.** The shortener leaves out whole rows and whole lines. It never changes a value
  that it keeps, never changes the order, and never removes or renames a field of an object.
- **SHA-004.** Each shortened answer says so, in the answer: a field `shortened` with how many
  rows or lines of which list were left out, by which rule each kept group was kept, the handle
  of the original, and the name of the tool that gets it. A shortened list is never given as if
  it were whole.
- **SHA-005.** The same answer with the same settings gives the same shortened answer, byte for
  byte: no clock, no random number, no order of a hash map.
- **SHA-006.** A fault in the shortener or in the store gives the answer unchanged (after
  MCP-008, as today). An answer is shortened only when its original is stored first.
- **SHA-007.** An answer below the gate (default 2,000 bytes of JSON, a list below 5 rows) is
  not touched. A change that saves less than 15 % is not made.
- **SHA-008.** The shortener runs after MCP-008 (`mcp/untrusted.rs`): the stored original is
  the cleaned answer. The tool for an original cannot give text that MCP-008 would have
  removed. The limit of 60,000 bytes (`fitted`) stays as the last limit.
- **SHA-009.** The shortener crate is pure: no file, no network, no clock, no log.

The store of originals:

- **SHA-010.** Originals are files of the user on the computer that runs `anima mcp`, in a
  folder with mode 0700, each file 0600. Nothing of the store is sent anywhere.
- **SHA-011.** The handle is the first 24 hexadecimal characters of the SHA-256 of the
  original. A handle that is not 24 such characters is refused before the disk is read.
- **SHA-012.** The store holds at most 64 MB and no original older than 24 hours (OD-SHA4).
  The oldest go first. One original is at most 4 MB; a larger answer is not shortened.
- **SHA-013.** The store holds no token and no secret by its design: an MCP tool of Anima
  returns none (MCP-004). The record of the measure holds the tool name, two sizes and a time,
  no content.
- **SHA-014.** A handle that is gone gives a sentence that says so and says to call the first
  tool again.

The ways to use it (CMD-030):

- **SHA-015.** `anima mcp original <handle>` prints an original. `anima mcp originals` lists
  the handles with tool, size and age; `anima mcp originals clear` removes them.
- **SHA-016.** The MCP tool `anima_original_get` (read, each tool set) takes `handle` and an
  optional `from` (a row or line number) and gives the original in parts that fit the limit of
  SHA-008. Its own answer is never shortened.
- **SHA-017.** A setting turns the shortening off: `anima mcp shortening off` (saved, with
  `on` and `show`) and the flag `anima mcp --no-shortening` for one run. Default: on (OD-SHA3).
- **SHA-018.** `anima mcp shortening measure` prints, for each tool, the number of answers, the
  size before and after, and the estimate of tokens before and after. The estimate is bytes
  divided by 4 and the command says that it is an estimate (OD-SHA6).
- **SHA-019.** The graphical client gets no screen in Part 1. The settings panel of the MCP
  server ([cli-mcp-plan.md](../plans/cli-mcp-plan.md), "Settings panel") gets the switch in a
  later task, after the measure.

The licence:

- **SHA-020.** Each file with code from Headroom starts with: "Parts of this file are from
  Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025 Headroom Contributors,
  licensed under the Apache License, Version 2.0. Changed by Anima.ing, 2026: " and one line
  that says what changed, with the path of the source file.
- **SHA-021.** The crate holds `LICENSE-APACHE` (the full text) and `NOTICE` (the notice of
  Headroom and the list of the source files). Anima is AGPL-3.0; code under Apache-2.0 can be
  part of it, the other direction is not possible.
- **SHA-022.** The commit that adds the code names the commit of Headroom that it was read at.

Tests and proof:

- **SHA-023.** The tests of the crate use real answers of Anima's MCP tools, recorded and made
  anonymous (OD-SHA2). A property test proves SHA-001, SHA-003 and SHA-005 on built lists.
- **SHA-024.** Part 1 merges only with a table of the measured saving on at least five real
  tool answers and a review by a session that did not write it.

## 5. Where it goes

- `crates/shortening/` (new): `rows` (lists in JSON), `lines` (log lines), `mark` (SHA-004).
  Pure (SHA-009).
- `crates/anima-cli/src/mcp/shortening/` (new folder): the store (the only code that touches
  its files), the setting, the measure, the commands. `mcp/server.rs` calls it between
  `cleaned_result` and `fitted`.
- `crates/anima-cli/src/mcp/tools/`: the tool `anima_original_get`.

## 6. Tasks

| Task | What | State |
| --- | --- | --- |
| SA0 | This proposal, its rows and the ruling. | Done (this commit). |
| SA1 | The crate `shortening`: `rows`, `lines`, `mark`, with tests on built lists (SHA-001 to SHA-009, SHA-020 to SHA-022). | Not started. |
| SA2 | The store, the setting, the commands and the tool in `anima-cli`; the call in `mcp/server.rs` (SHA-010 to SHA-018). | Not started. |
| SA3 | Real answers recorded and made anonymous; the table of the saving; the review (SHA-023, SHA-024). | Not started. Waits for OD-SHA2. |
| SA4 | The pages of the user documentation and the reference of the new command and tool. | Not started. |
| SA5 | The design note of Part 2 (Section 7), with a threat list. No code. | Not started. |

## 7. Part 2, the direction only

Facts that the design note (SA5) must respect:

- Anima does not call the model. Claude Code and Codex do. Part 2 can only be a proxy on the
  computer, in the daemon, on their path to the provider. Each prompt and the sign-in of the
  provider pass through it.
- The thread runner starts each program with an empty environment and a list that it names
  (`crates/daemon/src/threads/program.rs`). So a thread of Anima, and only it, can get
  `ANTHROPIC_BASE_URL` or the `openai_base_url` of Codex with a secret of that thread. The
  runner session owns that code; SA5 names what it needs.
- Headroom's own notes: a Codex with a ChatGPT sign-in goes to `chatgpt.com/backend-api/codex`
  whatever provider is configured, and "the exact ChatGPT backend path is not confirmed";
  Claude Code loses Remote Control behind a different base address and stops deferring tool
  schemas unless `ENABLE_TOOL_SEARCH` is set. Subscription sign-ins through a proxy are not
  tested by Anima.
- The provider's prompt cache holds only when the bytes before a cache mark stay the same.
  Headroom changes only the newest tool results, after the last cache mark, by cutting bytes in
  place. Claude Code usually puts a cache mark on the last message; then nothing is left to
  shorten in a request, and the shortener must act on a tool result when it first appears and
  give the same bytes in each later request. SA5 must measure this on a real thread.
- Off by default until measured. A fault sends the request unchanged.

## 8. Open decisions

| Number | Decision | Recommendation | Result |
| --- | --- | --- | --- |
| OD-SHA1 | "Copy" as the rules, numbers and lists of Headroom in small files of Anima (about 1,900 lines), or the files of Headroom as they are (about 16,000 lines, needs 12 entries of the quality baseline, which cannot be added by hand) | The rules in small files. The notices stay the same. | Open. |
| OD-SHA2 | The real answers for the tests: (A) read-only calls to the operator's own hub, made anonymous (names, addresses, identifiers replaced) before the commit to this public repository; (B) a local hub with seeded records | A. B is not "real". The operator must allow the read of his hub. | Open. |
| OD-SHA3 | Shortening of MCP answers on or off by default | On, after SA3 shows the table. Until then off in `dev`. | Open. |
| OD-SHA4 | The limits of the store: 64 MB and 24 hours, or different numbers | 64 MB and 24 hours. | Open. |
| OD-SHA5 | The number of rows kept: fixed (15, plus each row of SHA-001), or Headroom's adaptive number | Fixed first. Look again with the measure of SA3. | Open. |
| OD-SHA6 | The count of tokens: an estimate (bytes divided by 4), or a tokenizer crate (large, and not the tokenizer of Claude) | The estimate, named as an estimate. | Open. |
