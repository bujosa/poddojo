+++
id = "ckad-kustomize"
title = "Kustomize bases and overlays"
module = "deployment"
minutes = 50
domain = "deployment"
needs = "cluster"
objectives = ["Compose reusable bases and environment overlays", "Transform names, namespaces, labels, and images", "Apply strategic merge and JSON 6902 patches", "Generate hashed ConfigMaps and Secrets and track rewritten references"]

[[quiz]]
q = "What does kubectl apply -k overlays/dev do?"
choices = ["Sends kustomization.yaml as a Kubernetes resource", "Builds the overlay then applies the rendered resources", "Applies only the patches, leaving base resources untouched", "Applies the base, then the overlay as a second request"]
answer = 1
why = "Kustomization is build input. kubectl builds its resources, transformations, patches, and generators into ordinary API objects before applying them."

[[quiz]]
q = "Why do configMapGenerator names normally include a hash suffix?"
choices = ["To avoid name clashes with ConfigMaps in other namespaces", "To change its name when generated content changes", "To mark the ConfigMap immutable", "To let kubectl prune old ConfigMaps automatically"]
answer = 1
why = "A content change creates a new generated name. Kustomize rewrites recognized references, changing the Pod template so a Deployment can roll out the new configuration."

[[quiz]]
q = "What identifies a container entry for a strategic merge patch?"
choices = ["Always its numeric list position", "Its name merge key for built-in Pod container lists", "Its image field", "Its first containerPort"]
answer = 1
why = "For supported built-in schemas, strategic merge knows merge keys such as containers.name. JSON 6902 instead uses explicit document paths, where a list index can be fragile."

[[quiz]]
q = "Why can commonLabels be hazardous on an existing Deployment?"
choices = ["It applies only to metadata, never to Pod templates", "It can change selectors that are immutable", "It removes labels that the overlay does not list", "It only works inside a Kustomize component"]
answer = 1
why = "commonLabels propagates into selectors as well as metadata. Prefer labels with deliberate includeSelectors/includeTemplates settings, and plan selector changes before first creation."

[[quiz]]
q = "Does apply -k automatically delete older hashed ConfigMaps?"
choices = ["Yes, every old generator result", "Yes, when the generator sets behavior: replace", "No; apply does not by itself garbage-collect old generated objects", "Yes; unreferenced ConfigMaps are garbage-collected"]
answer = 2
why = "Applying a new hash creates or updates current objects but normally leaves old generator outputs. Use a deliberate cleanup policy; this lab removes its isolated namespaces."

[lab]
summary = "Build development and production overlays from supplied files, apply both, and trigger a configuration rollout through generated-name changes."
setup = [
'''
kubectl create namespace kustomize-dev --dry-run=client -o yaml | kubectl apply -f -
kubectl create namespace kustomize-prod --dry-run=client -o yaml | kubectl apply -f -
'''
]
cleanup = [
'''
kubectl delete namespace kustomize-dev kustomize-prod --ignore-not-found --wait=false || true
'''
]

[lab.files]
"base/kustomization.yaml" = '''
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- deployment.yaml
- service.yaml
configMapGenerator:
- name: dojo-settings
  literals:
  - MODE=development
secretGenerator:
- name: dojo-credentials
  literals:
  - TOKEN=practice-only
'''
"base/deployment.yaml" = '''
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
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
        image: nginx:1.26-alpine
        envFrom:
        - configMapRef: {name: dojo-settings}
        env:
        - name: TOKEN
          valueFrom:
            secretKeyRef: {name: dojo-credentials, key: TOKEN}
        readinessProbe:
          httpGet: {path: /, port: 80}
          periodSeconds: 2
'''
"base/service.yaml" = '''
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
spec:
  selector: {app: dojo-web}
  ports:
  - port: 80
    targetPort: 80
'''
"overlays/dev/kustomization.yaml" = '''
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- ../../base
namespace: kustomize-dev
namePrefix: dev-
'''
"overlays/prod/kustomization.yaml" = '''
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- ../../base
namespace: kustomize-prod
namePrefix: prod-
nameSuffix: -v1
'''

[[lab.steps]]
title = "Customize and apply development"
task = '''
Update overlays/dev/kustomization.yaml to add environment=dev to resource metadata and Pod templates without changing selectors, and transform nginx to 1.27-alpine. Preserve namespace kustomize-dev and prefix dev-. Render with kubectl kustomize, then apply -k. Wait for dev-dojo-web to become available.
'''
hints = ["Use labels with includeTemplates true and includeSelectors false.", "The images transformer matches the image name, not the container name."]
solution = '''
```bash
cat > overlays/dev/kustomization.yaml <<'EOF'
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- ../../base
namespace: kustomize-dev
namePrefix: dev-
labels:
- pairs:
    environment: dev
  includeTemplates: true
  includeSelectors: false
images:
- name: nginx
  newTag: 1.27-alpine
EOF
kubectl kustomize overlays/dev
kubectl apply -k overlays/dev
kubectl -n kustomize-dev rollout status deployment/dev-dojo-web --timeout=120s
```
'''

[[lab.steps.checks]]
name = "development transformation is live"
run = '''
kubectl -n kustomize-dev get deployment/dev-dojo-web -o name >/dev/null 2>&1 || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.spec.template.spec.containers[0].image} {.metadata.labels.environment} {.spec.template.metadata.labels.environment}')" = "nginx:1.27-alpine dev dev" ] && [ "$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Build production using two patch styles"
task = '''
Configure the prod overlay with prefix prod-, suffix -v1, namespace kustomize-prod (the development and production Pods share labels and selectors, so they must not share a namespace), environment=prod metadata and Pod labels, nginx:1.27-alpine, and MODE=production through a merged generator. Use a strategic merge patch for replicas=2 and a JSON 6902 patch for web requests cpu=10m,memory=16Mi. Render and apply; wait for prod-dojo-web-v1.
'''
hints = ["Place the strategic merge patch in overlays/prod/replicas.yaml and reference it from patches.", "A JSON patch can add the resources map because the base has no resources field."]
solution = '''
```bash
cat > overlays/prod/kustomization.yaml <<'EOF'
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- ../../base
namespace: kustomize-prod
namePrefix: prod-
nameSuffix: -v1
labels:
- pairs:
    environment: prod
  includeTemplates: true
  includeSelectors: false
images:
- name: nginx
  newTag: 1.27-alpine
patches:
- path: replicas.yaml
- target:
    group: apps
    version: v1
    kind: Deployment
    name: dojo-web
  patch: |-
    - op: add
      path: /spec/template/spec/containers/0/resources
      value:
        requests:
          cpu: 10m
          memory: 16Mi
configMapGenerator:
- name: dojo-settings
  behavior: merge
  literals:
  - MODE=production
EOF
cat > overlays/prod/replicas.yaml <<'EOF'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
spec:
  replicas: 2
EOF
kubectl kustomize overlays/prod
kubectl apply -k overlays/prod
kubectl -n kustomize-prod rollout status deployment/prod-dojo-web-v1 --timeout=120s
```
'''

[[lab.steps.checks]]
name = "production patches and rewritten configuration are live"
run = '''
kubectl -n kustomize-prod get deployment/prod-dojo-web-v1 -o name >/dev/null 2>&1 || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n kustomize-prod get deployment/prod-dojo-web-v1 -o jsonpath='{.spec.template.spec.containers[0].resources.requests.cpu} {.spec.template.spec.containers[0].resources.requests.memory} {.metadata.labels.environment}')" = "10m 16Mi prod" ] && [ "$(kubectl -n kustomize-prod get deployment/prod-dojo-web-v1 -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n kustomize-prod get deployment/prod-dojo-web-v1 -o jsonpath='{.metadata.generation}') 2 2 2 2" ] && cm=$(kubectl -n kustomize-prod get deployment/prod-dojo-web-v1 -o jsonpath='{.spec.template.spec.containers[0].envFrom[0].configMapRef.name}') && [ "$(kubectl -n kustomize-prod get configmap "$cm" -o jsonpath='{.data.MODE}')" = production ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps.checks]]
name = "each environment's Service reaches only its own Pods"
run = '''
eps() { kubectl -n "$1" get endpointslices -l kubernetes.io/service-name="$2" -o jsonpath='{range .items[*].endpoints[*]}{.targetRef.namespace}/{.targetRef.name}{"\n"}{end}' | sort -u; }
envs() { while read -r ref; do [ -n "$ref" ] && kubectl -n "${ref%%/*}" get pod "${ref#*/}" -o jsonpath='{.metadata.labels.environment}{"\n"}'; done; }
for i in $(seq 1 30); do
  dev=$(eps kustomize-dev dev-dojo-web | envs | sort | tr '\n' ' ')
  prod=$(eps kustomize-prod prod-dojo-web-v1 | envs | sort | tr '\n' ' ')
  if [ "$dev" = "dev " ] && [ "$prod" = "prod prod " ]; then echo ok; exit 0; fi
  sleep 2
done
echo "dev-dojo-web endpoints: [$dev], prod-dojo-web-v1 endpoints: [$prod]"
'''
equals = "ok"
timeout = 90

[[lab.steps.negatives]]
name = "deploys production into the development namespace, so both Services select both environments"
run = '''
mkdir -p overlays/prod-shared
cat > overlays/prod-shared/kustomization.yaml <<'YAML'
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- ../../base
namespace: kustomize-dev
namePrefix: prod-
nameSuffix: -v1
labels:
- pairs:
    environment: prod
  includeTemplates: true
  includeSelectors: false
images:
- name: nginx
  newTag: 1.27-alpine
patches:
- target:
    kind: Deployment
    name: dojo-web
  patch: |-
    - op: replace
      path: /spec/replicas
      value: 2
    - op: add
      path: /spec/template/spec/containers/0/resources
      value:
        requests:
          cpu: 10m
          memory: 16Mi
configMapGenerator:
- name: dojo-settings
  behavior: merge
  literals:
  - MODE=production
YAML
kubectl apply -k overlays/prod-shared
kubectl -n kustomize-dev rollout status deployment/prod-dojo-web-v1 --timeout=120s
'''
undo = '''
kubectl delete -k overlays/prod-shared --ignore-not-found --wait=true
rm -rf overlays/prod-shared
'''

[[lab.steps]]
title = "Roll development configuration forward"
task = '''
Change only the base generator literal MODE from development to learning, leaving production override intact. Apply the dev overlay. Keep generator hashes enabled and confirm the Deployment references a hashed ConfigMap containing MODE=learning and a hashed Secret containing TOKEN=practice-only. Wait for the new dev rollout.
'''
hints = ["Use sed to replace the one MODE literal in base/kustomization.yaml.", "The generated reference changes automatically; do not hard-code the hash."]
solution = '''
```bash
sed 's/MODE=development/MODE=learning/' base/kustomization.yaml > base/kustomization.next
mv base/kustomization.next base/kustomization.yaml
kubectl kustomize overlays/dev
kubectl apply -k overlays/dev
kubectl -n kustomize-dev rollout status deployment/dev-dojo-web --timeout=120s
```
'''

[[lab.steps.checks]]
name = "hashed references resolve to the updated configuration and practice Secret"
run = '''
cm=$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.spec.template.spec.containers[0].envFrom[0].configMapRef.name}'); [ "$(kubectl -n kustomize-dev get configmap "$cm" -o jsonpath='{.data.MODE}')" = learning ] || exit 1
for i in $(seq 1 45); do
  if cm=$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.spec.template.spec.containers[0].envFrom[0].configMapRef.name}'); secret=$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.spec.template.spec.containers[0].env[0].valueFrom.secretKeyRef.name}'); case "$cm:$secret" in dev-dojo-settings-*:dev-dojo-credentials-*) [ "$(kubectl -n kustomize-dev get configmap "$cm" -o jsonpath='{.data.MODE}')" = learning ] && [ "$(kubectl -n kustomize-dev get secret "$secret" -o jsonpath='{.data.TOKEN}')" = cHJhY3RpY2Utb25seQ== ] && [ "$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n kustomize-dev get deployment/dev-dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ] ;; *) false ;; esac; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120
+++

## One base, deliberate differences

Kustomize builds Kubernetes manifests by composing ordinary YAML and applying transformations. You do not put template expressions inside a Deployment. Instead, a base describes shared resources and an overlay describes differences for an environment. `kubectl` includes Kustomize support: `kubectl kustomize DIRECTORY` renders documents, and `kubectl apply -k DIRECTORY` renders and applies them.

```text
base/
  deployment.yaml    service.yaml
  kustomization.yaml (generators included)
                  /                 \
       overlays/dev/            overlays/prod/
       dev- names               prod- names, -v1 suffix
       one replica              two replicas, resource requests
```

A base is another Kustomization referenced through `resources`. The directory layout is a convention; there is no special server-side Base object. The API server sees only rendered Deployments, Services, ConfigMaps, and Secrets. Do not run `kubectl apply -f kustomization.yaml`: the Kustomization build input is not a built-in cluster API resource.

## Start with resources

A minimal file is named `kustomization.yaml` and lists resources relative to its directory:

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- deployment.yaml
- service.yaml
```

An overlay can reference that directory:

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
- ../../base
namespace: kustomize-dev
namePrefix: dev-
images:
- name: nginx
  newTag: 1.27-alpine
```

`namespace` assigns namespaced output resources; it does not create a Namespace unless you include one. The lab setup creates its namespaces idempotently. Give each environment its own namespace: the overlays share the base's `app: dojo-web` selector, so a dev Service in the same namespace as the prod Pods would send traffic to production too (prefixes rename objects, not the labels their selectors match). A namespace transformer also does not turn a ClusterRole into a namespaced object. Know each resource's scope before applying a large build.

Render before applying:

```bash
kubectl kustomize overlays/dev
kubectl apply -k overlays/dev
kubectl -n kustomize-dev rollout status deployment/dev-dojo-web --timeout=120s
```

A successful build proves the local composition worked. Server validation, scheduling, image pulling, and application readiness still happen later. Read the rendered YAML when an overlay produces a surprising image or reference; it is the final document that determines behavior.

## Transform names, labels, and images

`namePrefix` and `nameSuffix` transform resource names. For example, `dojo-web` becomes `prod-dojo-web-v1`. Kustomize also rewrites recognized name references, such as a Deployment's ConfigMap reference. It cannot infer that every arbitrary string in a custom resource or shell command is a resource reference; those may need explicit configuration or replacements.

| Transformer | Useful for | Watch for |
|---|---|---|
| namespace | Deploying the same base into an environment | Namespace must exist |
| namePrefix/nameSuffix | Distinguishing multiple installations | Scripts containing literal names |
| labels | Adding metadata and optional template/selector labels | Immutable selectors |
| images | Changing repository, tag, or digest | Matching the image name correctly |

The modern `labels` transformer lets you choose propagation:

```yaml
labels:
- pairs:
    environment: dev
  includeTemplates: true
  includeSelectors: false
```

This adds metadata and Pod template labels while preserving existing selectors. The older `commonLabels` form also modifies selectors, which is dangerous after a Deployment exists because its selector is immutable. It can also accidentally change Service routing. Decide label propagation before creation, and do not attempt to fix an immutable-field error by deleting unrelated workloads.

`images` matches an image name such as `nginx`, not the container's name `web`. Use `newName` to change the repository, `newTag` to change a tag, or `digest` for immutable content addressing. Quote numeric-looking tags so YAML does not interpret them as another type. A tag is a label in a registry; pinning a digest is stronger when reproducibility is required.

## Patches express the exceptions

Use `patches` for specific fields not covered by a convenient transformer. It supports strategic merge and JSON 6902 patch documents. For built-in types with known merge rules, strategic merge understands map keys and selected list merge keys. Containers merge by `name`, so a patch for container `web` need not repeat the complete container list.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
spec:
  replicas: 2
```

Reference that file from the overlay:

```yaml
patches:
- path: replicas.yaml
```

Use the base resource identity when targeting its patch, and render to confirm name transformations and patches composed as intended. Strategic merge support depends on schema; do not assume arbitrary custom resource lists have the same merge semantics as built-in container lists.

JSON 6902 is explicit about operations and paths:

```yaml
patches:
- target:
    group: apps
    version: v1
    kind: Deployment
    name: dojo-web
  patch: |-
    - op: add
      path: /spec/template/spec/containers/0/resources
      value:
        requests:
          cpu: 10m
          memory: 16Mi
```

`add`, `replace`, `remove`, and `test` operate on JSON Pointer paths. A parent path must exist, and `replace` requires an existing target. A numeric list index is positional: adding another container before index zero changes what the patch touches. Strategic merge by name is often safer for container edits. Escape `/` as `~1` and `~` as `~0` inside JSON Pointer keys, especially annotation names.

## Generators make configuration changes visible

`configMapGenerator` and `secretGenerator` can read literals, files, or environment-style files. The lab uses harmless practice data; never place real credentials in a committed generator input.

```yaml
configMapGenerator:
- name: dojo-settings
  literals:
  - MODE=development
secretGenerator:
- name: dojo-credentials
  literals:
  - TOKEN=practice-only
```

By default a content hash is appended to generated names. The base Deployment references `dojo-settings`, and Kustomize rewrites that reference to the generated transformed name. Changing MODE changes the ConfigMap name, which changes the Deployment's Pod template and starts a rollout. Environment variables in already running containers do not change merely because another ConfigMap was created.

An overlay can set `behavior: merge` for an existing generator name to override selected literals. The production overlay therefore keeps MODE=production while the base's development setting changes. `behavior: replace` replaces the generator's content instead. Inspect output to distinguish these behaviors from patching an already generated API object.

Secret generation encodes data; it does not encrypt source files or rendered output. Do not disable the name hash just to make checks easier. Checks should follow live references, not assume a particular hash string. Ordinary `apply -k` does not automatically delete old hashed objects, so plan cleanup explicitly. This lab deletes only its isolated namespace when reset.

## Exam tips

Find the correct overlay, render it, inspect names and references, then apply with `-k`. Keep base configuration shared and environment-specific choices in overlays. Use one small patch at a time when diagnosing build errors. Finally inspect the live Deployment image, labels, requests, replica count, and generated references; a neatly organized directory alone does not prove the workload runs.
