+++
id = "docker-lifecycle-and-inspect"
title = "Lifecycle, signals and inspect"
module = "containers"
minutes = 40
needs = "docker"
objectives = ["Interpret container state, exit codes and OOM evidence", "Handle PID 1 signals and choose a stop grace period", "Extract lifecycle and configuration fields with Go templates", "Investigate processes, resource use and filesystem changes"]

[[quiz]]
q = "A container exited with code 137. What can you conclude?"
choices = ["It definitely ran out of memory", "Its executable could not be found", "It exited after a graceful SIGTERM", "It conventionally indicates SIGKILL; inspect OOMKilled and other evidence"]
answer = 3
why = "137 is conventionally 128 + SIGKILL (9). An OOM kill, docker kill, or a stop timeout can produce it; the number alone does not identify the cause, and a program can explicitly exit with that number."

[[quiz]]
q = "Why use exec form for an image entrypoint?"
choices = ["It adds a shell automatically", "It lets the application receive signals directly as the main process", "It expands variables such as $PORT in the arguments", "It keeps the container running after the application exits"]
answer = 1
why = "A shell wrapper can absorb or fail to forward signals. Exec form starts the application directly; a wrapper should finish with exec so the application replaces the shell."

[[quiz]]
q = "What distinguishes always from unless-stopped after an explicit manual stop?"
choices = ["Neither restarts after a manual stop, even when the daemon restarts", "always restarts only on nonzero exits", "Only always can restart after the daemon restarts", "Only unless-stopped can restart after the daemon restarts"]
answer = 2
why = "A manual stop suppresses immediate automatic restart for both. always can start the container again when the daemon restarts; unless-stopped preserves the stopped intention."

[[quiz]]
q = "What does docker diff show?"
choices = ["Added, changed and deleted paths in a container writable layer", "A comparison of two registry manifests", "Differences between the container config and its image config", "Changes in mounted volumes as well as in the writable layer"]
answer = 0
why = "diff reports filesystem changes relative to the image, with A, C and D markers. It is not a backup, a Dockerfile generator, or a report of changes inside independent volume storage."

[[quiz]]
q = "Which observation proves a process is healthy?"
choices = ["The container state is running", "`docker ps` shows the port as published", "The restart count stays at zero", "A successful application-level request matching the expected behavior"]
answer = 3
why = "Running means the main process exists. It does not prove that a dependency works or the application can serve requests; verify application behavior separately."

[lab]
summary = "Observe a signal-aware process, copy out its evidence, stop it gracefully, and change its restart policy."
setup = ["docker rm -f dojo-life 2>/dev/null || true", "rm -f report.txt"]
cleanup = ["docker rm -f dojo-life 2>/dev/null || true", "rm -f report.txt || true"]

[lab.files]
"worker.sh" = '''
#!/bin/sh
trap 'echo "TERM received"; exit 0' TERM
echo "worker ready"
while :; do
  sleep 1 &
  wait $!
done
'''

[[lab.steps]]
title = "Run a signal-aware main process"
task = "Create **dojo-life** from **alpine:3.20** with **no automatic restarts**. Run the supplied **worker.sh** as its main process. Keep it running and confirm its logs contain **worker ready**. Avoid bind mounts so the exercise also works with an SSH engine."
hints = ["docker create can define an object before starting it.", "docker cp can copy the local worker.sh into a stopped container."]
solution = '''
```bash
docker create --name dojo-life --restart no alpine:3.20 sh /worker.sh
docker cp worker.sh dojo-life:/worker.sh
docker start dojo-life
docker top dojo-life
docker stats --no-stream dojo-life
```

Copying through the API works when the engine is remote. The shell script deliberately installs a TERM handler.
'''

[[lab.steps.checks]]
name = "the worker is running with no restart policy"
run = "docker inspect -f '{{.Config.Image}} {{.State.Running}} {{.HostConfig.RestartPolicy.Name}}' dojo-life"
equals = "alpine:3.20 true no"

[[lab.steps.checks]]
name = "the worker has announced startup"
run = "for i in $(seq 1 20); do docker logs dojo-life 2>/dev/null | grep -q '^worker ready$' && echo ready && exit 0; sleep 1; done; exit 1"
equals = "ready"

[[lab.steps]]
title = "Capture writable-layer evidence"
task = "Write **lifecycle evidence** followed by a newline into **/tmp/report.txt** inside **dojo-life**. Copy that file to **report.txt** in the lab folder. Inspect filesystem changes and resource usage without altering the image."
hints = ["Use docker exec to write the report inside the running container.", "docker cp CONTAINER:/path local-path copies evidence out."]
solution = '''
```bash
docker exec dojo-life sh -c 'printf "%s\n" "lifecycle evidence" > /tmp/report.txt'
docker diff dojo-life
docker cp dojo-life:/tmp/report.txt report.txt
docker stats --no-stream dojo-life
```

The local report is a copy. Removing the container later will remove the container file but will not erase this local artifact.
'''

[[lab.steps.checks]]
name = "the container contains the evidence"
run = "docker exec dojo-life cat /tmp/report.txt"
equals = "lifecycle evidence"

[[lab.steps.checks]]
name = "the evidence was copied into the lab folder"
run = "cat report.txt"
equals = "lifecycle evidence"

[[lab.steps]]
title = "Verify graceful termination"
task = "Stop **dojo-life** with up to **10 seconds** of grace. Its handler should log **TERM received** and exit successfully. Retain the container."
hints = ["docker stop sends TERM unless the image or command overrides the stop signal.", "Inspect .State.Status and .State.ExitCode after stopping."]
solution = '''
```bash
docker stop --time 10 dojo-life
docker inspect -f '{{.State.Status}} exit={{.State.ExitCode}} oom={{.State.OOMKilled}}' dojo-life
docker logs --tail 5 dojo-life
```

A handled SIGTERM can produce exit code 0. Signal-derived codes describe unhandled termination, not an obligation for every handler.
'''

[[lab.steps.checks]]
name = "the worker exited successfully"
run = "docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' dojo-life"
equals = "exited 0"

[[lab.steps.checks]]
name = "the main process handled TERM"
run = "docker logs dojo-life"
contains = "TERM received"

[[lab.steps]]
title = "Resume with a deliberate restart policy"
task = "Change **dojo-life** to restart policy **unless-stopped**, then start it again. Keep the existing report available inside the container. Inspect recent events with a bounded query."
hints = ["docker update --restart unless-stopped changes this policy on the existing object.", "docker events --since 5m --until now is a bounded history query."]
solution = '''
```bash
docker update --restart unless-stopped dojo-life
docker start dojo-life
docker events --since 5m --until "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --filter container=dojo-life
```

The same object and writable layer survive. A restart policy changes recovery behavior; it does not make a failing application correct.
'''

[[lab.steps.checks]]
name = "the resumed worker has the requested recovery policy and retained evidence"
run = "test \"$(docker inspect -f '{{.State.Running}} {{.HostConfig.RestartPolicy.Name}}' dojo-life)\" = 'true unless-stopped' && test \"$(docker exec dojo-life cat /tmp/report.txt)\" = 'lifecycle evidence' "
+++

## State belongs to the container object

A container has both creation configuration and runtime state. Its image, environment, entrypoint and port mappings describe what should start. Its state records whether a process exists, when it started, and how the last run ended. Keeping these categories separate makes `docker inspect` much easier to read.

```text
create → created → start → running → process exits → exited → rm → absent
                            │  ▲                       │
                          pause/unpause                └── start again
                            │
                     restarting, if policy requests it
```

`created` means the object exists but has not started. `running` means its main process exists. `paused` means process execution is frozen. `restarting` indicates recovery attempts, often with backoff. `exited` retains the object's configuration, logs and writable layer. Rare `removing` or `dead` states indicate teardown or cleanup trouble, not an ordinary successful application exit.

Use `docker ps -a --filter name=^/dojo-life$` to include the stopped object. An empty running-container list does not prove the object has been deleted. That distinction matters when reusing names and investigating failed startup.

## Exit codes are evidence, not a diagnosis

| Exit code | Typical interpretation | What to check next |
|---|---|---|
| `0` | The program reported success | Did success mean the intended work finished? |
| `1` | Generic application failure | Logs and application-specific documentation |
| `137` | Conventionally `128 + 9`, SIGKILL | OOM flag, stop timeout, explicit kill, host evidence |
| `143` | Conventionally `128 + 15`, SIGTERM | Whether graceful termination was expected |

Programs can explicitly return these same numbers. A TERM handler may flush data and exit 0 instead of producing 143. An OOM kill commonly produces 137, but so does a forced kill; do not label every 137 as memory exhaustion.

```bash
docker inspect -f '{{.State.Status}} {{.State.ExitCode}} {{.State.OOMKilled}}' dojo-life
docker inspect -f '{{.State.Error}}' dojo-life
docker logs --tail 30 dojo-life
```

`.State.Error` can reveal runtime startup errors even when application logging never began. Exit status from `docker exec` belongs to that additional command, not necessarily to the main process. A failing diagnostic command need not stop the container.

## PID 1 must handle shutdown

In its PID namespace, the main process is PID 1. Linux gives PID 1 special signal behavior, so an application should explicitly handle termination rather than assuming every default action behaves like an ordinary shell process. PID 1 must also reap orphaned child processes; a small init process, available through Docker's `--init`, can help with forwarding signals and reaping children.

Exec-form startup makes the application the main process:

```dockerfile
ENTRYPOINT ["python", "/app/server.py"]
```

A shell-form command instead introduces `/bin/sh -c`. Unless that shell forwards signals or replaces itself, the application may not receive the stop request. A wrapper should end with `exec application arguments` when the wrapper has completed its setup. The lab script is intentionally a supervisor-like shell with an explicit TERM trap, so its shutdown is observable.

`docker stop --time 10 dojo-life` requests termination, waits up to ten seconds, and then uses SIGKILL if needed. SIGKILL cannot be caught to flush data. The image's `STOPSIGNAL` or an explicit stop-signal setting can change the initial signal; never assume every image uses TERM. `docker kill` defaults to KILL and is a different operational choice.

## Recovery is a policy, not a repair

Docker supports these restart policies:

| Policy | Recovery behavior |
|---|---|
| `no` | Do not restart automatically; the default |
| `on-failure:3` | Retry nonzero application exits with a maximum retry count |
| `always` | Recover after exit; a manual stop suppresses recovery until an explicit start or daemon restart |
| `unless-stopped` | Recover while preserving an explicitly stopped container across daemon restarts |

`on-failure` does not start a container merely because the daemon restarted. Docker's restart manager also tracks successful startup time and uses backoff to avoid tight loops; do not treat retry timing as an application scheduling guarantee. Details are in the [restart policy documentation](https://docs.docker.com/engine/containers/start-containers-automatically/).

```bash
docker update --restart unless-stopped dojo-life
docker inspect -f '{{.HostConfig.RestartPolicy.Name}} {{.RestartCount}}' dojo-life
```

A restart uses the same object and writable layer. Repeated restarts can hide a bad configuration behind brief periods of running state. Read logs and inspect the exit information before increasing retries. An unhealthy Docker health status alone does not cause the Engine restart policy to kill and restart a still-running process.

## Ask inspect one precise question

Go templates navigate the JSON data returned by inspect. Field names are case-sensitive. Quote the template so your local shell does not reinterpret it:

```bash
docker inspect -f '{{.State.Pid}}' dojo-life
docker inspect -f '{{.Config.WorkingDir}}' dojo-life
docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dojo-life
docker inspect -f '{{index .Config.Labels "dojo.lesson"}}' dojo-life
```

`index` handles map keys containing dots. `range` iterates a list. Prefer a field-level query over matching the entire formatted JSON document: irrelevant changes should not break your investigation. Do not share unredacted inspect output from workloads containing credentials.

## Processes, resources, events and files

`docker top dojo-life` observes processes from the engine host's viewpoint; `docker exec dojo-life ps` observes the container namespace. Their PID numbers can differ without describing different processes.

`docker stats --no-stream dojo-life` takes one resource sample. It is useful for orientation but not proof of a sustained trend, and its memory display accounts for cache differently across platforms. Avoid leaving a streaming command running inside a grading solution.

`docker events` reports lifecycle events such as start, die and stop. Bound both the time range and the container filter; otherwise it remains open waiting for future events. Event history is not a durable audit archive, so absence from recent history is not proof that an old action never happened.

`docker diff dojo-life` marks added, changed and deleted filesystem paths with A, C and D. Volume content is separate from the image writable layer. `docker cp dojo-life:/tmp/report.txt report.txt` copies a file out, including from a stopped container. Unlike a host bind mount, this works through the engine API even when the daemon runs over SSH. These tools recover evidence; they do not convert manual edits into a reproducible image build.

## Why this matters for Kubernetes

Kubernetes reports similar evidence under container status, including the last termination reason and exit code. Pod phase `Running` still does not prove application readiness. Kubernetes also requests graceful termination before forced shutdown, with a configurable grace period:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-life
  namespace: lifecycle-lab
spec:
  terminationGracePeriodSeconds: 10
  restartPolicy: OnFailure
  containers:
    - name: worker
      image: alpine:3.20
      command: ["sh", "-c", "echo starting; sleep 3600"]
```

This translation illustrates the fields, not a production signal-handling pattern. Docker's `unless-stopped` has no identical Pod policy. For CKAD, learn the Kubernetes controller's recovery contract rather than copying Docker policy names, and inspect previous container logs when a restart erased the live process you hoped to examine.
