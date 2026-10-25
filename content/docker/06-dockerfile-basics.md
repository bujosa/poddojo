+++
id = "docker-dockerfile-basics"
title = "Build an application with a Dockerfile"
module = "images"
minutes = 45
needs = "docker"
objectives = ["Write a reproducible Dockerfile from a pinned base image", "Separate build context, image configuration and runtime overrides", "Combine ENTRYPOINT and CMD while preserving signal delivery", "Build and verify a non-root HTTP service"]
domain = "design-build"

[[quiz]]
q = "What does EXPOSE 8080 do?"
choices = ["Publishes host port 8080", "Changes the application listening port", "Documents the intended container port without publishing it", "Creates a Kubernetes Service"]
answer = 2
why = "EXPOSE is metadata. The process must actually listen on that port, and Docker requires -p or another explicit publishing mechanism for host access."

[[quiz]]
q = "Which input is available during build without automatically becoming a runtime environment variable?"
choices = ["ARG", "ENV", "USER", "EXPOSE"]
answer = 0
why = "ARG is scoped build-time input. ENV persists in image runtime configuration. Neither is a safe place for secrets; build metadata, history or resulting files can reveal inputs."

[[quiz]]
q = "With ENTRYPOINT [\"python\"] and CMD [\"app.py\"], what does docker run IMAGE other.py do?"
choices = ["Starts a shell", "Runs app.py and then other.py", "Replaces the entrypoint with other.py", "Runs python other.py"]
answer = 3
why = "Arguments after the image replace CMD and are appended to the exec-form ENTRYPOINT. --entrypoint is the separate flag for replacing the executable."

[[quiz]]
q = "Why prefer COPY over ADD for an ordinary source file?"
choices = ["COPY automatically extracts every archive", "COPY expresses a straightforward local copy without ADD special behavior", "ADD cannot copy files", "COPY downloads a newer application version"]
answer = 1
why = "ADD has additional behaviors such as local archive extraction and remote sources. Use COPY for normal context files so the Dockerfile communicates exactly what is intended."

[[quiz]]
q = "Why place frequently changing application files after stable dependency installation?"
choices = ["Docker requires alphabetical instructions", "COPY does not affect the cache", "A source change then invalidates fewer expensive cache-dependent steps", "It guarantees no registry access"]
answer = 2
why = "Cache reuse depends on inputs and previous steps. Keeping stable dependency inputs earlier avoids repeating expensive work for every source edit, while changes to those inputs still invalidate the right work."

[lab]
summary = "Build the supplied Python HTTP app as a non-root image, publish it with a runtime override, and replace its startup command for a one-shot check."
setup = ["docker rm -f dojo-build-web dojo-build-check 2>/dev/null || true", "docker image rm dojo/dockerfile:v1 2>/dev/null || true", "rm -f app/Dockerfile"]
cleanup = ["docker rm -f dojo-build-web dojo-build-check 2>/dev/null || true", "docker image rm dojo/dockerfile:v1 2>/dev/null || true", "rm -f app/Dockerfile || true"]

[lab.files]
"app/app.py" = '''
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = (os.environ.get("GREETING", "hello from image") + "\n").encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

HTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
'''
"app/.dockerignore" = '''
__pycache__
*.pyc
.git
*.local
'''
"app/notes.local" = '''
local build notes; exclude this file from the image
'''

[[lab.steps]]
title = "Describe and build the image"
task = "Write **app/Dockerfile** for the supplied **app.py**. Use **python:3.12-alpine**, working directory **/app**, default **GREETING=hello from image**, and runtime user **10001:10001**. Copy the context while respecting the supplied .dockerignore. Document port **8080**, use exec-form Python startup, and set OCI version label **1.0** through build argument **APP_VERSION**. Build **dojo/dockerfile:v1** using **app** as the context."
hints = ["WORKDIR sets the destination for relative COPY and the runtime directory.", "Use ARG APP_VERSION=1.0 and LABEL org.opencontainers.image.version=$APP_VERSION."]
solution = '''
```bash
cat > app/Dockerfile <<'EOF'
FROM python:3.12-alpine
ARG APP_VERSION=1.0
LABEL org.opencontainers.image.version=$APP_VERSION
WORKDIR /app
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 GREETING="hello from image"
COPY . .
RUN python -m py_compile app.py
EXPOSE 8080
USER 10001:10001
ENTRYPOINT ["python"]
CMD ["app.py"]
EOF
docker build --build-arg APP_VERSION=1.0 -t dojo/dockerfile:v1 app
```

The context is app, not the whole lab folder. The runtime identity can be numeric without adding a named account to the base image.
'''

[[lab.steps.checks]]
name = "the image has the requested runtime identity and directory"
run = "docker image inspect -f '{{.Config.User}} {{.Config.WorkingDir}}' dojo/dockerfile:v1"
equals = "10001:10001 /app"

[[lab.steps.checks]]
name = "the image documents its release and listening port"
run = "docker image inspect -f '{{index .Config.Labels \"org.opencontainers.image.version\"}} {{range $port, $bindings := .Config.ExposedPorts}}{{if eq $port \"8080/tcp\"}}8080/tcp{{end}}{{end}}' dojo/dockerfile:v1"
equals = "1.0 8080/tcp"

[[lab.steps.checks]]
name = "the image retains its default greeting"
run = "docker image inspect -f '{{range .Config.Env}}{{println .}}{{end}}' dojo/dockerfile:v1 | grep -Fx 'GREETING=hello from image'"
equals = "GREETING=hello from image"

[[lab.steps]]
title = "Run and verify the application"
task = "Run **dojo-build-web** from **dojo/dockerfile:v1**, detached, publishing **18081:8080**. Override **GREETING=hello from runtime**. Its HTTP response must equal that greeting. Confirm the process runs as UID **10001** and the excluded **/app/notes.local** was not copied."
hints = ["-e changes the runtime environment without rebuilding.", "The app listens on 0.0.0.0:8080; use PODDOJO_HOST for the machine address."]
solution = '''
```bash
docker run -d --name dojo-build-web -p 18081:8080 -e 'GREETING=hello from runtime' dojo/dockerfile:v1
```

The immutable image supplies defaults; this container supplies an environment override. Python uses an unprivileged port, so it does not need root.
'''

[[lab.steps.checks]]
name = "the HTTP response uses the runtime greeting"
run = "for i in $(seq 1 20); do v=$(curl -fsS --connect-timeout 2 --max-time 3 \"http://$PODDOJO_HOST:18081/\" 2>/dev/null) && [ \"$v\" = 'hello from runtime' ] && echo ready && exit 0; sleep 1; done; exit 1"
equals = "ready"
timeout = 120

[[lab.steps.checks]]
name = "the container is non-root and the local-only file is absent"
run = "test \"$(docker inspect -f '{{.Config.Image}}' dojo-build-web)\" = dojo/dockerfile:v1 && test \"$(docker exec dojo-build-web id -u)\" = 10001 && docker exec dojo-build-web test ! -e /app/notes.local"

[[lab.steps]]
title = "Override startup without modifying the image"
task = "Run a one-shot container named **dojo-build-check** from **dojo/dockerfile:v1**. Replace the entrypoint with **/bin/sh**, set its working directory to **/tmp**, and have it print exactly **override:/tmp** and exit successfully. Keep the exited container for logs and inspection."
hints = ["--entrypoint /bin/sh replaces the executable.", "Pass -c and the shell script after the image name; do not use -it or --rm."]
solution = '''
```bash
docker run --name dojo-build-check -w /tmp --entrypoint /bin/sh dojo/dockerfile:v1 -c 'printf "override:%s\n" "$PWD"'
docker logs dojo-build-check
```

This is a runtime override on a new object. The image default remains suitable for starting the HTTP app.
'''

[[lab.steps.checks]]
name = "the override ran from the requested directory and exited successfully"
run = "test \"$(docker inspect -f '{{.State.Status}} {{.State.ExitCode}} {{.Config.WorkingDir}}' dojo-build-check)\" = 'exited 0 /tmp' && docker logs dojo-build-check"
equals = "override:/tmp"
+++

## A Dockerfile is a build recipe, not a startup script

A Dockerfile describes how to construct an image and which defaults to attach to it. The build executes instructions and produces immutable content. A later `docker run` creates a container from that result. Keeping these phases distinct prevents a common misunderstanding: `RUN python app.py` starts Python **during the build**, whereas `CMD` describes a default invocation **when a container starts**.

```text
source files + Dockerfile + context
                 │ docker build
                 ▼
       immutable image + defaults
                 │ docker run + overrides
                 ▼
          container process
```

`RUN python -m py_compile app.py` checks source syntax during build; it does not start the server. The lab uses Python's standard-library HTTP support, so there is no package manager or framework to distract from the build contract. Python must remain in the runtime image because the application is Python source; copying the script alone into an empty filesystem would not make it executable.

## Start from a deliberate base

```dockerfile
FROM python:3.12-alpine
WORKDIR /app
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1
COPY app.py .
RUN python -m py_compile app.py
EXPOSE 8080
USER 10001:10001
ENTRYPOINT ["python"]
CMD ["app.py"]
```

`FROM` selects the base filesystem and its inherited configuration. Use a versioned tag, and use a verified digest when stronger content pinning is required. A tag fixes a release family more narrowly than an unversioned reference but can still receive updates. Rebuild intentionally when adopting fixes.

`WORKDIR /app` creates the directory if needed and sets the location for subsequent relative build instructions and the container's default working directory. It is clearer and safer than repeatedly writing `RUN cd /app && ...`, whose directory change lasts only for that instruction's shell.

`USER 10001:10001` sets the runtime UID and GID for later instructions and the default process. It does not automatically transfer ownership of copied files. The lab files are readable, and the app needs no write access to its source directory. If an application writes data, prepare an appropriate writable path and ownership rather than reverting to root.

## Know which instructions create which effects

| Instruction | Build-time or runtime effect |
|---|---|
| `RUN` | Executes a build command, recording resulting filesystem changes |
| `COPY` | Copies selected context files into the image |
| `ADD` | Adds files with extra capabilities such as local archive extraction |
| `ARG` | Declares a scoped build input |
| `ENV` | Persists environment defaults for subsequent build work and runtime |
| `EXPOSE` | Documents a container port; publishes nothing |
| `CMD` | Supplies default command or arguments |
| `ENTRYPOINT` | Defines the primary invocation to which arguments can be appended |

Use `COPY` for ordinary application files. Choose `ADD` only when its additional behavior is intentional. Neither should be a workaround for a badly chosen context. The [Dockerfile reference](https://docs.docker.com/reference/dockerfile/) documents instruction details and scope rules.

A shell-form `RUN` can combine related operations so temporary artifacts are removed before the changeset is recorded. That does not undo files added in an earlier immutable layer. Avoid adding secrets to any layer, even if a later instruction deletes them.

## The build context is the input boundary

```bash
docker build -t dojo/dockerfile:v1 app
```

Here `app` is the context directory, and Docker normally reads `app/Dockerfile`. `COPY . .` means copy the context contents into the current image working directory; it does not mean copy files next to whatever shell directory you imagined. A Dockerfile cannot freely copy arbitrary parent directories outside its context.

A `.dockerignore` in the context root excludes inputs before they are made available to ordinary copies:

```text
.git
__pycache__
*.pyc
*.local
```

Exclude irrelevant files, editor output and local-only material. This reduces transfer and cache churn. It also avoids accidentally including files that should never enter the image, although exclusion rules are not a substitute for proper secret handling. When the daemon is remote, build tooling transfers context inputs to the builder; `COPY` does not read an arbitrary same-named directory on the SSH server.

The lab includes `notes.local` and checks that it is absent from the running filesystem. This tests the result rather than insisting on a particular sequence of Dockerfile lines.

## Separate build arguments from environment

```dockerfile
ARG APP_VERSION=1.0
LABEL org.opencontainers.image.version=$APP_VERSION
ENV GREETING="hello from image"
```

```bash
docker build --build-arg APP_VERSION=1.0 -t dojo/dockerfile:v1 app
docker run -d --name dojo-build-web -e 'GREETING=hello from runtime' -p 18081:8080 dojo/dockerfile:v1
```

The argument selects a value while building. The label records that release value as metadata. The environment default remains available to the runtime process and can be overridden per container. An `ARG` declared before `FROM` has different scope from one declared inside a stage; redeclare it inside a stage when that stage needs it.

Do not use ARG or ENV to smuggle credentials into builds. Build inputs can appear in metadata, provenance, history or generated output. Real secret handling uses a mechanism designed for temporary secret mounts, with careful attention to what the build writes.

## Understand the command matrix

With exec-form `ENTRYPOINT ["python"]` and `CMD ["app.py"]`:

| Launch | Effective invocation |
|---|---|
| `docker run IMAGE` | `python app.py` |
| `docker run IMAGE other.py` | `python other.py` |
| `docker run --entrypoint /bin/sh IMAGE -c 'pwd'` | `/bin/sh -c 'pwd'` |

Exec form is a JSON array with double-quoted strings. It does not invoke a shell, so `$HOME` is not automatically expanded and `&&` is not a command separator. Shell form introduces `/bin/sh -c`; that can affect signal forwarding. If a wrapper is necessary, end it with `exec` so the final application replaces the wrapper process.

For a service, start the real foreground process. Do not use `RUN` to start a server during build, and do not use a shell background operator to make PID 1 exit immediately. Prefer predictable argument arrays and put configuration into explicit environment variables or files.

## Cache follows inputs and dependencies

A builder can reuse an earlier result when an instruction and its inputs are unchanged. Put stable, expensive dependency work before frequently changing source code. For a Python project with dependencies, copy the dependency manifest, install dependencies, then copy the app. The lab has no external dependencies, so it needs no artificial installation step.

A changed `COPY` input invalidates dependent work. An unchanged package installation instruction can remain cached even when newer packages exist upstream; cache reuse is not a security update policy. Use deliberate pulls and rebuilds when adopting updates. `--no-cache` forces rebuild work but does not, by itself, guarantee a newly pulled base.

Inspect `docker image history dojo/dockerfile:v1` and the build output to understand reuse. Measure application behavior after a rebuild; a successful build only proves the instructions completed, not that the server answers requests.

## Why this matters for Kubernetes

Image defaults become Pod defaults. Kubernetes `command` replaces the image entrypoint; `args` replaces its default arguments:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-build-web
  namespace: dockerfile-lab
spec:
  containers:
    - name: web
      image: dojo/dockerfile:v1
      imagePullPolicy: IfNotPresent
      command: ["python"]
      args: ["app.py"]
      env:
        - name: GREETING
          value: hello from Kubernetes
      ports:
        - containerPort: 8080
```

This is an illustrative translation. Load the image into kind before a future cluster exercise uses it. For CKAD, remember the command/args mapping and that declaring a port neither starts a listener nor creates a Service. Check the process and its response, not just the image tag.
