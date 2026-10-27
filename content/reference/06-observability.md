+++
id = "ref-observability"
title = "Observe and debug workloads"
order = 6
summary = "Use health semantics, events, logs, metrics, and targeted debug containers to move from symptom to root cause."
+++

## Probe decisions

Health checks are independent of whether a process exists. A running process may be unable to receive traffic, and a ready process may later deadlock. Choose checks that exercise the intended failure mode, with enough startup time for the application. The kubelet runs probes against the individual container or Pod, not through the Service.

| Probe | Success means | Repeated failure does | Typical use |
|---|---|---|---|
| startupProbe | Initialization finished | Restarts that container, subject to restart policy | Slow startup; gates readiness and liveness |
| readinessProbe | This instance can serve requests | Makes the container unready; normally removes ready Service endpoints | Dependency/configuration readiness |
| livenessProbe | Restarting this instance may recover it | Restarts that container, subject to restart policy | Deadlock or unrecoverable internal failure |

| Handler | Definition | Trap |
|---|---|---|
| HTTP | httpGet.path and port | Success is 200–399; a generic 200 root page may prove little |
| TCP | tcpSocket.port | A socket accepting connections does not prove business readiness |
| exec | exec.command array | Exit zero succeeds; shell expressions require sh -c |
| gRPC | grpc.port and optional service | Requires numeric port and the gRPC health protocol; no named-port or TLS options |

```yaml
# Fragment inside a container; the process must really provide these endpoints.
startupProbe:
  httpGet: {path: /startup, port: 8080}
  periodSeconds: 2
  failureThreshold: 30
readinessProbe:
  httpGet: {path: /ready, port: 8080}
  periodSeconds: 3
  timeoutSeconds: 1
  failureThreshold: 2
livenessProbe:
  httpGet: {path: /live, port: 8080}
  periodSeconds: 5
  timeoutSeconds: 1
  failureThreshold: 3
```

The rough startup allowance is periodSeconds × failureThreshold, plus timing details and any initial delay. failureThreshold and successThreshold default to 3 and 1 respectively; startup and liveness require successThreshold 1. Probe timeout defaults to one second. Never use an unconditional `true` check to repair a real readiness failure. Avoid liveness checks that fail whenever a shared dependency is unavailable: restarting every replica can amplify an outage. See [probe configuration](https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/).

## First minute of investigation

The following commands assume a workload named web and a selected Pod name stored in pod. Replace names and namespace with those from the task. Keep PodDojo's supplied context unchanged.

```bash
kubectl -n ref-practice get deploy,rs,pods -o wide
kubectl -n ref-practice get pods --show-labels
pod=$(kubectl -n ref-practice get pods -l app=web -o jsonpath='{.items[0].metadata.name}')
kubectl -n ref-practice describe pod "$pod"
kubectl -n ref-practice get events --sort-by=.metadata.creationTimestamp
kubectl -n ref-practice logs "$pod" -c web --tail=100 --timestamps
kubectl -n ref-practice logs "$pod" -c web --previous --tail=100
kubectl -n ref-practice describe deployment web
kubectl -n ref-practice describe replicaset
kubectl -n ref-practice top pods --containers
kubectl top nodes
```

A Deployment with no Pods may be failing admission: inspect ReplicaSet events for quota, LimitRange, Pod Security or missing ServiceAccount errors. If Pods exist, distinguish waiting, running and terminated container states from the Pod phase. STATUS in the default table is a convenient display, not a separate Pod phase. `kubectl top` needs resource metrics; PodDojo has no metrics-server by default. Missing metrics is not evidence that usage is zero.

## Debugging flowchart

```text
Workload unavailable
  |
  +-- No Pod created? --> ReplicaSet/controller events --> admission/quota/identity
  |
  +-- Pod Pending?
  |     +-- No node assigned --> FailedScheduling: requests, selectors, taints, PVC
  |     +-- Node assigned --> pull errors, mounts, sandbox, init-container state
  |
  +-- ImagePullBackOff? --> exact image/tag, registry auth, connectivity, pull policy
  |
  +-- CrashLoopBackOff? --> current + previous logs --> last exit reason/code
  |     +-- OOMKilled --> memory limit and actual use
  |     +-- Error --> command, config, permissions, dependencies, probe events
  |     +-- Completed --> finite command inside an Always-restarting workload
  |
  +-- Running but unready? --> readiness handler, listener, startup probe, gates
  |
  +-- Ready but unreachable? --> Pod IP, Service endpoints/ports, DNS, policy, Ingress
```

BackOff is a retry delay, not the root cause. Read the earlier event explaining what failed. OOMKilled is a termination reason; exit code 137 alone also occurs with other SIGKILL causes. Raising a limit can hide a leak or exceed quota; measure and inspect the failure first. Resource requests influence scheduling, while CPU limits throttle and memory limits can terminate a process. See [container-state details](https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/).

For ImagePullBackOff, inspect the full image reference, pull Secret namespace and registry response. If an image was built locally for kind, load it into the selected lab cluster and use IfNotPresent or Never as appropriate. The host Docker image store and kind node stores are separate. For Pending PVCs, inspect claim events, StorageClass and WaitForFirstConsumer behavior rather than immediately creating a PV.

## Focused inspection and debugging

```bash
kubectl -n ref-practice get pod "$pod" -o jsonpath='{range .status.containerStatuses[*]}{.name}{" "}{.restartCount}{" "}{.lastState.terminated.reason}{" "}{.lastState.terminated.exitCode}{"\n"}{end}'
kubectl -n ref-practice logs "$pod" --all-containers=true --prefix=true --since=5m
kubectl -n ref-practice logs "$pod" -c web -f
kubectl -n ref-practice exec "$pod" -c web -- sh -c 'id; ls -ld /tmp; cat /etc/resolv.conf'
kubectl -n ref-practice debug "$pod" --image=busybox:1.36 --target=web --container=inspect -- sleep 600
kubectl -n ref-practice exec "$pod" -c inspect -- ps
```

Ephemeral containers are useful when the application image lacks a shell. They share the Pod network, but seeing target processes through --target depends on runtime support. They do not automatically share the application's root filesystem or every mount. Ephemeral containers are not restarted and cannot be deleted from the Pod independently. Security policy and RBAC still apply. Use an approved debug image and profile; inspect `kubectl debug --help` for the installed client.

If the application exits too quickly, copy the Pod while changing its failing container command:

```bash
kubectl -n ref-practice debug "$pod" --copy-to=web-inspect --container=web -- sh -c 'sleep 600'
kubectl -n ref-practice exec web-inspect -c web -- sh -c 'id; printenv'
# Remove only your named temporary copy when finished:
kubectl -n ref-practice delete pod web-inspect --ignore-not-found
```

This requires a shell in that image. A copied Pod can have different scheduling, storage or identity effects; inspect its manifest and do not assume it reproduces every original condition. The [debugging guide](https://kubernetes.io/docs/tasks/debug/debug-application/debug-running-pod/) explains ephemeral containers and copies.

## Repair and retain evidence

Change the source of desired state: a Deployment template, CronJob jobTemplate, mounted configuration or package values. Observe a complete rollout, then test the operation the user needs. A passing probe alone does not prove a Service or Ingress works. Save requested evidence before deleting failed Pods or rolling them away.

```bash
kubectl -n ref-practice logs "$pod" -c web --previous > "$PODDOJO_LAB_DIR/previous-web.log"
kubectl -n ref-practice get events --sort-by=.metadata.creationTimestamp > "$PODDOJO_LAB_DIR/events.txt"
kubectl -n ref-practice rollout status deployment/web --timeout=120s
```

Collect only the requested logs and metadata, avoiding tokens or real credentials. A previous log may legitimately be unavailable if no container instance has terminated. Preserve the useful diagnostic error instead of replacing evidence with invented output.
