+++
id = "ckad-blue-green-and-canary"
title = "Blue/green and canary delivery"
module = "deployment"
minutes = 40
domain = "deployment"
needs = "cluster"
objectives = ["Design non-overlapping Deployment selectors and shared Service labels", "Switch blue/green traffic through a Service selector", "Build a replica-ratio canary and inspect ready endpoints", "Roll back routing without changing application images"]

[[quiz]]
q = "Which object normally changes at a basic blue/green cutover?"
choices = ["The Service selector", "The IngressClass", "The green Deployment's replica count only", "The blue Deployment selector"]
answer = 0
why = "The Service keeps its address while its selector changes the eligible backend set. Deployment selectors should remain stable and disjoint."

[[quiz]]
q = "Four ready stable Pods and one ready canary Pod share a Service. What is guaranteed?"
choices = ["Exactly every fifth HTTP request reaches canary", "Canary gets all new TCP connections", "The eligible endpoint count is 4:1, not an exact request percentage", "Canary gets 20% of requests once sessionAffinity is set"]
answer = 2
why = "A plain Service balances connections, subject to implementation and traffic policy. Connection reuse, session affinity, locality, and readiness can skew observed requests."

[[quiz]]
q = "Why should two Deployments not both select only app=dojo-web?"
choices = ["Services cannot select that label", "The controllers could have overlapping ownership intentions", "The Service would route only to the newer Deployment", "The API rejects the second Deployment's duplicate selector"]
answer = 1
why = "Use an additional stable identity such as track to separate Deployment selectors. The Service may intentionally select the wider app label to include both groups."

[[quiz]]
q = "Which API objects directly record a Service's backend addresses and readiness conditions?"
choices = ["The Service's spec.clusterIP", "kubectl get pods -l app=dojo-web", "EndpointSlices selected by kubernetes.io/service-name", "The Deployment's status.availableReplicas"]
answer = 2
why = "EndpointSlices record endpoint addresses, target Pods, and readiness conditions. Compare these with Pod track/version labels; selector intent alone does not prove backends are ready."

[lab]
summary = "Keep blue and green running, cut traffic to green, open a 4:1 canary pool, then restore stable routing."
setup = [
'''
kubectl create namespace delivery-lab --dry-run=client -o yaml | kubectl apply -f -
'''
]
cleanup = [
'''
kubectl delete namespace delivery-lab --ignore-not-found --wait=false || true
'''
]

[[lab.steps]]
title = "Build two disjoint versions"
task = '''
Create dojo-blue (four nginx:1.26-alpine replicas, app=dojo-web, track=stable, version=v1) and dojo-green (one nginx:1.27-alpine replica, app=dojo-web, track=canary, version=v2) in delivery-lab. Each Deployment must select app plus its own track. Add HTTP readiness on /:80. Create Service dojo-web port 80 targeting 80, selecting app=dojo-web,track=stable. Wait for ready workloads.
'''
hints = ["Use separate track identities in each Deployment selector.", "A Service selector matches Pod labels, not Deployment metadata."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-blue
  namespace: delivery-lab
spec:
  replicas: 4
  selector:
    matchLabels: {app: dojo-web, track: stable}
  template:
    metadata:
      labels: {app: dojo-web, track: stable, version: v1}
    spec:
      containers:
      - name: web
        image: nginx:1.26-alpine
        readinessProbe:
          httpGet: {path: /, port: 80}
          periodSeconds: 2
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-green
  namespace: delivery-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: dojo-web, track: canary}
  template:
    metadata:
      labels: {app: dojo-web, track: canary, version: v2}
    spec:
      containers:
      - name: web
        image: nginx:1.27-alpine
        readinessProbe:
          httpGet: {path: /, port: 80}
          periodSeconds: 2
---
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
  namespace: delivery-lab
spec:
  selector: {app: dojo-web, track: stable}
  ports:
  - port: 80
    targetPort: 80
EOF
kubectl -n delivery-lab rollout status deployment/dojo-blue --timeout=120s
kubectl -n delivery-lab rollout status deployment/dojo-green --timeout=120s
```
'''

[[lab.steps.checks]]
name = "blue is the only ready backend pool"
run = '''
kubectl -n delivery-lab get service/dojo-web -o name >/dev/null 2>&1 || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.app} {.spec.selector.track}')" = "dojo-web stable" ] && [ "$(kubectl -n delivery-lab get deployment/dojo-blue -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n delivery-lab get deployment/dojo-blue -o jsonpath='{.metadata.generation}') 4 4 4 4" ] && [ "$(kubectl -n delivery-lab get deployment/dojo-green -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n delivery-lab get deployment/dojo-green -o jsonpath='{.metadata.generation}') 1 1 1 1" ] && names=$(kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{"\n"}{end}'); [ "$(printf "%s\n" "$names" | grep -c "^dojo-blue-" || true)" = 4 ] && [ "$(printf "%s\n" "$names" | grep -c "^dojo-green-" || true)" = 0 ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Cut over to green"
task = '''
Switch Service dojo-web to app=dojo-web,track=canary. Keep both Deployments running. Verify only the one green Pod appears as a ready backend.
'''
hints = ["Patch the Service selector; do not change Deployment selectors.", "EndpointSlice updates are asynchronous."]
solution = '''
```bash
kubectl -n delivery-lab patch service dojo-web --type=merge -p '{"spec":{"selector":{"app":"dojo-web","track":"canary"}}}'
kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o wide
```
'''

[[lab.steps.checks]]
name = "green alone receives new Service connections"
run = '''
[ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.track}')" = "canary" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.app} {.spec.selector.track}')" = "dojo-web canary" ] && names=$(kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{"\n"}{end}'); [ "$(printf "%s\n" "$names" | grep -c "^dojo-blue-" || true)" = 0 ] && [ "$(printf "%s\n" "$names" | grep -c "^dojo-green-" || true)" = 1 ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Open a replica-ratio canary"
task = '''
Change the Service selector to only app=dojo-web, removing track entirely. Keep stable replicas at four and canary at one. Confirm five ready endpoint targets, four blue and one green. This is an approximate connection split, not a guarantee of 20% of HTTP requests.
'''
hints = ["A merge patch that leaves track in place will not open the pool.", "Replace the whole selector map or explicitly set track to null."]
solution = '''
```bash
kubectl -n delivery-lab patch service dojo-web --type=json -p '[{"op":"replace","path":"/spec/selector","value":{"app":"dojo-web"}}]'
kubectl -n delivery-lab get pods -l app=dojo-web -L track,version
kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o yaml
```
'''

[[lab.steps.checks]]
name = "ready backends form a four-to-one canary pool"
run = '''
[ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.track}')" = "" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.app} {.spec.selector.track}')" = "dojo-web " ] && [ "$(kubectl -n delivery-lab get deployment/dojo-blue -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n delivery-lab get deployment/dojo-blue -o jsonpath='{.metadata.generation}') 4 4 4 4" ] && [ "$(kubectl -n delivery-lab get deployment/dojo-green -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n delivery-lab get deployment/dojo-green -o jsonpath='{.metadata.generation}') 1 1 1 1" ] && names=$(kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{"\n"}{end}'); [ "$(printf "%s\n" "$names" | grep -c "^dojo-blue-" || true)" = 4 ] && [ "$(printf "%s\n" "$names" | grep -c "^dojo-green-" || true)" = 1 ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Roll routing back to stable"
task = '''
Restore app=dojo-web,track=stable on Service dojo-web. Keep the green Deployment available for investigation; ensure only the four blue Pods are ready Service backends.
'''
hints = ["Rollback changes routing; retaining the unused version makes investigation easier.", "Do not scale blue to zero or edit Pod labels to fake selection."]
solution = '''
```bash
kubectl -n delivery-lab patch service dojo-web --type=json -p '[{"op":"replace","path":"/spec/selector","value":{"app":"dojo-web","track":"stable"}}]'
kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o wide
```
'''

[[lab.steps.checks]]
name = "stable routing excludes every green endpoint"
run = '''
[ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.track}')" = "stable" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n delivery-lab get service/dojo-web -o jsonpath='{.spec.selector.app} {.spec.selector.track}')" = "dojo-web stable" ] && names=$(kubectl -n delivery-lab get endpointslices -l kubernetes.io/service-name=dojo-web -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.targetRef.name}{"\n"}{end}'); [ "$(printf "%s\n" "$names" | grep -c "^dojo-blue-" || true)" = 4 ] && [ "$(printf "%s\n" "$names" | grep -c "^dojo-green-" || true)" = 0 ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120
+++

## Separate delivery from routing

A rolling update couples replacement and exposure: newly Ready Pods normally join the Service while the old ReplicaSet shrinks. Blue/green and canary patterns separate those decisions. Two Deployments keep two application versions alive, while a Service decides which ready Pods can receive new connections. You can change traffic without replacing an image and can keep an old version available for fast recovery.

A Service selects **Pods**, not Deployments. Both Deployments may carry a shared application label, but their own selectors must identify different Pod sets. Kubernetes does not include a built-in Deployment strategy named BlueGreen or Canary. You compose ordinary objects, or use a separate rollout controller when more advanced traffic policy is needed.

```text
                        Service dojo-web
                       selector app=dojo-web
                           /           \
           track=stable, version=v1   track=canary, version=v2
                  dojo-blue                dojo-green
                  four Pods                 one Pod
```

The same pair can demonstrate both patterns. For blue/green, the Service includes one track in its selector. For a simple canary, it selects the shared application label and includes both groups.

## Labels are the routing contract

Use each label for a clear purpose. `app` identifies the logical application. `track` identifies a long-lived delivery lane such as stable or canary. `version` describes the deployed application build. These names are a convention, not reserved Kubernetes fields; the selectors make them meaningful.

| Label | Example | Usually changes when |
|---|---|---|
| app | dojo-web | The application identity changes |
| track | stable or canary | A Pod belongs to a different delivery lane |
| version | v1 or v2 | A new application build is deployed |

A Deployment's `spec.selector` is immutable after creation, so avoid putting a frequently changing version in it unless you intend one Deployment per version. A stable track selector lets you update the image and version template label later. The template's labels must satisfy its Deployment selector. Do not have both Deployments select only `app=dojo-web`: overlapping selectors make controller ownership and operations confusing.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-green
  namespace: delivery-lab
spec:
  replicas: 1
  selector:
    matchLabels:
      app: dojo-web
      track: canary
  template:
    metadata:
      labels:
        app: dojo-web
        track: canary
        version: v2
    spec:
      containers:
      - name: web
        image: nginx:1.27-alpine
        readinessProbe:
          httpGet: {path: /, port: 80}
```

Changing only the Deployment's top-level labels does not change Pod labels. Inspect `spec.template.metadata.labels` when Service selection looks wrong. A surprising number of empty-endpoint problems are simply a mismatch between these two locations.

## Blue/green: one active pool

Blue serves production while green starts independently. Validate green readiness, configuration, and behavior before changing the production selector. A temporary preview Service can select green without exposing it through the stable address. The lab avoids an external ingress controller and checks the ready backend set directly.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
  namespace: delivery-lab
spec:
  selector:
    app: dojo-web
    track: stable
  ports:
  - port: 80
    targetPort: 80
```

Switching `track` to canary cuts over new Service connections to the green lane:

```bash
kubectl -n delivery-lab patch service dojo-web --type=merge \
  -p '{"spec":{"selector":{"app":"dojo-web","track":"canary"}}}'
```

The ClusterIP and DNS name remain the same. EndpointSlice controllers and node networking still need time to observe the selector update. Existing established connections may continue reaching blue until they close; this is not a single globally atomic change of every active request. Applications with long-lived connections need an explicit draining plan.

Keeping blue ready costs extra capacity but makes rollback simple: restore the old selector. Scaling blue down immediately saves resources while sacrificing that recovery speed. Neither a selector change nor an image rollback reverses a database migration or messages already emitted by the new version.

## Canary: expose a small eligible population

A basic canary uses one Service selecting both tracks. Four stable replicas plus one canary replica gives an eligible backend population of 4:1. With equally usable endpoints and many independent connections, expect approximately one fifth of connections to reach the canary. This is a statistical expectation, not a request-level service guarantee.

```bash
kubectl -n delivery-lab scale deployment/dojo-blue --replicas=4
kubectl -n delivery-lab scale deployment/dojo-green --replicas=1
kubectl -n delivery-lab patch service dojo-web --type=json \
  -p '[{"op":"replace","path":"/spec/selector","value":{"app":"dojo-web"}}]'
```

Notice the full selector replacement. A merge patch containing just `app` retains any existing `track` key unless you remove it explicitly. That would silently keep routing to a single lane. JSON Patch `replace` is a useful time saver when the whole map should change.

Session affinity, persistent HTTP connections, client behavior, topology preferences, and unequal readiness can all skew the observed split. Four stable replicas configured does not imply four ready stable endpoints. If one stable Pod becomes unready, the eligible population becomes 3:1. If the canary becomes unready, ordinary new-connection routing excludes it after endpoint updates propagate. This assumes the default `publishNotReadyAddresses: false`; existing connections may still continue. For exact weighted routing, use an appropriate gateway, ingress, service mesh, or progressive-delivery controller; do not invent a weight field on a core Service.

## Verify the backend set

Inspect intent and observation separately:

```bash
kubectl -n delivery-lab get service dojo-web -o yaml
kubectl -n delivery-lab get pods -l app=dojo-web -L track,version
kubectl -n delivery-lab get endpointslices \
  -l kubernetes.io/service-name=dojo-web -o yaml
```

EndpointSlices use `discovery.k8s.io/v1` and are the current place to examine Service backends. The `kubernetes.io/service-name` label identifies their Service within the namespace; several slices may belong to one Service, so inspect all matching slices rather than only the first. Read each endpoint's `conditions.ready` and `targetRef.name`; correlate target Pods with their track labels. A Pod existing in `Running` phase does not prove it is Ready. Also inspect Service `port` versus `targetPort`: selecting the right Pod with the wrong destination port still produces failed connections.

Endpoint membership proves the configured eligible split, not successful application responses or a measured traffic percentage. A production canary also needs error rate, latency, and business correctness signals over a useful observation window. A handful of curl responses is a smoke test, not a statistically strong acceptance criterion.

## Roll back the smallest decision

When a canary is unhealthy, first exclude its track from the Service, then investigate its Pods. Keeping the failing version isolated preserves logs and makes comparison possible. If the application version is healthy but its configuration is wrong, repair the green Deployment before the next exposure. Avoid changing both routing and images simultaneously unless the task requires both; independent changes are easier to verify.

## Exam tips

Write down the namespace, Service name, both selectors, replica ratio, and target port. Use `get pods --show-labels` before patching. Preserve each Deployment's immutable selector. After cutover or rollback, wait for EndpointSlices to converge and count **ready** targets. Explain approximate canary distribution accurately: replica ratio controls eligible capacity, not an exact sequence of HTTP requests.
