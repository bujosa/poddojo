+++
id = "docker-multi-stage-builds"
title = "Multi-stage builds and small runtime images"
module = "images"
minutes = 45
needs = "docker"
objectives = ["Separate build tools from the final runtime filesystem", "Order build inputs for useful cache reuse", "Choose a runtime base with the required libraries and tools", "Run a non-root image with release labels and an application health check"]
domain = "design-build"

[[quiz]]
q = "What does a new FROM instruction do?"
choices = ["Adds the previous stage to the final image automatically", "Starts a new build stage with its own base and configuration", "Deletes the build context", "Starts a container in Kubernetes"]
answer = 1
why = "A stage begins from its specified base. Only deliberately copied artifacts or an explicitly inherited stage carry earlier filesystem content into it."

[[quiz]]
q = "What should COPY --from=build usually transfer?"
choices = ["The entire compiler installation", "The Docker socket", "Every cached layer", "Only the runtime artifacts and required supporting files"]
answer = 3
why = "The separation works because the final image receives only what execution needs. Copying an entire build filesystem defeats the size and attack-surface benefit."

[[quiz]]
q = "Why can a binary copied into scratch fail to start even though the file exists?"
choices = ["The binary may require a missing dynamic loader or shared libraries", "scratch always requires a shell", "scratch is an ARM-only image", "COPY cannot preserve executable files"]
answer = 0
why = "An empty image has no loader, libraries, CA bundle, timezone data or shell unless supplied. A genuinely static binary still needs any non-binary runtime data its behavior depends on."

[[quiz]]
q = "What does an unhealthy Docker HEALTHCHECK status do by itself?"
choices = ["Deletes the container", "Restarts the container under every restart policy", "Reports failed health observations without automatically restarting the process", "Makes Kubernetes create a readiness probe"]
answer = 2
why = "The Engine records health status. A separate orchestrator would need to act on it; Kubernetes does not derive its readiness or liveness probes from a Dockerfile HEALTHCHECK."

[[quiz]]
q = "Which image is necessarily the best production choice?"
choices = ["Whichever has the fewest bytes", "A base with exactly the needed runtime compatibility, security maintenance and operational support", "Whichever includes every compiler", "Always scratch regardless of application"]
answer = 1
why = "Size matters, but an image must still run correctly, receive updates and support an operational strategy. libc compatibility, certificates, user IDs and debugging needs all influence the choice."

[lab]
summary = "Compare a Python build-and-serve image with a two-stage static site image, then verify non-root serving and health."
setup = ["docker rm -f dojo-multi-web 2>/dev/null || true", "docker image rm dojo/multi:single dojo/multi:runtime 2>/dev/null || true", "rm -f site/Dockerfile"]
cleanup = ["docker rm -f dojo-multi-web 2>/dev/null || true", "docker image rm dojo/multi:single dojo/multi:runtime 2>/dev/null || true", "rm -f site/Dockerfile || true"]

[lab.files]
"site/build.py" = '''
import os
from pathlib import Path

version = os.environ.get("APP_VERSION", "1.0")
template = Path("template.html").read_text()
Path("/out").mkdir(exist_ok=True)
Path("/out/index.html").write_text(template.replace("{{VERSION}}", version))
'''
"site/template.html" = '''
<!doctype html>
<html><body>dojo release {{VERSION}}</body></html>
'''
"site/Dockerfile.single" = '''
FROM python:3.12-alpine
ARG APP_VERSION=1.0
WORKDIR /src
COPY build.py template.html ./
RUN python build.py
LABEL org.opencontainers.image.title="dojo-site-single"
USER 10001:10001
EXPOSE 8080
CMD ["python", "-m", "http.server", "8080", "--bind", "0.0.0.0", "--directory", "/out"]
'''
"site/.dockerignore" = '''
.git
__pycache__
*.pyc
'''

[[lab.steps]]
title = "Measure a build-and-runtime baseline"
task = "Build **dojo/multi:single** from **site/Dockerfile.single**, using **site** as context and **APP_VERSION=1.0**. Inspect its byte size. The supplied generator creates the site at build time, while this baseline retains Python to serve it."
hints = ["Use -f site/Dockerfile.single and context site.", "docker image inspect -f reads .Size in bytes."]
solution = '''
```bash
docker build -f site/Dockerfile.single --build-arg APP_VERSION=1.0 -t dojo/multi:single site
docker image inspect -f '{{.Size}}' dojo/multi:single
```

The baseline is functional but contains a build-time interpreter that static content does not require.
'''

[[lab.steps.checks]]
name = "the baseline image exists with its identifying metadata"
run = "docker image inspect -f '{{index .Config.Labels \"org.opencontainers.image.title\"}}' dojo/multi:single"
equals = "dojo-site-single"

[[lab.steps]]
title = "Keep only the generated site in a final stage"
task = "Write **site/Dockerfile** with a named **build** stage based on **python:3.12-alpine**. Generate the same version **1.0** site there. Use **busybox:1.36** for the final stage and copy only **/out/** into **/www/**. Run as **10001:10001**, serve port **8080** with a foreground HTTP server, record OCI title **dojo-site** and version **1.0**, and define an HTTP health check. Build **dojo/multi:runtime**; it must be smaller than the baseline."
hints = ["COPY --from=build /out/ /www/ selects only generated artifacts.", "BusyBox supplies httpd and wget, allowing a small foreground server and health command."]
solution = '''
```bash
cat > site/Dockerfile <<'EOF'
FROM python:3.12-alpine AS build
ARG APP_VERSION=1.0
WORKDIR /src
COPY build.py template.html ./
RUN python build.py

FROM busybox:1.36
ARG APP_VERSION=1.0
LABEL org.opencontainers.image.title="dojo-site" org.opencontainers.image.version=$APP_VERSION
COPY --from=build /out/ /www/
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=2s --timeout=2s --start-period=2s --retries=10 CMD wget -q -O /dev/null http://127.0.0.1:8080/ || exit 1
CMD ["httpd", "-f", "-p", "8080", "-h", "/www"]
EOF
docker build --build-arg APP_VERSION=1.0 -t dojo/multi:runtime site
docker image inspect -f '{{.Size}}' dojo/multi:single dojo/multi:runtime
```

The final stage does not inherit Python, the build script or the builder filesystem. The same release argument is declared in both stages because ARG scope does not implicitly cross independent FROM instructions.
'''

[[lab.steps.checks]]
name = "the runtime image is smaller than the single-stage image"
run = "small=$(docker image inspect -f '{{.Size}}' dojo/multi:runtime) && large=$(docker image inspect -f '{{.Size}}' dojo/multi:single) && test \"$small\" -gt 0 && test \"$small\" -lt \"$large\""

[[lab.steps.checks]]
name = "runtime identity and release labels are configured"
run = "docker image inspect -f '{{.Config.User}} {{index .Config.Labels \"org.opencontainers.image.title\"}} {{index .Config.Labels \"org.opencontainers.image.version\"}}' dojo/multi:runtime"
equals = "10001:10001 dojo-site 1.0"

[[lab.steps]]
title = "Verify service health and the generated release"
task = "Run **dojo-multi-web** from **dojo/multi:runtime**, detached, publishing **18083:8080**. The image must become **healthy**, return a page containing **dojo release 1.0**, and run as UID **10001**. The runtime must contain **/www/index.html** and must not contain the builder input **/src/build.py**."
hints = ["A health check runs inside the container, so its URL uses container port 8080 and loopback.", "The external request uses PODDOJO_HOST and published port 18083."]
solution = '''
```bash
docker run -d --name dojo-multi-web -p 18083:8080 dojo/multi:runtime
```

Internal health and external reachability test different boundaries. The service can be healthy internally even if a host firewall prevents the client from reaching its published port.
'''

[[lab.steps.checks]]
name = "the internal HTTP health check passes"
run = "for i in $(seq 1 40); do v=$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{end}}' dojo-multi-web 2>/dev/null); [ \"$v\" = healthy ] && echo healthy && exit 0; sleep 1; done; exit 1"
equals = "healthy"
timeout = 60

[[lab.steps.checks]]
name = "the published site serves the generated version"
run = "for i in $(seq 1 20); do curl -fsS --connect-timeout 2 --max-time 3 \"http://$PODDOJO_HOST:18083/\" 2>/dev/null | grep -q 'dojo release 1.0' && echo ready && exit 0; sleep 1; done; exit 1"
equals = "ready"
timeout = 120

[[lab.steps.checks]]
name = "the runtime is non-root and contains artifacts without builder inputs"
run = "test \"$(docker exec dojo-multi-web id -u)\" = 10001 && docker exec dojo-multi-web test -f /www/index.html && docker exec dojo-multi-web test ! -e /src/build.py"
+++

## Build machines and runtime machines need different things

A compiler, package manager or static-site generator may be necessary to create an artifact without being necessary to serve it. Keeping every build tool in the runtime image increases download size, patching obligations and the tools available to an attacker. Multi-stage builds express that boundary in one Dockerfile.

```text
source + build tools ── build stage ──▶ generated artifacts
                                              │ selected COPY
                                              ▼
small runtime base ───────────────────▶ final runtime image
```

Every `FROM` starts a stage. The final selected stage determines the image output; earlier stages are not automatically layered underneath it. `COPY --from=build` transfers chosen paths from a named stage. This is fundamentally different from deleting a compiler in a later layer of one large image, where earlier immutable layers can still contain its bytes.

The lab deliberately uses static-site generation. Python reads a template and writes HTML during build. Serving that HTML requires only an HTTP server, not the generator. This demonstrates the same boundary as compiling a Go application without introducing a compiler download or third-party dependency installation.

## Name stages and copy exact artifacts

```dockerfile
FROM python:3.12-alpine AS build
ARG APP_VERSION=1.0
WORKDIR /src
COPY build.py template.html ./
RUN python build.py

FROM busybox:1.36
COPY --from=build /out/ /www/
USER 10001:10001
EXPOSE 8080
CMD ["httpd", "-f", "-p", "8080", "-h", "/www"]
```

`/out/` is a path in the build stage, not on the laptop. The final `COPY` receives only the generated files. Python and `/src/build.py` stay behind. A name such as `build` is clearer than a numeric stage index and remains meaningful if stages are reordered.

A final stage beginning `FROM build` would inherit that stage's filesystem, defeating this particular separation. BuildKit evaluates the stages needed for the requested target; it need not build unrelated stages. A named intermediate stage can be selected with `--target` for investigation, but an intermediate build output is not necessarily a suitable runtime release. See Docker's [multi-stage build guide](https://docs.docker.com/build/building/multi-stage/).

## Cache order should follow change frequency

For a dependency-heavy project, arrange inputs so stable work appears before frequently edited source:

```text
base image
  → dependency manifest
  → dependency installation
  → application source
  → compilation or generation
  → copy runtime artifacts
```

Copying the entire project before dependency installation makes a documentation edit capable of invalidating expensive work. Copy the dependency lock file first, install dependencies, then copy the source needed for compilation. In the lab there are no external dependencies, so the two input files can be copied together without inventing unnecessary layers.

A multi-stage build is not automatically a fast build. Its cache still depends on instruction inputs, previous results and the selected builder. Keep `.dockerignore` useful and generated output outside the source context where practical. Cache mounts can accelerate package downloads, but they are build infrastructure, not runtime artifacts or a substitute for dependency locking.

## Choose the smallest compatible runtime

| Base approach | What it offers | What you must consider |
|---|---|---|
| Alpine | Small Linux userspace with musl libc and package tools | Compatibility with software expecting glibc |
| BusyBox | Small collection of utilities, including this lab's HTTP server | Limited utilities and applet feature set |
| Distroless | A curated runtime without a normal shell or package manager | Matching runtime family and a separate debugging strategy |
| `scratch` | An empty starting filesystem | Supply every required executable and runtime data yourself |

Smaller is useful, but compatibility comes first. A dynamically linked binary needs its loader and libraries. A Python script needs an interpreter. HTTPS clients usually need trusted CA certificates; applications may require timezone data, name-service support or writable temporary directories. A static executable still depends on any files its behavior reads.

Copying a binary into `scratch` can produce a confusing "no such file" error when the missing object is actually the dynamic loader. Do not solve that by blindly copying the builder root filesystem. Determine the runtime contract and choose a base that satisfies it. A shell-free image also means `docker exec ... sh` will fail; plan a debugging method before an incident.

## Non-root is an image contract too

The lab uses UID and GID `10001:10001` and port 8080. Numeric identity avoids depending on a named account in every base. Files must still have readable permissions, directories need traversal permissions, and writable data requires an explicitly writable location. `USER` does not recursively fix ownership.

The service stays in the foreground using `httpd -f`. Detaching Docker is the client's concern; daemonizing the application inside its container can make the main process exit. Exec-form CMD also avoids an unnecessary shell between the runtime and server process.

Being non-root is one defense, not proof that arbitrary mounts or privileges are safe. Keep the runtime's permissions aligned with the behavior it actually requires. For a static site, reading generated files and binding an unprivileged port is enough.

## Health checks observe application behavior

```dockerfile
HEALTHCHECK --interval=2s --timeout=2s --start-period=2s --retries=10 CMD wget -q -O /dev/null http://127.0.0.1:8080/ || exit 1
```

The command runs **inside** the container and uses its network namespace. It tests the container port, not the host's published port. A shell-form command requires a shell, and `wget` must exist in the final image; tools present only in the builder are unavailable.

Health progresses from `starting` to `healthy` or `unhealthy` as probes run. The start period allows initialization failures before normal failure counting. A health check should be bounded and lightweight; repeatedly invoking an expensive operation can damage the service it is meant to observe.

Inspect the result and recent probe output with `docker inspect`. Docker's ordinary restart policies act on process exit, not an unhealthy label, so health status alone does not restart a stuck process. The lab separately checks internal health and an external HTTP response because they cover different network paths.

## Label releases and measure honestly

```dockerfile
ARG APP_VERSION=1.0
LABEL org.opencontainers.image.title="dojo-site" org.opencontainers.image.version=$APP_VERSION
```

```bash
docker build --build-arg APP_VERSION=1.0 -t dojo/multi:runtime site
docker image inspect -f '{{.Size}}' dojo/multi:runtime
```

OCI annotation keys provide recognizable metadata conventions when used as image labels. They do not implement signature verification or prove the claimed source. Keep release values accurate, and never place secrets in labels or build arguments. Redeclare an argument inside each independent stage that consumes it.

Compare byte sizes for the same local platform and similar functionality. The displayed logical image size is not the same as compressed registry transfer size or extra host disk usage after shared-layer reuse. The lab compares runtime and baseline directly instead of depending on an exact upstream image byte count.

Buildx supports explicit target platforms, for example `docker buildx build --platform linux/amd64 --load -t dojo/multi:amd64 site`. Foreign execution may require emulation, and multi-platform exports depend on the selected builder and image store. A smaller image is useful only if its artifacts run on the intended CPU.

## Why this matters for Kubernetes

Kubernetes uses the final runtime image, not your build stages. Image non-root defaults help, but workload security settings should make the requirement explicit:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-multi-web
  namespace: multistage-lab
spec:
  containers:
    - name: web
      image: dojo/multi:runtime
      imagePullPolicy: IfNotPresent
      securityContext:
        runAsNonRoot: true
        runAsUser: 10001
      readinessProbe:
        httpGet:
          path: /
          port: 8080
```

This translation is illustrative; a future kind lab must load the local image first. Kubernetes does not translate Dockerfile HEALTHCHECK into probes. For CKAD, define readiness and liveness deliberately, verify runtime identity, and ensure the final image contains everything the chosen command needs.
