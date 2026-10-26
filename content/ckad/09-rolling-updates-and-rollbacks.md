+++
id = "ckad-rolling-updates-and-rollbacks"
title = "Rolling updates and rollbacks"
module = "deployment"
minutes = 45
domain = "deployment"
needs = "cluster"
objectives = ["Control Deployment availability with RollingUpdate and Recreate", "Batch template changes with pause and resume", "Diagnose a stalled rollout and restore a known revision", "Record meaningful rollout history"]

[[quiz]]
q = "With four replicas, maxSurge 25% and maxUnavailable 25%, what are the normal rollout bounds?"
choices = ["Exactly four Pods at all times", "Up to five non-terminating Pods and at least three available", "Up to eight Pods and at least one available", "Five available Pods are required before any deletion"]
answer = 1
why = "Surge percentages round up and unavailable percentages round down. Each is one here. Terminating Pods can temporarily increase total resource consumption beyond the surge bound."

[[quiz]]
q = "Which change creates a new Deployment revision?"
choices = ["Changing spec.replicas", "Adding an annotation only to the Deployment metadata", "Changing a container image in spec.template", "Changing spec.strategy.rollingUpdate.maxSurge"]
answer = 2
why = "Deployment history records Pod template changes. Scaling changes desired counts without changing the template; an annotation outside the template does not itself start a rollout."

[[quiz]]
q = "What does ProgressDeadlineExceeded do automatically?"
choices = ["Rolls back to the previous image", "Reports stalled progress without automatically rolling back", "Marks the rollout Failed and stops creating Pods", "Scales the new ReplicaSet back to zero"]
answer = 1
why = "The controller records a failed progress condition. You must diagnose the reason and change the template, fix the dependency, or explicitly undo the rollout."

[[quiz]]
q = "What must happen before rolling back a paused Deployment?"
choices = ["Scale the new ReplicaSet to zero", "Scale to zero", "Resume the Deployment", "Run kubectl rollout restart"]
answer = 2
why = "A paused Deployment does not process a rollback. Resume it, then use rollout undo and verify convergence. Pausing batches template edits; it is not a rollback mechanism."

[[quiz]]
q = "How does minReadySeconds differ from readiness?"
choices = ["It delays container startup", "It requires continuous readiness before a Pod counts as available", "It is the delay before the first readiness probe", "It replaces progressDeadlineSeconds"]
answer = 1
why = "A Ready Pod can already receive Service traffic, while the Deployment waits for the continuous readiness interval before counting it as available for rollout decisions."

[lab]
summary = "Roll out a web server, batch an image change, deliberately break readiness, and recover using retained revision history."
setup = [
'''
kubectl create namespace rolling-lab --dry-run=client -o yaml | kubectl apply -f -
'''
]
cleanup = [
'''
kubectl delete namespace rolling-lab --ignore-not-found --wait=false || true
'''
]

[[lab.steps]]
title = "Create a conservative rolling Deployment"
task = '''
In rolling-lab create Deployment dojo-web with three replicas, container web using nginx:1.26-alpine, an HTTP readiness probe on / port 80, RollingUpdate maxSurge 1/maxUnavailable 0, minReadySeconds 3, and progressDeadlineSeconds 180. Record change-cause baseline. Wait until all three replicas are available.
'''
hints = ["Write an apps/v1 Deployment; strategy belongs beside replicas, not inside the Pod template.", "progressDeadlineSeconds must exceed minReadySeconds."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
  namespace: rolling-lab
  annotations:
    kubernetes.io/change-cause: baseline
spec:
  replicas: 3
  minReadySeconds: 3
  progressDeadlineSeconds: 180
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels: {app: dojo-web}
  template:
    metadata:
      labels: {app: dojo-web}
    spec:
      containers:
      - name: web
        image: nginx:1.26-alpine
        readinessProbe:
          httpGet: {path: /, port: 80}
          periodSeconds: 2
EOF
kubectl -n rolling-lab rollout status deployment/dojo-web --timeout=120s
```
'''

[[lab.steps.checks]]
name = "three baseline replicas are available with the requested strategy"
run = '''
kubectl -n rolling-lab get deployment/dojo-web -o name >/dev/null 2>&1 || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image} {.spec.strategy.type} {.spec.strategy.rollingUpdate.maxSurge} {.spec.strategy.rollingUpdate.maxUnavailable} {.spec.minReadySeconds} {.spec.progressDeadlineSeconds} {.metadata.annotations.kubernetes\.io/change-cause}')" = "nginx:1.26-alpine RollingUpdate 1 0 3 180 baseline" ] && [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 3 3 3 3" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Pause and stage an image change"
task = '''
Pause dojo-web. Stage image nginx:1.27-alpine for container web and change-cause upgrade-to-1.27. Leave the Deployment paused so you can inspect the new desired template before rollout.
'''
hints = ["kubectl rollout pause deployment/dojo-web freezes reconciliation of new template changes.", "Use kubectl set image and kubectl annotate --overwrite."]
solution = '''
```bash
kubectl -n rolling-lab rollout pause deployment/dojo-web
kubectl -n rolling-lab annotate deployment/dojo-web kubernetes.io/change-cause=upgrade-to-1.27 --overwrite
kubectl -n rolling-lab set image deployment/dojo-web web=nginx:1.27-alpine
kubectl -n rolling-lab rollout history deployment/dojo-web
```
'''

[[lab.steps.checks]]
name = "new template is staged while paused"
run = '''
[ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.paused} {.spec.template.spec.containers[0].image} {.metadata.annotations.kubernetes\.io/change-cause}')" = "true nginx:1.27-alpine upgrade-to-1.27" ] && echo ok
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Resume and complete the rollout"
task = '''
Resume dojo-web. Wait for three available, updated nginx:1.27-alpine replicas. Inspect rollout history and revision 1 before continuing.
'''
hints = ["Use rollout resume, then rollout status with a timeout.", "History is Deployment history, not a list of container restarts."]
solution = '''
```bash
kubectl -n rolling-lab rollout resume deployment/dojo-web
kubectl -n rolling-lab rollout status deployment/dojo-web --timeout=120s
kubectl -n rolling-lab rollout history deployment/dojo-web
kubectl -n rolling-lab rollout history deployment/dojo-web --revision=1
```
'''

[[lab.steps.checks]]
name = "upgrade has converged and is no longer paused"
run = '''
[ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.paused}')" != true ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.paused}')" != true ] && [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.27-alpine" ] && [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 3 3 3 3" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Observe a stalled rollout"
task = '''
Change the web readiness HTTP path to /missing and shorten progressDeadlineSeconds to 40 for this controlled failure. Record change-cause broken-readiness. Wait for the Progressing condition reason ProgressDeadlineExceeded. Keep maxUnavailable 0 so old healthy replicas remain available.
'''
hints = ["Patch the template, not an individual managed Pod.", "A timed rollout status failure is expected here; inspect conditions rather than assuming automatic rollback."]
solution = '''
```bash
kubectl -n rolling-lab annotate deployment/dojo-web kubernetes.io/change-cause=broken-readiness --overwrite
kubectl -n rolling-lab patch deployment dojo-web --type=strategic -p '{"spec":{"progressDeadlineSeconds":40,"template":{"spec":{"containers":[{"name":"web","readinessProbe":{"httpGet":{"path":"/missing","port":80}}}]}}}}'
for i in $(seq 1 65); do
  reason=$(kubectl -n rolling-lab get deployment dojo-web -o jsonpath='{.status.conditions[?(@.type=="Progressing")].reason}')
  [ "$reason" = ProgressDeadlineExceeded ] && break
  sleep 2
done
[ "$reason" = ProgressDeadlineExceeded ]
kubectl -n rolling-lab describe deployment dojo-web
```
'''

[[lab.steps.checks]]
name = "broken readiness has exhausted the progress deadline"
run = '''
[ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].readinessProbe.httpGet.path}')" = "/missing" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].readinessProbe.httpGet.path} {.status.conditions[?(@.type=="Progressing")].reason}')" = "/missing ProgressDeadlineExceeded" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 150

[[lab.steps]]
title = "Restore the baseline revision"
task = '''
Undo dojo-web to retained revision 1. Confirm the baseline image nginx:1.26-alpine, readiness path /, and three available updated replicas. Inspect history again: rollback produces a new current revision.
'''
hints = ["rollout undo --to-revision=1 selects the baseline Pod template.", "Strategy and replica settings are outside revisioned Pod templates; they remain in force."]
solution = '''
```bash
kubectl -n rolling-lab rollout undo deployment/dojo-web --to-revision=1
kubectl -n rolling-lab rollout status deployment/dojo-web --timeout=120s
kubectl -n rolling-lab rollout history deployment/dojo-web
```
'''

[[lab.steps.checks]]
name = "baseline image and healthy readiness are restored"
run = '''
[ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.26-alpine" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image} {.spec.template.spec.containers[0].readinessProbe.httpGet.path}')" = "nginx:1.26-alpine /" ] && [ "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n rolling-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 3 3 3 3" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120
+++

## A rollout is controlled replacement

A Deployment describes the Pod template you want and a replica count. It owns ReplicaSets; each ReplicaSet owns Pods matching one template. Updating an image does not edit running containers in place. The Deployment creates or reuses a ReplicaSet for the new template, then moves capacity from the old ReplicaSet to the new one. The controller continually compares desired state with observed state, so a successful API write means the request was accepted, not that replacement has finished.

```text
Deployment dojo-web
  old ReplicaSet: v1  3 -> 2 -> 1 -> 0
  new ReplicaSet: v2  0 -> 1 -> 2 -> 3
                    readiness gates progress
```

A change under `spec.template` triggers a rollout: images, environment variables, Pod labels, probes, and template annotations all qualify. Scaling `spec.replicas` does not create a new revision. Changing a Deployment's own metadata annotation does not start new Pods either. This distinction matters when recording history and when deciding whether a configuration update will restart an application.

## Availability is a budget

`RollingUpdate` is the default strategy. Its two limits describe how much spare capacity you can use and how much availability you can lose during replacement. `maxSurge` is extra Pods above the desired count; `maxUnavailable` is the allowed shortage of available Pods below it. An available Pod has been Ready continuously for `minReadySeconds`.

| Setting | Three replicas, value 1 | Operational cost |
|---|---|---|
| maxSurge | Normally up to four non-terminating Pods | Needs spare scheduling capacity |
| maxUnavailable | As few as two available Pods | Some serving capacity may disappear |
| minReadySeconds | Ready continuously for that interval | Slower but more cautious replacement |
| progressDeadlineSeconds | Deadline for lack of rollout progress | Reports failure; does not repair it |

Both budgets may be integers or percentages. Unavailable percentages round **down**; surge percentages round **up**. With three replicas and the default 25% values, unavailable is zero and surge is one. They cannot both be zero. Terminating Pods may continue consuming resources during their grace period, so do not interpret desired replicas plus surge as an absolute limit on all existing processes or memory use.

```yaml
spec:
  replicas: 3
  minReadySeconds: 5
  progressDeadlineSeconds: 120
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
```

This policy keeps the old serving capacity until new Pods become available. It can stall on a full cluster: the extra Pod cannot schedule, and the controller cannot remove a healthy old Pod without violating the budget. Look at Pending Pod events before blaming the Deployment controller. Requests, quotas, storage, and node affinity can all prevent that extra replica from starting.

## Recreate makes a different promise

`Recreate` terminates old replicas before creating replacements for a template update. It is useful when old and new application versions must not serve simultaneously, but it introduces downtime. It is not a distributed lock: manually deleting a Pod can cause its ReplicaSet to replace it, and external processes are outside this guarantee.

```yaml
spec:
  strategy:
    type: Recreate
```

When switching an existing Deployment to Recreate, remove `strategy.rollingUpdate`; that field is invalid with the Recreate strategy. A merge patch can do both together:

```bash
kubectl -n rolling-lab patch deployment dojo-web --type=merge \
  -p '{"spec":{"strategy":{"type":"Recreate","rollingUpdate":null}}}'
```

Do not run that demonstration during the graded rolling lab. Choose the strategy required by the task, and reason about application compatibility separately from Kubernetes health. Two healthy versions can still disagree about a database schema.

## Change, wait, inspect

Use the container name, not the Deployment name, on the right side of `set image`:

```bash
kubectl -n rolling-lab set image deployment/dojo-web web=nginx:1.27-alpine
kubectl -n rolling-lab rollout status deployment/dojo-web --timeout=120s
kubectl -n rolling-lab get deployment,replicaset,pod -l app=dojo-web
kubectl -n rolling-lab rollout history deployment/dojo-web
```

Read the `READY`, `UP-TO-DATE`, and `AVAILABLE` counts together. A Deployment can remain Available because old Pods still serve while its newest rollout is failing. Conversely, a new Pod may be Ready and already eligible for Service traffic before `minReadySeconds` allows the Deployment to count it as available. That delay observes stability; it is not a traffic quarantine.

Record intent with `kubernetes.io/change-cause`. Annotate before the template update so the resulting ReplicaSet history gets a meaningful description. Do not rely on the old `--record` flag.

```bash
kubectl -n rolling-lab annotate deployment/dojo-web \
  kubernetes.io/change-cause='upgrade web to tested nginx build' --overwrite
kubectl -n rolling-lab set image deployment/dojo-web web=nginx:1.27-alpine
kubectl -n rolling-lab rollout history deployment/dojo-web --revision=1
```

The annotation describes why a change happened; it is not proof that the rollout succeeded. Inspect conditions and Pods for that evidence. Keep human descriptions free of credentials or private data.

## Pause batches template edits

`kubectl rollout pause deployment/dojo-web` lets you make several template changes without starting separate rollouts. Change the image, environment, and resource settings, inspect the desired template, then `kubectl rollout resume deployment/dojo-web`. The controller reconciles the combined template when resumed. Existing containers keep running; pause does not suspend processes or stop traffic.

A paused Deployment does not run rollback reconciliation. Resume before undoing. Also remember that you are inspecting two different things while paused: the Deployment may show a newly requested image while its current Pods still run the old one. Check the Pods when asked what is actually serving.

## Recover from evidence

A bad image, failing readiness probe, missing Secret, or unschedulable Pod can prevent progress. `progressDeadlineSeconds` must exceed `minReadySeconds`; it measures failure to progress rather than imposing one fixed total rollout duration. On expiry, the Progressing condition reports `ProgressDeadlineExceeded`. Kubernetes does **not** automatically undo the change.

Inspect `describe deployment`, the newest ReplicaSet, Pod events, and container logs. Fix the dependency when the new template is correct. Otherwise restore a known template:

```bash
kubectl -n rolling-lab rollout undo deployment/dojo-web --to-revision=1
kubectl -n rolling-lab rollout status deployment/dojo-web --timeout=120s
```

Without `--to-revision`, undo selects the previous retained revision. History retention is controlled by `revisionHistoryLimit`, normally ten old ReplicaSets. A deleted revision cannot be resurrected by its number. Undo changes the Pod template and records a new current revision; it does not restore replica counts, Services, external ConfigMap contents, or database data. A fast application rollback therefore still needs compatible configuration and storage.

## Exam tips

Read the requested namespace, container name, strategy, and exact image before editing. Use `kubectl explain deployment.spec.strategy` for field placement. Generate a starting Deployment with `kubectl create deployment ... --dry-run=client -o yaml`, then add probes and budgets. Finish with rollout status and inspection of the resulting image and availability. A submitted manifest and a completed rollout are different outcomes.
