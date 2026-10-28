+++
id = "ref-config-and-secrets"
title = "Configuration, identity, and resource admission"
order = 4
summary = "Create and consume configuration, authorize ServiceAccounts precisely, and budget workloads under quota and limit rules."
+++

## ConfigMap and Secret creation

Use namespace ref-practice, created with an idempotent namespace manifest. Files named below are local inputs: `app.conf` contains application settings, `settings.env` contains KEY=value lines, and `config-dir/` contains files to become keys. A ConfigMap carries non-confidential strings in data and base64 bytes in binaryData. A Secret carries base64 bytes in data and accepts unencoded input through stringData. Base64 is an encoding, not encryption.

```bash
kubectl create namespace ref-practice --dry-run=client -o yaml | kubectl apply -f -
printf 'mode=practice\n' > app.conf
printf 'MODE=practice\nZONE=west\n' > settings.env
mkdir -p config-dir
cp app.conf config-dir/app.conf
kubectl -n ref-practice create configmap literal-settings --from-literal=MODE=practice --from-literal=ZONE=west --dry-run=client -o yaml
kubectl -n ref-practice create configmap file-settings --from-file=app.conf --dry-run=client -o yaml
kubectl -n ref-practice create configmap renamed-settings --from-file=runtime.conf=app.conf --dry-run=client -o yaml
kubectl -n ref-practice create configmap directory-settings --from-file=config-dir/ --dry-run=client -o yaml
kubectl -n ref-practice create configmap env-settings --from-env-file=settings.env --dry-run=client -o yaml
kubectl -n ref-practice create secret generic literal-identity --from-literal=ACCOUNT=training --dry-run=client -o yaml
kubectl -n ref-practice create secret generic file-identity --from-file=client.conf=app.conf --dry-run=client -o yaml
kubectl -n ref-practice create secret generic directory-identity --from-file=config-dir/ --dry-run=client -o yaml
kubectl -n ref-practice create secret generic env-identity --from-env-file=settings.env --dry-run=client -o yaml
# Prepared certificate, private key, and registry config are prerequisites:
kubectl -n ref-practice create secret tls site-tls --cert=server.crt --key=server.key --dry-run=client -o yaml
kubectl -n ref-practice create secret docker-registry pull-auth --from-file=.dockerconfigjson=registry-config.json --dry-run=client -o yaml
```

Combine repeated --from-literal, --from-file and --from-env-file flags where supported. Directory import uses filenames as keys and does not recursively import a tree. Env-file parsing is not shell evaluation: do not expect variable expansion or shell quote removal. Pipe generated YAML to apply when ready. Prefer file inputs for real credentials to avoid exposing them in shell history and process arguments.

```yaml
apiVersion: v1
kind: ConfigMap
metadata: {name: settings, namespace: ref-practice}
data: {MODE: practice, app.conf: "mode=practice\n"}
binaryData: {marker.bin: aGk=}
---
apiVersion: v1
kind: Secret
metadata: {name: identity, namespace: ref-practice}
type: Opaque
stringData: {ACCOUNT: training, client.conf: "identity=training\n"}
```

These are harmless example values. TLS Secrets use type kubernetes.io/tls with tls.crt and tls.key; registry Secrets use kubernetes.io/dockerconfigjson with .dockerconfigjson. Type-specific validation does not guarantee credentials work. `immutable: true` prevents data changes; replace the object and update references when needed. stringData is convenient for authoring but does not work well with server-side apply ownership. See [Secret configuration](https://kubernetes.io/docs/concepts/configuration/secret/).

## All common consumption paths

This Pod consumes the YAML objects above. A registry pull Secret is optional and would be referenced under spec.imagePullSecrets, separately from application mounts.

```yaml
apiVersion: v1
kind: Pod
metadata: {name: consumer, namespace: ref-practice}
spec:
  automountServiceAccountToken: false
  containers:
  - name: app
    image: busybox:1.36
    command: [sh, -ec]
    args: ['echo "$MODE $IDENTITY_ACCOUNT"; cat /etc/runtime/app.conf; sleep 3600']
    env:
    - name: MODE
      valueFrom: {configMapKeyRef: {name: settings, key: MODE}}
    envFrom:
    - secretRef: {name: identity}
      prefix: IDENTITY_
    volumeMounts:
    - {name: config, mountPath: /etc/runtime, readOnly: true}
    - {name: combined, mountPath: /run/combined, readOnly: true}
  volumes:
  - name: config
    configMap:
      name: settings
      items: [{key: app.conf, path: app.conf}]
  - name: combined
    projected:
      sources:
      - secret:
          name: identity
          items: [{key: client.conf, path: client.conf, mode: 0444}]
      - configMap:
          name: settings
          items: [{key: MODE, path: mode}]
```

| Need | Form |
|---|---|
| One environment variable | env.valueFrom.configMapKeyRef or secretKeyRef |
| Every key as environment | envFrom.configMapRef or secretRef; optional prefix |
| All keys as files | volumes.configMap.name or volumes.secret.secretName |
| Selected/renamed files | items with key/path; optional per-item mode |
| Several sources in one directory | projected.sources |
| One file within an existing directory | volumeMount.subPath, accepting its update limitation |
| Image registry authentication | spec.imagePullSecrets, namespaced Secret |
| Program reads API directly | ServiceAccount, RBAC and API client; mounted data is not required |

Container arguments support Kubernetes `$(VAR_NAME)` expansion from environment variables; `$VAR_NAME` needs an explicit shell. Existing environment values do not refresh after source changes. Normal mounted configuration updates eventually; the application must reopen/reload it. subPath mounts do not receive those updates. Missing required objects or keys block startup; optional references must be intentional. ConfigMap and Secret references must be in the Pod's namespace. See [projected volumes](https://kubernetes.io/docs/concepts/storage/projected-volumes/).

## ServiceAccount and RBAC

```yaml
apiVersion: v1
kind: ServiceAccount
metadata: {name: reader, namespace: ref-practice}
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata: {name: workload-reader, namespace: ref-practice}
rules:
- apiGroups: ['']
  resources: [pods]
  verbs: [get, list, watch]
- apiGroups: ['']
  resources: [pods/log]
  verbs: [get]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata: {name: read-workloads, namespace: ref-practice}
subjects:
- {kind: ServiceAccount, name: reader, namespace: ref-practice}
roleRef: {apiGroup: rbac.authorization.k8s.io, kind: Role, name: workload-reader}
```

Use spec.serviceAccountName in a Pod. Projected short-lived API tokens are preferred to manually created long-lived ServiceAccount-token Secrets. automountServiceAccountToken=false disables the default mount; explicit serviceAccountToken projections can request another audience and expiration. A ServiceAccount alone grants no application-specific permissions.

A ClusterRole is cluster-scoped rule storage. Binding it with a RoleBinding restricts namespaced permissions to that binding's namespace. A ClusterRoleBinding grants cluster-wide scope. RoleBinding roleRef is immutable. RBAC is additive, without explicit deny rules; inspect other bindings if a supposedly denied action succeeds. resourceNames can restrict named get/update requests, but list/watch need a matching metadata.name field selector; it cannot restrict create by future object name. The [RBAC guide](https://kubernetes.io/docs/reference/access-authn-authz/rbac/) covers these boundaries.

```bash
kubectl auth can-i list pods -n ref-practice --as=system:serviceaccount:ref-practice:reader
kubectl auth can-i get pods --subresource=log -n ref-practice --as=system:serviceaccount:ref-practice:reader
kubectl auth can-i get secrets -n ref-practice --as=system:serviceaccount:ref-practice:reader
```

Impersonation itself requires permission; an impersonation error is not proof that the account lacks the tested permission.

## ResourceQuota and LimitRange

```yaml
apiVersion: v1
kind: ResourceQuota
metadata: {name: budget, namespace: ref-practice}
spec:
  hard: {requests.cpu: '1', requests.memory: 1Gi, limits.cpu: '2', limits.memory: 2Gi, pods: '10'}
---
apiVersion: v1
kind: LimitRange
metadata: {name: per-container, namespace: ref-practice}
spec:
  limits:
  - type: Container
    min: {cpu: 10m, memory: 8Mi}
    max: {cpu: 500m, memory: 256Mi}
    defaultRequest: {cpu: 50m, memory: 32Mi}
    default: {cpu: 100m, memory: 64Mi}
```

Quota bounds aggregate namespace consumption; LimitRange defaults or constrains each admitted container. Neither resizes existing containers automatically. Requests must not exceed limits; 1000m CPU equals one CPU, while Mi and M differ. Include sidecars, surge and other workloads when budgeting. If a Deployment has no Pods, describe its ReplicaSet and quota: admission failures may appear before scheduling can happen. See [resource quotas](https://kubernetes.io/docs/concepts/policy/resource-quotas/) and [LimitRanges](https://kubernetes.io/docs/concepts/policy/limit-range/).
