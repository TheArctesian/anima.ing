# Record: the isolation of a job, with a prototype on one Mac and one Linux computer (task W2)

Version: 2. Date: 2026-11-23. Version 1 (2026-11-18) measured the Mac; version 2 adds the Linux computer `pop-os` (Section 3.5).
Status: Partly measured. One Mac and the Linux computer `pop-os` are measured. No second user of macOS was made, no network rule for a job was tried and no virtual machine for each job was measured: Section 5 says what each needs. Two separate reviewers accepted the design in Section 4 with conditions, and the operator delegated the decision to the session (R-NEW-w2isolation). Section 5 gives the remaining measurements for W4 and S.

This document is a record (work plan, Section 9; PLN-010). It adds no requirement. Its Section 4 is the input for the owner of the requirements, [projects-and-builds-plan.md](../../plans/projects-and-builds-plan.md) (PRO-022 to PRO-024, PRO-029), and for the contract [work-route.md](../../../contracts/work-route.md).

## 1. Purpose

Task W2 asks for a design with measured results for four subjects: the container runtime on Linux, a workflow that needs Docker, the build user on a Mac, and a signing identity for one job. The code of a job is not trusted (PRO-029): it can be the code of a pull request from a fork (fact F20 of the plan).

## 2. Method

- The computer: a Mac with Apple Silicon, 18 processor cores, 48 GiB of memory, macOS 27.3, Docker Desktop 29.9.3 (a Linux virtual machine with the kernel 7.0.14-linuxkit).
- The Linux computer (Section 3.5): `pop-os`, Pop!_OS 24.04, kernel 7.2.15, x86_64, 28 processor cores, 94 GiB of memory, Podman 4.9.3 with no root right (cgroup v2, systemd, runc, overlay, netavark with pasta). The load average was about 2 at the start and 0.6 at the end.
- The scripts are in this folder: `seatbelt.sh`, `job-keychain.sh`, `container-job.sh`, `docker-in-job.sh`, and `linux-job.sh` for the Linux computer. They are throwaway code of a prototype, not code of the product; nothing of the product calls them. Their logs are in [results/](results/). In the logs, the home folder of the user is written as `<home>` and the name of the user as `<user>`.
- The computer did other work at the same time: the load average was between 18 and 45 for Sections 3.1 to 3.3, and 7 for Section 3.4. Each time below was taken under that load. Read a time as an upper value, not as the time of a quiet machine.
- No administrator right was used. Thus no user was made, no system unit was installed and no system setting was changed. No certificate of Apple and no key of the operator was read: the signing test made its own throwaway certificate.
- On `pop-os` a session ran `linux-job.sh` on 2026-11-23, as the steps of HT-065 say, as the user of the operator. Podman was already installed. Podman had a stored login for Docker Hub that Docker Hub refused, so the session gave Podman an empty login file for the run (`REGISTRY_AUTH_FILE`) and pulled the images with no login; it did not read or change the stored login. Three results of the script were not clear; the session measured them again by hand, with the same flags (Section 3.5, "Measured again").

## 3. Results

### 3.1 A native job on a Mac with a sandbox profile and no second user (`seatbelt.sh`)

The job ran with `sandbox-exec` and a profile that permits each thing by default, denies the reading of the home folder and of a stand-in for the daemon token, and denies each write outside the job folder.

| Attempt of the job | Result |
| --- | --- |
| List the home folder, list `~/.ssh`, read the file of the login keychain, read the stand-in daemon token | Denied |
| Write to the home folder, to `/tmp`, to the stand-in folder of the daemon | Denied |
| Read and write in the job folder; start a program from it | Permitted |
| `git` with `HOME` set to the job folder; `cc` and `swiftc` of a small program; an ad hoc `codesign` | Worked. `git` with the usual `HOME` failed, because it reads `~/.gitconfig` |
| `curl` to a public address | Permitted |
| List the processes of the computer (`ps`) | Denied |
| `security find-identity` | Ran. It found 0 identities; outside the sandbox the same command found 6 |
| `security find-generic-password` for an item that does not exist | **The keychain service answered** ("could not be found"). Thus the job can ask the keychain service of the user. It still answered with the service names of the keychain denied in the profile, and with each service lookup denied. The test read no real item on purpose, so it does not show if a real item can be read |
| Connect to the Docker socket of the computer | **Worked.** The file of the socket is in the home folder, but a connection is not a file read |
| Start overhead of `sandbox-exec`, `/usr/bin/true`, 20 runs | Median 8.6 ms with it, 2.5 ms without it |

`man sandbox-exec` marks the program "DEPRECATED".

What this shows: a profile on the user of the operator hides files. It does not hide the services of that user. The Docker socket of the computer stayed usable, and the keychain service still answered. A job with the Docker socket controls the computer. A profile alone thus does not meet PRO-022.

### 3.2 A signing identity for one job (`job-keychain.sh`)

| Step | Time | Result |
| --- | --- | --- |
| Make a keychain file in a temporary folder, with a random password | 12 ms | The search list of the user did not change |
| Import a key and a certificate for `codesign` only; set the partition list | 31 ms, 15 ms, 14 ms | No window asked a question; no command waited |
| `codesign --keychain <file> -s <name>` | 38 ms | Worked with the keychain **not** in the search list |
| `codesign --verify` | 15 ms | "valid on disk" |
| `security delete-keychain`, remove the folder | 10 ms | The file is gone. `codesign` with that name then fails: "no identity found" |
| The search list and the default keychain, before and after | — | The same |

A limit of the tool: the password of the keychain is an argument of `security`, so a process of the same user can read it from the list of processes while the command runs.

Not shown: a Developer ID identity and the notarization from such a keychain. The test used a certificate that it made itself.

### 3.3 A container job (`container-job.sh`)

Each job: a new container of `alpine:3` with `--rm --cap-drop ALL --security-opt no-new-privileges --pids-limit 512 --memory 2g --read-only`, a temporary file system as its work folder, a user that is not root, one new empty folder of the host, and no Docker socket.

| Measure | Result |
| --- | --- |
| From `docker run` to the end of the first command, 10 runs | Median 349 ms (191 to 594) |
| Memory of an idle container | 2.5 MiB |
| Inside: the user, the capabilities, new privileges | uid 1000; no capability; `NoNewPrivs: 1` |
| Inside: a Docker socket; a write to `/` | No socket; "Read-only file system" |
| Host paths inside | The one job folder, and three files that Docker gives (`hostname`, `hosts`, `resolv.conf`) |
| After the end, and after `docker kill` in the middle of a `sleep` | The container and the job folder are gone; the removal took 184 ms |

Not shown: this is Docker Desktop, where each container is already inside a virtual machine. A Linux computer has no such second wall. The network of the job was not limited in this test.

### 3.4 A job that needs Docker (`docker-in-job.sh`)

A second Docker service ran beside the job, on a network of its own. The job reached it over TLS and had no socket of the host. The run of the log is the last one, made when the load average of the computer was 7; two earlier runs under a high load and one run that was stopped are not kept.

| Image of the service | Flags of the service | Result |
| --- | --- | --- |
| `docker:dind` | Seven sets with no `--privileged`: none; no seccomp and no AppArmor profile; `/dev/net/tun`; `SYS_ADMIN` and `NET_ADMIN`; and their sums | The service did not start: "mount: permission denied" |
| `docker:dind` | `--privileged` | The service answered after 1.3 s. Idle: 30 MiB, 15 processes. The job ran `hello-world` in 1.8 s, with the pull |
| `docker:dind-rootless` | None; no seccomp and no AppArmor profile; `/dev/net/tun` alone | The service did not start |
| `docker:dind-rootless` | No seccomp profile and `/dev/net/tun`; the same with no AppArmor profile | The service answered after 1.0 to 1.8 s. The job could not start a container in it: the inner runtime could not mount `proc` |
| `docker:dind-rootless` | No seccomp profile, no AppArmor profile, `systempaths=unconfined`, `/dev/net/tun`. **Not privileged**; the service runs as a user that is not root | The service answered after 0.9 s. Idle: 60 MiB, 38 processes. The job ran `hello-world` in 1.7 s, with the pull, and a container that stays |

In each case that worked: the job had no socket of the host; the inner containers were not in the list of the host; after the removal of the service no inner process and no volume stayed (about 150 ms); the lists of containers, networks and volumes of the host were the same before and after.

The size of the two images: 106 MB and 122 MB to pull, 421 MB and 470 MB on disk.

What this shows: a job can get a Docker service of its own with no privileged container. The rootless service needs three protections of its container set off (the seccomp profile, the AppArmor profile, the masked system paths) and the device `/dev/net/tun`. That is less than `--privileged`: the service has no added capability and does not run as root. It is still a weaker wall than the container of a job with no Docker (Section 3.3), and it was measured only inside the virtual machine of Docker Desktop.

### 3.5 Linux: a rootless container job on `pop-os` (`linux-job.sh`)

The log is [results/linux-job-pop-os-24.04.log](results/linux-job-pop-os-24.04.log). Each job had the flags of Section 3.3 under Podman with no root right: `--cap-drop ALL --security-opt no-new-privileges --pids-limit 512 --memory 2g --read-only`, a work folder `/work` on a temporary file system of 64 MiB, the user 1000 inside, and a label to find it. The user of the operator has the controllers `cpu memory pids` from systemd.

| Measure | Result |
| --- | --- |
| E1: from `podman run` to the end of the first command, 10 runs | Median 446 ms (417 to 473) |
| E2: inside: the user, the capabilities, new privileges | The user of the host (uid 1000, with `--userns keep-id`); no capability (`CapPrm`, `CapEff` and `CapBnd` 0); `NoNewPrivs: 1` |
| E2: a write to `/`; to `/work`; to the folder of the job | "Read-only file system"; permitted; permitted. A file that the job wrote in its folder belongs to the user on the host |
| E2: a container socket inside; the limits that the job reads | No socket; `memory.max` 2147483648, `pids.max` 512 |
| E3: a job that fills the memory (limit 256 MiB) | Stopped by the kernel (exit 137) |
| E3: a job that fills its work folder (limit 64 MiB) | "No space left on device" after 67108864 bytes |
| E3: a job that starts 300 processes (limit 64) | The limit holds. The log shows "1 1", because the shell of the job ends at its first refused start. Measured again: of 300 starts 62 ran, and `pids.current` was 64, the same as `pids.max` |
| E3: a job past its time: `timeout 5` around `podman run`, the job `sleep 60` | **The job was not stopped at 5 s.** `podman run` ended after 60.4 s (exit 124). Measured again: the signal of `timeout` goes to the program of the job, which is process 1 in its container and has no handler, so it ignores the signal. With `--init` the same job ended after 5.2 s. No container stayed in both cases |
| E4: the public internet (`example.com`) | Reached |
| E4: a server on the computer, bound to each address: by `10.0.2.2`; by the name `host.containers.internal`; by the address of the computer on its network; by its address of the link or VPN (100.64.0.0/10) | Not reached; **reached**; **reached**; **reached** |
| E4: the router of the network, port 80 | Not reached |
| E4: `--network none`, the public internet | Not reached |
| E5: an idle container | 53 kB of memory, 1 process |
| E5: `kill -9` of `conmon`, the program of Podman that holds the job | **The process of the job still ran**, and Podman still said `running`. `podman rm -f` then took 10.2 s, and after it the process was gone |
| E6: the rootless Docker service (`docker:dind-rootless`) in a rootless container, with the four sets of flags of the script: none; no seccomp profile and `/dev/net/tun`; the same and `unmask=ALL`; `--privileged` | **The service did not start with any set.** Measured again: its log says `newuidmap: write to uid_map failed: Operation not permitted`. The user of the operator has 65536 subordinate IDs (`/etc/subuid` and `/etc/subgid`: 100000, 65536), so the outer container has 65537 IDs. The rootless service in it asks for a second range of 65536 IDs, and the outer container cannot give it. `--privileged` does not change that |
| Clean-up | No container of the script stayed; the list of containers was the same before and after. The images `alpine:3` (17 MB) and `docker:dind-rootless` (355 MB) stay |

What this shows:

- On a Linux computer with no virtual machine around the containers, a rootless container with the flags of Section 3.3 holds its limits of memory, disk and processes. Its start costs about 0.45 s and its idle memory is very small.
- **With no network rule, a job reaches the services of the computer.** The default network of Podman (pasta) gives the job the name `host.containers.internal` and lets it connect to the address of the computer on its network and on the link or VPN. The rule of Section 4.1 is thus necessary, not only a second layer. A service of the computer that listens only on its loopback address was not reached by `10.0.2.2`; by the name `host.containers.internal` the server of the test was reached, but it listened on each address, so the test does not show if a loopback-only service is reached by that name.
- A limit of time with `timeout` around the client is not a limit: the job must be stopped by its container (`podman stop` or `podman kill`, or `--init` so that process 1 handles the signal).
- The program that holds a job (`conmon`) is not the job: when it ends, the job runs on. The daemon must stop the container itself and must find a container that it lost at its next start (by its label). The script did not measure `kill -9` of the daemon.
- A Docker service of its own for a job does not work on this computer as it is: rootless in rootless needs more subordinate IDs for the user of the jobs than the 65536 that a new user gets.

## 4. The design that these results support (a proposal)

### 4.1 Linux: the container runtime

- One job, one new container, made by a runtime that has no root right: Podman without root, or Docker in its rootless mode, under a system user that exists only for jobs. That user is not the user of `anima-daemon`, has no login and owns nothing else.
- The flags of Section 3.3 are the floor: no capability, no new privileges, a read-only root, a work folder with a size limit, limits for processes, memory and time, a user that is not root inside.
- The network of a job: the public internet only. No loopback address of the computer, no interface of the link or of the VPN, no private address range, and not the origin of the hub. This closes the path of finding SF-004 for a container job and gives a job no way to the hub (contract K10, Section 6.3).
- The job folder is the only path of the computer in the job. It is made new and deleted at each end, also at the start of the daemon after a stop.
- For a repository that is public and takes pull requests from forks, a container on a shared kernel is one wall. The proposal for the security reviewer: such a repository gets a small virtual machine for each job, not a container. Not measured.

To measure on the Linux computer: the start time and the memory of a rootless container; that the limits hold (a job that fills the memory, the disk, the process table); that the network rule holds; what stays after `kill -9` of the daemon in the middle of a job.

Note of 2026-11-23, after the measurements of Section 3.5: the start time, the memory and the limits of memory, disk and processes are measured and hold. They change three points of this section, for the owner of the requirements to decide: the network rule is necessary, because with no rule a job reached the computer by its address on the network, on the link and by `host.containers.internal`; the limit of time must stop the container, not the client (`--init`, or `podman stop` or `kill` at the deadline); the daemon must remove the container of a job itself, because a job runs on after the end of `conmon`. Still to measure: that a network rule holds, and `kill -9` of the daemon in the middle of a job.

### 4.2 A workflow that needs Docker

- The Docker socket of the computer never goes into a job (the open point of the plan). The contract has no member that could ask for it.
- First version: a job has no Docker. A workflow with a `container:` job or a Docker action fails with a sentence that says so. The set-up says it before the press, and the text to copy tells the coding agent to stop (PRO-058).
- Later: a job that needs Docker gets a Docker service of its own beside it, the rootless service of Section 3.4, on a network that only that job can reach, removed with the job. It costs about 1 second and 60 MiB for each job. A privileged service is never used.
- The security reviewer must judge the three protections that the rootless service needs set off. If they are not acceptable on a shared kernel, such a job gets a small virtual machine of its own in place of the service.
- To measure on the Linux computer: the same script with a runtime that has no root right, where there is no virtual machine around the service; and one small virtual machine for each job (start time, memory, disk).
- Note of 2026-11-23, after Section 3.5: on `pop-os` the rootless service did not start in a rootless container with any flags, `--privileged` included, because the user of the jobs has only 65536 subordinate IDs. The user of the jobs of Section 4.1 would need a larger range in `/etc/subuid` and `/etc/subgid` (an administrator step), and the service must then be measured again. Until then, a small virtual machine for each such job is the other way.

### 4.3 The Mac: the build user

- A profile alone is not sufficient (Section 3.1). The job runs as a second user of macOS that is made for builds (PRO-022): a standard user, not an administrator, with a home folder of its own and an empty keychain. Then the keychain service and the Docker socket that the job reaches are those of the build user, not those of the operator.
- The profile of Section 3.1 stays as a second layer on the build user: it denies the home folder of the operator by path, and it limits the writes to the job folder. The file modes alone are not a sure wall: on this Mac the home folder has the mode `drwxr-x---`, so a user in its group can read it, and the usual group of a standard user of macOS is that group. The build user must thus not be in the group of the operator; to check with an administrator.
- An administrator makes the build user one time, and installs one small program that starts a job as that user. That program is owned by root, takes no command from the daemon, and starts only the fixed job starter with the folder of one job. This is the step that PRO-024 gives to the operator.
- At each end the machine ends each process of the build user, and deletes the job folder, the temporary folder of that user and the keychain of the job.
- One Mac has one build user, so it runs one native job at a time.
- For code of a fork on a public repository, the stronger wall is a virtual machine of macOS for each job. Not measured.

To measure with an administrator on a Mac: that the build user cannot read the home folder and the keychain of the operator, with and without the profile; that no process of the build user stays after a stop; the time to make and to delete the job folder with a real checkout; that Xcode builds as that user.

### 4.4 A signing identity for one job

Two different needs have two different answers.

- Note of 2026-11-18, after this section was written: R-173 says that the job of the release workflow signs the updater file with the updater key as a secret of GitHub. The paragraph below was written for a hub that signs. If the same job also signs the Mac bundle with the Apple secrets of GitHub (R-116) is open (OD-W36 of the work plan). The reason below still applies and is now a reason for the measures of OD-W36.
- The release of Anima. No job gets the Developer ID identity. A warm runner takes the next job that names its labels, and the hub does not select that job (contract K10, Section 6.2), so an identity inside a runner job could reach a job of a fork. The proposal: the job builds a bundle with no signature and gives it to the hub that checks it (OD-W31, choice A). The signature of the Mac is then a step of its own, outside each job: a fixed recipe of the daemon on the Mac that has the identity in its keychain (R-116, BLD-083 to BLD-085). It signs, notarizes and staples the bytes whose SHA-256 the hub gives, and it runs no code of the repository. R-116 does not change: the identity stays in the keychain of that Mac. A second way, for a later decision: the hub that signs also signs the Mac bundle. That moves the identity to a server and changes R-116.
- The project of a different repository, whose workflow signs by itself. The owner of that repository gives the identity as a secret of GitHub, and the workflow imports it. The keychain of Section 3.2 is the form that the machine supports: a file in the job folder, never in the search list, deleted with the job. Anima gives such a job no identity (PRO-030).

K10 version 0.3 (R-209) and later have no secret purpose `signing_identity`. The accepted design does not add one.

## 5. What is not shown, and what it needs

| Not shown | Needs |
| --- | --- |
| A network rule for a job on Linux, and that it holds | The machine `pop-os` (R-188, R-264). Section 3.5 measured what a job reaches with no rule; no rule was tried |
| `kill -9` of the daemon in the middle of a job on Linux | The machine `pop-os`, with a daemon that starts jobs. Section 3.5 measured only `kill -9` of `conmon` |
| A rootless Docker service in a rootless container on Linux | An administrator on `pop-os`: a larger range of subordinate IDs for the user of the jobs (Section 3.5, E6). Then the same script again |
| The build user (Section 4.3) | An administrator on a Mac, one time: make the user and the starter program. Then an agent can measure |
| A virtual machine for each job, on Linux and on a Mac | The same two computers |
| A Developer ID signature and a notarization by the recipe of Section 4.4 | The operator: the identity and the notary profile are in his keychain. An agent does not read them |
| The times of Sections 3.1 to 3.3 on a quiet machine | A second run of those three scripts. Section 3.4 was measured again at a load average of 7 |
| A list of the build tools of this Mac with their versions | Not made: the measurements stopped before it |
| The network limits of a job | Linux; on a Mac, a rule of the firewall needs an administrator |

## 6. For the acceptance

The original proposed acceptance sentence is below. R-NEW-w2isolation records the actual decision and its limits.

> I accept the isolation design of task W2 as Section 4 of `docs/research/work-isolation/w2-isolation.md` proposes it, with the measurements of Section 5 still to do on my Linux computer and with an administrator on my Mac.

Before that sentence, the requirements that Section 4 would add (the network rule of a job, no Docker in the first version, the sign recipe) go into [projects-and-builds-plan.md](../../plans/projects-and-builds-plan.md) with identifiers, as a change of that plan.
