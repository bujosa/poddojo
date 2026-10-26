+++
id = "ckad-resources-and-quotas"
title = "Resources, QoS and namespace budgets"
module = "config-security"
minutes = 45
domain = "config-security"
needs = "cluster"
objectives = [
  "Distinguish scheduling requests from runtime limits and interpret resource units",
  "Derive QoS classes and diagnose OOM termination versus CPU throttling",
  "Set LimitRange defaults, minimums and maximums",
  "Apply compute and object-count ResourceQuotas",
  "Inspect admitted Pod resources and quota usage rather than assuming defaults",
]

[[quiz]]
q = "What does a CPU request of 250m mean?"
choices = ["A quarter of the node's CPU", "One quarter of a CPU", "250 MiB of memory", "A hard cap of a quarter CPU"]
answer = 1
why = "1000 millicores equals one CPU. The scheduler accounts for a 250m request when placing the Pod; this request is not itself a hard usage cap. A separate CPU limit controls throttling."

[[quiz]]
q = "With only container-level resources, which Pod is Guaranteed?"
choices = ["One container has a memory limit only", "Every container has equal CPU request/limit and equal memory request/limit", "Every container has a CPU request only", "The Pod has no resource settings"]
answer = 1
why = "For this container-level case, every container, including init containers, must meet both CPU and memory equality conditions. Other nonempty configurations are Burstable; a Pod without CPU or memory requests or limits is BestEffort. Pod-level budgets can also influence QoS in current Kubernetes."

[[quiz]]
q = "A Linux container repeatedly exceeds its CPU limit. What normally happens?"
choices = ["It is OOMKilled", "The kubelet restarts it", "Its CPU execution is throttled", "It is evicted from the node"]
answer = 2
why = "CPU is compressible: the kernel can delay execution. Memory exhaustion may instead trigger an OOM kill. CPU throttling can still cause latency and probe failures without directly terminating the process."

[[quiz]]
q = "A quota tracks requests.cpu and requests.memory. A new Pod supplies neither, and there are no defaults. What happens?"
choices = ["It is always BestEffort and bypasses quota", "Admission rejects it for missing resource declarations", "It is admitted and counted as zero", "The quota assigns it the namespace's average request"]
answer = 1
why = "Compute quota needs resource declarations to account for new Pods. Explicit requests, limit-to-request defaulting, or LimitRange defaults can supply them. A quota tracking only object counts does not impose this particular compute requirement."

[[quiz]]
q = "What changes when a LimitRange is added after Pods already exist?"
choices = ["All running Pods immediately receive its defaults", "It influences newly admitted objects; existing Pods are not retroactively rewritten", "Existing Pods restart", "Existing Pods that violate it are evicted"]
answer = 1
why = "LimitRange admission defaults and validates new requests. It does not resize or restart existing Pods, so inspect the admitted resource fields rather than assuming all Pods in a namespace share today's policy."

[lab]
summary = "Compare all three QoS classes, add a namespace quota, then use LimitRange defaults and explicit resources to satisfy admission."
setup = ["kubectl create namespace resources-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace resources-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Create three QoS examples before installing policy"
task = """
In `resources-lab`, create three single-container Pods using `busybox:1.36` and `sleep 3600`: `guaranteed` with CPU request/limit `50m` and memory request/limit `32Mi`; `burstable` with requests `25m`, `16Mi` and limits `100m`, `64Mi`; and `besteffort` with no resources. Wait for all three to be Ready.
"""
hints = ["Set resources under each container, not under metadata.", "Read status.qosClass after the kubelet has observed the Pod."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: guaranteed
  namespace: resources-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    resources:
      requests: {cpu: 50m, memory: 32Mi}
      limits: {cpu: 50m, memory: 32Mi}
---
apiVersion: v1
kind: Pod
metadata:
  name: burstable
  namespace: resources-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    resources:
      requests: {cpu: 25m, memory: 16Mi}
      limits: {cpu: 100m, memory: 64Mi}
---
apiVersion: v1
kind: Pod
metadata:
  name: besteffort
  namespace: resources-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
EOF
kubectl -n resources-lab wait --for=condition=Ready pod/guaranteed pod/burstable pod/besteffort --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the equal-budget Pod is Guaranteed"
run = '''kubectl -n resources-lab get pod guaranteed >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n resources-lab get pod guaranteed -o jsonpath='{.status.qosClass}' 2>/dev/null); [ "$v" = Guaranteed ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the unequal-budget Pod is Burstable"
run = '''kubectl -n resources-lab get pod burstable >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n resources-lab get pod burstable -o jsonpath='{.status.qosClass}' 2>/dev/null); [ "$v" = Burstable ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the unbudgeted Pod is BestEffort"
run = '''kubectl -n resources-lab get pod besteffort >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n resources-lab get pod besteffort -o jsonpath='{.status.qosClass}' 2>/dev/null); [ "$v" = BestEffort ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Install a namespace budget"
task = """
Create ResourceQuota `team-budget` in `resources-lab` with requests CPU `800m`, requests memory `256Mi`, limits CPU `2`, limits memory `512Mi`, `pods: 6`, and `configmaps: 2`. Inspect it with `kubectl describe quota`. These are namespace totals, not per-container settings.
"""
hints = ["Use spec.hard with keys such as requests.cpu.", "Wait for status.hard to appear before attempting quota-dependent creation."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: ResourceQuota
metadata:
  name: team-budget
  namespace: resources-lab
spec:
  hard:
    requests.cpu: 800m
    requests.memory: 256Mi
    limits.cpu: "2"
    limits.memory: 512Mi
    pods: "6"
    configmaps: "2"
EOF
for i in $(seq 1 30); do
  v=$(kubectl -n resources-lab get quota team-budget -o jsonpath='{.status.hard.pods}')
  [ "$v" = 6 ] && break
  sleep 2
done
[ "$v" = 6 ]
kubectl -n resources-lab describe quota team-budget
```
Without defaults, another unbudgeted Pod would now be rejected for missing the tracked compute resources. The existing BestEffort Pod is not rewritten.
'''
[[lab.steps.checks]]
name = "the compute quota contains the requested namespace totals"
run = '''kubectl -n resources-lab get quota team-budget -o jsonpath='{.spec.hard.requests\.cpu} {.spec.hard.requests\.memory} {.spec.hard.limits\.cpu} {.spec.hard.limits\.memory}' '''
equals = "800m 256Mi 2 512Mi"
[[lab.steps.checks]]
name = "the quota accounts for object counts"
run = "kubectl -n resources-lab get quota team-budget -o jsonpath='{.spec.hard.pods} {.spec.hard.configmaps}'"
equals = "6 2"

[[lab.steps]]
title = "Default resources at admission"
task = """
Create LimitRange `container-policy` in `resources-lab` for type `Container`: minimum `10m` CPU and `8Mi` memory, maximum `200m` CPU and `128Mi` memory, default requests `25m` CPU and `16Mi` memory, default limits `100m` CPU and `64Mi` memory. Then create Pod `defaulted` using `busybox:1.36`, `sleep 3600`, and no resources in your input manifest. Verify that admission supplies all four defaults.
"""
hints = ["defaultRequest supplies requests; default supplies limits.", "Create the LimitRange before the new Pod."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: LimitRange
metadata:
  name: container-policy
  namespace: resources-lab
spec:
  limits:
  - type: Container
    min: {cpu: 10m, memory: 8Mi}
    max: {cpu: 200m, memory: 128Mi}
    defaultRequest: {cpu: 25m, memory: 16Mi}
    default: {cpu: 100m, memory: 64Mi}
EOF
kubectl -n resources-lab run defaulted --image=busybox:1.36 --restart=Never --command -- sleep 3600
kubectl -n resources-lab wait --for=condition=Ready pod/defaulted --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the LimitRange sets container minimums and maximums"
run = "kubectl -n resources-lab get limitrange container-policy -o jsonpath='{.spec.limits[0].type} {.spec.limits[0].min.cpu} {.spec.limits[0].min.memory} {.spec.limits[0].max.cpu} {.spec.limits[0].max.memory}'"
equals = "Container 10m 8Mi 200m 128Mi"
[[lab.steps.checks]]
name = "the admitted Pod contains all resource defaults"
run = "kubectl -n resources-lab get pod defaulted -o jsonpath='{.spec.containers[0].resources.requests.cpu} {.spec.containers[0].resources.requests.memory} {.spec.containers[0].resources.limits.cpu} {.spec.containers[0].resources.limits.memory}'"
equals = "25m 16Mi 100m 64Mi"

[[lab.steps]]
title = "Fit an explicit workload into the remaining budget"
task = """
Create Pod `budgeted` in `resources-lab`, using `busybox:1.36` and `sleep 3600`, with requests `50m` CPU and `32Mi` memory and limits `100m` CPU and `64Mi` memory. Wait for Ready. Inspect the quota: the five running Pods should account for `150m` requested CPU and `96Mi` requested memory.
"""
hints = ["Explicit settings override defaults but must still satisfy the LimitRange and quota.", "Quota usage is updated asynchronously; poll instead of assuming it is immediate."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: budgeted
  namespace: resources-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    resources:
      requests: {cpu: 50m, memory: 32Mi}
      limits: {cpu: 100m, memory: 64Mi}
EOF
kubectl -n resources-lab wait --for=condition=Ready pod/budgeted --timeout=120s
kubectl -n resources-lab describe quota team-budget
```
'''
[[lab.steps.checks]]
name = "the explicit workload has the requested budget"
run = "kubectl -n resources-lab get pod budgeted -o jsonpath='{.spec.containers[0].resources.requests.cpu} {.spec.containers[0].resources.requests.memory} {.spec.containers[0].resources.limits.cpu} {.spec.containers[0].resources.limits.memory}'"
equals = "50m 32Mi 100m 64Mi"
[[lab.steps.checks]]
name = "quota usage includes five Pods and their aggregate requests"
run = '''kubectl -n resources-lab get pod budgeted >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n resources-lab get quota team-budget -o jsonpath='{.status.used.pods} {.status.used.requests\.cpu} {.status.used.requests\.memory}' 2>/dev/null); [ "$v" = '5 150m 96Mi' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
+++

## Placement and enforcement answer different questions

A **request** tells Kubernetes how much capacity to account for when placing a workload. A **limit** tells the runtime how much use to permit. Neither is a measurement of current consumption. A sleeping process can request more CPU than a busy process uses, and the scheduler still reserves placement capacity according to the declared request.

```text
Pod declaration → admission defaults and policy → scheduler checks requests
                                                    ↓
                                              node runs Pod
                                                    ↓
                                          runtime enforces limits
```

If a node has spare capacity, a container may use more than its request, subject to limits and contention. Requests are not hard ceilings. CPU requests also influence relative CPU allocation under contention. Memory requests inform placement and eviction decisions; they do not preallocate an isolated block of RAM to the process.

## Units are part of the answer

| Quantity | Meaning | Common mistake |
|---|---|---|
| CPU `1` | One CPU unit | Assuming it means one dedicated physical core |
| CPU `250m` | 0.25 CPU | Reading `m` as megabytes |
| Memory `128Mi` | 128 × 1024² bytes | Treating it as decimal megabytes |
| Memory `128M` | 128 × 1000² bytes | Assuming it equals `128Mi` |
| Memory `400m` | 0.4 bytes | Trying to request 400 MiB |

CPU quantities finer than `1m` are invalid. Use `Mi` and `Gi` deliberately for memory. An API may normalize quantities when printing them, so `0.1` CPU can appear as `100m`. Requests cannot exceed corresponding limits. If a limit exists without a request, Kubernetes normally defaults that request to the limit unless another admission default supplies it.

```yaml
resources:
  requests:
    cpu: 100m
    memory: 64Mi
  limits:
    cpu: 500m
    memory: 128Mi
```

These fields sit under a container. The scheduler accounts for the Pod's effective resource needs, including special handling for init containers, restartable init sidecars and Pod overhead; do not blindly sum every container that appears in YAML. For an ordinary Pod with only regular application containers, their concurrent requests add together.

## CPU throttling and OOM termination

CPU is compressible: the kernel can delay a process that exhausts its CPU allowance. This **throttling** can make requests slow and cause probes to time out without directly killing the container. Raising a memory limit will not fix CPU throttling.

Memory cannot be handled in the same way. Exceeding a container's memory limit can lead to an out-of-memory kill. Inspect `status.containerStatuses[].lastState.terminated.reason`, restart counts and `kubectl describe pod`; `OOMKilled` is more informative than simply noticing exit code 137, which can have other causes. A restart follows the Pod's restart policy. Node pressure can also evict a Pod, which is a different mechanism from an individual container exceeding its memory limit.

`kubectl top` needs a metrics API; it is not available by default in this lab. Diagnose declarations, events and termination state without assuming metrics-server exists. Resource sizing should ultimately use measurements from realistic workload behavior, not arbitrary low limits chosen only to make YAML look tidy.

## QoS is derived, not selected by a label

For Pods using **container-level** CPU and memory settings:

| Class | Derivation |
|---|---|
| Guaranteed | Every container, including init containers, has CPU and memory requests and limits, and each request equals its corresponding limit |
| BestEffort | No container has CPU or memory requests or limits |
| Burstable | Any other CPU/memory resource configuration |

CPU and memory values need not equal each other; `50m` CPU and `32Mi` memory are incomparable units. Each resource's request must equal its own limit for Guaranteed. One resource-free sidecar can change a whole Pod's class. Read the actual result with `kubectl get pod NAME -o jsonpath='{.status.qosClass}'`.

Kubernetes v1.37 also supports Pod-level CPU and memory budgets through `spec.resources` when the relevant feature is available. Those Pod-level declarations take precedence in QoS derivation, so the table above is explicitly the container-only case used in this lab. Inspect both levels in an unfamiliar manifest. QoS influences behavior under resource pressure, but Guaranteed is not a promise of immunity to eviction or OOM termination.

## LimitRange is per-object policy

A namespaced LimitRange can provide default requests and limits and impose minimums, maximums or request-to-limit ratios. `type: Container` constrains containers; other types address Pods or storage claims. Defaults apply at admission to new objects, not retroactively to existing Pods. Keep one clear policy rather than relying on ambiguous interactions among several defaulting LimitRanges.

```yaml
apiVersion: v1
kind: LimitRange
metadata:
  name: container-policy
  namespace: resources-lab
spec:
  limits:
  - type: Container
    min: {cpu: 10m, memory: 8Mi}
    max: {cpu: 200m, memory: 128Mi}
    defaultRequest: {cpu: 25m, memory: 16Mi}
    default: {cpu: 100m, memory: 64Mi}
```

Validate defaults against their own bounds. An explicit value outside policy is rejected; defaults do not override it into compliance. Inspect the stored Pod to learn what admission actually supplied. A Forbidden creation error is different from a Pending Pod: rejection means the Pod may not exist at all.

## ResourceQuota is the namespace total

ResourceQuota limits aggregate usage using `spec.hard`. Compute keys include `requests.cpu`, `requests.memory`, `limits.cpu` and `limits.memory`. Object keys include `pods`, `configmaps` and `count/deployments.apps`. A compute quota requires the tracked resource declarations on newly admitted Pods, either explicitly or through defaulting. A count-only quota does not impose CPU or memory settings.

```bash
kubectl -n resources-lab create quota team-budget \
  --hard=requests.cpu=800m,requests.memory=256Mi,pods=6 \
  --dry-run=client -o yaml
kubectl -n resources-lab describe quota team-budget
kubectl -n resources-lab describe limitrange container-policy
```

`describe quota` compares Used and Hard. Requested capacity, not live CPU utilization, consumes compute quota. Pending Pods can consume budget too. Quotas do not reserve node capacity: a namespace can be within its quota while a Pod remains unschedulable. Existing objects are not deleted merely because you introduce a restrictive quota; future admissions must satisfy it.

Workload controllers make admission failures less obvious. A Deployment can be accepted while its ReplicaSet fails to create the requested Pods because the namespace budget is exhausted. You may see fewer Pods than the desired replica count, rather than a Pending Pod for every missing replica. Describe the Deployment and ReplicaSet and read `FailedCreate` events. Lowering a container's actual CPU consumption will not free its declared request from quota; changing the resource declaration or removing a quota-consuming workload does. A rolling update also needs room for overlapping old and new Pods, so account for surge capacity when choosing replica counts and namespace budgets.

## Exam tips

For a rejected workload, inspect LimitRange and ResourceQuota before changing the container image. For Pending, inspect scheduler events. For restarts, inspect termination reasons. Generate YAML imperatively, put resources at the correct level, and verify admitted values and quota totals. Avoid repeatedly increasing replicas while a namespace budget is already exhausted; fix the declaration or the requested policy first.
