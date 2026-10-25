+++
id = "ckad-crds-and-operators"
title = "CRDs and operators: extending the Kubernetes API"
module = "config-security"
minutes = 45
domain = "config-security"
needs = "cluster"
objectives = [
  "Discover custom API resources and their definitions",
  "Define a namespaced resource with a structural OpenAPI schema",
  "Create and update custom objects using their served API version",
  "Add printer columns and recognize the status subresource",
  "Explain why a CRD needs a reconciling controller to implement behavior",
]

[[quiz]]
q = "What happens immediately after installing a CRD for backups, without a controller?"
choices = ["Kubernetes starts taking database backups", "The API can store and validate backup objects, but no backup behavior is implemented", "Every Pod gains a backup sidecar", "The scheduler creates a CronJob automatically"]
answer = 1
why = "A CRD registers an API type and its schema. A separate controller must watch instances and implement reconciliation. The name Backup is meaningful to humans but does not itself execute a backup."

[[quiz]]
q = "A CRD declares plural dojo-backups and group dojo.example.com. What must its metadata.name be?"
choices = ["DojoBackup", "dojo.example.com/dojo-backups", "dojo-backups.dojo.example.com", "v1.dojo-backups"]
answer = 2
why = "CRD names follow plural.group. The CRD object is cluster-scoped even when spec.scope makes its custom objects namespaced. This lab uses a dojo-prefixed plural so its cluster-scoped definition is uniquely identifiable."

[[quiz]]
q = "What is the relationship between served and storage in spec.versions?"
choices = ["Every served version must be storage", "Exactly one version is the storage version; served controls which versions clients may use", "storage controls the namespace", "served selects the controller image"]
answer = 1
why = "The API can serve multiple versions, but exactly one version is used for new persisted representations. Changing the storage version does not automatically rewrite every existing object, and conversion may require additional implementation."

[[quiz]]
q = "Which command discovers resource types in this lab's custom group?"
choices = ["kubectl get pods --api-group=dojo.example.com", "kubectl api-resources --api-group=dojo.example.com", "kubectl create operator dojo.example.com", "kubectl logs crd"]
answer = 1
why = "api-resources queries discovery and shows names, kinds, versions and whether resources are namespaced. get crd lists definitions, while get dojo-backups lists instances of the custom resource."

[[quiz]]
q = "Why do operators reconcile repeatedly instead of executing an instruction exactly once?"
choices = ["The API cannot store status", "Desired state and real state can drift; repeated, idempotent reconciliation repairs that drift", "Controllers run only during installation", "Every API watch event is guaranteed to be unique"]
answer = 1
why = "Controllers can restart, receive duplicate observations or encounter transient failures. Reconciliation compares desired and observed state, performs safe incremental work, and records progress rather than treating each event as a once-only command."

[lab]
summary = "Register a small backup API, create and modify its objects, then add discovery-friendly columns and a status subresource without pretending to install a backup controller."
setup = ["kubectl create namespace crd-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl -n crd-lab delete dojo-backups.dojo.example.com nightly --ignore-not-found --wait=false 2>/dev/null || true", "kubectl delete crd dojo-backups.dojo.example.com --ignore-not-found --wait=false || true", "kubectl delete namespace crd-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Register a validated namespaced API"
task = """
Create CRD `dojo-backups.dojo.example.com`: group `dojo.example.com`, plural `dojo-backups`, singular `dojo-backup`, kind `DojoBackup`, scope `Namespaced`, version `v1` served and storage. Require `spec` and `spec.target`; target must be a nonempty string. Define `spec.retentionDays` as integer 1–30 with default 7, and `spec.paused` as boolean with default false. Add printer columns `Target` and `Retention` for their spec fields. Wait for Established.
"""
hints = ["The CRD itself uses apiextensions.k8s.io/v1; custom objects use dojo.example.com/v1.", "Schema goes under versions[].schema.openAPIV3Schema; every object level needs type: object."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: dojo-backups.dojo.example.com
spec:
  group: dojo.example.com
  scope: Namespaced
  names:
    plural: dojo-backups
    singular: dojo-backup
    kind: DojoBackup
  versions:
  - name: v1
    served: true
    storage: true
    additionalPrinterColumns:
    - name: Target
      type: string
      jsonPath: .spec.target
    - name: Retention
      type: integer
      jsonPath: .spec.retentionDays
    schema:
      openAPIV3Schema:
        type: object
        required: [spec]
        properties:
          spec:
            type: object
            required: [target]
            properties:
              target:
                type: string
                minLength: 1
                description: Name of the application to back up.
              retentionDays:
                type: integer
                minimum: 1
                maximum: 30
                default: 7
                description: Number of days a controller should retain backups.
              paused:
                type: boolean
                default: false
                description: Whether a controller should pause new backups.
EOF
kubectl wait --for=condition=Established crd/dojo-backups.dojo.example.com --timeout=120s
kubectl api-resources --api-group=dojo.example.com
```
'''
[[lab.steps.checks]]
name = "the API definition becomes Established"
run = '''kubectl get crd dojo-backups.dojo.example.com >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl get crd dojo-backups.dojo.example.com -o jsonpath='{.status.conditions[?(@.type=="Established")].status}' 2>/dev/null); [ "$v" = True ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the custom type has the required group, scope and version"
run = "kubectl get crd dojo-backups.dojo.example.com -o jsonpath='{.spec.group} {.spec.scope} {.spec.names.plural} {.spec.names.kind} {.spec.versions[0].name} {.spec.versions[0].served} {.spec.versions[0].storage}'"
equals = "dojo.example.com Namespaced dojo-backups DojoBackup v1 true true"
[[lab.steps.checks]]
name = "the schema constrains retention and requires a target"
run = "kubectl get crd dojo-backups.dojo.example.com -o jsonpath='{.spec.versions[0].schema.openAPIV3Schema.required[*]} {.spec.versions[0].schema.openAPIV3Schema.properties.spec.required[*]} {.spec.versions[0].schema.openAPIV3Schema.properties.spec.properties.target.minLength} {.spec.versions[0].schema.openAPIV3Schema.properties.spec.properties.retentionDays.minimum} {.spec.versions[0].schema.openAPIV3Schema.properties.spec.properties.retentionDays.maximum}'"
equals = "spec target 1 1 30"
[[lab.steps.checks]]
name = "the table columns expose target and retention"
run = "kubectl get crd dojo-backups.dojo.example.com -o jsonpath='{.spec.versions[0].additionalPrinterColumns[?(@.name==\"Target\")].jsonPath} {.spec.versions[0].additionalPrinterColumns[?(@.name==\"Retention\")].jsonPath}'"
equals = ".spec.target .spec.retentionDays"

[[lab.steps]]
title = "Create an instance and inspect defaulted data"
task = """
Create custom object `nightly` in `crd-lab` with `apiVersion: dojo.example.com/v1`, kind `DojoBackup`, and `spec.target: inventory`. Omit retention and paused from your input so the schema supplies 7 and false. Use `kubectl get dojo-backups` and `kubectl explain` to inspect the API; no backup workload should be expected because no operator is installed.
"""
hints = ["The custom object is separate from its CustomResourceDefinition.", "Use --api-version=dojo.example.com/v1 with explain to select the exact group and version."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: dojo.example.com/v1
kind: DojoBackup
metadata:
  name: nightly
  namespace: crd-lab
spec:
  target: inventory
EOF
kubectl -n crd-lab get dojo-backups
kubectl explain dojo-backups.spec --api-version=dojo.example.com/v1
```
The API stores an object and defaults its fields. It does not execute the desired operation.
'''
[[lab.steps.checks]]
name = "the namespaced instance has target and schema defaults"
run = "kubectl -n crd-lab get dojo-backups.dojo.example.com nightly -o jsonpath='{.apiVersion} {.kind} {.metadata.namespace} {.spec.target} {.spec.retentionDays} {.spec.paused}'"
equals = "dojo.example.com/v1 DojoBackup crd-lab inventory 7 false"

[[lab.steps]]
title = "Change desired state with a merge patch"
task = "Update `nightly` in `crd-lab` to `retentionDays: 14` and `paused: true`, keeping target `inventory`. Use a merge patch or a correct updated manifest. A custom resource does not support strategic merge patch."
hints = ["Specify --type=merge explicitly for kubectl patch on a CRD-backed resource.", "Updating spec records desired state even when no controller exists to act on it."]
solution = '''
```bash
kubectl -n crd-lab patch dojo-backups.dojo.example.com nightly --type=merge -p '{"spec":{"retentionDays":14,"paused":true}}'
kubectl -n crd-lab get dojo-backups.dojo.example.com nightly -o yaml
```
'''
[[lab.steps.checks]]
name = "the instance records the updated retention and pause request"
run = "kubectl -n crd-lab get dojo-backups.dojo.example.com nightly -o jsonpath='{.spec.target} {.spec.retentionDays} {.spec.paused}'"
equals = "inventory 14 true"

[[lab.steps]]
title = "Expose operator-facing status and a useful display column"
task = """
Extend the CRD's `v1` version with a status subresource and schema for `status.phase` (string) and `status.observedGeneration` (integer, int64). Keep its existing spec schema and printer columns, and add boolean printer column `Paused` using `.spec.paused`. Inspect `nightly` again. Do not fabricate a Completed status: there is still no operator or backup implementation.
"""
hints = ["subresources: {status: {}} belongs on the version, alongside schema.", "A JSON patch can add schema.properties.status and append a printer column without replacing the whole versions list."]
solution = '''
```bash
kubectl patch crd dojo-backups.dojo.example.com --type=json -p '[{"op":"add","path":"/spec/versions/0/subresources","value":{"status":{}}},{"op":"add","path":"/spec/versions/0/schema/openAPIV3Schema/properties/status","value":{"type":"object","properties":{"phase":{"type":"string"},"observedGeneration":{"type":"integer","format":"int64"}}}},{"op":"add","path":"/spec/versions/0/additionalPrinterColumns/-","value":{"name":"Paused","type":"boolean","jsonPath":".spec.paused"}}]'
kubectl -n crd-lab get dojo-backups.dojo.example.com
```
An operator could later report observed state through the status endpoint using its own RBAC grant. This exercise deliberately stops at defining that contract.
'''
[[lab.steps.checks]]
name = "the version exposes a status subresource with typed status fields"
run = '''s=$(kubectl get crd dojo-backups.dojo.example.com -o go-template='{{range .spec.versions}}{{if eq .name "v1"}}{{if ne .subresources nil}}{{if ne .subresources.status nil}}status{{end}}{{end}}{{end}}{{end}}'); t=$(kubectl get crd dojo-backups.dojo.example.com -o jsonpath='{.spec.versions[0].schema.openAPIV3Schema.properties.status.properties.phase.type} {.spec.versions[0].schema.openAPIV3Schema.properties.status.properties.observedGeneration.type} {.spec.versions[0].schema.openAPIV3Schema.properties.status.properties.observedGeneration.format}'); [ "$s $t" = 'status string integer int64' ] && echo ok'''
equals = "ok"
[[lab.steps.checks]]
name = "the paused column is available through the API's table response"
run = '''[ "$(kubectl get crd dojo-backups.dojo.example.com -o jsonpath='{.spec.versions[0].additionalPrinterColumns[?(@.name=="Paused")].jsonPath}')" = .spec.paused ] || exit 1; for i in $(seq 1 30); do v=$(kubectl -n crd-lab get dojo-backups.dojo.example.com nightly 2>/dev/null); case "$v" in *PAUSED*) echo ok; exit 0;; esac; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
+++

## Kubernetes can learn new nouns

Pods, Deployments and Services are built-in resource types. A **CustomResourceDefinition**, or CRD, registers another resource type with the API server. Once registered, instances participate in familiar operations: create, get, list, watch, patch and delete. They have metadata, resource versions and optional spec and status fields just like other Kubernetes objects.

```text
CRD: defines the DojoBackup API shape
  └── DojoBackup nightly: declares one desired backup configuration
        └── operator, if installed: reconciles that intent into real work
```

The distinction matters: an API extension is not an implementation of its business meaning. A custom object named `nightly` does not create a timer, contact a database or store an archive merely because its kind contains “Backup”. This lab deliberately installs no controller, so all observations concern the API contract rather than an imaginary successful backup.

## Discover before assuming

Clusters often contain APIs installed by networking, certificate, storage or application operators. Discover what exists instead of memorizing every extension's resource names:

```bash
kubectl get crd
kubectl api-resources --api-group=dojo.example.com
kubectl -n crd-lab get dojo-backups.dojo.example.com
kubectl explain dojo-backups.spec --api-version=dojo.example.com/v1
```

`get crd` lists **definitions**. `api-resources` shows resource names, API versions, kinds and namespace scope. `get dojo-backups` lists **instances**. Fully qualifying a resource as `plural.group` avoids ambiguity if several groups expose similarly named resources. `kubectl explain` uses the published schema, including descriptions; it is especially useful for unfamiliar custom types in an exam task.

Discovery and schema publication can take a little time after registration. Wait for the CRD's Established condition, then retry discovery when needed. “The server doesn't have a resource type” can mean the extension is missing, not yet published, or named incorrectly; it is not automatically a namespace error.

## A definition has its own version and names

The CRD object uses `apiextensions.k8s.io/v1`. Its instances use the API group and version declared inside that definition, here `dojo.example.com/v1`. These are different versioning layers.

| Field | Lab value | Purpose |
|---|---|---|
| `metadata.name` | `dojo-backups.dojo.example.com` | Must be plural plus group |
| `spec.group` | `dojo.example.com` | API group for instances |
| `spec.names.plural` | `dojo-backups` | Resource name used by kubectl |
| `spec.names.kind` | `DojoBackup` | Kind used in instance YAML |
| `spec.scope` | `Namespaced` | Scope of custom objects |
| `spec.versions[].name` | `v1` | A version clients can request |

A CRD is cluster-scoped even when its instances are namespaced. A Cluster-scoped custom type would instead omit instance namespaces and need cluster-level permissions. In this lab, the dojo-prefixed plural gives the cluster-scoped definition a dojo-prefixed name, while instances stay confined to `crd-lab`.

## Structural schemas make the API teachable

Each served version defines `schema.openAPIV3Schema`. Describe object levels with `type: object`, then their `properties`. Put `required` at the object level that owns the fields. Requiring `target` inside spec does not itself require spec to be present; require both levels when that is your contract.

```yaml
schema:
  openAPIV3Schema:
    type: object
    required: [spec]
    properties:
      spec:
        type: object
        required: [target]
        properties:
          target:
            type: string
            minLength: 1
          retentionDays:
            type: integer
            minimum: 1
            maximum: 30
            default: 7
```

Types reject incompatible input; bounds reject values outside the contract; defaults supply omitted values. A schema that validates `retentionDays` as an integer does not understand whether a particular database exists. Semantic validation may need CEL rules, admission or controller logic. Unknown fields are normally pruned unless the schema explicitly preserves them in an allowed region, so a misspelled field can disappear rather than becoming working configuration.

Descriptions improve `explain` output. Add constraints that reflect a real contract, not arbitrary restrictions that make future versions unusable. Preserve compatibility when existing custom objects may still carry earlier valid values.

## Versions, printer columns and status

`served: true` exposes a version to clients. Exactly one version has `storage: true`, selecting the persisted representation for new writes. Serving multiple versions can require conversion; changing the storage marker alone neither performs arbitrary semantic conversion nor migrates every existing stored object. Version migration is operational work beyond creating a second YAML entry.

`additionalPrinterColumns` controls the default table display. Each column has a name, type and JSONPath such as `.spec.retentionDays`. It improves `kubectl get`; it does not create or compute the underlying field. A missing value produces an empty cell rather than reconciling desired state.

The optional `status` subresource separates desired-state updates from observed-state reports. With it enabled, ordinary writes do not manage status as if it were user configuration; a controller uses the status endpoint with suitable permissions. `status.observedGeneration` commonly records which desired-state generation the controller processed, but the CRD does not maintain that field automatically. Conditions and phase conventions also need controller implementation.

Read controller reports against the desired-state generation rather than trusting an old success flag. Suppose generation 3 completed, then you change the target and the object advances to generation 4. A still-visible Completed phase from the previous reconciliation does not prove the new target was processed. A well-designed controller records its observed generation and updates conditions so clients can distinguish current success from stale success. Resource version serves a different purpose: it changes with API updates and supports concurrency and watching; it is not a business progress counter. In this lab, an absent status is expected and must never be interpreted as evidence that a backup was taken.

## Operators reconcile rather than execute once

An operator is a controller that encodes domain knowledge. It watches custom objects, compares desired with observed state, creates or updates supporting resources, and reports progress. A backup operator might manage Jobs, schedules and retention, retry transient failures and coordinate external storage. A database operator may implement upgrades, recovery and membership changes.

Reconciliation must tolerate duplicate observations, interrupted work and controller restarts. An event is a prompt to compare state, not a unique command guaranteed to arrive exactly once. Permissions, leader election, finalizers and cleanup behavior are important operational parts of an operator, but none are installed automatically by creating a CRD. A finalizer with no working controller can leave deletion pending; do not remove unfamiliar finalizers blindly.

## Exam tips

Start with discovery, then read the CRD's scope, served version and schema. Create the exact required custom object using its own group/version. For changes, use an updated manifest, JSON merge patch with `--type=merge`, or JSON patch; custom resources do not support strategic merge patch. Check spec acceptance separately from controller status. Deleting a CRD removes its API and its stored custom objects, so cleanup must target only the definition owned by your exercise.
