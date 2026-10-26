+++
id = "docker-compose"
title = "Compose: one application, several containers"
module = "data-network"
minutes = 50
needs = "docker"
objectives = ["Define a multi-service application with Compose v2", "Gate startup on a dependency health check", "Use environment files and named persistent data", "Activate a profile, scale a service and remove project resources"]

[[quiz]]
q = "Does short-form depends_on guarantee Redis is ready for commands?"
choices = ["Yes; Compose waits until the Redis port accepts connections", "No; use a healthcheck and condition: service_healthy", "No; set `restart: always` on the dependent service", "No; use `links:` instead, which waits for readiness"]
answer = 1
why = "Creation order is not readiness. Long-form depends_on can wait for a passing dependency healthcheck, but applications still need recovery logic for later outages."

[[quiz]]
q = "What is the difference between .env and a service env_file?"
choices = ["Both always inject every variable into every container", ".env is normally an interpolation source; env_file populates the service environment", "env_file changes the daemon environment", "env_file feeds interpolation; .env populates the containers"]
answer = 1
why = "Compose uses .env values to interpolate the model. A service env_file explicitly passes variables to that service; environment entries can override those values."

[[quiz]]
q = "Why does every lab command specify -p dojo-compose?"
choices = ["It changes the Docker context", "It supplies an explicit project identity and predictable resource scope", "It publishes every service port", "It selects which compose file to load"]
answer = 1
why = "The project name scopes Compose-managed resources. An explicit dojo- project avoids accidental dependence on the lab directory name and makes targeted cleanup reliable."

[[quiz]]
q = "Which service is easiest to scale with --scale tools=2?"
choices = ["One with container_name fixed to dojo-tools", "One publishing the same fixed host port for each replica", "One without a fixed container_name or fixed host-port binding", "One publishing 8080:80 without a container_name"]
answer = 2
why = "Replicas need distinct container identities, and a fixed host port cannot be bound by two replicas on the same address. Scale a service whose configuration allows multiple instances."

[[quiz]]
q = "What extra action does down -v take in this lab?"
choices = ["Deletes all daemon volumes", "Removes project-declared named volumes and attached anonymous volumes, excluding external volumes", "Also removes the images built for the project", "Removes named volumes, including those marked external"]
answer = 1
why = "down removes this project containers and networks; -v also removes its managed volumes. External volumes are not removed. Treat the flag as deliberate data deletion, not routine application restart."

[lab]
summary = "Run a Python/Redis counter with health-gated startup, environment files, a debug profile and a complete project teardown."
setup = []
cleanup = ["docker compose -p dojo-compose -f compose.yaml --profile debug down -v --remove-orphans 2>/dev/null || true", "docker image rm dojo/compose-counter:v1 2>/dev/null || true"]

[lab.files]
"compose.yaml" = '''services:
  redis:
    image: redis:7.4-alpine
    command: ["redis-server", "--appendonly", "yes"]
    volumes:
      - type: volume
        source: data
        target: /data
    networks: [app]
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 2s
      timeout: 2s
      retries: 30
  web:
    build: .
    image: dojo/compose-counter:v1
    env_file: [app.env]
    environment:
      COUNTER_LABEL: "${COUNTER_LABEL:-dojo-counter}"
    ports: ["18100:8000"]
    networks: [app]
    depends_on:
      redis:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "python", "-c", "import urllib.request; assert urllib.request.urlopen('http://127.0.0.1:8000/health', timeout=1).read().strip() == b'ok'"]
      interval: 2s
      timeout: 2s
      retries: 30
  tools:
    image: redis:7.4-alpine
    command: ["sleep", "3600"]
    profiles: [debug]
    networks: [app]
networks:
  app:
    name: dojo-compose-net
    ipam:
      config:
        - subnet: 10.214.11.0/24
volumes:
  data:
    name: dojo-compose-data
'''
"Dockerfile" = '''FROM python:3.12-alpine
WORKDIR /app
COPY app.py /app/app.py
EXPOSE 8000
CMD ["python", "-u", "/app/app.py"]
'''
"app.env" = '''REDIS_HOST=redis
'''
".env" = '''COUNTER_LABEL=dojo-counter
'''
"app.py" = '''import os
import socket
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def redis_command(*parts):
    # One RESP command per connection: no external Python packages required.
    request = f"*{len(parts)}\r\n".encode()
    for part in parts:
        value = part.encode()
        request += f"${len(value)}\r\n".encode() + value + b"\r\n"
    with socket.create_connection((os.environ["REDIS_HOST"], 6379), timeout=2) as sock:
        sock.sendall(request)
        with sock.makefile("rb") as stream:
            line = stream.readline()
            if not line:
                raise RuntimeError("Redis closed the connection")
            kind, payload = line[:1], line[1:-2]
            if kind in (b"+", b":"):
                return payload.decode()
            if kind == b"$":
                length = int(payload)
                if length == -1:
                    return "0"
                value = stream.read(length)
                stream.read(2)
                return value.decode()
            raise RuntimeError("Unexpected Redis response")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        try:
            if self.path == "/health":
                if redis_command("PING") != "PONG":
                    raise RuntimeError("Redis not ready")
                value = "ok"
            elif self.path == "/value":
                value = redis_command("GET", "dojo-counter")
            elif self.path == "/label":
                value = os.environ["COUNTER_LABEL"]
            elif self.path == "/":
                # Remember when the counter was first used (UTC, like docker inspect's Created).
                redis_command("SET", "dojo-first-hit", time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime()), "NX")
                value = redis_command("INCR", "dojo-counter")
            else:
                self.send_error(404)
                return
            data = (value + "\n").encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except (OSError, ValueError, RuntimeError):
            self.send_error(503, "Redis unavailable")


ThreadingHTTPServer(("0.0.0.0", 8000), Handler).serve_forever()
'''

[[lab.steps]]
title = "Start the stateful dependency"
task = "Inspect the supplied **compose.yaml**, **Dockerfile**, **app.py**, **app.env** and **.env**. Validate the model and start only **redis** under project **dojo-compose**. Wait until it is healthy; its data must use **dojo-compose-data**."
hints = ["Always specify -p dojo-compose and -f compose.yaml.", "up -d --wait redis waits for the Redis healthcheck."]
solution = '''```bash
docker compose -p dojo-compose -f compose.yaml config -q
docker compose -p dojo-compose -f compose.yaml up -d --wait --wait-timeout 90 redis
```'''

[[lab.steps.checks]]
name = "Redis is healthy and stores data on the named project volume"
run = "id=$(docker compose -p dojo-compose -f compose.yaml ps -q redis) && test -n \"$id\" && test \"$(docker inspect -f '{{.State.Health.Status}}' \"$id\")\" = healthy && test \"$(docker inspect -f '{{range .Mounts}}{{if eq .Destination \"/data\"}}{{.Name}}{{end}}{{end}}' \"$id\")\" = dojo-compose-data && echo ok"
equals = "ok"

[[lab.steps]]
title = "Build and start the frontend"
task = "Build and start **web**, keeping the supplied service_healthy dependency on Redis. Wait for it to become healthy. Its **/health** endpoint on host port **18100** must return **ok** and **/label** must return **dojo-counter** from the supplied environment configuration."
hints = ["Compose sends the build context to the selected engine; no bind mount is needed.", "The .env interpolation source and app.env container environment have separate roles."]
solution = '''```bash
docker compose -p dojo-compose -f compose.yaml up -d --build --wait --wait-timeout 90 web
curl -fsS "http://$PODDOJO_HOST:18100/health"
curl -fsS "http://$PODDOJO_HOST:18100/label"
```'''

[[lab.steps.checks]]
name = "healthy web serves the configured label through port 18100"
run = "for i in $(seq 1 30); do id=$(docker compose -p dojo-compose -f compose.yaml ps -q web); if test -n \"$id\" && test \"$(docker inspect -f '{{.State.Health.Status}}' \"$id\")\" = healthy && test \"$(curl -fsS --max-time 2 \"http://$PODDOJO_HOST:18100/health\")\" = ok && test \"$(curl -fsS --max-time 2 \"http://$PODDOJO_HOST:18100/label\")\" = dojo-counter; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Exercise the counter and preserve its data"
task = "Request **/** at least twice so the counter reaches **2 or more**. Force-recreate **web** and **redis** without removing their volume, then confirm **/value** still reports at least 2. Use ps and logs to inspect this project."
hints = ["/ increments the counter; /value only reads it.", "Redis uses append-only persistence; flush it before deliberately recreating the server."]
solution = '''```bash
curl -fsS "http://$PODDOJO_HOST:18100/"
curl -fsS "http://$PODDOJO_HOST:18100/"
docker compose -p dojo-compose -f compose.yaml exec -T redis redis-cli SAVE
docker compose -p dojo-compose -f compose.yaml up -d --force-recreate --wait --wait-timeout 90 redis web
curl -fsS "http://$PODDOJO_HOST:18100/value"
docker compose -p dojo-compose -f compose.yaml ps
docker compose -p dojo-compose -f compose.yaml logs --tail 20 web redis
```'''

[[lab.steps.checks]]
name = "counter state is at least two and available in Redis-backed web"
run = "v=$(curl -fsS --max-time 3 \"http://$PODDOJO_HOST:18100/value\") && case \"$v\" in ''|*[!0-9]*) exit 1;; esac; test \"$v\" -ge 2 && echo ok"
equals = "ok"

[[lab.steps.checks]]
name = "web and redis were recreated after the counter was used"
run = '''
hit=$(docker compose -p dojo-compose -f compose.yaml exec -T redis redis-cli GET dojo-first-hit | cut -c1-19)
[ -n "$hit" ] || { echo "the counter has not been used yet"; exit 1; }
for svc in redis web; do
  id=$(docker compose -p dojo-compose -f compose.yaml ps -q "$svc")
  [ -n "$id" ] || { echo "$svc is not running"; exit 1; }
  created=$(docker inspect -f '{{.Created}}' "$id" | cut -c1-19)
  [ "$(printf '%s\n%s\n' "$hit" "$created" | sort | tail -n 1)" = "$created" ] || { echo "$svc was created at $created, before the first request at $hit"; exit 1; }
done
echo ok
'''
equals = "ok"
timeout = 60

[[lab.steps.negatives]]
name = "increments the counter but never recreates the containers"
run = '''
curl -fsS "http://$PODDOJO_HOST:18100/"
curl -fsS "http://$PODDOJO_HOST:18100/"
sleep 1
'''

[[lab.steps.negatives]]
name = "restarts the containers instead of recreating them"
run = '''
docker compose -p dojo-compose -f compose.yaml restart redis web
docker compose -p dojo-compose -f compose.yaml up -d --wait --wait-timeout 90 redis web
'''

[[lab.steps.negatives]]
name = "recreates everything but deletes the volume with down -v"
run = '''
docker compose -p dojo-compose -f compose.yaml down -v
docker compose -p dojo-compose -f compose.yaml up -d --wait --wait-timeout 90 redis web
'''

[[lab.steps.alternatives]]
name = "down without -v, then up again"
run = '''
sleep 1
curl -fsS "http://$PODDOJO_HOST:18100/"
curl -fsS "http://$PODDOJO_HOST:18100/"
docker compose -p dojo-compose -f compose.yaml exec -T redis redis-cli SAVE
docker compose -p dojo-compose -f compose.yaml down
docker compose -p dojo-compose -f compose.yaml up -d --wait --wait-timeout 90 redis web
'''

[[lab.steps]]
title = "Activate and scale a debug profile"
task = "Activate profile **debug** and run **two tools replicas**. Keep web and Redis healthy. Each tools container must be able to resolve **redis** and get **PONG** using redis-cli."
hints = ["Use --profile debug before up and --scale tools=2 with up.", "There is no fixed container_name or published host port on tools."]
solution = '''```bash
docker compose -p dojo-compose -f compose.yaml --profile debug up -d --scale tools=2 tools
for id in $(docker compose -p dojo-compose -f compose.yaml --profile debug ps -q tools); do
  docker exec "$id" redis-cli -h redis ping
done
```'''

[[lab.steps.checks]]
name = "two running tools replicas can reach healthy Redis while web remains healthy"
run = '''ids=$(docker compose -p dojo-compose -f compose.yaml --profile debug ps -q tools) && test "$(printf '%s
' "$ids" | sed '/^$/d' | wc -l | tr -d ' ')" = 2 && for id in $ids; do test "$(docker exec "$id" redis-cli -h redis ping)" = PONG || exit 1; done && test "$(curl -fsS --max-time 3 "http://$PODDOJO_HOST:18100/health")" = ok && echo ok'''
equals = "ok"

[[lab.steps]]
title = "Remove this disposable application and its data"
task = "Stop and remove **dojo-compose**, including its debug-profile containers, **dojo-compose-net** and **dojo-compose-data**. This lab explicitly authorizes deleting this disposable counter data. Leave other projects untouched."
hints = ["down -v removes project-managed volumes.", "Include --profile debug so the tools service is in the active model."]
solution = '''```bash
docker compose -p dojo-compose -f compose.yaml --profile debug down -v --remove-orphans
```'''

[[lab.steps.checks]]
name = "no project containers remain"
run = "test -z \"$(docker ps -a --filter label=com.docker.compose.project=dojo-compose --format '{{.ID}}')\" && echo ok"
equals = "ok"

[[lab.steps.checks]]
name = "project network is gone"
run = "test -z \"$(docker network ls --filter name=^dojo-compose-net$ --format '{{.Name}}')\" && echo ok"
equals = "ok"

[[lab.steps.checks]]
name = "disposable counter volume is gone"
run = "test -z \"$(docker volume ls --filter name=^dojo-compose-data$ --format '{{.Name}}')\" && echo ok"
equals = "ok"
+++

## Describe the application, not a sequence of terminals

A web application often needs several cooperating processes: a frontend, a database and occasional diagnostic tools. Separate `docker run` commands work, but they scatter configuration across shell history. Compose v2 reads one application model and reconciles it into containers, networks and volumes on the selected Docker engine. Use `docker compose`, the CLI plugin, rather than the older standalone `docker-compose` spelling.

The main unit is a **service**: a definition from which Compose creates one or more containers. A **project** is one deployment of that model. Explicitly selecting `-p dojo-compose` makes the project independent of your lab directory name. It also scopes labels and default resource names so you can inspect and remove only the intended application.

```text
project dojo-compose
  web :8000 ── DNS name redis:6379 ── redis
      ▲                                  │
host :18100                       dojo-compose-data
             network dojo-compose-net
```

## Read the model from its dependencies outward

The supplied app uses Python's standard library and speaks a tiny subset of Redis's protocol. No package download or remote bind mount is required. A Docker build sends the local build context to the selected engine, which copies `app.py` into the image. Redis stores a counter and enables append-only persistence on its named volume.

```yaml
services:
  redis:
    image: redis:7.4-alpine
    command: ["redis-server", "--appendonly", "yes"]
    volumes:
      - data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 2s
      timeout: 2s
      retries: 30
  web:
    build: .
    image: dojo/compose-counter:v1
    ports: ["18100:8000"]
    depends_on:
      redis:
        condition: service_healthy
volumes:
  data:
    name: dojo-compose-data
```

This excerpt shows the central relationship; the supplied file also declares an explicit application network, frontend health check, environment sources and debug tools. Modern Compose does not need a top-level `version` field to select an old file format. Validate the resolved model before starting anything: indentation and interpolation mistakes are easier to fix before containers exist.

```bash
docker compose -p dojo-compose -f compose.yaml config -q
docker compose -p dojo-compose -f compose.yaml up -d --build --wait
docker compose -p dojo-compose -f compose.yaml ps
```

Compose normally provides service-name DNS on shared application networks. The web service connects to `redis:6379`, not to localhost and not to a published host port. Redis does not need a published port for this private connection. Publish only the frontend endpoint the external client needs.

## Startup order is not readiness

Starting the Redis process does not prove it can answer commands. Short-form `depends_on: [redis]` establishes order; a passing health check is a stronger signal. Long-form `condition: service_healthy` tells Compose to wait for the dependency's configured health check before creating the dependent service. The [startup-order documentation](https://docs.docker.com/compose/how-tos/startup-order/) explains this distinction.

The health check executes inside the Redis container, so `redis-cli ping` uses Redis's own local listener. The web health check executes inside the web container and tests its HTTP `/health` route, which itself performs a Redis PING. Together these distinguish a running process from a functioning application path.

This is startup coordination, not continuous dependency orchestration. If Redis fails later, Compose does not automatically stop every dependent service or promise to restart them in a dependency-aware recovery sequence. Applications need retries, bounded timeouts and useful error responses. Our frontend returns HTTP 503 when its Redis connection fails instead of silently claiming success.

`up --wait` waits for services to be running or healthy, depending on whether they define a health check. Use `--wait-timeout` to bound that wait. A health check that calls a nonexistent executable will fail even if the application itself works; the test tool must be present in the image.

## Environment files have two different jobs

The automatically read `.env` file is normally a source for **Compose interpolation**, such as `${COUNTER_LABEL:-dojo-counter}`. It does not inject every variable into every container by itself. A service's `env_file: [app.env]` explicitly populates that container's environment. Explicit `environment` entries can override values from `env_file`.

| Mechanism | Consumer | Lab example |
|---|---|---|
| `.env` | Compose model interpolation | `COUNTER_LABEL=dojo-counter` |
| `env_file` | Selected service environment | `REDIS_HOST=redis` |
| `environment` | Selected service environment | Interpolated counter label |
| Shell environment | Compose process, including interpolation precedence | A temporary label override |

Use `config` to understand the final model, but remember resolved output can reveal environment values. Do not put real credentials in committed environment files or diagnostic logs. Container environment variables are not a dedicated secrets vault. If a shell command in Compose should expand a variable **inside** the container rather than during model interpolation, use `$$` to escape Compose's dollar processing.

## Observe before recreating

`ps` shows project service state and published ports. `logs --tail 20 web redis` combines recent output with service attribution. `exec -T` runs a non-interactive command inside an existing service container; `-T` avoids allocating a terminal for scripted work. `run --rm` creates a separate one-off service container and has different lifecycle and port-publication behavior.

The counter's `/` route increments Redis state. `/value` only reads it; `/health` probes connectivity without changing the counter. This distinction matters for monitoring: a readiness test should not accidentally create business transactions. A database health check can be read-only even though a normal application endpoint changes state.

Recreating web should preserve the counter because state lives in Redis, not the Python process. Recreating Redis should preserve it because its data lives on the named volume. Persistence still has a durability policy: an abrupt failure can lose writes not yet flushed. The lab requests `SAVE` before a deliberate recreation and uses append-only persistence; a mounted directory alone does not configure database durability.

## Profiles and replicas express optional work

The `tools` service belongs to profile `debug`, so normal startup leaves it inactive. Enable that profile deliberately and scale the tool service to two replicas:

```bash
docker compose -p dojo-compose -f compose.yaml --profile debug \
  up -d --scale tools=2 tools
docker compose -p dojo-compose -f compose.yaml --profile debug ps
```

Do not set a fixed `container_name` on a service you intend to scale. Likewise, two replicas cannot both bind the same fixed host port on the same host address. Our tools publish no ports, so both can join the network and contact Redis. Explicitly targeting a profiled service can activate it for that command; using `--profile` makes the intent visible. See the [Compose service reference](https://docs.docker.com/reference/compose-file/services/) for profiles, replicas and dependency syntax.

## Why this matters for Kubernetes

`stop` preserves containers and data. `down` removes project containers and managed networks; `down -v` additionally deletes managed volumes, including this lab's disposable counter storage. External networks and volumes have separate ownership. Every command here names the project to avoid affecting an unrelated deployment.

Kubernetes uses Deployments for replaceable application replicas, Services for stable discovery, PVCs for persistent storage and ConfigMaps or Secrets for configuration. Compose's `depends_on` has no direct Pod-field equivalent. Use readiness probes, application retry logic and, where justified, init containers. A Kubernetes readiness probe controls traffic eligibility; it does not mean other applications must wait before their Pods are created. Carry the dependency mental model forward without translating Compose keys mechanically into Pod YAML.
