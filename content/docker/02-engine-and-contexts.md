+++
id = "docker-engine-and-contexts"
title = "The engine, the API and Docker contexts"
module = "start"
minutes = 35
needs = "docker"
objectives = ["Trace a Docker request from the client to the container process", "Inspect the selected engine without changing its context", "Distinguish registry addresses, image names and OCI standards", "Identify containers through API metadata and labels"]

[[quiz]]
q = "Which machine starts the process when your selected context uses SSH?"
choices = ["The laptop running the Docker CLI", "The remote machine running the selected daemon", "Docker Hub", "Both machines run identical copies"]
answer = 1
why = "The client sends API requests over SSH. The daemon on the remote machine creates the container there; the local CLI only presents its output."

[[quiz]]
q = "Why can docker version show a working Client section but no Server section?"
choices = ["Containers need rebuilding", "The registry password has expired", "The CLI is installed but cannot reach the selected daemon", "An image has no CMD"]
answer = 2
why = "Client information is local. Server information requires an API connection, so check the context endpoint, SSH access and daemon availability before debugging images."

[[quiz]]
q = "What does access to a conventional rootful Docker API socket imply?"
choices = ["Powerful control over the host through the daemon", "Permission only to list containers", "An encrypted network connection", "Automatic isolation from host files"]
answer = 0
why = "A caller can ask the rootful daemon to create privileged containers and mount host paths. Treat socket access like host administrator access; do not expose an unauthenticated API."

[[quiz]]
q = "What is docker.io/library/alpine:3.20?"
choices = ["A container ID", "A local filesystem path", "An SSH context", "A registry, repository and tag identifying an image reference"]
answer = 3
why = "docker.io is the registry; library/alpine is the repository; 3.20 is a tag. Pulling an image downloads content, whereas a context selects the daemon that stores and runs it."

[[quiz]]
q = "Which setting does PodDojo supply to select your Docker machine?"
choices = ["A change to your shell startup file", "DOCKER_CONTEXT in each lab command environment", "A new default Kubernetes namespace", "A Docker Hub organization"]
answer = 1
why = "DOCKER_CONTEXT selects a named Docker context for those commands. PodDojo does not need to change your saved default context, and a Docker context is separate from a Kubernetes context."

[lab]
summary = "Record facts about the chosen engine, create a container through its API, and make the resource discoverable with labels."
setup = ["docker rm -f dojo-engine 2>/dev/null || true", "rm -f engine-facts.txt engine-version.txt"]
cleanup = ["docker rm -f dojo-engine 2>/dev/null || true", "rm -f engine-facts.txt engine-version.txt || true"]

[[lab.steps]]
title = "Record the selected engine"
task = "Write **engine-facts.txt** containing the selected daemon name, operating-system type and architecture as three space-separated fields, followed by a newline. Write **engine-version.txt** containing its server version. Use live engine facts, not facts about the computer running the CLI."
hints = ["docker info --format selects daemon facts.", "docker version --format can select .Server.Version."]
solution = '''
```bash
docker info --format '{{.Name}} {{.OSType}} {{.Architecture}}' > engine-facts.txt
docker version --format '{{.Server.Version}}' > engine-version.txt
cat engine-facts.txt engine-version.txt
```

The files are a tiny inventory of the daemon chosen by the machine picker. The server may be a Linux VM or an SSH host.
'''

[[lab.steps.checks]]
name = "the recorded identity matches the selected engine"
run = "test -s engine-facts.txt && test \"$(cat engine-facts.txt)\" = \"$(docker info --format '{{.Name}} {{.OSType}} {{.Architecture}}')\""

[[lab.steps.checks]]
name = "the recorded version matches the server"
run = "test -s engine-version.txt && test \"$(cat engine-version.txt)\" = \"$(docker version --format '{{.Server.Version}}')\""

[[lab.steps]]
title = "Create an inspectable API object"
task = "Run **dojo-engine** from **alpine:3.20**, keeping it running for inspection. Find its image and running state with docker inspect, which reads the daemon API."
hints = ["A detached container can keep sleep running.", "docker inspect -f selects a field from the returned JSON object."]
solution = '''
```bash
docker run -d --name dojo-engine alpine:3.20 sleep 3600
docker inspect -f '{{.Id}} {{.Config.Image}} {{.State.Running}}' dojo-engine
```

Creation and inspection are API operations on the same selected daemon. There is no need to connect directly to a local socket.
'''

[[lab.steps.checks]]
name = "the selected daemon has the running Alpine container"
run = "docker inspect -f '{{.Config.Image}} {{.State.Running}}' dojo-engine"
equals = "alpine:3.20 true"

[[lab.steps]]
title = "Make ownership queryable"
task = "Recreate **dojo-engine** with container label **dojo.lesson=engine**, preserving its Alpine image and running state. Confirm an exact-name, label-filtered query can find it."
hints = ["Container labels are set at creation, so recreate this disposable container.", "Use --label dojo.lesson=engine and a label filter."]
solution = '''
```bash
docker rm -f dojo-engine
docker run -d --name dojo-engine --label dojo.lesson=engine alpine:3.20 sleep 3600
docker ps --filter label=dojo.lesson=engine --format '{{.Names}}'
```

A label is structured metadata on the container, independent of its human-readable name or its image tag.
'''

[[lab.steps.checks]]
name = "the running container has the lesson ownership label"
run = "test \"$(docker inspect -f '{{index .Config.Labels \"dojo.lesson\"}} {{.Config.Image}} {{.State.Running}}' dojo-engine)\" = 'engine alpine:3.20 true' && docker ps --filter name=^/dojo-engine$ --filter label=dojo.lesson=engine --format '{{.Names}}' "
equals = "dojo-engine"
+++

## One command crosses several boundaries

Typing `docker run` feels local, but the Docker CLI is a client. It describes work to a long-running server, the Docker daemon (`dockerd`). The daemon owns the containers, images, networks and volumes on its machine. Installing the client does not, by itself, provide an engine capable of starting Linux processes.

```text
Docker CLI on your computer
       │ Docker Engine HTTP API
       │ local Unix socket OR SSH transport
       ▼
dockerd on the selected machine
       │ manages containers through containerd
       ▼
containerd → shim → OCI runtime (typically runc)
                              │
                              ▼
                     isolated Linux process
```

`containerd` manages container execution and related lifecycle tasks. A runtime such as `runc` performs the low-level work of constructing namespaces, applying resource settings and starting the process. A shim remains to supervise process I/O and exit reporting without requiring the short-lived runtime command to stay running. You normally operate at the Docker API level rather than invoking these components yourself.

This distinction is useful when troubleshooting. A missing CLI, an unreachable daemon, an unavailable registry and an application crash are four different failures. Rebuilding the image cannot repair an SSH connection. Restarting an application cannot fix an image reference that does not exist.

## The socket is an API endpoint

On a conventional Linux installation the client reaches the engine through a Unix socket such as `/var/run/docker.sock`. HTTP requests can travel over that socket without opening an ordinary TCP listening port. Docker Desktop and rootless engines can use other socket locations; inspect the context rather than assuming a universal path.

Access to a rootful engine socket grants powerful control over the host. A caller can request privileged containers or host-directory mounts. A socket is not safe merely because it is a file. Do not mount it into an application as a convenient way to inspect Docker, and do not expose an unauthenticated TCP API to solve a connection problem. SSH contexts provide an authenticated transport without publishing a naked daemon port. See Docker's [socket access guidance](https://docs.docker.com/engine/security/protect-access/).

For this lesson, `docker inspect` is our API browser. It requests structured object data through whichever connection Docker already selected:

```bash
docker inspect dojo-engine
docker inspect -f '{{.Config.Image}}' dojo-engine
docker inspect -f '{{.State.Status}}' dojo-engine
```

The JSON includes configuration, runtime state and network information. A Go template extracts an individual field, which is more reliable in a script than visually reading columns designed for humans.

## Establish which engine answered

Use these before diagnosing a failed lab:

```bash
docker version
docker info
docker info --format '{{.Name}} {{.OSType}} {{.Architecture}}'
docker version --format '{{.Server.Version}}'
```

`version` separates Client and Server information, including their Engine API versions. The CLI negotiates a supported API version with the server; different client and server release numbers do not automatically mean failure. If only the Client section succeeds, investigate the connection before the workload.

`info` describes the selected daemon: its name, storage backend, architecture, operating system and resource capacity. On a Mac, the client runs on macOS while the server usually reports Linux because Docker Desktop runs containers inside a Linux VM. On an SSH context, the CPU and memory figures belong to the remote engine. The lab records stable identity fields instead of hard-coding your machine's hostname or processor type.

## Contexts are named connections

A Docker context bundles an endpoint and connection settings. It does not bundle a separate set of containers inside an engine. Two contexts pointing to the same daemon see the same resources.

| Endpoint idea | Where the client connects | Where containers run |
|---|---|---|
| `unix:///var/run/docker.sock` | A local Unix socket | That engine's Linux host |
| Docker Desktop socket | Desktop's managed engine | Desktop's Linux VM |
| `ssh://learner@server.example` | An engine through SSH | The remote server |

Inspect existing connections without switching them:

```bash
docker context ls
docker context show
docker context inspect
printf '%s\n' "$DOCKER_CONTEXT"
```

An SSH context can describe an endpoint such as `ssh://learner@server.example`. The context records a connection; it does not install Docker on that server or grant missing permissions. The remote account must already be able to use the daemon, and SSH authentication must work. Create and maintain machine connections outside this lab; here we only inspect the connection selected by PodDojo.

PodDojo's machine picker selects an existing Docker context and sets `DOCKER_CONTEXT` for lab commands. That environment variable overrides the saved default context and `DOCKER_HOST`. Leave it intact. Changing your shell's default context is unnecessary and risks redirecting unrelated work. The application also supplies `PODDOJO_HOST`, the address for reaching published application ports; an API endpoint and an application address serve different purposes.

## Registries hold images, engines run containers

A registry is an image distribution service. Docker Hub is a public registry; an organization can also operate a private one. The reference `alpine:3.20` is shorthand for `docker.io/library/alpine:3.20`. It names a repository and tag, not an engine or container.

```text
registry ──pull image content──▶ selected engine
                                  │
                                  ├── container dojo-engine
                                  └── another container from the same image
```

Pulling on an SSH context downloads into the remote engine's image store. Your laptop does not first receive and forward every layer. Registry reachability and authentication therefore matter at the engine side too. Public pulls can encounter rate limits; an SSH error and a registry authorization error should lead to different investigations.

The Open Container Initiative defines interoperable image, runtime and distribution specifications. Docker images and container workflows use these standards, which helps the same image work with different compatible tools. An OCI image is not tied to the Docker CLI, although its operating system and CPU architecture must still match the execution environment.

## Labels turn names into searchable metadata

Names are useful handles; labels express ownership and purpose. Supply them when creating a container:

```bash
docker run -d --name dojo-engine --label dojo.lesson=engine alpine:3.20 sleep 3600
docker ps --filter label=dojo.lesson=engine --format '{{.Names}}'
```

Container labels do not change an image's labels, and existing container labels cannot be edited in place. Recreate a disposable container when its creation metadata must change. Always target exact lesson resources when removing anything; a label filter is for discovery, not permission to delete other learners' work.

## Why this matters for Kubernetes

Kubernetes separates the API client from the machines executing containers too. Its kubelet uses a CRI-compatible runtime such as containerd; it does not require Docker Engine. A Docker context and a Kubernetes context select different APIs and must never be confused.

The same ownership idea appears in Kubernetes YAML:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-engine
  namespace: engine-lab
  labels:
    dojo.lesson: engine
spec:
  containers:
    - name: sleeper
      image: alpine:3.20
      command: ["sleep", "3600"]
```

This is a translation example, not a cluster task. For CKAD, learn to ask three questions before changing anything: which API am I addressing, where will the process run, and which metadata identifies my workload?
