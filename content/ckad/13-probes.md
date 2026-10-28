+++
id = "ckad-probes"
title = "Readiness, liveness, and startup probes"
module = "observability"
minutes = 45
domain = "observability"
needs = "cluster"
objectives = ["Choose probes according to traffic eligibility, restart recovery, and startup time", "Configure HTTP, TCP, exec, and gRPC health checks", "Trace readiness into Service endpoint eligibility", "Repair a liveness probe that repeatedly restarts a healthy application"]

[[quiz]]
q = "Which probe failure normally removes a Pod from ordinary Service traffic without restarting its container?"
choices = ["Readiness", "Liveness", "Startup before success", "Liveness with a high failureThreshold"]
answer = 0
why = "Readiness controls whether the Pod should receive traffic. Liveness and startup failures past their thresholds terminate the container and apply its restart policy. Readiness alone changes traffic eligibility without terminating it."

[[quiz]]
q = "What happens to liveness and readiness while a startup probe has not succeeded?"
choices = ["They run every second", "They are held until startup succeeds", "They run, but their failures are ignored", "Only liveness runs"]
answer = 1
why = "A startup probe protects slow initialization. It must succeed before liveness and readiness probes start. Repeated startup failure terminates the container; restart then depends on its policy."

[[quiz]]
q = "Which successThreshold is valid for liveness?"
choices = ["Any positive number", "Only 1", "Any value up to failureThreshold", "At least 2, to avoid flapping"]
answer = 1
why = "Liveness and startup require successThreshold 1. Readiness may require multiple consecutive successful checks to rejoin traffic more cautiously."

[[quiz]]
q = "What does exec command [\"test\", \"-f\", \"/tmp/ready\"] rely on?"
choices = ["A shell automatically interprets the array", "The executable test exists in the image and returns exit code zero", "The shell resolves test as a builtin command", "The kubelet runs test on the node, not in the container"]
answer = 1
why = "Exec launches the listed command without an implicit shell. Explicitly invoke sh -c for shell syntax, and ensure the image contains all required programs."

[[quiz]]
q = "Which gRPC probe constraint matters?"
choices = ["It supports named ports exactly like HTTP probes", "It requires a numeric port and the gRPC health checking protocol", "It works against any HTTP/2 route", "It requires TLS to be configured on the probe"]
answer = 1
why = "The built-in gRPC action checks the standard health protocol, takes a numeric port, and uses plaintext by default. Kubernetes v1.37 offers alpha TLS mode only behind GRPCContainerProbeTLS; it does not provide a full client's authentication configuration."

[lab]
summary = "Observe an unready web server, restore its endpoint, induce liveness restarts, then repair the probe and add startup protection."
setup = [
'''
kubectl create namespace probes-lab --dry-run=client -o yaml | kubectl apply -f -
'''
]
cleanup = [
'''
kubectl delete namespace probes-lab --ignore-not-found --wait=false || true
'''
]

[lab.files]
"probes.yaml" = '''
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
  namespace: probes-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-web}
  template:
    metadata:
      labels: {app: dojo-web}
    spec:
      containers:
      - name: web
        image: nginx:1.27-alpine
        readinessProbe:
          httpGet: {path: /missing, port: 80}
          periodSeconds: 2
          timeoutSeconds: 1
          failureThreshold: 1
---
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
  namespace: probes-lab
spec:
  selector: {app: dojo-web}
  ports:
  - port: 80
    targetPort: 80
'''

[[lab.steps]]
title = "Create a running but unready web server"
task = '''
Apply probes.yaml in probes-lab. It creates dojo-web with readiness probing /missing on port 80 and Service dojo-web. Wait for the container to be Running but not Ready; confirm there are no ready Service endpoints.
'''
hints = ["Running phase is not the Ready condition.", "The default nginx document root has no /missing resource."]
solution = '''
```bash
kubectl apply -f probes.yaml
for i in $(seq 1 60); do
  phase=$(kubectl -n probes-lab get pods -l app=dojo-web -o jsonpath='{.items[0].status.phase}' 2>/dev/null || true)
  [ "$phase" = Running ] && break
  sleep 2
done
[ "$phase" = Running ]
kubectl -n probes-lab describe pods -l app=dojo-web
```
'''

[[lab.steps.checks]]
name = "a running container is excluded from ready endpoints"
run = '''
kubectl -n probes-lab get deployment/dojo-web -o name >/dev/null 2>&1 || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n probes-lab get pods -l app=dojo-web -o jsonpath='{.items[0].status.phase} {.items[0].status.containerStatuses[0].ready}')" = "Running false" ] && [ -z "$(kubectl -n probes-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{end}')" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Restore readiness and traffic eligibility"
task = '''
Fix dojo-web readiness to HTTP / on port 80. Preserve the 2-second period, 1-second timeout, and failureThreshold 1. Wait for the rollout and a ready Service endpoint.
'''
hints = ["Patch the Deployment template so replacements inherit the fix.", "Use rollout status rather than sleeping for a guessed startup time."]
solution = '''
```bash
kubectl -n probes-lab patch deployment dojo-web --type=strategic -p '{"spec":{"template":{"spec":{"containers":[{"name":"web","readinessProbe":{"httpGet":{"path":"/","port":80}}}]}}}}'
kubectl -n probes-lab rollout status deployment/dojo-web --timeout=120s
```
'''

[[lab.steps.checks]]
name = "healthy readiness exposes the web endpoint"
run = '''
[ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].readinessProbe.httpGet.path}')" = "/" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].readinessProbe.httpGet.path}')" = "/" ] && [ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ] && [ -n "$(kubectl -n probes-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{end}')" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Observe liveness restarting a healthy process"
task = '''
Add a deliberately wrong HTTP liveness probe to web: path /missing, port 80, initialDelaySeconds 1, periodSeconds 2, timeoutSeconds 1, failureThreshold 2. Keep readiness healthy. Wait until a web container records at least one restart, then inspect events.
'''
hints = ["The server is running; the health check is wrong.", "Do not wait for a permanently healthy rollout; poll restartCount to observe this failure."]
solution = '''
```bash
kubectl -n probes-lab patch deployment dojo-web --type=strategic -p '{"spec":{"template":{"spec":{"containers":[{"name":"web","livenessProbe":{"httpGet":{"path":"/missing","port":80},"initialDelaySeconds":1,"periodSeconds":2,"timeoutSeconds":1,"failureThreshold":2}}]}}}}'
for i in $(seq 1 60); do
  if kubectl -n probes-lab get pods -l app=dojo-web -o jsonpath='{range .items[*].status.containerStatuses[*]}{.restartCount}{"\n"}{end}' | awk '$1>0 {found=1} END {exit !found}'; then break; fi
  sleep 2
done
kubectl -n probes-lab get events --sort-by=.metadata.creationTimestamp
```
'''

[[lab.steps.checks]]
name = "the broken liveness probe has caused a restart"
run = '''
[ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].livenessProbe.httpGet.path}')" = "/missing" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].livenessProbe.httpGet.path}')" = "/missing" ] && kubectl -n probes-lab get pods -l app=dojo-web -o jsonpath='{range .items[*].status.containerStatuses[*]}{.restartCount}{"\n"}{end}' | awk '$1>0 {found=1} END {exit !found}'; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 150

[[lab.steps]]
title = "Fix liveness and protect startup"
task = '''
Change liveness to HTTP /:80, keeping its existing timing. Add a startup TCP probe on port 80 with periodSeconds 2, timeoutSeconds 1, failureThreshold 30. Wait for the new rollout and ready endpoints. Leave readiness on /:80.
'''
hints = ["A startup probe gates liveness and readiness until initialization succeeds.", "The TCP startup action proves the listener exists, while HTTP readiness proves the web route responds."]
solution = '''
```bash
kubectl -n probes-lab patch deployment dojo-web --type=strategic -p '{"spec":{"template":{"spec":{"containers":[{"name":"web","livenessProbe":{"httpGet":{"path":"/","port":80}},"startupProbe":{"tcpSocket":{"port":80},"periodSeconds":2,"timeoutSeconds":1,"failureThreshold":30}}]}}}}'
kubectl -n probes-lab rollout status deployment/dojo-web --timeout=120s
```
'''

[[lab.steps.checks]]
name = "fixed health checks converge with startup protection"
run = '''
[ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].startupProbe.tcpSocket.port}')" = "80" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].livenessProbe.httpGet.path} {.spec.template.spec.containers[0].startupProbe.tcpSocket.port} {.spec.template.spec.containers[0].startupProbe.periodSeconds} {.spec.template.spec.containers[0].startupProbe.failureThreshold}')" = "/ 80 2 30" ] && [ "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n probes-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ] && [ -n "$(kubectl -n probes-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{end}')" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120
+++

## Three questions, three consequences

A process can be alive without being ready to serve requests. It can also be temporarily busy without needing a restart. Kubernetes probes let the application distinguish these states. The kubelet runs probes for individual containers and uses the results to decide traffic eligibility or restart behavior. A good probe encodes a useful recovery decision, not merely a command that happens to return zero.

| Probe | Question | Repeated failure means |
|---|---|---|
| startup | Has initialization finished? | Terminate after the failure threshold; restart according to policy |
| readiness | Can this container serve now? | Mark it unready and normally exclude its Pod from Service backends |
| liveness | Is this container stuck beyond useful self-recovery? | Restart the container according to restart behavior |

Readiness failure does not restart a container. Liveness failure does not inherently explain *why* it is unhealthy. With the ordinary Deployment Pod restart policy, liveness failure restarts that container within the same Pod; it does not require creating a new Deployment or changing its replica count.

```text
container starts -> startup succeeds -> readiness checks -> Ready -> traffic
                         |                    |
                   liveness begins       failure: stop traffic
                         |
                  failure threshold: restart container
```

Without a readiness probe, a running container can become Ready before the application is actually prepared. Without a liveness probe, a deadlocked process may stay running indefinitely. Neither omission should be fixed by adding an aggressive arbitrary check: decide what the application can honestly report first.

## Choose an action the application supports

`httpGet` asks the Pod IP for a path and port; an HTTP success status from 200 through 399 counts as success. It is not a request through the Service, ingress, or public load balancer. In nginx, `/` works by default, while `/healthz` does not magically exist because Kubernetes calls it a health endpoint.

```yaml
readinessProbe:
  httpGet:
    path: /
    port: 80
  periodSeconds: 2
  timeoutSeconds: 1
  failureThreshold: 1
  successThreshold: 1
```

`tcpSocket` attempts a TCP connection to the Pod's listener. It proves something accepts connections, not that a transaction works or a database query succeeds. It is useful for simple listeners and startup detection, but a process can accept TCP while its application is broken. HTTP and TCP probes can use a named container port, provided that name actually exists in the Pod specification.

`exec` runs a command **inside** the container. Exit code zero is success. There is no automatic shell expansion, pipe handling, or globbing. For shell syntax, explicitly use a shell that the image contains:

```yaml
livenessProbe:
  exec:
    command: [sh, -c, 'test -f /tmp/healthy']
  periodSeconds: 5
```

A minimal or distroless image may not contain `sh`, `cat`, or even the binary you expect. Repeated exec probes also create processes and consume resources. Prefer a native health endpoint when it expresses the same check clearly and cheaply.

`grpc` speaks the standard gRPC health checking protocol. Configure a numeric port and optionally a service name that the application's health server recognizes:

```yaml
readinessProbe:
  grpc:
    port: 50051
    service: readiness
  timeoutSeconds: 2
```

This fragment is for a gRPC application, not nginx. Built-in gRPC probes require numeric ports and normally connect to the Pod IP over plaintext; they do not offer a custom-hostname field or client authentication. Kubernetes v1.37 adds alpha `grpc.mode: TLS` behind the disabled-by-default `GRPCContainerProbeTLS` gate on the API server and kubelet. That mode skips certificate verification. Use the default plaintext behavior in this lab; see the [upstream probe guide](https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/#use-tls-with-grpc-probes). An ordinary HTTP/2 endpoint is not automatically a gRPC health endpoint. Match the probe to the server implementation instead of selecting the newest-looking action.

## Timing is a failure budget

`initialDelaySeconds` defaults to zero and delays initial probing. With a startup probe, readiness and liveness cannot run before startup succeeds; their initial delays then begin, as described in the [probe field reference](https://kubernetes.io/docs/concepts/workloads/pods/probes/#configuration-fields). `periodSeconds` sets the nominal check interval. `timeoutSeconds` bounds one attempt. `failureThreshold` requires consecutive failures before the action is taken; `successThreshold` requires consecutive successes to recover. Liveness and startup successThreshold must be one; readiness can require more than one success.

Defaults include a ten-second period, one-second timeout, three failures, and one success. These defaults are not appropriate for every application. A one-second timeout can be too short during normal load, while a long period may make recovery sluggish. Readiness may be checked more frequently while unready, so do not treat interval multiplication as an exact wall-clock promise.

A startup probe with period two seconds and failure threshold thirty provides roughly a minute of startup attempts. While it has not succeeded, readiness and liveness probes are held back. Once startup succeeds, it stops running for that container lifetime, and ordinary readiness/liveness take over. A container restart starts that sequence again.

```yaml
startupProbe:
  tcpSocket:
    port: 80
  periodSeconds: 2
  timeoutSeconds: 1
  failureThreshold: 30
livenessProbe:
  httpGet: {path: /, port: 80}
  periodSeconds: 10
  failureThreshold: 3
```

A large initial delay can protect startup too, but it delays liveness every time even when startup is fast. A startup probe adapts by allowing early success. Do not make readiness wait for an arbitrary long delay if the application can already report readiness accurately.

## Readiness controls endpoint eligibility

A normal selector-based Service gets its backend data through EndpointSlices. When a Pod becomes unready, its endpoint is normally marked not ready and excluded from ordinary new-connection routing. The endpoint may still exist in the slice: look at readiness conditions, not just whether an IP is present. Services configured to publish not-ready addresses behave differently, so inspect that setting before assuming default routing behavior.

```bash
kubectl -n probes-lab get pods -l app=dojo-web
kubectl -n probes-lab describe pods -l app=dojo-web
kubectl -n probes-lab get endpointslices \
  -l kubernetes.io/service-name=dojo-web -o yaml
```

The `Running` Pod phase only says containers have been started or are being started/restarted. A Pod showing `0/1 Running` can have no eligible ready Service backend. Endpoint updates take time, and established connections are a separate concern from eligibility for new ones.

## Repair the probe, preserve the application

The lab deliberately probes a nonexistent nginx path. Logs show an otherwise healthy web server, while events show probe failures and restart counts grow. Restarting the Pod manually repeats the same mistake because the Deployment template still contains the broken path. Fix the template and wait for its replacement Pods.

Use `kubectl patch deployment` for a small targeted change, or edit the manifest and apply it. In the exam, `kubectl edit` is also a valid interactive option. Most Pod probe fields cannot be changed in place, so modifying a managed Pod directly is the wrong repair level. Check that a successful rollout corresponds to correct probes and ready endpoints, not just a fleeting Ready state between liveness restarts.

## Exam tips

Identify whether the task asks for traffic removal, restart recovery, or slow-start protection. Inspect the actual container name, listening port, and health path. Keep liveness independent of a flaky external dependency when restarting cannot repair that dependency. Finish with Deployment rollout status, Pod readiness, events, and EndpointSlice conditions; these observations explain both the local container behavior and its Service effect.
