+++
id = "docker-limits-and-hardening"
title = "Resource limits and container hardening"
module = "operate"
minutes = 45
needs = "docker"
objectives = ["Apply memory, CPU and PID limits without stressing the host", "Run an application as a numeric non-root user with minimal privileges", "Provide bounded writable scratch space on a read-only root filesystem", "Map Docker controls to Kubernetes resources and securityContext"]

[[quiz]]
q = "What happens when a container demands more CPU than its --cpus budget?"
choices = ["It must be OOM-killed", "Its CPU execution is generally throttled", "Docker grants it another kernel", "It gains a higher scheduling priority"]
answer = 1
why = "A CPU quota limits how much execution time the cgroup gets. CPU pressure generally throttles work, whereas exceeding a hard memory limit can trigger an OOM kill."

[[quiz]]
q = "Does exit code 137 prove the kernel OOM-killed the container?"
choices = ["Yes, always", "No; it indicates SIGKILL by convention, so inspect OOMKilled and surrounding evidence", "It means a missing executable", "It means the healthcheck passed"]
answer = 1
why = "137 is conventionally 128 plus signal 9. A forced kill can produce it too. Docker State.OOMKilled or Kubernetes terminated reason OOMKilled provides the stronger memory-specific evidence."

[[quiz]]
q = "What can still be writable with --read-only?"
choices = ["Nothing, including every mount", "Explicit writable mounts such as a tmpfs at /tmp", "Every image layer", "Only the Dockerfile"]
answer = 1
why = "The flag protects the root filesystem. Separate writable volumes and tmpfs mounts can still provide intentionally limited write locations."

[[quiz]]
q = "Which Kubernetes container setting corresponds to --security-opt no-new-privileges?"
choices = ["privileged: true", "runAsUser: 0", "allowPrivilegeEscalation: false", "hostNetwork: true"]
answer = 2
why = "allowPrivilegeEscalation controls the Linux no_new_privs mechanism. It is separate from choosing a non-root UID and dropping capabilities; privileged containers and CAP_SYS_ADMIN have special implications."

[[quiz]]
q = "Is --pids-limit directly configured by a standard Pod container resources.limits.pids field?"
choices = ["Yes, with an integer", "No; kubelet/node PID management is separate from normal container CPU/memory resource fields", "Only with requests.pids", "It maps to restartPolicy"]
answer = 1
why = "The standard per-container resource list has no ordinary pids limit equivalent. Node administrators can configure Pod PID limits through kubelet; a CKAD task should not invent an unsupported YAML field."

[lab]
summary = "Constrain a quiet process, move it to a non-root identity, and harden its filesystem and privilege settings without generating an OOM or a fork storm."
setup = []
cleanup = ["docker rm -f dojo-limits-app dojo-limits-cap 2>/dev/null || true"]

[[lab.steps]]
title = "Bound resources"
task = "Start **dojo-limits-app** from **alpine:3.20**, detached with **sleep 3600**. Set memory to **64 MiB**, CPU quota to **0.5 CPUs**, and PID limit to **64**. Do not deliberately exhaust resources."
hints = ["Use --memory 64m --cpus 0.5 --pids-limit 64.", "64 MiB is 67108864 bytes; half a CPU is 500000000 NanoCPUs."]
solution = '''```bash
docker run -d --name dojo-limits-app --memory 64m --cpus 0.5 --pids-limit 64 alpine:3.20 sleep 3600
docker stats --no-stream dojo-limits-app
```'''

[[lab.steps.checks]]
name = "running application has memory, CPU and PID ceilings"
run = "test \"$(docker inspect -f '{{.State.Running}} {{.HostConfig.Memory}} {{.HostConfig.PidsLimit}}' dojo-limits-app)\" = 'true 67108864 64' && docker inspect -f '{{.HostConfig.NanoCpus}} {{.HostConfig.CpuQuota}} {{.HostConfig.CpuPeriod}}' dojo-limits-app | awk '{period = $3 > 0 ? $3 : 100000; if ($1 == 500000000 || ($2 > 0 && $2 / period == 0.5)) print \"ok\"}' "
equals = "ok"

[[lab.steps]]
title = "Choose a non-root identity"
task = "Replace **dojo-limits-app**, retaining its resource ceilings and sleep workload, but run it as numeric **UID 10001, GID 10001**. Verify the effective identity, rather than assuming a username exists in /etc/passwd."
hints = ["Use --user 10001:10001.", "docker exec ... id -u and id -g show the active identity."]
solution = '''```bash
docker rm -f dojo-limits-app
docker run -d --name dojo-limits-app --memory 64m --cpus 0.5 --pids-limit 64 --user 10001:10001 alpine:3.20 sleep 3600
docker exec dojo-limits-app id
```'''

[[lab.steps.checks]]
name = "effective UID and GID are 10001 and resource limits remain"
run = "test \"$(docker exec dojo-limits-app id -u)\" = 10001 && test \"$(docker exec dojo-limits-app id -g)\" = 10001 && test \"$(docker inspect -f '{{.HostConfig.Memory}} {{.HostConfig.PidsLimit}}' dojo-limits-app)\" = '67108864 64' && docker inspect -f '{{.HostConfig.NanoCpus}} {{.HostConfig.CpuQuota}} {{.HostConfig.CpuPeriod}}' dojo-limits-app | awk '{period = $3 > 0 ? $3 : 100000; if ($1 == 500000000 || ($2 > 0 && $2 / period == 0.5)) print \"ok\"}' "
equals = "ok"

[[lab.steps]]
title = "Protect the filesystem and privilege boundary"
task = "Replace **dojo-limits-app** again. Retain UID/GID and all limits; make its root filesystem read-only, give it a **16 MiB tmpfs at /tmp**, drop **ALL** capabilities, and enable **no-new-privileges**. Write **hardened dojo** to **/tmp/status.txt**, then keep sleep 3600 running."
hints = ["Use --read-only, --cap-drop ALL and --security-opt no-new-privileges.", "The tmpfs must allow UID 10001 to write; its normal /tmp-style mode does."]
solution = '''```bash
docker rm -f dojo-limits-app
docker run -d --name dojo-limits-app --memory 64m --cpus 0.5 --pids-limit 64 --user 10001:10001 --read-only --mount type=tmpfs,dst=/tmp,tmpfs-size=16777216,tmpfs-mode=1777 --cap-drop ALL --security-opt no-new-privileges alpine:3.20 sh -c 'printf "hardened dojo\n" > /tmp/status.txt; exec sleep 3600' 
```'''

[[lab.steps.checks]]
name = "process is non-root with a read-only root, no effective capabilities and no new privileges"
run = "for i in $(seq 1 15); do if test \"$(docker exec dojo-limits-app id -u)\" = 10001 && test \"$(docker exec dojo-limits-app id -g)\" = 10001 && test \"$(docker inspect -f '{{.HostConfig.ReadonlyRootfs}}' dojo-limits-app)\" = true && docker exec dojo-limits-app sh -c 'grep -Eq \"^CapEff:[[:space:]]+0+$\" /proc/1/status && grep -Eq \"^NoNewPrivs:[[:space:]]+1$\" /proc/1/status && test \"$(cat /tmp/status.txt)\" = \"hardened dojo\" && test \"$(stat -f -c %T /tmp)\" = tmpfs' && echo ok; then exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 60

[[lab.steps.checks]]
name = "hardening retains the resource bounds and limits tmpfs size"
run = "test \"$(docker inspect -f '{{.HostConfig.ReadonlyRootfs}} {{.HostConfig.Memory}} {{.HostConfig.PidsLimit}}' dojo-limits-app)\" = 'true 67108864 64' && docker exec dojo-limits-app df -k /tmp | awk 'END {print $2}' | grep -qx 16384 && docker inspect -f '{{.HostConfig.NanoCpus}} {{.HostConfig.CpuQuota}} {{.HostConfig.CpuPeriod}}' dojo-limits-app | awk '{period = $3 > 0 ? $3 : 100000; if ($1 == 500000000 || ($2 > 0 && $2 / period == 0.5)) print \"ok\"}' "
equals = "ok"

[[lab.steps]]
title = "Grant only one explicit capability"
task = "Create a separate demonstration container **dojo-limits-cap** from **alpine:3.20**, running **sleep 3600** as root, with **ALL capabilities dropped except NET_BIND_SERVICE** added back. Enable **no-new-privileges**, a read-only root and the same 64 MiB/0.5 CPU/64 PID ceilings. This demonstrates precise capability selection; the original non-root app must stay running."
hints = ["Use --cap-drop ALL --cap-add NET_BIND_SERVICE.", "NET_BIND_SERVICE is bit 10, so the effective capability mask is hexadecimal 400. Low-port binding can also be permitted by a network sysctl; a successful bind alone would not prove this grant."]
solution = '''```bash
docker run -d --name dojo-limits-cap --memory 64m --cpus 0.5 --pids-limit 64 --read-only --cap-drop ALL --cap-add NET_BIND_SERVICE --security-opt no-new-privileges alpine:3.20 sleep 3600
```'''

[[lab.steps.checks]]
name = "demonstration has only NET_BIND_SERVICE effective and retains the safety bounds"
run = "docker exec dojo-limits-cap sh -c 'grep -Eq \"^CapEff:[[:space:]]+0*400$\" /proc/1/status && grep -Eq \"^NoNewPrivs:[[:space:]]+1$\" /proc/1/status' && test \"$(docker inspect -f '{{.HostConfig.ReadonlyRootfs}} {{.HostConfig.Memory}} {{.HostConfig.PidsLimit}}' dojo-limits-cap)\" = 'true 67108864 64' && test \"$(docker inspect -f '{{.State.Running}}' dojo-limits-app)\" = true && docker inspect -f '{{.HostConfig.NanoCpus}} {{.HostConfig.CpuQuota}} {{.HostConfig.CpuPeriod}}' dojo-limits-cap | awk '{period = $3 > 0 ? $3 : 100000; if ($1 == 500000000 || ($2 > 0 && $2 / period == 0.5)) print \"ok\"}' "
equals = "ok"
+++

## Constrain both damage and consumption

Containers isolate views of the system, but a process can still consume CPU, memory and process slots or attempt operations its credentials permit. Resource limits reduce consumption; security settings reduce authority. Neither is a substitute for the other. A non-root application with no memory ceiling can exhaust a shared machine, while a tightly limited root process can still have unnecessary access.

Linux cgroups account for and constrain groups of processes. Namespaces determine what those processes see. User IDs, Linux capabilities, filesystem permissions and security mechanisms determine what operations they may perform. Keep these three questions separate: “what can it see?”, “what can it use?” and “what may it do?”

```text
application process
  namespaces       -> isolated views
  cgroup           -> CPU, memory, PID budgets
  credentials      -> UID, GID, capabilities
  filesystem       -> read-only root + chosen writable mounts
  no_new_privs     -> exec cannot gain new privilege
```

## Memory, CPU and PIDs fail differently

`--memory 64m` sets a hard memory limit of 64 MiB for the container. Memory accounting includes more than the language heap; leave room for runtime overhead and relevant filesystem memory. Swap behavior depends on daemon and host configuration and additional options. Do not infer a strict no-swap policy from `--memory` alone.

`--cpus 0.5` gives roughly half of one logical CPU's execution capacity as a scheduling quota. It does not reserve a dedicated physical core or pin the process to one processor. Under sustained demand, the scheduler throttles the cgroup when its budget is exhausted. More CPU demand generally produces latency rather than an immediate container exit.

`--pids-limit 64` restricts the number of tasks the container can create, including threads. A thread-heavy application may reach that limit before it has many visible processes. Failed forks or thread creation should lead you to inspect PID limits, not automatically increase memory. Read [Docker's resource constraint guide](https://docs.docker.com/engine/containers/resource_constraints/) for the distinct controls.

```bash
docker run -d --name dojo-limits-app \
  --memory 64m --cpus 0.5 --pids-limit 64 \
  alpine:3.20 sleep 3600
docker stats --no-stream dojo-limits-app
docker inspect -f '{{json .HostConfig}}' dojo-limits-app
```

Configuration inspection proves a requested ceiling, while statistics show present usage. A sleeping process using little CPU does not prove a CPU ceiling is configured. Our lab checks both the intended configuration and the live process without intentionally exhausting a shared host.

## Read OOM evidence precisely

When memory cannot be reclaimed within a hard limit, the kernel can OOM-kill a process. If the container's main process is killed, Docker commonly reports exit 137 and `State.OOMKilled=true`. But 137 conventionally means termination by SIGKILL, and a manual kill or forced stop can produce the same code. Inspect the OOM flag, logs and timing together.

```bash
docker inspect -f '{{.State.Status}} {{.State.ExitCode}} {{.State.OOMKilled}}' \
  dojo-limits-app
docker logs --tail 30 dojo-limits-app
```

A child process can be killed while the main process remains alive, so resource incidents do not always look like one clean container exit. Fix a leak, bad cache policy or unrealistic budget rather than blindly raising limits. Keep the OOM killer enabled; disabling it can put the wider machine under pressure. This lesson explains the evidence without manufacturing an OOM incident on your host.

## A numeric non-root user is explicit

`--user 10001:10001` selects numeric UID and GID without requiring a named account in `/etc/passwd`. An image may not contain the username you expect. Existing files retain their ownership and modes, so the selected user must be able to read application files and write only the intended data paths.

Dropping root does not automatically fix unsafe mounts or world-writable data. Mounting a powerful host socket into a non-root container can still be dangerous if that identity can access it. Avoid privileged mode as a shortcut for permission errors; determine the exact denied operation and the narrow permission needed.

## Make writable paths intentional

`--read-only` mounts the image-based root filesystem read-only. It does not make separately mounted volumes or tmpfs mounts read-only. Many programs need temporary files, PID files or sockets, so identify those paths and provide bounded writable storage. Logging to standard output often removes the need for a writable log directory.

```bash
docker run -d --name dojo-hardened --user 10001:10001 --read-only \
  --mount type=tmpfs,dst=/tmp,tmpfs-size=16777216,tmpfs-mode=1777 \
  --cap-drop ALL --security-opt no-new-privileges \
  --memory 64m --cpus 0.5 --pids-limit 64 \
  alpine:3.20 sleep 3600
```

The sticky, writable `/tmp` mode allows the numeric user to create scratch files. tmpfs data disappears on stop and consumes memory; its size limit and the memory ceiling must make sense together. A read-only root is also useful operationally: unexpected writes fail near their cause instead of silently modifying a container that will later be replaced.

## Capabilities and privilege escalation

Linux splits many root powers into capabilities. Docker grants a default set; `--cap-drop ALL` removes them, and `--cap-add NET_BIND_SERVICE` adds back one specific capability. Use the smallest set the application demonstrably needs. Do not add `SYS_ADMIN` as a generic fix: it grants a very broad collection of powers.

Low-port binding traditionally requires `NET_BIND_SERVICE`, but network-namespace sysctl settings can allow unprivileged low ports. Therefore, a successful listener on port 80 is not reliable proof that this capability is present. Inspect effective capabilities when the requirement is a specific privilege grant. Our separate demonstration uses a root process with only that one effective capability so the mask is easy to interpret.

`--security-opt no-new-privileges` prevents gaining additional privilege through `exec`, including privilege transitions from set-user-ID binaries or file capabilities. It does not remove capabilities already granted at startup and does not replace `--user`. Rootless Docker goes further by running the daemon and containers inside a user namespace as a non-root host user. It is a daemon deployment model, not a synonym for `--user`. Resource-control support in rootless mode depends on host cgroup configuration.

## Why this matters for Kubernetes

The following mappings let you translate a task requirement instead of guessing a field name:

| Docker control | Kubernetes equivalent or distinction |
|---|---|
| `--memory 64m` | `resources.limits.memory: 64Mi` |
| `--cpus 0.5` | `resources.limits.cpu: 500m` |
| `--pids-limit 64` | Kubelet/node Pod PID configuration; no ordinary container `limits.pids` field |
| `--user 10001:10001` | `securityContext.runAsUser` and `runAsGroup` |
| `--read-only` | Container `securityContext.readOnlyRootFilesystem: true` |
| tmpfs mount | `emptyDir.medium: Memory` plus `volumeMounts` |
| `--cap-drop ALL` / `--cap-add` | Container `securityContext.capabilities.drop` / `add` |
| no-new-privileges | Container `securityContext.allowPrivilegeEscalation: false` |
| Rootless engine | Runtime/host arrangement; no single Pod field switches Docker into rootless mode |

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-hardened
  namespace: security-lab
spec:
  securityContext:
    runAsUser: 10001
    runAsGroup: 10001
    runAsNonRoot: true
  containers:
    - name: app
      image: alpine:3.20
      command: ["sleep", "3600"]
      resources:
        requests: {cpu: 100m, memory: 32Mi}
        limits: {cpu: 500m, memory: 64Mi}
      securityContext:
        readOnlyRootFilesystem: true
        allowPrivilegeEscalation: false
        capabilities:
          drop: ["ALL"]
        seccompProfile:
          type: RuntimeDefault
      volumeMounts:
        - {name: scratch, mountPath: /tmp}
  volumes:
    - name: scratch
      emptyDir: {medium: Memory, sizeLimit: 16Mi}
```

Requests inform scheduling; limits bound usage. `runAsNonRoot` validates the non-root requirement rather than choosing a UID for you. Capabilities and read-only root settings belong at container scope. Use `kubectl explain pod.spec.containers.securityContext` and the [security-context guide](https://kubernetes.io/docs/tasks/configure-pod-container/security-context/) to check nesting. At exam speed, generate a Pod skeleton with `kubectl run ... --dry-run=client -o yaml`, add the required fields, and verify the actual running identity and termination evidence.
