+++
id = "docker-what-is-a-container"
title = "What a container really is"
module = "start"
minutes = 20
needs = "docker"
objectives = [
  "Explain a container as an isolated Linux process, not a small virtual machine",
  "Name the kernel features that make the isolation: namespaces and cgroups",
  "Tell an image (the template) from a container (a running instance)",
  "Run, list and remove your first containers",
]

[[quiz]]
q = "What is a container, at the operating-system level?"
choices = [
  "A lightweight virtual machine with its own kernel",
  "One or more ordinary processes isolated with kernel namespaces and limited with cgroups",
  "A compressed archive that the CPU executes directly",
  "A Docker-specific file format that only Docker can run",
]
answer = 1
why = "Containers share the host's kernel. Namespaces give the process its own view (PIDs, network, mounts, hostname), and cgroups cap the CPU and memory it may use. There is no second kernel, which is why containers start in milliseconds."

[[quiz]]
q = "You start three containers from the image `nginx:1.27-alpine`. How many copies of the image's files are stored on disk?"
choices = [
  "Three: one per container",
  "One: the image layers are shared read-only, each container only adds a thin writable layer",
  "None: images are streamed from the registry every time",
  "Four: the image plus one per container",
]
answer = 1
why = "Image layers are read-only and shared. Each container gets its own copy-on-write layer on top, so starting more containers costs almost no extra disk."

[[quiz]]
q = "Inside a container you run `ps` and see your process as PID 1. Why?"
choices = [
  "The container booted its own init system",
  "The PID namespace gives the container its own numbering, starting at 1",
  "Docker renames processes before showing them",
  "It is a bug in ps",
]
answer = 1
why = "In its own PID namespace the first process is PID 1. On the host the same process has an ordinary, larger PID. This also means PID 1 must handle signals properly, a theme you will meet again with Kubernetes."

[[quiz]]
q = "What happens to a container when its main process exits?"
choices = [
  "The container keeps running in the background",
  "Docker restarts it automatically",
  "The container stops (state `exited`), but it still exists until you remove it",
  "The container and its image are deleted",
]
answer = 2
why = "A container lives exactly as long as its main process. When it exits the container moves to `exited`; its writable layer stays until `docker rm`. Restarting only happens if you set a restart policy."

[lab]
summary = "Run your first containers on the machine you chose, look at them from the inside and the outside, and clean up."
cleanup = ["docker rm -f dojo-hello dojo-sleeper 2>/dev/null || true"]

[[lab.steps]]
title = "Run a one-shot container"
task = """
Run a container named **`dojo-hello`** from the image **`alpine:3.20`** that prints the kernel version with `uname -r` and exits.

Don't remove it afterwards: we want to see it in the list of stopped containers.
"""
hints = [
  "`docker run --name NAME IMAGE COMMAND…` runs COMMAND instead of the image's default.",
  "Without `--rm` the stopped container stays around for inspection.",
]
solution = """
```bash
docker run --name dojo-hello alpine:3.20 uname -r
docker ps -a --filter name=dojo-hello
```

The version printed is the **host's** kernel (on macOS: the Linux VM that Docker Desktop runs). The container has no kernel of its own.
"""

[[lab.steps.checks]]
name = "a container named dojo-hello exists"
run = "docker inspect -f '{{.Name}}' dojo-hello"
equals = "/dojo-hello"

[[lab.steps.checks]]
name = "it ran alpine:3.20"
run = "docker inspect -f '{{.Config.Image}}' dojo-hello"
equals = "alpine:3.20"

[[lab.steps.checks]]
name = "it exited successfully"
run = "docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' dojo-hello"
equals = "exited 0"

[[lab.steps]]
title = "Run a long-lived container in the background"
task = """
Start a container named **`dojo-sleeper`** from **`alpine:3.20`** that runs `sleep 3600`, **detached** (in the background).

Then look at its processes from the outside with `docker top dojo-sleeper` and from the inside with `docker exec dojo-sleeper ps`. Compare the PIDs.
"""
hints = [
  "`-d` (detach) returns your prompt and leaves the container running.",
  "`docker exec CONTAINER COMMAND` runs an extra process inside a running container.",
]
solution = """
```bash
docker run -d --name dojo-sleeper alpine:3.20 sleep 3600
docker top dojo-sleeper
docker exec dojo-sleeper ps
```

`docker top` shows the host's PID for `sleep`, `ps` inside the container shows PID 1: same process, two PID namespaces.
"""

[[lab.steps.checks]]
name = "dojo-sleeper is running"
run = "docker inspect -f '{{.State.Running}}' dojo-sleeper"
equals = "true"

[[lab.steps.checks]]
name = "its main process is sleep 3600"
run = "docker inspect -f '{{join .Config.Cmd \" \"}}' dojo-sleeper"
equals = "sleep 3600"

[[lab.steps]]
title = "Remove the one-shot container"
task = """
`dojo-hello` has done its job. Remove it, but keep `dojo-sleeper` running.
"""
hints = ["`docker rm NAME` removes a stopped container (`-f` would also stop a running one)."]
solution = """
```bash
docker rm dojo-hello
docker ps -a --filter name=dojo-
```
"""

[[lab.steps.checks]]
name = "dojo-hello is gone"
run = "docker ps -a --filter name=^/dojo-hello$ --format '{{.Names}}'"
equals = ""

[[lab.steps.checks]]
name = "dojo-sleeper still runs"
run = "docker inspect -f '{{.State.Running}}' dojo-sleeper"
equals = "true"
+++

## A container is a process

When you run a container you are **not** booting a computer. You are starting an ordinary Linux process (or a few) that the kernel *fences off* from everything else. Two kernel features do the fencing:

| Feature | What it controls | Example |
|---|---|---|
| **Namespaces** | What the process can *see* | its own PIDs, network interfaces, mounts, hostname, users |
| **cgroups** (control groups) | What the process can *use* | at most 512 MiB of RAM, half a CPU |

Put a process in fresh namespaces, limit it with a cgroup, give it its own root filesystem, and you have a container. That is the whole trick, and it is why containers start in milliseconds: there is no kernel to boot.

### Containers vs virtual machines

A **virtual machine** emulates hardware and runs a whole guest operating system with its **own kernel**. A **container** shares the **host's kernel** and only packages the application and its user-space files.

```text
 Virtual machines                     Containers
┌──────┐┌──────┐                    ┌──────┐┌──────┐┌──────┐
│ app  ││ app  │                    │ app  ││ app  ││ app  │
│ libs ││ libs │                    │ libs ││ libs ││ libs │
│kernel││kernel│                    └──────┘└──────┘└──────┘
└──────┘└──────┘                     container runtime
   hypervisor                        host kernel (shared)
   host kernel                       hardware
   hardware
```

Consequences you will rely on all the time:

- A Linux container needs a **Linux kernel**. On macOS, Docker Desktop runs a small Linux VM and your containers live inside it. On a Linux server they run directly on the host.
- Isolation is strong but **not** a hardware boundary: a container that runs as root with extra privileges can be dangerous. You will harden containers later (and Kubernetes `securityContext` builds on exactly these ideas).

## Images and containers

An **image** is a read-only template: a stack of filesystem **layers** plus metadata (the default command, environment variables, the user to run as…). A **container** is a running (or stopped) **instance** of an image, with a thin **writable layer** on top.

```text
container dojo-a   container dojo-b      ← writable layers (one each)
        └──────┬────────┘
     image alpine:3.20 (read-only layers, shared)
```

Think *class* and *object*: one image, many containers. Deleting a container removes only its writable layer; the image stays.

## The container lifecycle

```text
created ──start──▶ running ──main process exits──▶ exited ──rm──▶ (gone)
                     │  ▲
                 stop│  │start
                     ▼  │
                    exited
```

A container lives **exactly as long as its main process**. `docker run alpine:3.20 echo hi` prints `hi` and the container immediately moves to `exited`. A web server keeps running because its main process never ends.

## Your first commands

```bash
docker run alpine:3.20 echo "hello from a container"   # create + start, attached
docker run -d --name web nginx:1.27-alpine             # detached, with a name
docker ps                                              # running containers
docker ps -a                                           # all containers, stopped too
docker exec web ps                                     # a new process inside
docker rm -f web                                       # stop and remove
```

`docker run` = `docker pull` (if the image is missing) + `docker create` + `docker start`.

> **Where do these containers run?** On the machine you chose in PodDojo's top bar. PodDojo sets `DOCKER_CONTEXT` for you, so `docker` talks to that machine's engine, whether it is Docker Desktop on this Mac or a Linux server over SSH. Next lesson explains how.

## Why this matters for Kubernetes

Kubernetes never runs "VMs"; it runs these same containers, grouped in **Pods**. Everything here (one main process per container, exit means stopped, images are shared templates) carries straight over to the CKAD.
