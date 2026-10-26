+++
id = "ckad-debugging"
title = "Debug workloads with a repeatable method"
module = "observability"
minutes = 60
domain = "observability"
needs = "cluster"
objectives = ["Locate the first failing stage from scheduling through Service routing", "Repair image, configuration, command, and readiness failures at their owner", "Use previous logs and termination details to explain restart loops", "Add an ephemeral diagnostic container without replacing an application image"]

[[quiz]]
q = "A Pod is Pending with no assigned node. Where do you start?"
choices = ["Application HTTP logs", "Scheduling events, resource requests, node constraints, and PVC status", "Logs from the previous container instance", "The Service's EndpointSlices"]
answer = 1
why = "A container cannot produce useful application logs before it starts. Scheduling evidence identifies whether resources, selectors, taints, or storage placement prevent a node assignment."

[[quiz]]
q = "What does CreateContainerConfigError with a missing ConfigMap key imply?"
choices = ["The application process rejected the key", "The container could not be configured before startup", "The image could not be pulled", "The container started, then crashed"]
answer = 1
why = "Kubelet resolves required ConfigMap and Secret references before creating the container. Fix the referenced object/key or the Pod template reference, then allow retry."

[[quiz]]
q = "Why is deleting a managed crashing Pod rarely a complete fix?"
choices = ["The deleted Pod is restarted on the same node", "Its controller creates another Pod from the same broken template", "Deletion resets the Deployment to its previous revision", "The kubelet blocks deletion while restarts continue"]
answer = 1
why = "Repair the owning Deployment or its required dependency. Recreating the same template usually reproduces the failure and can discard useful diagnostic evidence."

[[quiz]]
q = "What distinguishes OOMKilled from a generic nonzero exit?"
choices = ["Only the displayed Pod phase", "The last terminated container reason and memory context", "Exit code 1 in the container status", "A Warning event with reason BackOff"]
answer = 1
why = "Inspect lastState.terminated.reason and limits/events. Exit code 137 indicates SIGKILL but is not alone proof of an out-of-memory kill."

[[quiz]]
q = "What can an ephemeral container help with?"
choices = ["Repairing the Deployment template automatically", "Adding diagnostic tools to a running Pod without replacing its application image", "Changing the container's resource limits in place", "Adding a restartable sidecar that survives Pod restarts"]
answer = 1
why = "Ephemeral containers share the Pod network and can target a container process namespace when supported. They are diagnostic additions, not ordinary restartable application containers."

[lab]
summary = "Repair five broken workloads, restore a Service backend, and inspect the repaired web Pod with an ephemeral diagnostic container."
setup = [
'''
kubectl create namespace debugging-lab --dry-run=client -o yaml | kubectl apply -f -
''',
'''
kubectl apply -f broken.yaml
'''
]
cleanup = [
'''
kubectl delete namespace debugging-lab --ignore-not-found --wait=false || true
'''
]

[lab.files]
"broken.yaml" = '''
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-pending
  namespace: debugging-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-pending}
  template:
    metadata:
      labels: {app: dojo-pending}
    spec:
      nodeSelector:
        poddojo.io/absent-node: "true"
      containers:
      - name: main
        image: busybox:1.36
        command: [sleep, "3600"]
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-image
  namespace: debugging-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-image}
  template:
    metadata:
      labels: {app: dojo-image}
    spec:
      containers:
      - name: main
        image: nginx:1.27-alpine-dojo-missing
        command: [nginx, "-g", "daemon off;"]
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-config
  namespace: debugging-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-config}
  template:
    metadata:
      labels: {app: dojo-config}
    spec:
      containers:
      - name: main
        image: busybox:1.36
        command: [sleep, "3600"]
        env:
        - name: MODE
          valueFrom:
            configMapKeyRef: {name: dojo-settings, key: MODE}
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-crash
  namespace: debugging-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-crash}
  template:
    metadata:
      labels: {app: dojo-crash}
    spec:
      containers:
      - name: main
        image: busybox:1.36
        command: [sh, -c, 'echo "fatal: bad startup command"; sleep 2; exit 1']
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-settings
  namespace: debugging-lab
data:
  OTHER: preserve-me
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
  namespace: debugging-lab
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
---
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
  namespace: debugging-lab
spec:
  selector: {app: dojo-wrong}
  ports:
  - port: 80
    targetPort: 80
'''

[[lab.steps]]
title = "Resolve an impossible placement rule"
task = '''
In debugging-lab investigate Deployment dojo-pending. Remove its impossible nodeSelector, preserving its image busybox:1.36 and sleep workload. Wait for one available replica. Do not change node labels or cluster scheduling policy.
'''
hints = ["describe the Pod and read FailedScheduling.", "Patch spec.template.spec.nodeSelector to null on the Deployment."]
solution = '''
```bash
kubectl -n debugging-lab get pods -l app=dojo-pending -o wide
kubectl -n debugging-lab describe pods -l app=dojo-pending
kubectl -n debugging-lab patch deployment dojo-pending --type=merge -p '{"spec":{"template":{"spec":{"nodeSelector":null}}}}'
kubectl -n debugging-lab rollout status deployment/dojo-pending --timeout=120s
```
'''

[[lab.steps.checks]]
name = "placement repair produces an available replica"
run = '''
[ "$(kubectl -n debugging-lab get deployment/dojo-pending -o jsonpath='{.spec.template.spec.nodeSelector}')" = "" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n debugging-lab get deployment/dojo-pending -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n debugging-lab get deployment/dojo-pending -o jsonpath='{.metadata.generation}') 1 1 1 1" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Repair the image reference"
task = '''
Find the image-pull failure in dojo-image. Set container main to nginx:1.27-alpine and preserve its nginx foreground command. Wait for one available replica.
'''
hints = ["Events distinguish an invalid image tag from application crashes.", "Use set image on the Deployment rather than patching a generated Pod."]
solution = '''
```bash
kubectl -n debugging-lab describe pods -l app=dojo-image
kubectl -n debugging-lab set image deployment/dojo-image main=nginx:1.27-alpine
kubectl -n debugging-lab rollout status deployment/dojo-image --timeout=120s
```
'''

[[lab.steps.checks]]
name = "the valid image runs successfully"
run = '''
[ "$(kubectl -n debugging-lab get deployment/dojo-image -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.27-alpine" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n debugging-lab get deployment/dojo-image -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.27-alpine" ] && [ "$(kubectl -n debugging-lab get deployment/dojo-image -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n debugging-lab get deployment/dojo-image -o jsonpath='{.metadata.generation}') 1 1 1 1" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Supply the missing configuration key"
task = '''
Investigate dojo-config. Add MODE=ready to ConfigMap dojo-settings, preserving OTHER=preserve-me and the Deployment reference to key MODE. Wait for one available replica without making the required reference optional.
'''
hints = ["Read the container waiting reason and the ConfigMap keys.", "A ConfigMap merge patch can add one key while retaining existing data."]
solution = '''
```bash
kubectl -n debugging-lab describe pods -l app=dojo-config
kubectl -n debugging-lab get configmap dojo-settings -o yaml
kubectl -n debugging-lab patch configmap dojo-settings --type=merge -p '{"data":{"MODE":"ready"}}'
kubectl -n debugging-lab rollout status deployment/dojo-config --timeout=120s
```
'''

[[lab.steps.checks]]
name = "required configuration is present and its consumer starts"
run = '''
[ "$(kubectl -n debugging-lab get configmap/dojo-settings -o jsonpath='{.data.MODE}')" = "ready" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n debugging-lab get configmap/dojo-settings -o jsonpath='{.data.MODE} {.data.OTHER}')" = "ready preserve-me" ] && [ "$(kubectl -n debugging-lab get deployment/dojo-config -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n debugging-lab get deployment/dojo-config -o jsonpath='{.metadata.generation}') 1 1 1 1" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Repair a crashing startup command"
task = '''
Inspect dojo-crash current and previous logs. Change its main startup command so busybox:1.36 prints recovered and then remains alive for at least an hour. Wait for a healthy replica and verify the recovered message.
'''
hints = ["Use --previous once the old instance has terminated.", "A Deployment uses restartPolicy Always; an immediately exiting command will keep restarting."]
solution = '''
```bash
kubectl -n debugging-lab logs deployment/dojo-crash -c main --tail=20 || true
pod=$(kubectl -n debugging-lab get pods -l app=dojo-crash -o jsonpath='{.items[0].metadata.name}')
kubectl -n debugging-lab logs "$pod" -c main --previous --tail=20 || true
kubectl -n debugging-lab patch deployment dojo-crash --type=strategic -p '{"spec":{"template":{"spec":{"containers":[{"name":"main","command":["sh","-c","echo recovered; exec sleep 3600"]}]}}}}'
kubectl -n debugging-lab rollout status deployment/dojo-crash --timeout=120s
kubectl -n debugging-lab logs deployment/dojo-crash -c main --tail=20
```
'''

[[lab.steps.checks]]
name = "repaired startup stays available and announces recovery"
run = '''
kubectl -n debugging-lab logs deployment/dojo-crash -c main --tail=20 2>/dev/null | grep -q recovered || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n debugging-lab get deployment/dojo-crash -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n debugging-lab get deployment/dojo-crash -o jsonpath='{.metadata.generation}') 1 1 1 1" ] && kubectl -n debugging-lab logs deployment/dojo-crash -c main --tail=20 | grep -q recovered; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Restore readiness and Service selection"
task = '''
Repair dojo-web without changing nginx:1.27-alpine: readiness must probe HTTP /:80. Fix Service dojo-web to select app=dojo-web while retaining port 80 and targetPort 80. Confirm one available replica and a ready endpoint.
'''
hints = ["There are two independent faults: readiness and the Service label selector.", "Inspect EndpointSlices after both repairs, not just Pod Running status."]
solution = '''
```bash
kubectl -n debugging-lab patch deployment dojo-web --type=strategic -p '{"spec":{"template":{"spec":{"containers":[{"name":"web","readinessProbe":{"httpGet":{"path":"/","port":80}}}]}}}}'
kubectl -n debugging-lab patch service dojo-web --type=json -p '[{"op":"replace","path":"/spec/selector","value":{"app":"dojo-web"}}]'
kubectl -n debugging-lab rollout status deployment/dojo-web --timeout=120s
kubectl -n debugging-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o yaml
```
'''

[[lab.steps.checks]]
name = "correct selection and readiness restore a ready backend"
run = '''
[ "$(kubectl -n debugging-lab get service/dojo-web -o jsonpath='{.spec.selector.app}')" = "dojo-web" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n debugging-lab get service/dojo-web -o jsonpath='{.spec.selector.app} {.spec.ports[0].port} {.spec.ports[0].targetPort}')" = "dojo-web 80 80" ] && [ "$(kubectl -n debugging-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n debugging-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ] && [ -n "$(kubectl -n debugging-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{end}')" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Inspect with an ephemeral container"
task = '''
Add ephemeral container dojo-inspector using busybox:1.36 to the repaired dojo-web Pod. Target container web, request the general debug profile, and run wget -qO- http://127.0.0.1:80 non-interactively. Leave the completed diagnostic container recorded in the Pod; verify its logs contain Welcome to nginx!
'''
hints = ["kubectl debug supports --attach=false so the command returns without opening a terminal.", "All containers in a Pod share its network namespace, so loopback reaches nginx."]
solution = '''
```bash
pod=$(kubectl -n debugging-lab get pods -l app=dojo-web --sort-by=.metadata.creationTimestamp -o jsonpath='{.items[-1:].metadata.name}')
kubectl -n debugging-lab debug "$pod" --container=dojo-inspector --image=busybox:1.36 --target=web --profile=general --attach=false -- sh -c 'wget -qO- http://127.0.0.1:80'
for i in $(seq 1 60); do
  if kubectl -n debugging-lab logs "$pod" -c dojo-inspector 2>/dev/null | grep -q 'Welcome to nginx!'; then break; fi
  sleep 2
done
kubectl -n debugging-lab logs "$pod" -c dojo-inspector
```
'''

[[lab.steps.checks]]
name = "ephemeral diagnostics reach the live web listener"
run = '''
pod=$(kubectl -n debugging-lab get pods -l app=dojo-web --sort-by=.metadata.creationTimestamp -o jsonpath='{.items[-1:].metadata.name}'); [ "$(kubectl -n debugging-lab get pod "$pod" -o jsonpath='{.spec.ephemeralContainers[?(@.name=="dojo-inspector")].image}')" = busybox:1.36 ] || exit 1
for i in $(seq 1 45); do
  if pod=$(kubectl -n debugging-lab get pods -l app=dojo-web --sort-by=.metadata.creationTimestamp -o jsonpath='{.items[-1:].metadata.name}'); [ "$(kubectl -n debugging-lab get pod "$pod" -o jsonpath='{.spec.ephemeralContainers[?(@.name=="dojo-inspector")].image}')" = busybox:1.36 ] && kubectl -n debugging-lab logs "$pod" -c dojo-inspector 2>/dev/null | grep -q "Welcome to nginx!"; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120
+++

## Find the first broken boundary

Debugging is a sequence of narrowing questions. Did the API accept the object? Did a controller create a Pod? Did the scheduler place it? Could the kubelet configure and start its containers? Did the process stay alive and become ready? Does the Service select that ready Pod and target its listener? Start at the earliest failed boundary instead of making random edits at several layers.

```text
manifest -> controller -> scheduling -> image/config -> process -> readiness
                                                               |
client -> Service selector -> ready EndpointSlice target -> port/listener
```

Use a repeatable command ladder: **get → describe → events → logs → exec → debug → port-forward**. It is a diagnostic order, not a requirement to run every command for every incident. Stop when evidence identifies a cause, repair the owning configuration, then verify the requested behavior.

```bash
kubectl -n debugging-lab get deployment,pod,service -o wide
kubectl -n debugging-lab describe pods -l app=dojo-image
kubectl -n debugging-lab get events --sort-by=.metadata.creationTimestamp
```

Preserve useful failure evidence before deleting resources. A replacement Pod may erase access to the previous process's local state and logs. Fix a Deployment template or its dependency rather than repeatedly deleting the Pods that faithfully reproduce it.

## Pending means investigate placement

A Pending Pod may be waiting for scheduling or another prerequisite. First look at `spec.nodeName` and scheduling conditions. If no node was assigned, inspect FailedScheduling events. Excessive CPU or memory **requests** can make a Pod unschedulable even when observed node usage looks low. A required nodeSelector or affinity rule may match no node. Taints may exclude the Pod unless a suitable toleration exists.

Storage can also block placement or startup. Read the PVC status, StorageClass, and events. With a `WaitForFirstConsumer` StorageClass, a Pending PVC can be normal until a consumer's placement is decided; do not treat every unbound claim as a broken provisioner. Never solve a namespaced application exercise by relabeling arbitrary nodes, changing cluster policy, or deleting unrelated storage.

```yaml
spec:
  template:
    spec:
      nodeSelector:
        poddojo.io/absent-node: "true"
```

In the lab that rule intentionally matches nothing. Remove the impossible requirement from the owner instead of manufacturing a matching node. Read the task before reducing resource requests: sometimes a particular request is a requirement that must be preserved.

## Waiting reasons are not interchangeable

| Displayed state or reason | Likely first evidence | Typical repair |
|---|---|---|
| ErrImagePull / ImagePullBackOff | Pull events and exact image reference | Correct tag, repository, or authorized pull credentials |
| CreateContainerConfigError | Required ConfigMap/Secret keys | Supply the missing key or correct the reference |
| CrashLoopBackOff | Last termination state and previous logs | Fix command, configuration, dependency, or resource problem |
| OOMKilled | Terminated reason, limits, memory behavior | Repair memory use or a demonstrably inadequate limit |
| Running but 0/1 Ready | Probe result and actual listener | Fix health path, timing, or application readiness |

ImagePullBackOff means repeated image retrieval failures are being delayed. It does not mean the application crashed. Check spelling, the registry, tag existence, authentication, and events. The lab uses one intentionally nonexistent pinned tag as the broken starting state; the repair uses the real nginx:1.27-alpine tag.

CreateContainerConfigError happens before the process runs. A required missing Secret key is enough to prevent container creation even if the Secret object itself exists. Do not make the reference optional just to turn the status green when the application requires that value. Restore the contract the manifest describes.

## Explain restarts before changing limits

CrashLoopBackOff is a retry delay after repeated container failure, not a root cause. Read current and previous logs, plus exit code and termination reason:

```bash
kubectl -n debugging-lab logs deployment/dojo-crash -c main --tail=30
kubectl -n debugging-lab get pods -l app=dojo-crash
# Substitute the actual Pod name printed above:
kubectl -n debugging-lab logs POD_NAME -c main --previous --tail=30
kubectl -n debugging-lab get pod POD_NAME -o yaml
```

A shell syntax error, missing executable, bad flag, application exception, or liveness failure can all cause restarts. A short-lived successful command can also restart under a Deployment's Always policy; use a Job when the intended workload is completion rather than continuous service.

OOMKilled specifically points to memory termination. Exit code 137 alone is not enough: it means SIGKILL and can have other causes. Inspect `lastState.terminated.reason`, limits, events, and application behavior. Raising a limit blindly can hide a leak or make node pressure worse. `kubectl top` may help with current samples but can miss the peak immediately before a crash.

## Exec needs a running container and real tools

Use `kubectl exec POD -c CONTAINER -- COMMAND` to inspect a live container. The double dash separates kubectl options from the command. Non-interactive commands are easy to reproduce:

```bash
kubectl -n debugging-lab exec POD_NAME -c main -- printenv MODE
kubectl -n debugging-lab exec POD_NAME -c web -- cat /etc/nginx/nginx.conf
```

A missing shell or curl binary is not proof the application is unhealthy. Minimal images intentionally omit diagnostic tools. Avoid installing packages into the running application as an ad hoc fix; changes disappear on replacement and obscure the reproducible image definition.

## Debug without replacing the application

`kubectl debug` can add an ephemeral container with diagnostic tools to an existing Pod. Its containers share the Pod network, so loopback can reach the application's listener. `--target=web` requests the target container's process namespace when the runtime supports that feature. This is distinct from network sharing, which already follows from being in the same Pod.

```bash
kubectl -n debugging-lab debug POD_NAME --container=dojo-inspector \
  --image=busybox:1.36 --target=web --profile=general --attach=false \
  -- sh -c 'wget -qO- http://127.0.0.1:80'
```

Admission policy and RBAC can restrict ephemeral containers. They are recorded on the Pod and are not ordinary restartable application containers. Avoid adding one when normal logs already answer the question. The lab's completed diagnostic container leaves inspectable logs without opening an interactive terminal.

When the original process exits too fast, create a diagnostic copy instead. For a source Pod with a shell-capable main container, `kubectl debug POD_NAME --copy-to=dojo-crash-copy --container=main -- sh -c 'sleep 3600'` changes the copied command. Inspect its inherited volumes, probes, and labels so the copy neither reproduces an unrelated failure nor accidentally joins serving traffic. Delete only that named diagnostic copy afterwards.

## Follow the request path

A Service with no ready endpoints usually has a selector mismatch, unready Pods, or both. Compare selector labels with Pod labels, then inspect EndpointSlice readiness and target ports. A ready Pod selected on the wrong targetPort still cannot serve a request.

`kubectl -n debugging-lab port-forward service/dojo-web 18080:80` creates an interactive local tunnel for testing. Keep it running in one terminal and request `http://127.0.0.1:18080` from another, then stop it. A successful port-forward tests a selected Pod and listener; it does not prove the whole ClusterIP, ingress, or external network path works.

## Exam tips

Read the exact namespace and preserve required behavior. Identify the first failing stage, make one justified repair, wait for convergence, and recheck the application outcome. Record useful evidence before replacement. Finishing with a healthy owner, correct configuration, and ready Service backend is stronger than merely clearing one alarming display string.
