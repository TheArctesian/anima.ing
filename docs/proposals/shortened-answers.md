---
kind: Proposal
state: Draft
evidence: Version 1.1 (2026-11-22). R-NEW-shortener (operator, 2026-11-22, relayed) names the port and the tasks SA1 to SA4 of Part 1. SA0 is done. SA1 and SA2 are built on the branch `shortener` (the crate `shortening`, and the store, the setting, the measure, the commands and the tool in `anima-cli`), off by default. SA3 waits for real answers: the `anima` command of the build computer is not signed in. R-NEW-relay (operator, relayed) names task SA6: the relay of Part 2 in pass-through only; no shortening in the relay has a ruling. SHA-001 to SHA-024 are not approved one by one.
---

# Proposal: shortened answers (the port of the Headroom shorteners)

Progress: the plan register in [work-plan.md](../work-plan.md) Section 9 holds the state of this proposal (PLN-005).

From: a side session of the orchestrating session, 2026-11-22.
Version: 1.1. Date: 2026-11-22.
Status: the header of this document holds the state (PLN-020). History: Draft. Version 1.1: OD-SHA1 to OD-SHA6 answered (relayed), and the text agrees with the build.
Map: [work-plan.md](../work-plan.md). This proposal serves row 4.24 of [priorities.md](../plan/priorities.md) (agent threads, the page Code): the answers that an agent reads cost less of its context. Part 1 changes the MCP server of [cli-mcp-plan.md](../plans/cli-mcp-plan.md) (MCP-008 stays as it is). Part 2 touches the thread runner of [agent-threads.md](agent-threads.md); this proposal names what it needs from the runner and changes no file of it.
Identifiers: SHA-001 to SHA-024. Tasks: SA0 to SA6. Open decisions: OD-SHA1 to OD-SHA9.

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
| `smart_crusher/error_keywords.rs` | The 12 error words | `rows/error_words.rs` | 60 |
| `smart_crusher/classifier.rs` | The kind of a list: objects or texts; numbers and mixed lists are not touched | `rows.rs` | 20 |
| `smart_crusher/outliers.rs` | A row with an error word; a row with a rare field (in fewer than 20 % of rows); a row with a rare value of a common field. Changed: a value is rare only below 10 % of the rows, and the name of a field counts as an error word only when the field holds something | `rows/error_words.rs`, `rows/rare.rs` | 180 |
| `smart_crusher/stats_math.rs`, parts of `analyzer.rs` | Mean, deviation; a number more than 2 deviations from the mean. Not taken: the change points | `rows/numbers.rs` | 60 |
| `smart_crusher/crushers.rs` (`compute_k_split`), `orchestration.rs` | The first rows and the last rows (30 % and 15 % of the kept number), equal rows removed, the rest by an even step; each error, rare and far row is kept above the limit | `choice.rs` | 110 |
| `smart_crusher/config.rs` | The numbers: lists from 5 rows, 15 rows kept, the fractions | `limits.rs` | 40 |
| `smart_crusher/crusher.rs` (the walk) | The walk through objects to the lists inside, depth 50; an answer that is not changed comes back byte for byte | `rows.rs` | 110 |
| `log_compressor.rs` (`LevelClassifier`, `normalize_for_dedupe`, the stack trace rules) | The level of a line by whole words; equal lines after numbers, addresses and paths are replaced; the lines of a stack trace stay with their error | `lines.rs`, `lines/level.rs`, `lines/same.rs`, `lines/trace.rs` | 130 |

Built: about 600 code lines and 14 tests, against about 16,000 lines of the same parts in
Headroom (the first estimate was 1,900). The crate needs only `serde_json`: the three regular
expressions of Headroom are one small scan. The template of log lines
(`pipeline/reformats/log_template.rs`) is not taken: the rule "one shape" does its work for a
list, and Part 2 can add it for plain text.

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
  The oldest go first. The limit counts the bytes of the files. One original is at most 1 MB
  (the limit of a private file of the command line); a larger answer is not shortened.
- **SHA-013.** The store holds no token and no secret by its design: an MCP tool of Anima
  returns none (MCP-004). The record of the measure holds the tool name, two sizes and a time,
  no content.
- **SHA-014.** A handle that is gone gives a sentence that says so and says to call the first
  tool again.

The ways to use it (CMD-030):

- **SHA-015.** `anima mcp shortening original <handle>` prints an original.
  `anima mcp shortening originals` lists the handles with size and age;
  `anima mcp shortening clear` removes them. `anima mcp shortening try <file>` shortens the
  answer in a file and prints the sizes; it stores nothing.
- **SHA-016.** The MCP tool `anima_original_get` takes `handle`, an optional `list` and an
  optional `from` (a row number) and gives the rows of one list in parts that fit the limit of
  SHA-008. Its own answer is never shortened. It is listed only when the shortening is on, in
  each tool set, and it has no saved permission: it gives only what a listed tool gave, to an
  agent that has the handle. A handle is the hash of the content, so it cannot be guessed
  without the content. The originals of one user are in one folder for each hub that the user
  uses: an agent with a handle of hub A can read it in a run for hub B (a point for the review).
- **SHA-017.** A setting turns the shortening off: `anima mcp shortening off` (saved, with
  `on` and `show`) and the flag `anima mcp --no-shortening` for one run. Default: off until SA3
  shows the table (OD-SHA3). A server that runs keeps the state of its start.
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
- `crates/anima-cli/src/shortening/` (new folder): the store (the only code that touches the
  files of the originals), the setting, the measure, the commands and the tool
  `anima_original_get`. `mcp/server.rs` calls it between `cleaned_result` and `fitted`.

## 6. Tasks

| Task | What | State |
| --- | --- | --- |
| SA0 | This proposal, its rows and the ruling. | Done (this commit). |
| SA1 | The crate `shortening`: `rows`, `lines`, `mark`, with tests on built lists (SHA-001 to SHA-009, SHA-020 to SHA-022). | Built on the branch `shortener` (commit 2c1aedfae); not in `dev`. |
| SA2 | The store, the setting, the commands and the tool in `anima-cli`; the call in `mcp/server.rs` (SHA-010 to SHA-018). | Built on the branch `shortener`; not in `dev`. Waits for the review of SHA-024. |
| SA3 | Real answers recorded and made anonymous; the table of the saving; the review (SHA-023, SHA-024). | In progress: the table is from built answers and says so. The real answers are a follow-up: the sign-in of the `anima` command does not work with SSO yet (operator, relayed, 2026-11-22). |
| SA4 | The pages of the user documentation and the reference of the new command and tool. | Not started. |
| SA5 | The design note of Part 2 (Section 7), with a threat list. No code. | Written (version 1.1). |
| SA6 | The relay in pass-through only (Section 7.12, first step; R-NEW-relay): the listener, the secret of each thread, the fixed upstreams, no redirect, the refusal of `Origin`, the test that no sign-in data reaches a log. No shortening. A branch of its own. | Not started. Starts after Part 1 is on a green branch. |

### 6.1 The measured saving (SA3): built answers, NOT real answers

These seven answers were built by a script with the field names of the tools (the GraphQL
documents of `anima-cli`), 4 % error rows, 6 % warning rows, and a page of 25, 100 or 10 rows.
They are not answers of a real hub: the `anima` command cannot sign in with SSO yet. The table
is here to show the method. The real table is a follow-up, and OD-SHA3 (on by default) waits
for it. Measured with `anima mcp shortening try <file> --json` on the branch `shortener`. Tokens
are an estimate: bytes of JSON divided by 4.

| Built answer (shape of the tool) | Rows | Bytes before | Bytes after | Tokens before (estimate) | Tokens after (estimate) | Saved |
| --- | --- | --- | --- | --- | --- | --- |
| Events, `anima_events_search`, limit 100 | 100 | 24,912 | 4,329 | 6,228 | 1,082 | 82 % |
| Events, the default page | 25 | 6,139 | 4,196 | 1,534 | 1,049 | 31 % |
| Audit trail, `anima_audit_list`, limit 100 | 100 | 29,561 | 9,918 | 7,390 | 2,479 | 66 % |
| Devices, `anima_devices_search`, limit 100 | 100 | 19,199 | 3,411 | 4,799 | 852 | 82 % |
| Devices, 10 rows | 10 | 1,945 | 1,945 | 486 | 486 | 0 % (below the gate, not touched) |
| Metric samples of one device | 100 | 10,823 | 2,179 | 2,705 | 544 | 79 % |
| Lines of a build log | 100 | 9,636 | 2,008 | 2,409 | 502 | 79 % |

What the table shows, and its limit: a page of Anima's tools has at most 100 rows, and 25 when
the agent names no limit. So the saving on a default page is small (about 500 tokens), and the
large numbers need a page of 100 rows. A real hub has more different rows than a built one,
so the real saving is expected to be lower.

## 7. Part 2: the design note of the relay (task SA5)

State: a design, for an answer of the orchestrating session and a ruling. No code of Part 2 exists.
The name in this note is "the relay": the part of the daemon that stands between an agent
program and its model provider.

### 7.1 What it is and where it runs

Anima does not call the model. Claude Code and Codex do. So the only place to shorten "before it
reaches the model" is on their path to the provider. The relay is an HTTP listener in
`anima-daemon`, in a module of its own (`crates/daemon/src/relay/`). It uses the crate
`shortening` of Part 1 and the same store of originals. The daemon has `axum`, `hyper` and
`reqwest` already.

Each prompt of a thread and the sign-in of the provider pass through the relay. This is the
price of the function, and the reason for each rule below.

### 7.2 How the agents are pointed at it, and only the threads of Anima

The thread runner starts each program with an empty environment and a list that it names
(`threads/program.rs`). So only a thread that Anima starts can get the address. A `claude` or a
`codex` that the person starts in a terminal never sees the relay.

- Claude Code: the environment variable `ANTHROPIC_BASE_URL`.
- Codex: the argument `-c openai_base_url="..."`.
- The address is `http://127.0.0.1:<port>/<secret>/`, where `<secret>` is 32 random bytes of
  this thread in hexadecimal. Not tested: that both programs keep a path before `/v1/...`. If
  one does not, its secret goes in a header (`ANTHROPIC_CUSTOM_HEADERS`; for Codex
  `model_providers.<name>.env_http_headers`).

Measured on 2026-11-22 on the build computer (Claude Code 2.1.311, Codex 0.170.0), with a
listener on 127.0.0.1 that answers 401 to each request, forwards nothing, and prints only the
method, the path and the names of the headers (no value of a header was read, printed or kept):

| Program and sign-in | What reached the local address |
| --- | --- |
| Claude Code, the sign-in of this computer (`authorization` with the word Bearer, no `x-api-key`: the subscription kind) | `HEAD /api/hello`, then `POST /v1/messages` with `anthropic-beta`, `anthropic-version`, `x-claude-code-session-id`; the body has `system`, `tools`, `messages`, `thinking`, `context_management`. |
| Codex, ChatGPT sign-in (the header `chatgpt-account-id` is present) | `GET /v1/models`; `GET /v1/responses` with `upgrade: websocket`; after the refusal, `POST /v1/responses` with `content-encoding` (a compressed body). |

So both programs, with a subscription sign-in, send their requests to a local address when they
are told to. Headroom's note that Codex with a ChatGPT sign-in ignores the address does not hold
for this version.

Not tested, and each is a check before the function is on for a person:

- That the provider accepts a request that the relay forwards. Nothing was forwarded: that
  would send the operator's sign-in through code that no review has read.
- The API key kind of both programs (no key was read or made).
- The renewal of a subscription token. It goes to a different host of the provider, not to the
  base address; the relay must not be in that path.
- What a program loses behind a different base address. Headroom reports that Claude Code loses
  Remote Control and stops deferring tool schemas unless `ENABLE_TOOL_SEARCH` is set.
- If the terms of a subscription allow a local relay that changes the request. This is a
  question for the operator, not a technical one.

### 7.3 The listener and the secret

- It binds `127.0.0.1` on a port that the system gives. Never `0.0.0.0`, never a name. A
  Unix socket is safer, but neither program can use one as a base address.
- A request whose path does not start with a live secret gets 404 and nothing is sent to a
  provider. The compare takes the same time for each input. A secret dies when its thread ends.
- The upstream is fixed in the code, by the program of the thread: `api.anthropic.com` for
  Claude Code; for Codex `api.openai.com`, or `chatgpt.com/backend-api/codex` when the request
  has the header `chatgpt-account-id`. No header and no part of the path can name a different
  host (Headroom has such a header, `X-Headroom-Base-Url`; Anima does not take it). The relay
  follows no redirect and checks the certificate against the public roots.
- So a different program on the computer cannot use the relay as an open way to a provider
  with the user's sign-in: it has no secret, and a relay request carries only the sign-in that
  the caller itself sent. The relay holds no sign-in of its own.
- A program of the same user can read the environment of the agent, and with it the secret.
  The same program can read the files where the agent keeps its sign-in. The relay gives it
  nothing new. A different user of the computer cannot read either.

### 7.4 Sign-in data

The headers `authorization`, `x-api-key`, `chatgpt-account-id` and each cookie pass through in
memory. They are never written to a file, a log, an event or a metric, and never kept after the
request. The log of the relay has the thread, the kind of the path, the sizes and the status. No
body is logged. The tests have a request with a marked sign-in and prove that the mark is in no
file and no log line (as LOG-007).

### 7.5 What is shortened, and what is never touched

- Shortened: only the result of a tool call (`tool_result` blocks of the Messages API;
  `function_call_output` items of the Responses API) that is a JSON list or a log of many
  lines, above the gate of SHA-007, and not marked as an error.
- Never touched (SHA-002): the system prompt, the text of the person, the text and the
  thinking of the model with its signatures, the tool definitions, the name and the arguments
  of a tool call, images, each cache mark, and each result that is an error.
- The change is made by cutting bytes in place: each byte outside the one result stays as the
  program wrote it (Headroom does the same). The body is not written again from parsed JSON.
- A shortened result ends with one line that says so, with the handle (SHA-004).
- The answer of the provider is passed on as it comes, as a stream, unchanged.

### 7.6 The prompt cache of the provider

The provider gives a low price for the start of a request that is byte-equal to an earlier one.
The agent sends the whole history each turn, with each old tool result in its long form. If the
relay shortened an old result in turn 5 but not in turn 4, the cache would miss from there on.

The rule: a result is shortened the first time the relay sees it, and each later request gets
the same bytes for it. SHA-005 gives this with no memory: the same result and the same limits
give the same bytes, also after the daemon starts again. Two more rules follow:

- The setting of a thread is fixed at its start. It never changes in a thread that runs.
- The limits of the shortener are stored with the thread. A new version of Anima with other
  limits does not apply to a thread that exists.

Not measured: cache hits with and without the relay on a real thread. It needs forwarding
(Section 7.2). The relay can measure it with no change to the stream: the answer of the
provider carries the counts (`cache_read_input_tokens`, `cache_creation_input_tokens`,
`input_tokens`), and the relay only reads them. The plan of the measure: the same task in two
threads, one with the relay in pass-through and one with shortening, three runs each; the
function stays off unless the tokens that are paid go down.

### 7.7 How the model gets an original

The originals are in the store of Part 1, files of the user. Three ways, in the order to build:

1. The mark names the command `anima mcp shortening original <handle>`. Each agent has a shell
   tool. No change to the request other than the shortened result. Recommended first.
2. If `anima mcp` is installed in the agent, the tool `anima_original_get` of Part 1.
3. A tool that the relay adds to the request and answers itself in the stream (the way of
   Headroom). Not recommended: it changes the tool list and the answer stream, and a fault
   there corrupts a turn.

The output of way 1 comes back through the relay as a tool result. It starts with a mark, and
the relay never shortens a result with that mark, so the model is never in a loop.

### 7.8 What a fault does (fail open)

- A body that does not parse, a kind of request that the relay does not know, a fault or a
  slow run (more than 50 ms) of the shortener, a full store: the request goes on unchanged,
  byte for byte. The relay never drops a request and never sends half of one.
- A body above 32 MB, a WebSocket and a compressed body that the relay cannot read are passed
  through as they are; they are not refused. (First version: the WebSocket of Codex is refused
  with the status that makes Codex use `POST`, as measured above. This must be tested.)
- A fault of the provider goes back to the agent as it is.
- One case cannot fail open: the daemon is not running, so nothing listens at the address. The
  programs live in the holder while the daemon starts again (THR-113). So the listener and the
  secrets must be back before the next request: the port and the secrets of the live threads
  are kept by the holder, in memory. This is the largest open point for the runner.

### 7.9 What the thread shows

- At the start of the thread: "Shortening is on for this thread: tool results pass a relay on
  this computer."
- With each turn: how many results were shortened, the bytes before and after, and the
  estimate of tokens, named as an estimate. From the counts of the provider: the tokens read
  from the cache.
- `anima code` and an MCP tool give the same numbers (CMD-030).
- Off by default, for each thread and for the whole computer, until the measure of 7.6 is done.

### 7.10 What is needed from the runner (its session owns `agent-threads` and `threads/`)

1. At the start of a thread with the setting on: a call to the relay that gives the address,
   and the address in the environment of Claude Code or the argument of Codex.
2. At the end of the thread: a call that ends the secret.
3. The setting of the thread in the start command, off when it is left out, fixed for the thread.
4. An event kind in the thread log for the lines of 7.9.
5. What wins when a folder also sets `ANTHROPIC_BASE_URL` in its settings (the notice of
   THR-046 already names such a folder): the thread must not start with the relay then.
6. The port and the secrets of live threads kept over a start of the daemon (7.8).

### 7.11 Threats

| Number | Threat | Answer |
| --- | --- | --- |
| T1 | A different program of the computer uses the relay to reach the provider | No secret, no relay (7.3). The relay has no sign-in of its own. |
| T2 | A web page in a browser calls `127.0.0.1` (DNS rebinding included) | The secret in the path; a request with an `Origin` header is refused; no CORS header is sent. |
| T3 | The relay is used to reach a host that is not a provider | The upstream is fixed in the code; no redirect (7.3). |
| T4 | The sign-in leaks to a file or a log | 7.4, with a test. |
| T5 | A prompt or a tool result leaks | No body is logged. The originals are private files with a limit of size and age (SHA-010 to SHA-012). A tool result can hold a secret that the agent read: the store of Part 2 holds what the store of Part 1 never holds. It needs the redaction of the crate `redaction` before a write, or a store in memory only. Open. |
| T6 | The shortener removes what the model needs, and the model acts on half of the facts | SHA-001 to SHA-004; each result says that it is shortened; the measure of 7.6 also compares the result of the task. |
| T7 | Text in a tool result is written to look like the mark, to make the model run a command | The mark is fixed text with a handle of 24 hexadecimal characters; the relay removes nothing else. A tool result is data for the model with or without the relay; the relay adds no new way in. |
| T8 | A fault of the relay stops each thread | 7.8; off by default; one switch turns it off for the computer, and new threads then start with no relay. |
| T9 | The relay breaks the cache and the cost goes up | 7.6; measured before it is on. |
| T10 | The provider changes its API, or refuses a relayed subscription request | A request of an unknown kind passes unchanged. If the provider refuses, the thread shows the fault of the provider and the setting says how to turn the relay off. |
| T11 | A different user of the computer | The listener is on loopback, so a different user can connect; without the secret the user gets 404. The store is mode 0700. |
| T12 | The relay as a place to change what the model reads (a compromised daemon) | The daemon already starts the agents and can change their arguments; the relay adds no new power. The review of the code that carries sign-in data is by a session that did not write it. |

### 7.12 Size and order

About 1,200 code lines: the listener and the secrets (250), the two request kinds with the cut
in place (400), the pass-through of streams (200), the text form of the shortener for a log
that is not JSON (150), the measure (200). Order: pass-through only, with the checks of 7.2;
then the measure of 7.6 with shortening on for the operator alone; then the ruling to offer it.

## 8. Open decisions

| Number | Decision | Recommendation | Result |
| --- | --- | --- | --- |
| OD-SHA1 | "Copy" as the rules, numbers and lists of Headroom in small files of Anima (about 1,900 lines), or the files of Headroom as they are (about 16,000 lines, needs 12 entries of the quality baseline, which cannot be added by hand) | The rules in small files. The notices stay the same. | Decided (R-NEW-shortener, relayed through the orchestrating session, 2026-11-22): the rules in small files, the notice in each file and in `NOTICE`. |
| OD-SHA2 | The real answers for the tests: (A) read-only calls to the operator's own hub, made anonymous (names, addresses, identifiers replaced) before the commit to this public repository; (B) a local hub with seeded records | A. B is not "real". The operator must allow the read of his hub. | Decided (operator, relayed, 2026-11-22): A. Read-only tools only; the raw answers stay out of the repository; what is committed is made anonymous and read by a second reader first; an answer that cannot be made anonymous gives only its sizes. |
| OD-SHA3 | Shortening of MCP answers on or off by default | On, after SA3 shows the table. Until then off in `dev`. | Decided (relayed, 2026-11-22): off in `dev` until the table exists. |
| OD-SHA4 | The limits of the store: 64 MB and 24 hours, or different numbers | 64 MB and 24 hours. | Decided (relayed, 2026-11-22): 64 MB and 24 hours, as named defaults. |
| OD-SHA7 | Part 2: build the relay in pass-through first (Section 7.12), to make the checks of Section 7.2 with a real sign-in | Yes, after a review of the pass-through code by a session that did not write it. | Decided (R-NEW-relay, operator, relayed, 2026-11-22): yes, with the conditions of the ruling. |
| OD-SHA8 | Part 2: may a relay on the computer change the requests of a subscription sign-in (the terms of the provider) | The operator reads the terms; until then only the API key kind is offered. | Open. |
| OD-SHA9 | Part 2: the originals of tool results can hold a secret that the agent read (T5): redact before the write, keep them in memory only, or store them as Part 1 does | In memory only, for the life of the thread. | Decided (the orchestrating session, 2026-11-22): in memory only, for the life of the thread. |
| OD-SHA5 | The number of rows kept: fixed (15, plus each row of SHA-001), or Headroom's adaptive number | Fixed first. Look again with the measure of SA3. | Open. |
| OD-SHA6 | The count of tokens: an estimate (bytes divided by 4), or a tokenizer crate (large, and not the tokenizer of Claude) | The estimate, named as an estimate. | Decided (relayed, 2026-11-22): the estimate, named as an estimate where it is shown. |
