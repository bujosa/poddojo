+++
id = "ckad-helm"
title = "Helm charts, releases, and revisions"
module = "deployment"
minutes = 45
domain = "deployment"
needs = "cluster"
objectives = ["Render and inspect a local chart before installation", "Install and upgrade a namespaced release using values files and overrides", "Inspect release status and revision history", "Roll back and uninstall a release"]

[[quiz]]
q = "What is a Helm release?"
choices = ["The upstream container image tag", "A named installation of a chart with specific values", "A Kubernetes API group", "The directory templates/"]
answer = 1
why = "A chart is a package of templates and defaults. A release is one named installation, scoped to a namespace, with its own revision history."

[[quiz]]
q = "Which input wins when the same value is supplied by defaults, a values file, and --set?"
choices = ["The chart default", "The leftmost values file", "The --set override", "The oldest release revision"]
answer = 2
why = "Chart defaults are merged with supplied values files, with later files taking precedence, then command-line overrides take precedence. Use --set-string for strings that look like numbers or booleans."

[[quiz]]
q = "Does helm template normally install anything?"
choices = ["Yes, all rendered resources", "Only Secrets", "No; it renders manifests for inspection", "Only the namespace"]
answer = 2
why = "Rendering produces YAML without installing the release. Local rendering cannot prove server admission or workload readiness; inspect and then validate against the actual cluster."

[[quiz]]
q = "After revisions 1 and 2, what does rollback to 1 normally create?"
choices = ["Revision 0", "A new current revision 3 using revision 1 configuration", "A new release name", "No history entry"]
answer = 1
why = "Rollback reuses an earlier stored configuration but advances release history. Helm release revisions and Deployment rollout revisions are separate counters."

[[quiz]]
q = "Why is --namespace needed with helm list, status, and rollback?"
choices = ["Helm has no default namespace", "Releases are namespace-scoped and the same name may exist elsewhere", "It changes Docker contexts", "It creates every namespace automatically"]
answer = 1
why = "Commands use a namespace when looking up release records. --create-namespace is an installation convenience, not a property of every Helm command."

[lab]
summary = "Create a local chart without downloading chart dependencies, render it, install and upgrade it, then roll back and remove the release."
setup = [
'''
kubectl create namespace helm-lab --dry-run=client -o yaml | kubectl apply -f -
'''
]
cleanup = [
'''
helm uninstall dojo-web --namespace helm-lab --wait --timeout 90s 2>/dev/null || true
''',
'''
kubectl delete namespace helm-lab --ignore-not-found --wait=false || true
''',
'''
rm -rf dojo-chart rendered.yaml || true
'''
]

[lab.files]
"values.yaml" = '''
fullnameOverride: dojo-web
replicaCount: 1
image:
  repository: nginx
  tag: "1.27-alpine"
  pullPolicy: IfNotPresent
service:
  type: ClusterIP
  port: 80
'''
"values-upgrade.yaml" = '''
replicaCount: 2
image:
  tag: "1.26-alpine"
'''

[[lab.steps]]
title = "Create and render a local chart"
task = '''
Run helm create dojo-chart inside the lab folder. Inspect helm show values. Render release dojo-web for namespace helm-lab into rendered.yaml with supplied values.yaml. The rendered Deployment must be dojo-web, use nginx:1.27-alpine and request one replica. No release should be installed yet.
'''
hints = ["helm create scaffolds files locally; it does not contact a chart repository.", "Use helm template dojo-web ./dojo-chart --namespace helm-lab -f values.yaml."]
solution = '''
```bash
helm create dojo-chart
helm show values ./dojo-chart
helm template dojo-web ./dojo-chart --namespace helm-lab -f values.yaml > rendered.yaml
cat rendered.yaml
```
'''

[[lab.steps.checks]]
name = "local chart renders the pinned image and a preview exists"
run = '''
test -s dojo-chart/Chart.yaml && test -s rendered.yaml && helm template dojo-web ./dojo-chart --namespace helm-lab -f values.yaml | grep -q 'nginx:1.27-alpine' && echo ok
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Install revision one"
task = '''
Install release dojo-web from ./dojo-chart into helm-lab using values.yaml and --namespace helm-lab --create-namespace. Wait for its one nginx:1.27-alpine replica. Inspect list and status.
'''
hints = ["Use helm install with --wait --timeout 120s.", "fullnameOverride in values.yaml gives the workload the exact name dojo-web."]
solution = '''
```bash
helm install dojo-web ./dojo-chart --namespace helm-lab --create-namespace -f values.yaml --wait --timeout 120s
helm list --namespace helm-lab
helm status dojo-web --namespace helm-lab
```
'''

[[lab.steps.checks]]
name = "installed release has one ready pinned replica"
run = '''
helm status dojo-web --namespace helm-lab >/dev/null 2>&1 || exit 1
for i in $(seq 1 45); do
  if helm status dojo-web --namespace helm-lab >/dev/null 2>&1 && [ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.27-alpine" ] && [ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Upgrade with a file and an override"
task = '''
Upgrade dojo-web using values.yaml then values-upgrade.yaml, and override replicaCount to 3 with --set. The result must be three available nginx:1.26-alpine replicas. Inspect helm history.
'''
hints = ["Rightmost values files win over earlier ones. --set wins over files.", "Use --wait so acceptance of the release is followed by workload convergence."]
solution = '''
```bash
helm upgrade dojo-web ./dojo-chart --namespace helm-lab -f values.yaml -f values-upgrade.yaml --set replicaCount=3 --wait --timeout 120s
helm history dojo-web --namespace helm-lab
```
'''

[[lab.steps.checks]]
name = "upgrade runs three replicas of the overridden image"
run = '''
[ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.26-alpine" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.26-alpine" ] && [ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 3 3 3 3" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Roll back the release"
task = '''
Roll dojo-web back to release revision 1. Wait for one available nginx:1.27-alpine replica. Inspect history to see the new revision representing that rollback.
'''
hints = ["helm rollback takes the release name and numeric revision, then the namespace.", "This restores values and manifests from the release record, not database contents."]
solution = '''
```bash
helm rollback dojo-web 1 --namespace helm-lab --wait --timeout 120s
helm history dojo-web --namespace helm-lab
```
'''

[[lab.steps.checks]]
name = "original image and replica count are restored"
run = '''
[ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.27-alpine" ] || exit 1
for i in $(seq 1 45); do
  if [ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.spec.template.spec.containers[0].image}')" = "nginx:1.27-alpine" ] && [ "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.status.observedGeneration} {.status.updatedReplicas} {.status.readyReplicas} {.status.availableReplicas} {.status.replicas}')" = "$(kubectl -n helm-lab get deployment/dojo-web -o jsonpath='{.metadata.generation}') 1 1 1 1" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Uninstall the release"
task = '''
Uninstall dojo-web from helm-lab without retaining its release history. Confirm the release and its Deployment are gone. Keep the local chart for inspection until lab cleanup.
'''
hints = ["helm uninstall removes release-managed resources.", "Use --wait; the namespace itself is not normally deleted."]
solution = '''
```bash
helm uninstall dojo-web --namespace helm-lab --wait --timeout 120s
helm list --namespace helm-lab --deployed --failed --pending --superseded --uninstalling
```
'''

[[lab.steps.checks]]
name = "release record and Deployment are gone"
run = '''
[ -z "$(helm list --namespace helm-lab --deployed --failed --pending --uninstalling -q --filter '^dojo-web$')" ] || exit 1
for i in $(seq 1 45); do
  if [ -z "$(helm list --namespace helm-lab --deployed --failed --pending --uninstalling -q --filter '^dojo-web$')" ] && [ -z "$(kubectl -n helm-lab get deployment dojo-web --ignore-not-found -o name)" ]; then echo ok; exit 0; fi
  sleep 2
done
exit 1
'''
equals = "ok"
timeout = 120
+++

## Package, installation, history

Helm combines reusable Kubernetes templates with values and a release record. A **chart** is the package; a **release** is one named installation of that package into a namespace; a **revision** is a stored change to that installation. You can install the same chart twice with different names or values without copying its templates. You can also install the same release name in different namespaces, so always include the namespace when inspecting or changing it.

```text
chart templates + default values + your overrides
                         |
                      render
                         |
                 Kubernetes manifests
                         |
              release dojo-web, helm-lab
                revision 1 -> 2 -> 3
```

Helm is a client and release manager, not an application runtime. After Helm creates a Deployment, ordinary Kubernetes controllers reconcile its Pods. A release can exist while its application is unhealthy. Helm's success message and the application's readiness are different observations unless you explicitly wait for the relevant resources.


> **Helm 3 or Helm 4?** Both are in use. Helm 4 removed `helm list --all`: to see releases in every
> state, name the states (`--deployed --failed --pending --superseded --uninstalling`). Install,
> upgrade, rollback, uninstall, `--set`, `-f` and `--namespace` work the same in both. Check the
> exam environment's version with `helm version` before relying on a flag.

## Know the chart directory

`helm create dojo-chart` scaffolds a local chart. It does not need a repository download. The generated example is a starting application, not a production policy. Inspect its image, probes, ports, and optional resources before installing. The lab supplies a values file that pins the image and gives the generated Deployment a predictable name.

| File or directory | Role |
|---|---|
| Chart.yaml | Chart name, chart version, application version metadata |
| values.yaml | Default inputs consumed by templates |
| templates/ | Go templates that render Kubernetes objects |
| charts/ | Packaged chart dependencies, if any |
| .helmignore | Files excluded when packaging |

The chart `version` describes the package release. `appVersion` is descriptive application metadata, often used by a template as a default image tag, but it does not automatically constrain every image. Values names are chart-specific: `replicaCount` works only because these templates read it. An arbitrary `--set replicas=3` can be accepted yet do nothing if no template uses that key.

```bash
helm create dojo-chart
helm show chart ./dojo-chart
helm show values ./dojo-chart
helm template dojo-web ./dojo-chart --namespace helm-lab -f values.yaml
```

Use `helm show values` to discover supported inputs rather than guessing. Template errors often identify a missing value or incorrect type. `helm lint ./dojo-chart -f values.yaml` provides an additional local check, but neither linting nor rendering proves that the server will accept every object.

## Values become ordinary YAML

A values file is input to the chart, not a Kubernetes resource. This file configures the generated starter chart:

```yaml
fullnameOverride: dojo-web
replicaCount: 1
image:
  repository: nginx
  tag: "1.27-alpine"
  pullPolicy: IfNotPresent
service:
  type: ClusterIP
  port: 80
```

The rendered Deployment contains ordinary API fields:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dojo-web
spec:
  replicas: 1
  # selector and template metadata are omitted from this fragment
  template:
    spec:
      containers:
      - name: dojo-chart
        image: nginx:1.27-alpine
```

This is an explanatory fragment, not a complete Deployment to apply. Read the actual rendered document for selectors and labels. It is those documents, rather than the values file, that Kubernetes validates.

Precedence runs from chart defaults to supplied files, in their command order, then command-line settings. Later inputs override earlier values at the same key. Prefer a version-controlled values file for durable configuration and `--set` for small explicit overrides. Use `--set-string` when a token such as `0012` must remain a string. Avoid putting real secrets in command lines or release examples; Helm values and release records are not a secret-management solution.

## Install into an explicit namespace

```bash
helm install dojo-web ./dojo-chart --namespace helm-lab \
  --create-namespace -f values.yaml --wait --timeout 120s
helm list --namespace helm-lab
helm status dojo-web --namespace helm-lab
kubectl -n helm-lab get deployment,pod,service
```

`--create-namespace` creates the release namespace if necessary. It does not cause uninstall to remove that namespace later. `--wait` waits for supported readiness conditions within the timeout; hooks and Jobs have their own behavior and options. Always inspect Kubernetes resources if installation times out. A missing image, unschedulable Pod, or broken probe is still a Kubernetes problem even when Helm submitted the manifests.

A release's resources normally carry chart labels such as `app.kubernetes.io/instance`. These help you find Pods without memorizing generated names. Avoid editing release-managed objects directly unless required: a later Helm upgrade can reconcile them from chart values and overwrite that manual change.

## Upgrade and inspect the revision

```bash
helm upgrade dojo-web ./dojo-chart --namespace helm-lab \
  -f values.yaml -f values-upgrade.yaml --set replicaCount=3 \
  --wait --timeout 120s
helm history dojo-web --namespace helm-lab
helm get values dojo-web --namespace helm-lab
helm get manifest dojo-web --namespace helm-lab
```

Supply the intended values explicitly so you can reason about the resulting state. Helm also has reuse/reset behavior flags; do not assume every combination of old values, new files, and those flags is equivalent. `helm get values --all` shows computed values including defaults, while the plain form focuses on supplied values. `helm get manifest` shows what Helm rendered, useful for distinguishing a template mistake from a controller failure.

Helm revision numbers do not equal Deployment rollout revision numbers. A Helm upgrade can change a Service alone and create a new release revision without changing the Deployment template. Conversely, manually changing that template can create Deployment history without a corresponding Helm release revision.

## Roll back, then remove

`helm rollback dojo-web 1 --namespace helm-lab --wait --timeout 120s` restores the configuration associated with release revision 1 and creates a new current release revision. Inspect `helm history` to see that progression. Rollback does not restore database data, external side effects, or resources deliberately excluded from ordinary Helm lifecycle handling.

`helm uninstall dojo-web --namespace helm-lab --wait` removes the release and its managed resources. By default release history is removed; `--keep-history` preserves records and changes what appears with `helm list --all`. Charts may contain keep annotations, hooks, or CRDs with distinct lifecycle behavior. Read the chart before assuming uninstall deletes everything it ever touched.

## Repositories and exam tips

A repository provides downloadable chart packages and an index. For example, `helm repo add dojo-example https://helm.github.io/examples`, followed by `helm repo update` and `helm search repo dojo-example`, illustrates discovery. `helm search repo` searches configured repositories, whereas hub search is a different discovery route. Pin a chart version when installing remote packages. The lab uses only a local chart, so repository availability is not a dependency; container images may still need downloading.

At exam speed, inspect values, render suspicious templates, specify the namespace, and verify the live workload after installation or rollback. Keep the release name separate from chart name, Deployment name, and application image tag. Those four names often differ.
