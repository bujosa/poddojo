+++
id = "ckad-multi-container-pods"
title = "Init containers, native sidecars and shared Pods"
module = "design-build"
minutes = 50
needs = "cluster"
objectives = [
  "Choose init, sidecar, adapter and ambassador patterns",
  "Configure native sidecars and startup ordering",
  "Share temporary data and localhost between containers",
  "Inspect specific containers and finish Jobs with native sidecars",
]
domain = "design-build"

[[quiz]]
q = "What happens after an ordinary init container exits successfully?"
choices = [
  "The Pod always ends",
  "The next init container or application startup can proceed",
  "It restarts whenever the main container restarts",
  "It keeps running alongside the app containers",
]
answer = 1
why = "Ordinary init containers establish prerequisites in sequence. They do not need to remain running alongside the application."

[[quiz]]
q = "How is a native sidecar expressed?"
choices = ["An init container with a startupProbe", "restartPolicy: Always on an initContainers entry", "An ordinary container listed first in containers", "restartPolicy: Never on the Pod"]
answer = 1
why = "A restartable init container is the native sidecar mechanism. Its container-level policy is distinct from the Pod policy."

[[quiz]]
q = "Two containers mount the same emptyDir at different paths. Can they share files?"
choices = ["No, paths must match", "Yes; the volume identity connects the storage", "No; each container gets its own emptyDir copy", "Yes, but only after the writing container exits"]
answer = 1
why = "Mount paths are local names for the same volume content. Without a shared mount, identical paths would refer to separate root filesystems."

[[quiz]]
q = "Why can a classic forever-running sidecar stop a Job from completing?"
choices = [
  "It holds the Pod in the Pending phase",
  "It keeps an ordinary application container active",
  "Its liveness probe resets the Job's backoff counter",
  "The Job waits for its terminationGracePeriodSeconds",
]
answer = 1
why = "A Job needs its work Pod to finish. Native sidecars have special lifecycle handling so they do not block completion of the ordinary work containers."

[[quiz]]
q = "Why might a native sidecar need a startup probe?"
choices = [
  "To restart it when it stops responding",
  "To delay subsequent startup until its prerequisite is established",
  "To remove the Pod from Service endpoints until ready",
  "To let the Job finish when the sidecar exits",
]
answer = 1
why = "A running process alone may not mean its service or prepared file is ready. A startup probe defines when that sidecar is considered started."

[lab]
summary = "Prepare shared files with an init container, run a native sidecar, adapt data, and complete a sidecar-equipped Job."
setup = ["kubectl create namespace multi-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace multi-lab --ignore-not-found --wait=false 2>/dev/null || true"]

[[lab.steps]]
title = "Prepare a shared document root"
task = "Create Pod prepared in multi-lab. Ordinary init container prepare uses busybox:1.36 to write initialized to index.html in emptyDir shared. Container web uses nginx:1.27-alpine and mounts shared at /usr/share/nginx/html. Wait for Ready."
hints = [
  "The init and web containers must mount the same named volume.",
  "Use a writable mount for preparation and a read-only consumer mount.",
]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: prepared
  namespace: multi-lab
spec:
  initContainers:
    - name: prepare
      image: busybox:1.36
      command: ["sh", "-c"]
      args: ["echo initialized > /work/index.html"]
      volumeMounts:
        - name: shared
          mountPath: /work
  containers:
    - name: web
      image: nginx:1.27-alpine
      volumeMounts:
        - name: shared
          mountPath: /usr/share/nginx/html
          readOnly: true
  volumes:
    - name: shared
      emptyDir: {}
EOF
kubectl -n multi-lab wait --for=condition=Ready pod/prepared --timeout=120s
```

The ordinary init container completes before web starts. The shared emptyDir, rather than either container's root filesystem, carries the prepared file.
'''

[[lab.steps.checks]]
name = "ordinary init completed successfully"
run = '''kubectl -n multi-lab get pod prepared -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab get pod prepared -o jsonpath='{.status.initContainerStatuses[?(@.name=="prepare")].state.terminated.exitCode}' 2>/dev/null); [ "$v" = "0" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "web can see the initialized file"
run = '''kubectl -n multi-lab get pod/prepared -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab exec prepared -c web -- cat /usr/share/nginx/html/index.html 2>/dev/null); [ "$v" = "initialized" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "init and web use the required images"
run = "kubectl -n multi-lab get pod prepared -o jsonpath='{.spec.initContainers[?(@.name==\"prepare\")].image} {.spec.containers[?(@.name==\"web\")].image}'"
equals = "busybox:1.36 nginx:1.27-alpine"

[[lab.steps.checks]]
name = "preparation and serving share emptyDir"
run = "kubectl -n multi-lab get pod prepared -o jsonpath='{.spec.volumes[?(@.name==\"shared\")].emptyDir} {.spec.initContainers[?(@.name==\"prepare\")].volumeMounts[0].name} {.spec.containers[?(@.name==\"web\")].volumeMounts[0].name}'"
equals = "{} shared shared"

[[lab.steps]]
title = "Keep a native helper running"
task = "Create Pod native in multi-lab with native sidecar helper under initContainers, busybox:1.36 and restartPolicy Always. It writes native-ready to shared/index.html and remains running. Use a startup probe to wait for that file. Container web uses nginx:1.27-alpine to serve shared at /usr/share/nginx/html. Wait for Ready and fetch the page from helper via localhost."
hints = ["Use an emptyDir named shared mounted by both containers.", "busybox wget -qO- can read http://127.0.0.1:80."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: native
  namespace: multi-lab
spec:
  initContainers:
    - name: helper
      image: busybox:1.36
      restartPolicy: Always
      command: ["sh", "-c"]
      args: ["echo native-ready > /data/index.html; echo helper-ready; exec sleep 3600"]
      startupProbe:
        exec:
          command: ["test", "-f", "/data/index.html"]
        periodSeconds: 1
        failureThreshold: 60
      volumeMounts:
        - name: shared
          mountPath: /data
  containers:
    - name: web
      image: nginx:1.27-alpine
      volumeMounts:
        - name: shared
          mountPath: /usr/share/nginx/html
          readOnly: true
  volumes:
    - name: shared
      emptyDir: {}
EOF
kubectl -n multi-lab wait --for=condition=Ready pod/native --timeout=120s
kubectl -n multi-lab logs native -c helper
kubectl -n multi-lab exec native -c helper -- wget -qO- http://127.0.0.1:80
```

The restartable init container continues alongside web. Its startup probe gates later startup, and localhost works because both containers share one network namespace.
'''

[[lab.steps.checks]]
name = "helper is configured as a native sidecar"
run = "kubectl -n multi-lab get pod native -o jsonpath='{.spec.initContainers[?(@.name==\"helper\")].restartPolicy} {.spec.initContainers[?(@.name==\"helper\")].image} {.spec.containers[?(@.name==\"web\")].image} {.spec.volumes[?(@.name==\"shared\")].emptyDir}'"
equals = "Always busybox:1.36 nginx:1.27-alpine {}"

[[lab.steps.checks]]
name = "native Pod becomes Ready"
run = '''kubectl -n multi-lab get pod native -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab get pod native -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "helper reaches web over shared localhost"
run = '''kubectl -n multi-lab get pod/native -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab exec native -c helper -- wget -qO- http://127.0.0.1:80 2>/dev/null); [ "$v" = "native-ready" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "helper defines a startup probe"
run = "kubectl -n multi-lab get pod native -o jsonpath='{.spec.initContainers[?(@.name==\"helper\")].startupProbe}'"
matches = '''^\{.+\}$'''

[[lab.steps]]
title = "Adapt a shared data format"
task = "Create Pod adapter in multi-lab with containers producer and convert, both busybox:1.36, sharing emptyDir data mounted at /data. Producer writes temperature=21 to /data/raw and stays running. Convert waits for raw, transforms it into temperature_celsius 21 in /data/metrics, and stays running."
hints = [
  "Classic containers have no guaranteed startup order.",
  "Use a bounded wait in the converter rather than assuming the file exists.",
]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: adapter
  namespace: multi-lab
spec:
  containers:
    - name: producer
      image: busybox:1.36
      command: ["sh", "-c"]
      args: ["echo temperature=21 > /data/raw; exec sleep 3600"]
      volumeMounts:
        - name: data
          mountPath: /data
    - name: convert
      image: busybox:1.36
      command: ["sh", "-c"]
      args:
        - |
          for i in $(seq 1 60); do
            if [ -f /data/raw ]; then
              sed 's/^temperature=/temperature_celsius /' /data/raw > /data/metrics
              exec sleep 3600
            fi
            sleep 1
          done
          exit 1
      volumeMounts:
        - name: data
          mountPath: /data
  volumes:
    - name: data
      emptyDir: {}
EOF
kubectl -n multi-lab wait --for=condition=Ready pod/adapter --timeout=120s
```

The converter waits because classic container list order is not a dependency guarantee. Both see the same data volume through their own mounts.
'''

[[lab.steps.checks]]
name = "producer writes the source format"
run = '''kubectl -n multi-lab get pod/adapter -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab exec adapter -c producer -- cat /data/raw 2>/dev/null); [ "$v" = "temperature=21" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "converter publishes transformed shared content"
run = '''kubectl -n multi-lab get pod/adapter -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab exec adapter -c convert -- cat /data/metrics 2>/dev/null); [ "$v" = "temperature_celsius 21" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "both named application containers use pinned images"
run = "kubectl -n multi-lab get pod adapter -o jsonpath='{.spec.containers[?(@.name==\"producer\")].image} {.spec.containers[?(@.name==\"convert\")].image} {.spec.volumes[?(@.name==\"data\")].emptyDir}'"
equals = "busybox:1.36 busybox:1.36 {}"

[[lab.steps]]
title = "Finish work despite a long-lived helper"
task = "Create Job sidecar-job in multi-lab with Pod restartPolicy Never. Its native sidecar helper uses busybox:1.36 and sleeps 3600. Its ordinary worker container uses busybox:1.36 to print job-complete and exit. The Job must complete without manually deleting or stopping the helper."
hints = ["Put helper in initContainers with container-level restartPolicy Always.", "The ordinary worker belongs in containers."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: batch/v1
kind: Job
metadata:
  name: sidecar-job
  namespace: multi-lab
spec:
  backoffLimit: 1
  template:
    spec:
      restartPolicy: Never
      initContainers:
        - name: helper
          image: busybox:1.36
          restartPolicy: Always
          command: ["sleep", "3600"]
      containers:
        - name: worker
          image: busybox:1.36
          command: ["echo", "job-complete"]
EOF
kubectl -n multi-lab wait --for=condition=complete job/sidecar-job --timeout=120s
kubectl -n multi-lab logs job/sidecar-job -c worker
```

The worker's successful exit satisfies the Job. Native sidecar lifecycle handling shuts down the helper rather than making it an endless ordinary work container.
'''

[[lab.steps.checks]]
name = "native helper is in the Job template"
run = "kubectl -n multi-lab get job sidecar-job -o jsonpath='{.spec.template.spec.initContainers[?(@.name==\"helper\")].restartPolicy} {.spec.template.spec.restartPolicy} {.spec.template.spec.initContainers[?(@.name==\"helper\")].image} {.spec.template.spec.containers[?(@.name==\"worker\")].image}'"
equals = "Always Never busybox:1.36 busybox:1.36"

[[lab.steps.checks]]
name = "Job completes with its sidecar present"
run = '''kubectl -n multi-lab get job sidecar-job -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n multi-lab get job sidecar-job -o jsonpath='{.status.conditions[?(@.type=="Complete")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "worker completed its actual work"
run = "kubectl -n multi-lab logs job/sidecar-job -c worker"
equals = "job-complete"
+++

## A Pod is a deliberate coupling boundary

Multi-container Pods solve a specific problem: some processes need to live together. They share scheduling, one Pod IP, localhost, and any volumes explicitly mounted into multiple containers. They are replaced together when the Pod is replaced.

This is useful for a helper that prepares files, transforms logs or proxies traffic for one application instance. It is less useful for components that need independent scaling or upgrades. A web frontend and a shared database usually belong in separate workloads.

```text
Pod network namespace: one IP
┌─────────────────────────────────────────────┐
│ app :8080 ← localhost → helper :9000         │
│     /data ─── shared emptyDir ─── /work      │
│ private rootfs             private rootfs   │
└─────────────────────────────────────────────┘
```

Containers do not share root filesystems. Identical paths in two containers refer to different files unless both paths mount the same volume. Mount paths may differ while referring to the same volume. Containers also do not normally share a process namespace unless the Pod explicitly requests it.

## Ordinary init containers run to completion

An ordinary init container performs prerequisite work before the application containers start. Kubernetes runs regular init containers in order, waiting for each to succeed before moving to the next. A failure prevents later initialization and application startup until policy permits a successful retry or the Pod fails.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: prepared
  namespace: multi-lab
spec:
  initContainers:
    - name: prepare
      image: busybox:1.36
      command: ["sh", "-c"]
      args: ["echo initialized > /work/index.html"]
      volumeMounts:
        - name: shared
          mountPath: /work
  containers:
    - name: web
      image: nginx:1.27-alpine
      volumeMounts:
        - name: shared
          mountPath: /usr/share/nginx/html
  volumes:
    - name: shared
      emptyDir: {}
```

The init container writes into an emptyDir that the web container mounts as its document root. Its own writable layer would not work: the web container cannot see that layer.

Initialization should be idempotent because init execution may repeat in some restart or recovery scenarios. Creating or validating a file safely is better than blindly appending a duplicate record or repeating a non-transactional database migration.

## Native sidecars are restartable init containers

A native sidecar is listed under initContainers but has container-level restartPolicy: Always. It starts during initialization and remains running alongside the application. This feature has been stable since Kubernetes v1.33; see the [sidecar documentation](https://kubernetes.io/docs/concepts/workloads/pods/sidecar-containers/).

```yaml
# Inside a Pod spec:
initContainers:
  - name: helper
    image: busybox:1.36
    restartPolicy: Always
    command: ["sh", "-c"]
    args: ["echo native-ready > /data/index.html; exec sleep 3600"]
    startupProbe:
      exec:
        command: ["test", "-f", "/data/index.html"]
      periodSeconds: 1
      failureThreshold: 60
    volumeMounts:
      - name: shared
        mountPath: /data
```

The kubelet proceeds when the sidecar is considered started. Without a startup probe, that can simply mean the process is running; it does not prove an application-specific prerequisite is ready. A startup probe can define that milestone. In this example, later startup waits for the file.

A sidecar's readiness probe, if present, contributes to the Pod's Ready condition. It answers a different question from whether initialization may proceed. Do not use a readiness probe as an assumed initialization gate.

Native sidecars have lifecycle support that classic helpers lack. They continue while the application runs and are terminated after the application containers have stopped, in reverse declaration order among sidecars. Their container-level restart policy operates independently of the Pod-level policy for ordinary application containers.

## Classic patterns still describe useful roles

“Sidecar,” “adapter” and “ambassador” describe architectural responsibilities. “Native sidecar” describes a Kubernetes lifecycle mechanism. An adapter can be implemented as either a classic application container or a native sidecar depending on its lifecycle needs.

| Pattern | Role | Example |
|---|---|---|
| Sidecar | Add a supporting capability | Ship an application's log files |
| Adapter | Convert an application's interface or data format | Convert key=value records into metrics text |
| Ambassador | Present a local proxy to an external service | Application talks to localhost while a helper handles upstream connections |
| Init container | Complete a prerequisite before the application | Produce static files or wait for a dependency |

Classic sidecars live in the containers list. The order of that list is not a dependency ordering guarantee. If one needs another's data, implement a bounded wait or use initialization to establish prerequisites.

An ambassador can let the application always call localhost:9000 while proxy configuration selects the real remote endpoint. It does not give each container a private port range: the Pod shares a network namespace, so two processes cannot both bind the same address and port.

## Share data intentionally

An emptyDir begins empty for a Pod and can survive individual container restarts. It is removed when the Pod is removed from the node. This makes it useful for temporary shared files, not durable application records.

Mount a shared volume read-only in a consumer where possible. A single writer can produce files into it, while readers consume them without permission to change them. Atomic rename is useful for publishing complete files so readers do not observe partially written content.

Resource planning must include concurrently running helpers. A tiny main application with a large sidecar is not a tiny Pod. Initialization and concurrent sidecar requirements also influence effective scheduling requests; inspect the full Pod rather than only its first container.

## Logs and execution need container names

```bash
kubectl -n multi-lab logs prepared -c prepare
kubectl -n multi-lab logs prepared -c web
kubectl -n multi-lab exec prepared -c web -- cat /usr/share/nginx/html/index.html
kubectl -n multi-lab get pod prepared -o yaml
```

Use explicit container names instead of relying on kubectl's default choice. Logs from completed ordinary init containers remain available while their container records are retained. Native sidecar status appears among initContainerStatuses, not the ordinary containerStatuses array.

For a localhost check, run a client in one container against the listener in another. Both use the same Pod network. You do not need a Service for communication inside a Pod.

## Jobs expose a classic-sidecar trap

A classic helper that runs forever in a Job's containers list can prevent the Pod from finishing after the worker exits. A native sidecar does not block Job completion once ordinary application containers finish successfully. The final lab demonstrates this difference without requiring a shutdown script.

## Exam tips

Read whether a helper must finish first or continue alongside the application. Put fields at the correct level: native restartPolicy belongs on the init container, while the ordinary Pod policy belongs under spec. Inspect both container status arrays, use explicit -c names, and verify shared-file or localhost behavior rather than merely counting containers.
