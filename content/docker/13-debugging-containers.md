+++
id = "docker-debugging-containers"
title = "Debug containers by following the evidence"
module = "operate"
minutes = 45
needs = "docker"
objectives = ["Diagnose container failures from state, logs, configuration and events", "Repair a crash loop and an invalid command without hiding the cause", "Distinguish a missing publication from an application listener failure", "Fix a failing health check while preserving a working service"]

[[quiz]]
q = "A container is absent from docker ps. What should you do first?"
choices = ["Delete its volume", "List docker ps -a for the named workload", "Assume its image disappeared", "Add a privileged flag"]
answer = 1
why = "docker ps shows running containers. A stopped or never-successfully-started container can still exist and contain useful State.Error, exit status and configuration evidence."

[[quiz]]
q = "docker exec fails because a container is stopped. What is the next useful tool?"
choices = ["More exec retries without examining state", "docker logs and docker inspect", "Changing the default Docker context", "Publishing every container port"]
answer = 1
why = "exec requires a running container. Logs and inspection work for retained stopped containers, and inspect can explain a startup failure that produced no application log at all."

[[quiz]]
q = "nginx responds from inside the container, but the host URL fails. What do you inspect next?"
choices = ["Only the image layers", "PortBindings, published host port and host reachability", "The CPU architecture first in every case", "The named volume contents"]
answer = 1
why = "An internal response proves part of the application path. The remaining path includes listener binding, publication, the selected host address and any external filtering."

[[quiz]]
q = "A standalone Docker container becomes unhealthy while its main process keeps running. What is automatic?"
choices = ["Docker always restarts it", "Docker deletes its image", "Docker records unhealthy status; ordinary restart policies do not restart solely for this status", "Docker switches the healthcheck to another port"]
answer = 2
why = "Health status and process lifecycle are separate. Inspect State.Health.Log for probe output. A restart policy is normally triggered by process exit, not merely an unhealthy result."

[[quiz]]
q = "Which pair most strongly supports a container-level OOM diagnosis?"
choices = ["Exit 0 and healthy", "Exit 137 plus OOMKilled true", "Exit 127 plus no logs", "An unpublished port plus exit 1"]
answer = 1
why = "Exit 137 alone can also mean another SIGKILL. The OOMKilled flag provides memory-specific evidence, which should be combined with logs, limits and observed behavior."

[lab]
summary = "Repair four deliberately broken, precisely named containers by diagnosing their process, network and health state."
setup = ["docker rm -f dojo-debug-crash dojo-debug-command dojo-debug-port dojo-debug-health 2>/dev/null || true", "docker run -d --name dojo-debug-crash --restart on-failure:5 alpine:3.20 sh -c 'echo \"fatal: required APP_MODE=serve\" >&2; exit 1' ", "docker create --name dojo-debug-command alpine:3.20 sleeeep 3600", "docker start dojo-debug-command >/dev/null 2>&1 || true", "docker run -d --name dojo-debug-port nginx:1.27-alpine", "docker run -d --name dojo-debug-health --health-cmd 'wget -qO- http://127.0.0.1:81/ >/dev/null || exit 1' --health-interval 2s --health-timeout 2s --health-retries 2 nginx:1.27-alpine"]
cleanup = ["docker rm -f dojo-debug-crash dojo-debug-command dojo-debug-port dojo-debug-health 2>/dev/null || true"]

[[lab.steps]]
title = "Fix the crash-looping workload"
task = "Diagnose **dojo-debug-crash** with ps -a, logs, State and RestartCount. Replace it with an **alpine:3.20** container of the same name that has **APP_MODE=serve**, prints **ready: serve**, then remains running with sleep 3600. Use no automatic restart policy after repair."
hints = ["A restart policy repeats a bad command; it does not repair configuration.", "Use a shell guard to make the environment requirement explicit, then exec sleep."]
solution = '''```bash
docker ps -a --filter name=^/dojo-debug-crash$
docker logs --tail 20 dojo-debug-crash
docker inspect -f '{{json .State}} restarts={{.RestartCount}}' dojo-debug-crash
docker rm -f dojo-debug-crash
docker run -d --name dojo-debug-crash -e APP_MODE=serve alpine:3.20 sh -c 'test "$APP_MODE" = serve || exit 1; echo "ready: serve"; exec sleep 3600' 
```'''

[[lab.steps.checks]]
name = "workload runs without restarts and has the required application mode"
run = "for i in $(seq 1 30); do if test \"$(docker inspect -f '{{.State.Running}} {{.HostConfig.RestartPolicy.Name}}' dojo-debug-crash)\" = 'true no' && test \"$(docker exec dojo-debug-crash printenv APP_MODE)\" = serve && docker logs dojo-debug-crash 2>/dev/null | grep -qx 'ready: serve'; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Correct a command that cannot start"
task = "Inspect **dojo-debug-command**, especially **State.Error**, **Path** and **Args**. Replace it with **alpine:3.20** running the valid command **sleep 3600**, detached with the same name."
hints = ["A misspelled executable can fail before the application writes logs.", "docker start reuses the existing command; create a corrected replacement."]
solution = '''```bash
docker inspect -f '{{json .State}} path={{.Path}} args={{json .Args}}' dojo-debug-command
docker rm -f dojo-debug-command
docker run -d --name dojo-debug-command alpine:3.20 sleep 3600
```'''

[[lab.steps.checks]]
name = "corrected command is running as sleep 3600"
run = "docker exec dojo-debug-command sh -c 'tr \"\\000\" \" \" < /proc/1/cmdline' | grep -Eq '(^|/)sleep 3600 $' && echo ok"
equals = "ok"

[[lab.steps]]
title = "Repair the missing published port"
task = "Verify nginx in **dojo-debug-port** answers internally on **80**. Then replace it, preserving **nginx:1.27-alpine**, and publish host port **18092** to container port **80**. The host URL **http://$PODDOJO_HOST:18092/** must serve the nginx welcome page."
hints = ["Use docker exec with wget to isolate the internal path.", "EXPOSE metadata does not publish a host port."]
solution = '''```bash
docker exec dojo-debug-port wget -qO- http://127.0.0.1:80/
docker inspect -f '{{json .HostConfig.PortBindings}}' dojo-debug-port
docker rm -f dojo-debug-port
docker run -d --name dojo-debug-port -p 18092:80 nginx:1.27-alpine
```'''

[[lab.steps.checks]]
name = "nginx responds through its required publication"
run = "for i in $(seq 1 30); do if docker inspect -f '{{range (index .HostConfig.PortBindings \"80/tcp\")}}{{println .HostPort}}{{end}}' dojo-debug-port | grep -qx 18092 && curl -fsS --max-time 2 \"http://$PODDOJO_HOST:18092/\" | grep -q 'Welcome to nginx'; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Fix the probe instead of disabling it"
task = "Inspect **dojo-debug-health** and its **State.Health.Log**. nginx serves port **80**, but the probe targets **81**. Replace it using **nginx:1.27-alpine** and a working HTTP health check against **127.0.0.1:80/**. Keep it running and wait for **healthy**. Do not disable the healthcheck or replace it with an unconditional success command."
hints = ["Healthcheck output is visible in State.Health.Log.", "wget -qO- returns nonzero on connection failures; explicitly propagate failure."]
solution = '''```bash
docker inspect -f '{{json .State.Health}}' dojo-debug-health
docker exec dojo-debug-health wget -qO- http://127.0.0.1:80/
docker rm -f dojo-debug-health
docker run -d --name dojo-debug-health --health-cmd 'wget -qO- http://127.0.0.1:80/ >/dev/null || exit 1' --health-interval 2s --health-timeout 2s --health-retries 3 nginx:1.27-alpine
for i in $(seq 1 30); do
  if test "$(docker inspect -f '{{.State.Health.Status}}' dojo-debug-health)" = healthy; then break; fi
  sleep 2
done
test "$(docker inspect -f '{{.State.Health.Status}}' dojo-debug-health)" = healthy
```'''

[[lab.steps.checks]]
name = "configured healthcheck is healthy and nginx serves HTTP internally"
run = "for i in $(seq 1 30); do if test \"$(docker inspect -f '{{.State.Running}} {{.State.Health.Status}}' dojo-debug-health)\" = 'true healthy' && docker exec dojo-debug-health wget -qO- -T 2 http://127.0.0.1:80/ | grep -q 'Welcome to nginx'; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90
+++

## Debug from the outside inward

A container problem is a chain of observable states, not an invitation to change random flags. First find whether the container exists. Then determine whether the main process started, whether it stays alive, whether it serves its intended interface, and whether clients can reach that interface. Finally compare the health probe with the real application contract.

```text
exists? -> process started? -> stays running? -> internal request works?
                                                 |
                                        host publication works?
                                                 |
                                        health probe agrees?
```

Capture the evidence before replacing a container. Removing it also removes its writable layer and convenient access to its state and log history. In production, preserve required diagnostics and data before a repair. In this disposable lab, each replacement is explicitly scoped to one broken `dojo-debug-` container, and no volumes hold application data.

## Start with inventory and state

`docker ps` shows running containers; `docker ps -a` also includes stopped, created and failed-start containers. Filter by the exact intended name so another application's status does not distract you. Inspect the whole State object before assuming a familiar exit code tells the whole story.

```bash
docker ps -a --filter name=^/dojo-debug-crash$
docker inspect -f '{{json .State}}' dojo-debug-crash
docker inspect -f 'path={{.Path}} args={{json .Args}} restarts={{.RestartCount}}' \
  dojo-debug-crash
```

`State.Status` distinguishes created, running, restarting and exited. `State.Error` can reveal runtime startup failures. `State.ExitCode` describes a completed process or startup outcome, and `StartedAt`/`FinishedAt` help reconstruct timing. A container in a restart loop may briefly look running between exits, so inspect RestartCount, logs and repeated observations rather than treating one snapshot as proof of stability.

| Evidence | Likely interpretation | Next useful question |
|---|---|---|
| Exited 0 | Main process completed successfully | Should this workload have been long-lived? |
| Exited 1 | Application-reported failure | What does stderr explain? |
| 126 / 127 | Commonly cannot execute / command not found | Is the path valid and executable? |
| 137 | Conventionally SIGKILL termination | Is OOMKilled true, or was it forcibly killed? |
| 143 | Conventionally SIGTERM termination | Was a normal stop requested? |
| Running, unhealthy | Main process alive, probe failing | Is the test correct and meaningful? |

Docker CLI errors also have their own codes, such as 125 for a Docker invocation failure; do not confuse the CLI's exit status with an application exit recorded in inspection. For a process that never starts, the runtime error may be more informative than any numeric code.

## Logs explain application decisions

`docker logs --tail 30 NAME` reads the retained container stdout/stderr stream with the configured logging support. Add `--timestamps` when ordering events matters. `--follow` is useful interactively but runs until interrupted, so use bounded output in scripted investigations. Logs do not automatically include arbitrary files written inside the container.

The first broken container deliberately prints a clear error and exits. Its `on-failure:5` policy retries the same broken workload. Waiting longer does not fix its missing configuration. Recreate it with the required environment, a valid long-lived command and no retry policy hiding the outcome.

```bash
docker logs --tail 30 --timestamps dojo-debug-crash
docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dojo-debug-crash
```

Real environments may contain secrets, so inspect only what you need and avoid copying unredacted environment dumps into tickets. The lab contains only harmless demonstration variables. A useful repair preserves the intended behavior and fixes its cause; changing every command to sleep merely hides a failing production application. Here sleep is the stated demonstration workload.

## Exec requires a running container

`docker exec NAME COMMAND` starts an additional process inside an already-running container. If the main process has exited, exec cannot resurrect it. Use logs and inspect first. If the image lacks a shell, `docker exec NAME sh` fails even while the application works; execute an available diagnostic binary or choose an appropriate external debugging approach.

```bash
docker exec dojo-debug-port wget -qO- http://127.0.0.1:80/
docker exec dojo-debug-command ps
docker top dojo-debug-command
```

A typo such as `sleeeep` can prevent the container's first process from starting at all. There may be no application log because no application ran. `docker start` repeats the stored executable and arguments; it does not let you supply a corrected command. Create a replacement with the right configuration. Likewise, Docker container `Cmd` overrides and entrypoints must be understood together when diagnosing an unexpected process.

## Separate listener failures from publication failures

First test the application internally. If nginx answers `http://127.0.0.1:80/` inside its container, you have evidence that the server is alive and responding there. Inspect the listener binding if a peer cannot reach it: binding only loopback inside the container would still prevent traffic to the container's external interface.

Then inspect publication and test through the selected host:

```bash
docker inspect -f '{{json .HostConfig.PortBindings}}' dojo-debug-port
docker port dojo-debug-port
curl -fsS --max-time 3 "http://$PODDOJO_HOST:18092/"
```

An `EXPOSE 80` declaration is not a host mapping. Port configuration is set during container creation, so replace the lab container with `-p 18092:80`. On a remote Docker machine, your computer's localhost is not the server. A correct mapping can still be blocked by host firewalls or an unreachable `PODDOJO_HOST`; trace that path before changing the application's port.

## Health is a separate state machine

A Docker health check runs a command inside the container and records its exit status and output. Zero means success; one indicates an unhealthy result. An interval, timeout, retry threshold and optional startup allowance determine when repeated failures change status. Inspect `State.Health.Log` to see what actually failed, not just the final word `unhealthy`.

The lab server listens on 80 while its probe targets 81. Disabling the probe would hide that mismatch. Correct the endpoint and verify the health status eventually becomes healthy. A standalone Docker restart policy does not restart a container merely because it is unhealthy. The main process can continue serving while the probe fails; the [Dockerfile health-check reference](https://docs.docker.com/reference/dockerfile/#healthcheck) explains the independent status.

The lab checks the resulting healthy state and the live HTTP response rather than requiring a particular diagnostic client. Keep a meaningful HTTP probe configured against the requested endpoint. A successful no-op such as `true` is not a meaningful substitute for checking the application. For real applications, choose an endpoint that reflects the intended contract without causing transactions as a side effect.

## Events add chronology

Events record lifecycle transitions such as start, die, restart and health status changes. Bound the time range so your diagnostic command finishes:

```bash
docker events --since 10m --until 0s \
  --filter container=dojo-debug-health
```

Events are not a permanent audit database; daemon history is limited. Combine them with timestamped logs and current inspection. A fast investigation follows a hypothesis: identify the failed layer, collect evidence for it, change the smallest relevant configuration, then repeat the request that originally failed.

## Why this matters for Kubernetes

The same method becomes `kubectl get pods`, `describe`, `logs`, `logs --previous`, `exec` and events. `CrashLoopBackOff` describes repeated container failures with backoff; it is not an application error message. Read the previous terminated state and previous logs. `ImagePullBackOff` means an earlier image acquisition problem, before your command or HTTP probe can run.

```yaml
# Excerpt inside spec.containers for a Pod in debugging-lab:
- name: web
  image: nginx:1.27-alpine
  ports:
    - containerPort: 80
  readinessProbe:
    httpGet: {path: /, port: 80}
    periodSeconds: 2
  livenessProbe:
    httpGet: {path: /, port: 80}
    initialDelaySeconds: 5
```

Readiness controls traffic eligibility; liveness can trigger a restart; startup probes protect slow initialization from premature checks. Kubernetes HTTP probes normally come from the kubelet to the Pod IP, unlike the shell command in this Docker exercise. For a quick evidence bundle use `kubectl -n debugging-lab describe pod dojo-web` and `kubectl -n debugging-lab logs dojo-web --previous`. If the application image lacks tools, consider `kubectl debug` subject to cluster permissions rather than modifying its image during the exam.
