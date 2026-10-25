+++
id = "ckad-pods"
title = "Pods: specification, processes and lifecycle"
module = "foundations"
minutes = 40
needs = "cluster"
objectives = [
  "Describe Pod identity, networking and container configuration",
  "Override image commands and arguments correctly",
  "Distinguish Pod phases, readiness and container states",
  "Recreate a bare Pod when its specification is immutable",
]
domain = "design-build"

[[quiz]]
q = "What do containers in one ordinary Pod share?"
choices = [
  "Their entire root filesystem",
  "A network namespace and any explicitly shared volumes",
  "Independent Pod IPs",
  "Separate node assignments",
]
answer = 1
why = "Containers share localhost and one Pod IP. Files are shared only through mounted volumes; each container image supplies its own root filesystem."

[[quiz]]
q = "Which field overrides an image ENTRYPOINT?"
choices = ["args", "ports", "command", "metadata.name"]
answer = 2
why = "command replaces the image entrypoint. args supplies arguments. Shell syntax works only when a shell is explicitly invoked."

[[quiz]]
q = "A short task must remain stopped after a successful exit. Which policy fits?"
choices = ["Always", "Never", "Restart", "Ready"]
answer = 1
why = "Never leaves the terminated result for inspection. Always would restart it even after exit code zero; OnFailure is another valid choice when failures should retry."

[[quiz]]
q = "Is CrashLoopBackOff a Pod phase?"
choices = [
  "Yes, equivalent to Failed",
  "Yes, equivalent to Pending",
  "No; it describes a container restart backoff",
  "No; it is a Service type",
]
answer = 2
why = "The phase can still be Running while a container repeatedly fails. Inspect container states, exit codes and logs rather than relying on the display column."

[lab]
summary = "Create a server Pod, run a configured one-shot task, and recreate it with a different immutable specification."
setup = ["kubectl create namespace pods-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace pods-lab --ignore-not-found --wait=false 2>/dev/null || true", "rm -f reporter.yaml || true"]

[[lab.steps]]
title = "Create a labeled server"
task = "Create Pod web in pods-lab with container web using nginx:1.27-alpine. Give the Pod app=web and annotation training=pods, and declare a named TCP container port http on 80. Wait for Ready."
hints = ["Generate YAML with run, then add the annotation and named port.", "A declared container port does not create a Service."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: web
  namespace: pods-lab
  labels:
    app: web
  annotations:
    training: pods
spec:
  containers:
    - name: web
      image: nginx:1.27-alpine
      ports:
        - name: http
          containerPort: 80
EOF
kubectl -n pods-lab wait --for=condition=Ready pod/web --timeout=120s
```

Port metadata documents the listener. The Ready condition confirms startup, while the labels and annotation make resource intent visible.
'''

[[lab.steps.checks]]
name = "server metadata and image are correct"
run = "kubectl -n pods-lab get pod web -o jsonpath='{.metadata.labels.app} {.metadata.annotations.training} {.spec.containers[?(@.name==\"web\")].image}'"
equals = "web pods nginx:1.27-alpine"

[[lab.steps.checks]]
name = "named port is declared"
run = "kubectl -n pods-lab get pod web -o jsonpath='{.spec.containers[?(@.name==\"web\")].ports[?(@.name==\"http\")].containerPort}'"
equals = "80"

[[lab.steps.checks]]
name = "server becomes Ready"
run = '''kubectl -n pods-lab get pod web -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n pods-lab get pod web -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Run a configured task"
task = "Create Pod reporter in pods-lab with container main, busybox:1.36, restartPolicy Never and MESSAGE=hello-pods. Its process must print exactly hello-pods and exit successfully."
hints = ["Use an explicit shell to expand the container environment variable.", "Wait for phase Succeeded rather than Ready."]
solution = '''
```bash
cat > reporter.yaml <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: reporter
  namespace: pods-lab
spec:
  restartPolicy: Never
  containers:
    - name: main
      image: busybox:1.36
      command: ["/bin/sh", "-c"]
      args: ['printf "%s\n" "$MESSAGE"']
      env:
        - name: MESSAGE
          value: hello-pods
EOF
kubectl apply -f reporter.yaml
kubectl -n pods-lab wait --for=jsonpath='{.status.phase}'=Succeeded pod/reporter --timeout=120s
```

The quoted heredoc preserves the container's variable expansion. Never lets the zero exit status remain visible instead of restarting the task.
'''

[[lab.steps.checks]]
name = "reporter has the required configuration"
run = "kubectl -n pods-lab get pod reporter -o jsonpath='{.spec.restartPolicy} {.spec.containers[?(@.name==\"main\")].image} {.spec.containers[?(@.name==\"main\")].env[?(@.name==\"MESSAGE\")].value}'"
equals = "Never busybox:1.36 hello-pods"

[[lab.steps.checks]]
name = "task succeeds"
run = '''kubectl -n pods-lab get pod reporter -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n pods-lab get pod reporter -o jsonpath='{.status.phase} {.status.containerStatuses[?(@.name=="main")].state.terminated.exitCode}' 2>/dev/null); [ "$v" = "Succeeded 0" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "task prints the configured greeting"
run = "kubectl -n pods-lab logs reporter -c main"
equals = "hello-pods"

[[lab.steps]]
title = "Replace an immutable specification"
task = "Recreate reporter with MESSAGE=hello-again. Keep its image, container name and restartPolicy. It must print exactly hello-again and succeed."
hints = ["Environment changes on an existing bare Pod require replacement.", "Delete this Pod normally, then apply a revised manifest."]
solution = '''
```bash
kubectl -n pods-lab delete pod reporter --wait=true
cat > reporter.yaml <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: reporter
  namespace: pods-lab
spec:
  restartPolicy: Never
  containers:
    - name: main
      image: busybox:1.36
      command: ["/bin/sh", "-c"]
      args: ['printf "%s\n" "$MESSAGE"']
      env:
        - name: MESSAGE
          value: hello-again
EOF
kubectl apply -f reporter.yaml
kubectl -n pods-lab wait --for=jsonpath='{.status.phase}'=Succeeded pod/reporter --timeout=120s
```

Deleting the old bare Pod allows a new environment specification. The recreated name identifies a new Pod instance, not an in-place environment edit.
'''

[[lab.steps.checks]]
name = "replacement has the new configuration"
run = "kubectl -n pods-lab get pod reporter -o jsonpath='{.spec.restartPolicy} {.spec.containers[?(@.name==\"main\")].image} {.spec.containers[?(@.name==\"main\")].env[?(@.name==\"MESSAGE\")].value}'"
equals = "Never busybox:1.36 hello-again"

[[lab.steps.checks]]
name = "replacement completed and printed the new greeting"
run = '''kubectl -n pods-lab get pod/reporter -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n pods-lab logs reporter -c main 2>/dev/null); [ "$v" = "hello-again" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "new task exits successfully"
run = "kubectl -n pods-lab get pod reporter -o jsonpath='{.spec.containers[?(@.name==\"main\")].env[?(@.name==\"MESSAGE\")].value} {.status.phase} {.status.containerStatuses[?(@.name==\"main\")].state.terminated.exitCode}'"
equals = "hello-again Succeeded 0"
+++

## A Pod is the unit Kubernetes places

A Pod groups one or more containers that must share placement and lifecycle. Kubernetes schedules the Pod onto one node. Its containers share a network namespace and can share declared volumes, but they do not automatically share their root filesystems.

For most ordinary applications, begin with one application container per Pod. Put tightly coupled helpers in the same Pod when they need localhost or shared files. Put independently scaled components in separate Pods behind Services. A web server and its database usually need different lifecycles and should not be bundled merely because they communicate.

```text
Pod: one IP, one node, one lifecycle
┌─────────────────────────────────────────┐
│ container web      container helper     │
│ private rootfs     private rootfs       │
│       └──── shared volume ────┘         │
│          shared localhost              │
└─────────────────────────────────────────┘
```

Pod identity includes a name and a UID. Deleting and recreating the same name creates a different Pod with a different UID. It may have a different IP and be placed on a different node. Controllers and Services let applications cope with that replacement.

## Read the object from the outside inward

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: greeter
  namespace: pods-lab
  labels:
    app: greeter
  annotations:
    training: command-and-args
spec:
  restartPolicy: Never
  containers:
    - name: main
      image: busybox:1.36
      command: ["/bin/sh", "-c"]
      args: ['printf "%s\n" "$MESSAGE"']
      env:
        - name: MESSAGE
          value: hello
```

The resource identity lives in metadata. Runtime intent lives in spec. Containers form a list even when there is only one. Each container needs a name and image; add command, arguments, environment, ports and mounts as needed.

Labels support selection, for example app=greeter. Annotations store descriptive information that is not used by label selectors. A Service selects Pod labels, not the Pod's container name or annotation. Use meaningful labels consistently so inspection and routing work.

For a quick scaffold:

```bash
kubectl -n pods-lab run greeter --image=busybox:1.36 \
  --restart=Never --dry-run=client -o yaml --command -- sh -c 'echo hello'
```

The run command creates a **Pod**, not a Deployment. Use a controller for an application that should be replaced after deletion or node failure.

## Command and args are different overrides

An image includes executable defaults. Docker calls these ENTRYPOINT and CMD. Kubernetes exposes the analogous overrides as command and args. The [Dockerfile reference](https://docs.docker.com/reference/dockerfile/#understand-how-cmd-and-entrypoint-interact) explains the image-side combination; a Pod changes that combination for one container instance.

| Pod fields | Effect |
|---|---|
| Neither provided | Use the image's defaults |
| args only | Keep the image entrypoint, replace its default arguments |
| command provided | Replace the image entrypoint; provide needed arguments explicitly |
| command and args | Use the specified executable and argument vector |

Both fields are lists of strings. Kubernetes does not automatically run them through a shell. A list containing echo, hello, and a pipe character does not create a pipeline. To use shell expansion, redirection, loops or pipes, explicitly run a shell with the script as one argument.

```yaml
command: ["/bin/sh", "-c"]
args:
  - 'printf "%s\n" "$MESSAGE"; exec sleep 3600'
```

The shell expands MESSAGE. Kubernetes also supports its own variable substitution syntax using $(VAR_NAME) in command and args, but that is separate from shell parsing. Quote scripts in manifests and in heredocs so the local shell does not substitute values intended for the container.

For long-running shell launchers, exec replaces the shell with the application so signals reach the main process directly. A container whose main process exits has stopped, even when another process or helper was the activity you intended to keep alive.

## Environment and ports describe container configuration

Explicit env entries pass strings to the process. YAML values that look like numbers or booleans should be quoted when the API expects a string. Environment variables are read when a container starts; changing external configuration does not rewrite a running process's environment.

```yaml
env:
  - name: MODE
    value: "practice"
ports:
  - name: http
    containerPort: 80
    protocol: TCP
```

A declared port documents the intended listener and gives it a name other objects can reference. It does not start the listener or expose traffic outside the Pod. A process can listen without a containerPort declaration; a declared port can exist without a listening process.

## Restart policy, phase and container state

The Pod-level restartPolicy normally defaults to Always. Never is useful for a single task whose successful exit you want to inspect. OnFailure restarts after nonzero exits while allowing successful exits to stay stopped. A Job requires Never or OnFailure rather than Always.

| Pod phase | Meaning |
|---|---|
| Pending | Accepted, but one or more containers have not started |
| Running | Bound to a node and at least one container is running, starting or restarting |
| Succeeded | All ordinary containers terminated successfully and will not restart |
| Failed | All containers terminated, at least one unsuccessfully, and will not restart |
| Unknown | The Pod's state could not be obtained |

Container states are Waiting, Running and Terminated. The familiar kubectl STATUS column is a display summary, not always the Pod phase. CrashLoopBackOff describes restart backoff for a container; it is not a Pod phase. A Running Pod can have a container that is repeatedly failing. Ready is a condition and can be false even while phase is Running.

```bash
kubectl -n pods-lab describe pod greeter
kubectl -n pods-lab logs greeter -c main
kubectl -n pods-lab get pod greeter -o jsonpath='{.status.phase}'
```

Use previous logs when diagnosing a container that restarted. Do not wait for Ready on a deliberately completed one-shot Pod: success is its Succeeded phase and zero exit code.

## Pods are mostly immutable

Changing environment, command or volumes on an existing Pod is generally rejected. Some fields, including container images, permit updates, but that does not make Pods general editable templates. For a bare Pod, save the desired manifest, delete the old instance, then create the replacement.

For managed workloads, edit the controller's Pod template instead. Directly editing one managed Pod leaves the controller's specification unchanged and is lost on replacement. Static Pods are a special case: the kubelet reads local manifest files and manages them directly, with mirror objects visible through the API. Deleting a mirror object does not remove the local manifest. You do not create static Pods in this lab.

## Exam tips

Identify whether the task asks for a Pod or a controller. Check command versus args carefully, choose an appropriate restart policy, and inspect logs plus termination status for one-shot tasks. Recreate a bare Pod when changing immutable fields, and verify the new outcome instead of assuming an accepted manifest means a successful process.
