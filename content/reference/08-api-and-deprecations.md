+++
id = "ref-api-and-deprecations"
title = "API discovery, migration, and extensions"
order = 8
summary = "Discover served APIs, migrate removed manifest versions safely, and distinguish CRD schemas from controller behavior."
+++

## Discover what this server serves

Kubernetes v1.37 is a cluster release, not an apiVersion string. Each resource group has independent versions: apps/v1, batch/v1 and networking.k8s.io/v1 coexist. Core resources use v1 with no group prefix. Discovery answers which APIs this cluster actually serves; local examples and client defaults may be older or newer.

```bash
kubectl version
kubectl api-versions
kubectl api-resources -o wide
kubectl api-resources --namespaced=true --verbs=list -o name
kubectl api-resources --api-group=apps -o wide
kubectl api-resources --api-group=batch -o wide
kubectl api-resources --api-group=networking.k8s.io -o wide
kubectl explain ingress --api-version=networking.k8s.io/v1
kubectl explain ingress.spec.rules.http.paths.backend --api-version=networking.k8s.io/v1
kubectl get --raw /apis/apps/v1
```

api-versions lists group/version pairs; api-resources maps resource names, kinds, scope and verbs. Use a fully qualified resource name to avoid ambiguity between extensions, such as `kubectl get deployments.apps -n ref-practice`. Discovery access and resource access are separate permissions. An API appearing in discovery does not mean your account can create it.

| Resource | Stable API to use |
|---|---|
| Pod, Service, Secret, ConfigMap, PVC, ServiceAccount | v1 |
| Deployment, DaemonSet, StatefulSet, ReplicaSet | apps/v1 |
| Job, CronJob | batch/v1 |
| Ingress, IngressClass, NetworkPolicy | networking.k8s.io/v1 |
| EndpointSlice | discovery.k8s.io/v1 |
| HPA with multiple metrics or behavior | autoscaling/v2 |
| Role, RoleBinding, ClusterRole, ClusterRoleBinding | rbac.authorization.k8s.io/v1 |
| PodDisruptionBudget | policy/v1 |
| CustomResourceDefinition | apiextensions.k8s.io/v1 |

## Convert a manifest, not just its version line

First preserve the source and identify what behavior must remain: names, selectors, schedule, ports, paths, storage and security. Read the target schema and the specific migration notes. A removed serving API does not imply that the persisted resource was deleted. Existing objects can often be read through the supported API version. Use the [API migration guide](https://kubernetes.io/docs/reference/using-api/deprecation-guide/) for exact field changes.

A legacy Ingress backend using serviceName/servicePort requires nested service.name and service.port in v1. Each path also needs pathType. This current manifest expects namespace ref-practice, Service web on port 80, and the traefik class:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata: {name: migrated-web, namespace: ref-practice}
spec:
  ingressClassName: traefik
  rules:
  - host: migrated.dojo.test
    http:
      paths:
      - path: /app
        pathType: Prefix
        backend:
          service:
            name: web
            port: {number: 80}
```

A string Service port becomes service.port.name, not number. Prefix expresses path-element matching; choose it only if that matches the intended behavior. Old spec.backend becomes spec.defaultBackend. For an apps/v1 Deployment migration, explicitly supply a selector matching template labels. For CronJobs, use batch/v1 and preserve the nested jobTemplate structure. For HPAs, v2 uses metrics and nested resource.target rather than the old single CPU target field.

```bash
# Optional: requires the separately installed kubectl-convert plugin.
kubectl convert -f legacy.yaml --output-version=networking.k8s.io/v1 > converted.yaml
# With or without the plugin, inspect and correct the resulting fields.
kubectl apply --dry-run=server --validate=strict -f converted.yaml
kubectl diff -f converted.yaml
kubectl apply -f converted.yaml
```

The converter is not a built-in command in a standard modern kubectl installation and cannot infer application intent. Manual migration is valid. `kubectl diff` returns 1 when differences exist; that is not automatically a validation error. Client dry-run alone does not verify server admission. After applying, wait for reconciliation and test the real workload, traffic path or scheduled-job template. Keep requested evidence under PODDOJO_LAB_DIR.

## Deprecation policy

Deprecation warns of replacement; removal stops serving an API version. Alpha APIs can disappear without advance notice. Stable APIs cannot be removed within the same Kubernetes major version, even if deprecated. Current beta policy gives a bounded lifetime: deprecation by the longer of nine months or three minor releases after introduction, followed by a corresponding deprecation-to-removal interval. Consult the [deprecation policy](https://kubernetes.io/docs/reference/deprecation-policy/) and release-specific guide rather than extrapolating a removal date from a version suffix.

Important historic examples remain relevant when repairing old files: extensions/v1beta1 Deployments stopped being served in v1.16; v1beta1 Ingress APIs stopped in v1.22; batch/v1beta1 CronJob stopped in v1.25; autoscaling/v2beta2 HPA stopped in v1.26. None are suitable manifest APIs for v1.37. Not every beta API is removed, and changing apiVersion without adjusting semantics can silently change behavior. A policy/v1 PodDisruptionBudget with an empty selector selects every Pod in its namespace, unlike the old beta behavior. API-version compatibility also differs from kubectl client/server version skew.

## CRDs and custom resources

A CRD registers a resource schema and REST API; a custom resource is one object using it. The CRD is cluster-scoped even when its custom objects are namespaced. A CRD alone does not deploy an application or run reconciliation. An operator/controller must implement those actions. The following original practice API stores a report request and validates its fields.

```yaml
apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: dojo-reportrequests.reference.poddojo.local
spec:
  group: reference.poddojo.local
  scope: Namespaced
  names:
    plural: dojo-reportrequests
    singular: dojo-reportrequest
    kind: DojoReportRequest
    shortNames: [drr]
  versions:
  - name: v1
    served: true
    storage: true
    schema:
      openAPIV3Schema:
        type: object
        properties:
          spec:
            type: object
            required: [format, copies]
            properties:
              format: {type: string, enum: [text, json]}
              copies: {type: integer, minimum: 1, maximum: 5}
```

Save as crd.yaml. Create the namespace first. Apply the CRD and wait for establishment before creating a custom object; discovery updates are asynchronous.

```bash
kubectl apply -f crd.yaml
kubectl wait --for=condition=Established crd/dojo-reportrequests.reference.poddojo.local --timeout=60s
kubectl api-resources --api-group=reference.poddojo.local -o wide
kubectl explain dojo-reportrequests.spec --api-version=reference.poddojo.local/v1
kubectl -n ref-practice apply -f - <<'EOF'
apiVersion: reference.poddojo.local/v1
kind: DojoReportRequest
metadata: {name: daily}
spec: {format: json, copies: 2}
EOF
kubectl -n ref-practice get dojo-reportrequests.reference.poddojo.local daily -o yaml
```

Exactly one served/storage configuration must designate the storage version: one storage=true across versions; other served versions may remain accessible. Structural schemas describe and validate fields; unknown fields are normally pruned unless explicitly preserved. Required fields must be declared at the object level where they are required. The example makes spec's children required when spec is present; add top-level required: [spec] if the whole spec must be mandatory. Status subresources, conversion webhooks and additional printer columns are separate CRD features. See [CRD schemas and lifecycle](https://kubernetes.io/docs/tasks/extend-kubernetes/custom-resources/custom-resource-definitions/).

Delete only these named practice resources when finished. Deleting a CRD removes its custom objects across namespaces, so inspect the exact target first. For routine cleanup, remove the named custom object before the dedicated dojo-prefixed CRD. Never delete an unrelated CRD to fix an application's manifest.
