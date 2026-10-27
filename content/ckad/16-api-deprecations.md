+++
id = "ckad-api-deprecations"
title = "Discover and migrate deprecated APIs"
module = "observability"
minutes = 40
domain = "observability"
needs = "cluster"
objectives = ["Discover served API groups, versions, resources, and field schemas", "Distinguish deprecation warnings from removed API errors", "Migrate Ingress, PodDisruptionBudget, and HorizontalPodAutoscaler manifests", "Validate schema and behavior separately after migration"]

[[quiz]]
q = "What does deprecated mean for an API version that is still served?"
choices = ["It cannot be used at all", "It remains usable but warns of planned removal or replacement", "It is no longer served unless a feature gate is set", "It is read-only: objects can be listed but not created"]
answer = 1
why = "Deprecation gives migration notice. A removed served endpoint instead rejects requests using that version; existing stored objects can often still be accessed through a supported version."

[[quiz]]
q = "Which command discovers namespaced resource kinds and their API versions?"
choices = ["kubectl api-versions", "kubectl api-resources --namespaced=true", "kubectl explain pod --api-version=v1", "kubectl get all -A"]
answer = 1
why = "api-resources reports discovered kinds, short names, scope, and API versions. api-versions lists served group/version combinations; explain shows fields in a chosen schema."

[[quiz]]
q = "What is dangerous about changing a policy/v1beta1 PDB with selector {} to policy/v1?"
choices = ["The new version has no selectors", "The empty selector changes from selecting no Pods to selecting all Pods in the namespace", "policy/v1 rejects an empty selector", "maxUnavailable becomes the default instead"]
answer = 1
why = "Migration must preserve intended behavior, not just pass schema validation. Use an explicit matchLabels selector when the intended target is one application."

[[quiz]]
q = "Which Ingress backend is valid in networking.k8s.io/v1?"
choices = ["serviceName plus servicePort directly under backend", "service.name plus service.port.number or service.port.name", "service.name plus service.targetPort.number", "service.name plus a top-level servicePort"]
answer = 1
why = "The stable Ingress schema nests the Service reference under backend.service. HTTP paths also require pathType, so changing only apiVersion is insufficient for the old manifest."

[[quiz]]
q = "Which statement about kubectl-convert is accurate?"
choices = ["It verifies controllers and external dependencies", "It preserves application behavior without manual review", "It transforms supported schemas, but output still needs review and server validation", "It is included in every kubectl installation"]
answer = 2
why = "kubectl-convert is an optional plugin, not a guaranteed built-in command. Its supported conversions are limited by its compiled schemas; inspect semantic changes and validate the result."

[lab]
summary = "Migrate three intentionally removed API versions into supported manifests without installing an ingress controller or requiring live autoscaling metrics."
setup = [
'''
kubectl create namespace api-lab --dry-run=client -o yaml | kubectl apply -f -
''',
'''
kubectl apply -f baseline.yaml
'''
]
cleanup = [
'''
kubectl delete namespace api-lab --ignore-not-found --wait=false || true
'''
]

[lab.files]
"baseline.yaml" = '''
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  replicas: 2
  selector:
    matchLabels: {app: dojo-web}
  template:
    metadata:
      labels: {app: dojo-web}
    spec:
      containers:
      - name: web
        image: nginx:1.27-alpine
        resources:
          requests: {cpu: 10m, memory: 16Mi}
        readinessProbe:
          httpGet: {path: /, port: 80}
---
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  selector: {app: dojo-web}
  ports:
  - port: 80
    targetPort: 80
'''
"ingress.yaml" = '''
apiVersion: extensions/v1beta1
kind: Ingress
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  rules:
  - host: dojo.example.test
    http:
      paths:
      - path: /
        backend:
          serviceName: dojo-web
          servicePort: 80
'''
"pdb.yaml" = '''
apiVersion: policy/v1beta1
kind: PodDisruptionBudget
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  minAvailable: 1
  selector: {}
'''
"hpa.yaml" = '''
apiVersion: autoscaling/v2beta2
kind: HorizontalPodAutoscaler
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: dojo-web
  minReplicas: 2
  maxReplicas: 4
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 60
'''

[[lab.steps]]
title = "Migrate the Ingress schema"
task = '''
Inspect ingress.yaml, api-resources, api-versions, and explain. Rewrite the removed extensions/v1beta1 Ingress dojo-web as networking.k8s.io/v1 in api-lab. Preserve host dojo.example.test, path /, and Service dojo-web port 80; add pathType Prefix. Server-dry-run the corrected file, then apply it. No Ingress controller is installed; grade the routing specification.
'''
hints = ["Use kubectl explain ingress.spec.rules.http.paths --api-version=networking.k8s.io/v1.", "backend.service.name and backend.service.port.number replace serviceName/servicePort."]
solution = '''
```bash
kubectl api-resources --api-group=networking.k8s.io
kubectl api-versions
kubectl explain ingress.spec.rules.http.paths --api-version=networking.k8s.io/v1
cat > ingress.yaml <<'EOF'
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  rules:
  - host: dojo.example.test
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: dojo-web
            port:
              number: 80
EOF
kubectl apply --dry-run=server -f ingress.yaml
kubectl apply -f ingress.yaml
```
'''

[[lab.steps.checks]]
name = "stable Ingress preserves the intended route"
run = '''
[ "$(kubectl -n api-lab get ingress/dojo-web -o jsonpath='{.apiVersion} {.spec.rules[0].host} {.spec.rules[0].http.paths[0].path} {.spec.rules[0].http.paths[0].pathType} {.spec.rules[0].http.paths[0].backend.service.name} {.spec.rules[0].http.paths[0].backend.service.port.number}')" = "networking.k8s.io/v1 dojo.example.test / Prefix dojo-web 80" ] && echo ok
'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Migrate a PDB without broadening its target"
task = '''
Rewrite pdb.yaml as policy/v1. The task intent is minAvailable 1 for only app=dojo-web in api-lab, so replace the legacy empty selector with explicit matchLabels. Validate on the server and apply. Do not drain or evict any node.
'''
hints = ["An empty selector has different behavior in stable PDB.", "This task supplies the intended selection; do not preserve the legacy accidental select-nothing behavior."]
solution = '''
```bash
kubectl explain poddisruptionbudget.spec --api-version=policy/v1
cat > pdb.yaml <<'EOF'
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  minAvailable: 1
  selector:
    matchLabels:
      app: dojo-web
EOF
kubectl apply --dry-run=server -f pdb.yaml
kubectl apply -f pdb.yaml
```
'''

[[lab.steps.checks]]
name = "stable PDB selects only the required app"
run = '''
[ "$(kubectl -n api-lab get poddisruptionbudget/dojo-web -o jsonpath='{.apiVersion} {.spec.minAvailable} {.spec.selector.matchLabels.app}')" = "policy/v1 1 dojo-web" ] && echo ok
'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Migrate the HPA while retaining its metric contract"
task = '''
Rewrite hpa.yaml from autoscaling/v2beta2 to autoscaling/v2. Preserve target apps/v1 Deployment dojo-web, minReplicas 2, maxReplicas 4, and CPU target Utilization averageUtilization 60. Validate and apply. Inspect conditions, but do not require successful metric collection: this lesson checks API migration, and metrics-server may be absent.
'''
hints = ["The supplied v2beta2 metric already uses the target object shape required by v2.", "Do not introduce targetAverageUtilization from the older v2beta1 schema."]
solution = '''
```bash
kubectl explain horizontalpodautoscaler.spec.metrics --api-version=autoscaling/v2
sed 's#autoscaling/v2beta2#autoscaling/v2#' hpa.yaml > hpa.next
mv hpa.next hpa.yaml
kubectl apply --dry-run=server -f hpa.yaml
kubectl apply -f hpa.yaml
kubectl -n api-lab describe horizontalpodautoscaler dojo-web
```
'''

[[lab.steps.checks]]
name = "stable HPA keeps the same scale target and utilization goal"
run = '''
[ "$(kubectl -n api-lab get horizontalpodautoscaler/dojo-web -o jsonpath='{.apiVersion} {.spec.scaleTargetRef.apiVersion} {.spec.scaleTargetRef.kind} {.spec.scaleTargetRef.name} {.spec.minReplicas} {.spec.maxReplicas} {.spec.metrics[0].type} {.spec.metrics[0].resource.name} {.spec.metrics[0].resource.target.type} {.spec.metrics[0].resource.target.averageUtilization}')" = "autoscaling/v2 apps/v1 Deployment dojo-web 2 4 Resource cpu Utilization 60" ] && echo ok
'''
equals = "ok"
timeout = 90
+++

## The API version is a schema contract

A manifest identifies an API group/version and a kind. `apiVersion: apps/v1` with `kind: Deployment` describes a different schema from `apiVersion: v1` with `kind: Pod`. The core group is written simply as `v1`; named groups include a slash, such as `networking.k8s.io/v1`. A resource's namespace and name identify the particular object, while its group and kind identify the API family.

```text
apiVersion: networking.k8s.io/v1
            [    group    ]/[version]
kind: Ingress
      [resource kind]
metadata: namespace + name
      [object identity within its scope]
```

A Kubernetes cluster release number is not the same thing as an API version. Many unrelated resources use `v1`, and some extension APIs may remain beta while core APIs are stable. Do not “upgrade everything to v1” by a global text replacement. Discover what the target server serves, then read the exact resource schema.

## Stability and deprecation are different axes

Alpha APIs can change or disappear without the guarantees of stable APIs. Beta APIs offer stronger compatibility and migration notice but can still be replaced. GA APIs provide the strongest compatibility contract. An API version can be stable and later marked deprecated; deprecation and alpha/beta/GA describe different things.

| Stability | Typical spelling | Planning consequence |
|---|---|---|
| Alpha | v1alpha1 | Expect change; availability may require explicit enablement |
| Beta | v1beta1, v2beta2 | Watch migration notices and replacement versions |
| GA | v1, v2 | Prefer stable APIs supported by the target cluster |

API stability does not guarantee enablement. Newly introduced built-in beta API versions are generally disabled by default and require server configuration to enable; beta feature gates have their own defaults. Discovery, rather than the version's spelling, tells you what this server accepts.

The Kubernetes deprecation policy protects served APIs with defined notice periods. In general, beta removal follows at least nine months or three minor releases after deprecation, whichever is longer; alpha versions may disappear without prior notice. GA versions are not removed within a Kubernetes major version. Consult the [deprecation policy](https://kubernetes.io/docs/reference/deprecation-policy/) and release migration notes for the actual resource and any documented exception instead of calculating a deadline from memory.

A deprecated but still served API can succeed while returning a warning. Read that warning: it identifies a version that should be migrated before a later upgrade. A removed API endpoint fails, often with a message such as no matches for kind in version. `--validate=false` does not bring an endpoint back or make the server understand a removed schema.

## Discover before editing

```bash
kubectl api-versions
kubectl api-resources --namespaced=true
kubectl api-resources --api-group=networking.k8s.io
kubectl explain ingress --api-version=networking.k8s.io/v1
kubectl explain ingress.spec.rules.http.paths --api-version=networking.k8s.io/v1
```

`api-versions` lists served group/version combinations. `api-resources` lists discovered resources, kinds, names, short names, and scope, generally using preferred versions. `explain` shows field-level schema documentation for a selected resource version. Specify `--api-version` when learning a migration so your client does not silently explain a different preferred version.

Discovery is also useful for distinguishing a removed built-in API from a missing custom resource definition or optional extension. A no-matches error does not automatically mean every unknown kind is deprecated. Check spelling, group, whether the extension is installed, and the target server's discovery results.

## Migrate structure, then meaning

Changing the top-level version may be necessary but insufficient. A new schema can rename fields, require fields that used to default, or change the meaning of an empty object. Review the official [migration guide](https://kubernetes.io/docs/reference/using-api/deprecation-guide/) for the specific transition, then compare the intended application behavior with the rendered new document.

The lab includes three removed API versions as **deliberately invalid starting files**. They should not apply successfully on the target cluster. Keep them distinct from the current-API manifests you write as the repair. The target objects are absent before their steps, so checks prove that migration created a valid object with the required configuration.

| Removed example | Current target | Main issue to inspect |
|---|---|---|
| extensions/v1beta1 Ingress | networking.k8s.io/v1 | Backend structure and required pathType |
| policy/v1beta1 PDB | policy/v1 | Empty selector semantics |
| autoscaling/v2beta2 HPA | autoscaling/v2 | Retain the metric target schema and scaling intent |

These are historical removals, not claims that they first disappeared in this curriculum's cluster release. A modern cluster can still encounter old copied manifests, which is why recognizing their migration patterns remains useful.

## Ingress needs nested Service references

Old Ingress documents used `backend.serviceName` and `backend.servicePort`. The stable API uses `backend.service.name` with `backend.service.port.number` or `.name`. Each HTTP path also requires `pathType`. For a prefix route, the corrected fragment is:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: dojo-web
  namespace: api-lab
spec:
  rules:
  - host: dojo.example.test
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: dojo-web
            port:
              number: 80
```

If migrating an old default backend, it becomes `spec.defaultBackend` with the same nested service reference. IngressClass selection and controller behavior also matter in a real deployment. This lab installs no Ingress controller and does not grade an external address or HTTP routing; it grades the current API specification. An admitted Ingress object alone does not make traffic flow.

## PDB migration can silently broaden protection

An explicitly empty selector `{}` in policy/v1beta1 selected no Pods, while `{}` in policy/v1 selects all Pods in the namespace. An omitted selector selects none in both versions. A simple version replacement can therefore change the scope dramatically while still passing validation.

The lab tells you the intended target is only `app=dojo-web`, so write that explicit selector. This repairs intent rather than mechanically preserving the legacy select-nothing behavior. `minAvailable: 1` requires one selected Pod to remain healthy when evaluating voluntary evictions; it does not scale the Deployment. A PDB constrains evictions through the eviction API, not direct Pod deletion or a Deployment's rolling update. It cannot prevent every outage. Do not test it by draining the learner's real nodes.

## HPA migration still needs runtime prerequisites

The supplied autoscaling/v2beta2 HPA already uses `resource.target.type: Utilization` and `averageUtilization`. Migrating that document to autoscaling/v2 keeps the nested target shape. Older v2beta1 examples can use fields such as `targetAverageUtilization` and need additional restructuring. Do not accidentally combine fields from different historical versions.

The HPA target still points at `apps/v1`, kind Deployment, name dojo-web. CPU utilization is relative to CPU requests, so those requests are supplied on the baseline workload. Metrics availability is a separate dependency. The object can be admitted while reporting an unknown metric or a failed collection condition if metrics-server is absent. Grade migration by its schema and preserved intent, then diagnose runtime autoscaling separately when the task requires it.

## Tools and exam tips

The optional `kubectl-convert` plugin can help with supported built-in conversions, for example `kubectl convert -f ingress.yaml --output-version networking.k8s.io/v1`. It is not guaranteed to be installed and cannot infer every semantic intention. Manually reviewing a small migration is often faster than installing a plugin during a timed exercise.

Use `kubectl apply --dry-run=server -f FILE` to check the corrected manifest against the actual server without persisting it, then apply normally. Client-only dry-run cannot fully prove server acceptance. Reinspect the live object after creation and watch controller conditions when relevant. Update the source manifest as well as the live object so the next apply does not reintroduce the removed API. Preserve names, selectors, ports, and requested behavior; migration is complete when the current schema expresses the intended contract.
