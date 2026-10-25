+++
id = "docker-running-containers"
title = "Run, reach and control containers"
module = "containers"
minutes = 40
needs = "docker"
objectives = ["Choose run flags for services and one-shot commands", "Reach a published application port on the selected machine", "Inspect logs and execute additional processes inside a container", "Stop, start and remove named containers deliberately"]

[[quiz]]
q = "What does -p 18080:80 publish?"
choices = ["Container port 18080 on host port 80", "Port 80 only inside the container", "Host port 18080 to container port 80", "Every container port on the host"]
answer = 2
why = "The order is host:container. The process must listen on the container port and a reachable interface; EXPOSE alone would not publish anything."

[[quiz]]
q = "What does docker exec do?"
choices = ["Starts an additional process inside a running container", "Replaces PID 1 permanently", "Starts a stopped container", "Changes the image default command"]
answer = 0
why = "exec starts another process sharing the running container environment and namespaces. It neither changes the image nor restarts the main application."

[[quiz]]
q = "Why avoid --rm for a container whose failed startup you need to investigate?"
choices = ["It disables logs", "It removes the base image", "It prevents detached operation", "It removes the container after exit, including its inspectable state"]
answer = 3
why = "Automatic removal is useful for disposable commands, but the stopped object disappears. Without --rm you can inspect its exit code and logs after failure."

[[quiz]]
q = "Which command returns the last 20 log lines and then follows new output?"
choices = ["docker exec dojo-web logs", "docker logs --tail 20 -f dojo-web", "docker ps --tail 20 dojo-web", "docker logs --since 20 dojo-web"]
answer = 1
why = "--tail bounds the initial history, while -f streams subsequent output. --since instead selects logs using a timestamp or duration, such as 5m."

[[quiz]]
q = "What happens when you run docker start on a stopped container?"
choices = ["Docker builds its Dockerfile again", "Docker replaces it with the newest tag from the registry", "The existing container starts with its existing creation settings", "Docker deletes its writable layer"]
answer = 2
why = "start reuses the object, configuration and writable layer. Changing an environment variable or port mapping normally requires recreating the container."

[lab]
summary = "Publish an nginx service, change its response through exec, stop and resume it, and remove only the named lab container."
setup = ["docker rm -f dojo-run-web 2>/dev/null || true"]
cleanup = ["docker rm -f dojo-run-web 2>/dev/null || true"]

[[lab.steps]]
title = "Publish a background service"
task = "Run **dojo-run-web** from **nginx:1.27-alpine**, detached, with host port **18080** forwarding to container port **80**. Set container environment variable **DOJO_MODE=practice** and restart policy **unless-stopped**. Reach it using **http://$PODDOJO_HOST:18080**."
hints = ["Flags precede the image name: -d, --name, -p, -e and --restart.", "The first port in -p is the port on the machine selected in PodDojo."]
solution = '''
```bash
docker run -d --name dojo-run-web -p 18080:80 -e DOJO_MODE=practice --restart unless-stopped nginx:1.27-alpine
```

The engine owns the listener on port 18080. On an SSH machine, use PODDOJO_HOST rather than assuming localhost.
'''

[[lab.steps.checks]]
name = "nginx serves its default page on the published port"
run = "for i in $(seq 1 20); do if [ \"$(docker inspect -f '{{.Config.Image}} {{.State.Running}}' dojo-run-web 2>/dev/null)\" = 'nginx:1.27-alpine true' ] && curl -fsS --connect-timeout 2 --max-time 3 \"http://$PODDOJO_HOST:18080/\" | grep -q 'Welcome to nginx!'; then echo ready; exit 0; fi; sleep 1; done; exit 1"
equals = "ready"
timeout = 120

[[lab.steps.checks]]
name = "practice mode and restart policy are configured"
run = "docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}restart={{.HostConfig.RestartPolicy.Name}}' dojo-run-web | grep -E '^(DOJO_MODE=practice|restart=unless-stopped)$' "
equals = '''
DOJO_MODE=practice
restart=unless-stopped
'''

[[lab.steps]]
title = "Change the served content through exec"
task = "Use an additional process inside **dojo-run-web** to replace **/usr/share/nginx/html/index.html** with the single line **dojo running containers**. Read the access logs after requesting the new page."
hints = ["docker exec can run sh -c with a redirect inside the container.", "Use docker logs --tail 10 after curl; a file change needs no nginx restart."]
solution = '''
```bash
docker exec dojo-run-web sh -c 'printf "%s\n" "dojo running containers" > /usr/share/nginx/html/index.html'
curl -fsS --connect-timeout 2 --max-time 5 "http://$PODDOJO_HOST:18080/"
docker logs --tail 10 dojo-run-web
```

The file lives in the container writable layer. The immutable nginx image remains unchanged.
'''

[[lab.steps.checks]]
name = "the published service returns the new lesson page"
run = "for i in $(seq 1 20); do v=$(curl -fsS --connect-timeout 2 --max-time 3 \"http://$PODDOJO_HOST:18080/\" 2>/dev/null) && [ \"$v\" = 'dojo running containers' ] && echo ready && exit 0; sleep 1; done; exit 1"
equals = "ready"
timeout = 120

[[lab.steps]]
title = "Stop without deleting"
task = "Stop **dojo-run-web** gracefully, allowing up to **10 seconds**. Keep the stopped object for inspection."
hints = ["docker stop --time 10 NAME requests graceful shutdown before forcing termination.", "The desired state is exited, not absent."]
solution = '''
```bash
docker stop --time 10 dojo-run-web
```

A manual stop suppresses automatic restarts. The container object and its changed index page remain.
'''

[[lab.steps.checks]]
name = "the service container is stopped and retained"
run = "docker inspect -f '{{.State.Status}}' dojo-run-web"
equals = "exited"

[[lab.steps]]
title = "Resume the existing container"
task = "Start **dojo-run-web** again. Its published URL must still return **dojo running containers** without creating a replacement."
hints = ["docker start reuses stored port mappings and environment.", "You do not pass the image name to docker start."]
solution = '''
```bash
docker start dojo-run-web
```

The original writable layer survives stop/start, so the page changed in the earlier step remains available.
'''

[[lab.steps.checks]]
name = "the resumed container serves its retained page"
run = "for i in $(seq 1 20); do if [ \"$(docker inspect -f '{{.State.Running}}' dojo-run-web 2>/dev/null)\" = true ]; then v=$(curl -fsS --connect-timeout 2 --max-time 3 \"http://$PODDOJO_HOST:18080/\" 2>/dev/null) && [ \"$v\" = 'dojo running containers' ] && echo ready && exit 0; fi; sleep 1; done; exit 1"
equals = "ready"
timeout = 120

[[lab.steps]]
title = "Release the lab port"
task = "Stop and remove **dojo-run-web**. Remove only this lesson container; leave other workloads and all images alone."
hints = ["Stop first, then docker rm the exact name."]
solution = '''
```bash
docker stop --time 10 dojo-run-web
docker rm dojo-run-web
```

Removing the container releases its published port and discards its writable layer.
'''

[[lab.steps.checks]]
name = "the named lab container has been removed"
run = "docker ps -a --filter name=^/dojo-run-web$ --format '{{.Names}}' "
equals = ""
+++

## Read docker run from left to right

A container launch combines creation settings with a command. The dividing line is the image name:

```text
docker run [Docker options] IMAGE [command or arguments]
           └ engine settings ┘   └ application invocation ┘
```

For example, `docker run -d --name dojo-run-web -p 18080:80 nginx:1.27-alpine` creates an nginx container and returns immediately. Put `-d` after the image and it becomes an argument to the image's command instead of a Docker option. This position mistake can look like an application failure even though the flag itself is valid.

A service normally runs continuously in the foreground **inside** its container. `-d` only detaches the client. Do not make the server daemonize inside the container and leave PID 1 with nothing to do: when the main process exits, the container stops.

## Choose creation flags deliberately

| Flag | Purpose | Common mistake |
|---|---|---|
| `-d` | Detach the client from the running process | Assuming detach keeps an exited process alive |
| `--name dojo-run-web` | Assign a predictable handle | Reusing a name before removing its old object |
| `-p 18080:80` | Publish host TCP port to container TCP port | Reversing the two port numbers |
| `-e DOJO_MODE=practice` | Supply a process environment variable | Expecting it to alter application code automatically |
| `--rm` | Remove the container automatically after exit | Losing evidence from a failed service |
| `-it` | Keep stdin open and allocate a terminal | Using it in unattended scripts |
| `--restart unless-stopped` | Request automatic restarts | Combining it with incompatible `--rm` |
| `-w /tmp` | Override the process working directory | Confusing it with a host-directory mount |
| `--entrypoint /bin/sh` | Replace the image entrypoint | Forgetting to supply the new command's arguments |

`-i` and `-t` are separate options. A pipeline often needs only `-i`; a human shell usually benefits from both. In an interactive terminal, `docker exec -it dojo-run-web sh` enters a new shell. PodDojo solutions use non-interactive commands so they can run reliably without a terminal.

Environment variables are creation configuration. Running `docker start` later does not accept a replacement `-e` value. Recreate the disposable container when changing this configuration. Avoid embedding secrets in example environment values: inspect output exposes container configuration to anyone with daemon access.

## Publishing connects two network spaces

The container has its own network namespace. nginx listens on port 80 there, while the engine publishes port 18080 on its machine:

```text
curl on your computer
       │ http://$PODDOJO_HOST:18080
       ▼
selected Docker machine :18080 ──forward──▶ container :80
```

Use the supplied host address:

```bash
curl -fsS --connect-timeout 2 --max-time 5 "http://$PODDOJO_HOST:18080/"
docker port dojo-run-web
```

On a remote engine, `localhost:18080` on your laptop addresses your laptop, not the server. A container IP shown by inspect is also usually not directly reachable from the laptop. Published ports cross this boundary deliberately. Inside a container, a server binding only to `127.0.0.1` cannot normally be reached through its container interface; listen on an appropriate interface such as `0.0.0.0`.

Without a host IP, `-p` normally publishes on all host interfaces, subject to host networking and firewall configuration. That may expose the lab service to other machines. The lab uses a designated high port and removes the listener when finished. A local-only binding can be appropriate outside these remote-capable exercises, but binding to remote loopback would not make the service reachable from your laptop.

## List precisely, then inspect

`docker ps` shows running containers; `docker ps -a` includes stopped ones. Filters reduce noise, and formatting avoids fragile column parsing:

```bash
docker ps -a --filter name=^/dojo-run-web$ --format '{{.Names}} {{.Status}}'
docker ps --filter ancestor=nginx:1.27-alpine --format '{{.Names}}'
docker inspect -f '{{.State.Status}}' dojo-run-web
```

Name filters otherwise match substrings, which can accidentally include another lesson. Use anchored filters when an exact name matters. Listing matching objects is safe discovery; it is not a reason to feed every container ID into a destructive command. In this shared environment, remove only named lesson resources.

## Logs and exec answer different questions

Docker logs collects what the container process writes to stdout and stderr through the configured logging system. It does not automatically read every file in `/var/log`.

```bash
docker logs --tail 20 dojo-run-web
docker logs --since 5m dojo-run-web
```

For live diagnosis, `docker logs -f --tail 20 dojo-run-web` follows new lines until interrupted. Stop following with Ctrl-C; the detached server continues running. Prefer bounded history when investigating a noisy process so that useful recent output is visible immediately.

`docker exec` starts another process inside an existing running container:

```bash
docker exec dojo-run-web sh -c 'printf "%s\n" "$DOJO_MODE"'
docker exec -w /usr/share/nginx/html dojo-run-web ls
```

The quoted shell runs inside the container, so `$DOJO_MODE` expands there. A stopped container cannot accept exec. A minimal image may lack a shell or diagnostic tools; that is an image property, not proof that Docker is broken. Changes made with exec are useful experiments, but production changes belong in an image build or external configuration.

## Stop, kill, start, restart, remove

`docker stop --time 10 dojo-run-web` sends the configured stop signal, normally SIGTERM, then allows a grace period before SIGKILL. nginx images may specify a different stop signal for graceful shutdown. `docker kill dojo-run-web` sends SIGKILL by default and skips that opportunity. Use graceful stop for normal operations.

`docker start` starts the existing object; `docker restart` stops and starts it. Both retain its writable layer, environment, image ID and port mappings. Neither fetches a newer image just because its tag has moved. `docker rm` deletes a stopped container; `docker rm -f` also forcibly terminates a running one. Removal discards its writable layer, so it differs fundamentally from stop.

A quick diagnostic sequence is: list the exact container, inspect its state, read recent logs, then test its published endpoint. This separates process startup failures from application responses and network reachability problems without guessing.

## Why this matters for Kubernetes

The command-line choices have recognizable Kubernetes equivalents:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-run-web
  namespace: running-lab
spec:
  containers:
    - name: web
      image: nginx:1.27-alpine
      env:
        - name: DOJO_MODE
          value: practice
      ports:
        - containerPort: 80
```

This YAML describes the process environment and documents its listening port. `containerPort` does **not** publish a host port; Kubernetes normally uses a Service for stable network access. Docker's port mapping is not a direct translation to that field.

For CKAD, transfer the habits: choose the right container, read bounded logs, distinguish a process from its declaration, and verify a request end to end. Later, `kubectl logs` and `kubectl exec` will feel familiar, while controllers rather than your terminal manage replacement containers.
