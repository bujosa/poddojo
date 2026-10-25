+++
id = "ckad-logs-and-monitoring"
title = "Logs, events, and resource monitoring"
module = "observability"
minutes = 45
domain = "observability"
needs = "cluster"
objectives = ["Select current and previous container logs efficiently", "Correlate Pod state, events, and labels without guessing names", "Read CPU and memory usage through the optional Metrics API", "Save concise diagnostic evidence in the lab folder"]

[[quiz]]
q = "What does kubectl logs --previous retrieve?"
choices = ["All deleted Pod logs in the namespace", "The preceding terminated instance of the selected container in the same Pod", "Only logs older than one hour", "The previous Deployment revision"]
answer = 1
why = "The kubelet can expose the previous container instance after a restart. It is not a durable archive of deleted Pods or every historical restart."

[[quiz]]
q = "Why might kubectl logs -l app=dojo-stream show less history than expected?"
choices = ["Selectors disable logs", "Selector-based logs default to a limited tail unless overridden", "It reads only events", "It always filters to errors"]
answer = 1
why = "Specify --tail explicitly, such as --tail=100 or --tail=-1, when selecting multiple Pods. Also choose containers deliberately and use --prefix to identify the source."

[[quiz]]
q = "What does a 100m CPU usage reading mean?"
choices = ["One hundred CPU cores", "One tenth of a CPU core averaged over the sampling window", "A 100 MiB memory limit", "One hundred percent of any limit"]
answer = 1
why = "CPU is reported in millicores: 1000m equals one core. Usage is measured activity, distinct from the request used for scheduling and the configured limit."

[[quiz]]
q = "What should you first infer from Metrics API not available?"
choices = ["The application must have crashed", "The cluster may lack a working metrics-server or aggregated metrics API", "The Pod has no labels", "CPU requests are zero"]
answer = 1
why = "Metrics collection is an optional cluster capability. Check discovery, APIService health, metrics-server Pods, certificates, and permissions before interpreting absent data as application behavior."

[[quiz]]
q = "Which selector finds only events for one named Pod?"
choices = ["-l pod=NAME always works", "--field-selector involvedObject.kind=Pod,involvedObject.name=NAME on get events", "--all-containers", "--since=Pod/NAME"]
answer = 1
why = "Field selectors target supported API fields. Event involvedObject fields differ from arbitrary labels, and supported selectors vary by resource kind."

[lab]
summary = "Collect multi-container logs, preserve previous crash evidence, explain a scheduling event, and capture current resource usage."
setup = [
'''
kubectl create namespace logs-monitoring-lab --dry-run=client -o yaml | kubectl apply -f -
''',
'''
set -e
if kubectl get apiservice v1beta1.metrics.k8s.io >/dev/null 2>&1; then
  echo 'Using the existing metrics API registration; leaving its installation unchanged.'
else
  curl -fsSL --retry 3 https://github.com/kubernetes-sigs/metrics-server/releases/download/v0.8.0/components.yaml -o metrics-official.yaml
  sed -e 's/metrics-server/dojo-logs-metrics-server/g' -e 's/system:dojo-logs-metrics-server/dojo-logs-metrics-reader/g' -e 's/system:aggregated-metrics-reader/dojo-logs-aggregated-metrics-reader/g' -e 's/namespace: kube-system/namespace: logs-monitoring-lab/g' -e '/kind: RoleBinding/,/roleRef:/s/namespace: logs-monitoring-lab/namespace: kube-system/' metrics-official.yaml > metrics-components.yaml
  kubectl apply -f metrics-components.yaml
  kubectl label apiservice v1beta1.metrics.k8s.io poddojo.io/lesson=logs-monitoring --overwrite
  kubectl -n logs-monitoring-lab patch deployment dojo-logs-metrics-server --type=json -p '[{"op":"add","path":"/spec/template/spec/containers/0/args/-","value":"--kubelet-insecure-tls"}]'
fi
''',
'''
kubectl apply -f workloads.yaml
'''
]
cleanup = [
'''
if [ "$(kubectl get apiservice v1beta1.metrics.k8s.io -o jsonpath='{.metadata.labels.poddojo\.io/lesson}' 2>/dev/null)" = logs-monitoring ]; then
  kubectl delete apiservice v1beta1.metrics.k8s.io --ignore-not-found || true
fi
kubectl delete clusterrole dojo-logs-metrics-reader dojo-logs-aggregated-metrics-reader --ignore-not-found || true
kubectl delete clusterrolebinding dojo-logs-metrics-reader dojo-logs-metrics-server:system:auth-delegator --ignore-not-found || true
kubectl -n kube-system delete rolebinding dojo-logs-metrics-server-auth-reader --ignore-not-found || true
''',
'''
kubectl delete namespace logs-monitoring-lab --ignore-not-found --wait=false || true
''',
'''
rm -f stream.log previous.log scheduling.txt usage.txt metrics-official.yaml metrics-components.yaml || true
'''
]

[lab.files]
"workloads.yaml" = '''
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-stream
  namespace: logs-monitoring-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-stream}
  template:
    metadata:
      labels: {app: dojo-stream}
    spec:
      containers:
      - name: main
        image: busybox:1.36
        command: [sh, -c, 'while true; do echo "main tick"; sleep 2; done']
        resources:
          requests: {cpu: 10m, memory: 8Mi}
      - name: audit
        image: busybox:1.36
        command: [sh, -c, 'while true; do echo "audit tick"; sleep 2; done']
        resources:
          requests: {cpu: 10m, memory: 8Mi}
---
apiVersion: v1
kind: Pod
metadata:
  name: dojo-crash
  namespace: logs-monitoring-lab
spec:
  containers:
  - name: main
    image: busybox:1.36
    command: [sh, -c, 'echo "fatal: CONFIG_PATH not set"; sleep 2; exit 1']
---
apiVersion: v1
kind: Pod
metadata:
  name: dojo-pending
  namespace: logs-monitoring-lab
spec:
  nodeSelector:
    poddojo.io/absent-node: "true"
  containers:
  - name: main
    image: busybox:1.36
    command: [sleep, "3600"]
'''

[[lab.steps]]
title = "Collect both containers without following forever"
task = '''
The setup created Deployment dojo-stream in logs-monitoring-lab. Wait for it, then save recent logs from both main and audit to stream.log in the lab folder. Include at least one main tick and one audit tick. Use labels and --all-containers; inspect prefixed output.
'''
hints = ["kubectl logs -l app=dojo-stream --all-containers=true --prefix --tail=20 captures both sources.", "Do not use -f for a command that must finish and save an artifact."]
solution = '''
```bash
kubectl -n logs-monitoring-lab rollout status deployment/dojo-stream --timeout=120s
kubectl -n logs-monitoring-lab logs -l app=dojo-stream --all-containers=true --prefix --tail=20 --since=5m > stream.log
cat stream.log
```
'''

[[lab.steps.checks]]
name = "saved evidence contains both application and audit output"
run = '''
test -s stream.log && grep -q "main tick" stream.log && grep -q "audit tick" stream.log && echo ok
'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Retrieve the previous crash instance"
task = '''
Pod dojo-crash repeatedly exits with an application error. Use --previous for container main and save its diagnostic message to previous.log. Do not fix or delete the Pod; preserve the failure for investigation.
'''
hints = ["Poll until a previous terminated container exists.", "A current logs call and a previous logs call address different instances of the same container."]
solution = '''
```bash
for i in $(seq 1 60); do
  if kubectl -n logs-monitoring-lab logs dojo-crash -c main --previous --tail=20 > previous.log 2>/dev/null && grep -q 'fatal: CONFIG_PATH not set' previous.log; then
    break
  fi
  sleep 2
done
grep 'fatal: CONFIG_PATH not set' previous.log
```
'''

[[lab.steps.checks]]
name = "previous container evidence records the startup failure"
run = '''
test -s previous.log && grep -q "fatal: CONFIG_PATH not set" previous.log && echo ok
'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Explain Pending from events"
task = '''
Inspect dojo-pending with get and describe. Save a scheduling.txt report containing the Pod name and its FailedScheduling event from get events, using a field selector and chronological sorting. Keep the workload unchanged.
'''
hints = ["Filter involvedObject.kind=Pod,involvedObject.name=dojo-pending.", "A nodeSelector mismatch is a scheduling issue, so container logs do not exist yet."]
solution = '''
```bash
kubectl -n logs-monitoring-lab get pod dojo-pending -o wide
kubectl -n logs-monitoring-lab describe pod dojo-pending
for i in $(seq 1 45); do
  kubectl -n logs-monitoring-lab get events --field-selector involvedObject.kind=Pod,involvedObject.name=dojo-pending --sort-by=.metadata.creationTimestamp > scheduling.txt
  grep -q FailedScheduling scheduling.txt && break
  sleep 2
done
grep FailedScheduling scheduling.txt
```
'''

[[lab.steps.checks]]
name = "saved events identify the unschedulable Pod"
run = '''
test -s scheduling.txt && grep -q dojo-pending scheduling.txt && grep -q FailedScheduling scheduling.txt && echo ok
'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Capture a real resource sample"
task = '''
Wait for the optional metrics API and a sample for dojo-stream. Save kubectl top pod -l app=dojo-stream --no-headers output to usage.txt. Inspect per-container usage too. Do not treat requests as measured usage or require an exact CPU value.
'''
hints = ["Initial metrics can take several collection intervals; allow up to four minutes.", "kubectl top pods --containers separates the main and audit containers."]
solution = '''
```bash
for i in $(seq 1 115); do
  if kubectl -n logs-monitoring-lab top pod -l app=dojo-stream --no-headers > usage.txt 2>/dev/null && grep -q '^dojo-stream-' usage.txt; then
    break
  fi
  sleep 2
done
grep '^dojo-stream-' usage.txt
kubectl -n logs-monitoring-lab top pods -l app=dojo-stream --containers
```
'''

[[lab.steps.checks]]
name = "a saved numeric sample accompanies working live Pod metrics"
run = '''
test -s usage.txt && awk '$1 ~ /^dojo-stream-/ && $2 ~ /^[0-9]+m$/ && $3 ~ /^[0-9]+Mi$/ {found=1} END {exit !found}' usage.txt && kubectl -n logs-monitoring-lab top pod -l app=dojo-stream --no-headers | awk '$1 ~ /^dojo-stream-/ && $2 ~ /^[0-9]+m$/ && $3 ~ /^[0-9]+Mi$/ {found=1} END {exit !found}' && echo ok
'''
equals = "ok"
timeout = 270
+++

## Observe before changing

Kubernetes exposes several kinds of evidence, each answering a different question. Object status says what controllers and kubelets currently report. Events describe significant transitions and failures. Container logs describe what the application wrote. Resource metrics show recent CPU and memory usage. None is a complete substitute for the others, and none should be mistaken for a permanent audit archive.

```text
get / watch -> current state and transitions
   describe -> configuration plus conditions and recent events
     events -> scheduling, pulling, probes, lifecycle failures
       logs -> application output for a selected container instance
        top -> sampled CPU and memory through metrics.k8s.io
```

Start with a precise namespace and a label selector. A short scoped command is faster to interpret than a cluster-wide wall of output. When a Pod has never scheduled or its image never pulled, there may be no application logs yet. Read the event explaining why a container could not start before trying more log flags.

## Select the right log source

`kubectl logs` reads standard output and standard error captured by the container runtime. It does not automatically read an arbitrary file inside the image. Applications that only write `/var/log/app.log` need a deliberate logging design; `kubectl logs` cannot infer which file matters.

```bash
kubectl -n logs-monitoring-lab logs deployment/dojo-stream -c main --tail=50
kubectl -n logs-monitoring-lab logs -l app=dojo-stream \
  --all-containers=true --prefix --tail=100 --since=5m
kubectl -n logs-monitoring-lab logs dojo-crash -c main --previous --tail=30
```

A Deployment resource argument conveniently selects a Pod belonging to that workload. Do not assume `logs deployment/NAME` aggregates every replica automatically. Use a label selector or the appropriate all-Pods option when aggregation is required, and distinguish aggregation across Pods from `--all-containers`, which addresses container selection within each Pod.

For multi-container Pods, `-c NAME` is the clearest choice for one source. `--all-containers=true` includes all applicable containers. `--prefix` identifies the Pod and container on each line, which matters when identical messages come from several replicas. Selector-based logging has its own default tail limit; specify `--tail` deliberately. `--tail=-1` requests all available lines, but available logs remain limited by runtime retention and rotation.

`--since=10m` filters to a recent duration. `--since-time` accepts an absolute timestamp when comparing an incident window. `--timestamps` adds log timestamps for correlation. Avoid collecting huge outputs when a bounded window answers the question, and do not paste private application data into shared notes.

## Follow and previous mean different things

`kubectl logs -f` follows new output until interrupted. It is useful interactively, but it is not suitable for a lab solution that must terminate by itself. You can combine it with `--tail` to inspect recent context before following. When following many selected Pods, concurrent request limits can matter; inspect `--max-log-requests` rather than assuming a selector has no scale cost.

`--previous` retrieves the prior terminated instance of the selected container in the **same Pod**, if available. It does not retrieve the old Deployment revision, a deleted Pod, or every earlier crash. For CrashLoopBackOff, current logs may contain almost nothing while previous logs reveal the startup error. A container can restart many times before you inspect it, so collect evidence promptly.

The lab preserves a known diagnostic string in `previous.log`. That is an evidence artifact, not a task to modify the workload. Reproducing the exact logging command is unnecessary if another correct observation captures the required outcome.

## Events explain the platform's decisions

Events expose reasons such as FailedScheduling, FailedMount, BackOff, Unhealthy, and Pulled. A single warning can summarize repeated occurrences, so read its count and last occurrence as well as the reason. Events expire and are best-effort diagnostic information, not a guaranteed history of everything that happened.

```bash
kubectl -n logs-monitoring-lab events --for pod/dojo-pending
kubectl -n logs-monitoring-lab get events \
  --field-selector involvedObject.kind=Pod,involvedObject.name=dojo-pending \
  --sort-by=.metadata.creationTimestamp
kubectl -n logs-monitoring-lab describe pod dojo-pending
```

`kubectl events` is a convenient task-oriented view. `get events` offers familiar formatting and selectors. Sorting by creationTimestamp orders event objects; repeated updates to an existing event do not create a perfect incident timeline. `describe` puts configuration, conditions, and recent relevant events together, making it a useful first diagnosis command.

| Symptom | Evidence to read first |
|---|---|
| Pending, no node assigned | Scheduling events, requests, selectors, PVCs |
| Container waiting for configuration | Missing ConfigMap/Secret reference events |
| Repeated restarts | Last termination state and previous logs |
| Running but not serving | Readiness, Service selector, EndpointSlices |

## Watch and filter precisely

`kubectl get pods -w` prints an initial list and subsequent changes. Stop an interactive watch when the relevant transition appears; it is not a finite command for an automated solution. `-o wide` adds node and IP details. `-L track,version` displays selected labels as columns, and `--show-labels` helps find the correct selector.

Labels describe arbitrary application groupings: `-l app=dojo-stream` selects this lab's stream workload. Field selectors operate on supported API fields, such as `--field-selector=status.phase=Pending` for Pods. They are not arbitrary JSONPath predicates. Supported fields vary by resource, and event involvedObject fields differ from Pod status fields. A label selector that returns nothing may simply be targeting the wrong label key.

## Metrics are optional sampled usage

`kubectl top` needs a working Metrics API, usually provided by metrics-server. It is not installed in every cluster or in PodDojo by default. This lab setup downloads the official, pinned metrics-server v0.8.0 `components.yaml`, gives its supporting objects lesson-specific names, and patches `--kubelet-insecure-tls` for the local kind kubelet certificates. That flag disables kubelet certificate verification and is a lab accommodation, not production guidance.

The aggregation registration must have the canonical name `v1beta1.metrics.k8s.io`; APIService names encode their served version and group. Setup reuses an existing registration and labels an installation it creates so cleanup only removes its own registration. Supporting RBAC names use `dojo-`; a narrowly named authentication-reader binding lives in kube-system because that authentication ConfigMap belongs there. Workloads run in the lesson namespace.

```bash
kubectl -n logs-monitoring-lab top pods -l app=dojo-stream
kubectl -n logs-monitoring-lab top pods -l app=dojo-stream --containers
kubectl top nodes
```

CPU appears in millicores: 1000m is one CPU core. Memory is typically shown in Mi and represents a working-set-style usage measurement, not the requested or limited amount. Requests influence scheduling; limits constrain execution; top reports observed usage over a collection window. An idle process can legitimately report 0m. Metrics may need several intervals after Pod startup, so poll for availability instead of grading an exact number.

## Exam tips

Choose the evidence source that matches the failure stage. Specify namespace, container, and time window. Save requested output paths exactly. If top is unavailable, distinguish missing metrics infrastructure from application failure; on the real exam, do not install cluster components unless the task authorizes it. Close the investigation with a concrete observation, not a guessed diagnosis based only on the Pod's display status.
