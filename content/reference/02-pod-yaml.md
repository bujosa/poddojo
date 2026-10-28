+++
id = "ref-pod-yaml"
title = "Pod YAML reference"
order = 2
summary = "Use an annotated Pod manifest to connect configuration, storage, health, security, sidecars, and scheduling."
+++

## A complete working Pod

This example serves a shared directory using a non-root BusyBox process. A regular init container writes the first page. A native sidecar publishes the Pod name continuously and establishes startup ordering. All containers share a network namespace, so localhost reaches another container's listener; each still has its own root filesystem. A volume makes files shared. Create the namespace and dependencies first:

```bash
kubectl create namespace ref-practice --dry-run=client -o yaml | kubectl apply -f -
kubectl -n ref-practice create configmap settings --from-literal=MODE=practice --from-literal=note=hello --dry-run=client -o yaml | kubectl apply -f -
kubectl -n ref-practice create secret generic identity --from-literal=ACCOUNT=training --dry-run=client -o yaml | kubectl apply -f -
kubectl -n ref-practice create serviceaccount app --dry-run=client -o yaml | kubectl apply -f -
```

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: field-guide
  namespace: ref-practice
  labels: {app: field-guide, tier: frontend}
  annotations: {owner: practice}
spec:
  serviceAccountName: app
  automountServiceAccountToken: false # No API access needed.
  restartPolicy: Always
  terminationGracePeriodSeconds: 10
  securityContext:                   # Defaults for every container.
    runAsNonRoot: true
    runAsUser: 1000
    runAsGroup: 1000
    fsGroup: 1000                    # Group access to supported volumes.
    seccompProfile: {type: RuntimeDefault}
  nodeSelector: {kubernetes.io/os: linux}
  affinity:
    nodeAffinity:
      preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 20
        preference:
          matchExpressions:
          - {key: disktype, operator: In, values: [ssd]}
  tolerations:                       # Permits a taint; does not attract the Pod.
  - {key: dedicated, operator: Equal, value: practice, effect: NoSchedule}
  volumes:
  - name: pages
    emptyDir: {}                     # Lives for this Pod's lifetime.
  - name: config
    configMap:
      name: settings
      items: [{key: note, path: note.txt}]
  - name: identity
    secret: {secretName: identity, defaultMode: 0440}
  - name: metadata
    downwardAPI:
      items:
      - path: namespace
        fieldRef: {fieldPath: metadata.namespace}
  initContainers:
  - name: seed                        # Ordinary init: must finish successfully.
    image: busybox:1.36
    command: [sh, -ec, 'echo hello > /www/index.html']
    resources:
      requests: {cpu: 10m, memory: 8Mi}
      limits: {cpu: 50m, memory: 16Mi}
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities: {drop: [ALL]}
    volumeMounts: [{name: pages, mountPath: /www}]
  - name: identity-publisher           # Native sidecar, not a one-shot init.
    image: busybox:1.36
    restartPolicy: Always
    env:
    - name: POD_NAME
      valueFrom: {fieldRef: {fieldPath: metadata.name}}
    command: [sh, -ec, 'while true; do echo "$POD_NAME" > /www/identity; sleep 5; done']
    startupProbe:
      exec: {command: [test, -s, /www/identity]}
      periodSeconds: 1
      failureThreshold: 30
    resources:
      requests: {cpu: 10m, memory: 8Mi}
      limits: {cpu: 50m, memory: 16Mi}
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities: {drop: [ALL]}
    volumeMounts: [{name: pages, mountPath: /www}]
  containers:
  - name: http
    image: busybox:1.36
    imagePullPolicy: IfNotPresent
    command: [httpd]
    args: [-f, -p, '8080', -h, /www]
    ports: [{name: http, containerPort: 8080}]
    env:
    - {name: GREETING, value: hello}
    - name: ACCOUNT
      valueFrom: {secretKeyRef: {name: identity, key: ACCOUNT}}
    - name: POD_IP
      valueFrom: {fieldRef: {fieldPath: status.podIP}}
    envFrom:
    - configMapRef: {name: settings}
      prefix: APP_
    resources:
      requests: {cpu: 50m, memory: 16Mi}
      limits: {cpu: 100m, memory: 32Mi}
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities: {drop: [ALL]}
    startupProbe:
      tcpSocket: {port: http}
      periodSeconds: 2
      failureThreshold: 30
    readinessProbe:
      httpGet: {path: /identity, port: http}
      periodSeconds: 2
      timeoutSeconds: 1
    livenessProbe:
      httpGet: {path: /, port: http}
      periodSeconds: 5
      failureThreshold: 3
    volumeMounts:
    - {name: pages, mountPath: /www, readOnly: true}
    - {name: config, mountPath: /etc/practice, readOnly: true}
    - {name: identity, mountPath: /run/identity, readOnly: true}
    - {name: metadata, mountPath: /run/metadata, readOnly: true}
```

Save this as `pod.yaml`, then apply it and wait:

```bash
kubectl apply --dry-run=server -f pod.yaml
kubectl apply -f pod.yaml
kubectl -n ref-practice wait --for=condition=Ready pod/field-guide --timeout=120s
kubectl -n ref-practice exec field-guide -c http -- wget -qO- http://127.0.0.1:8080/identity
kubectl -n ref-practice logs field-guide -c seed
```

## Fields that change runtime behavior

| Field | Meaning and frequent mistake |
|---|---|
| command / args | Override image ENTRYPOINT / CMD; arrays do not invoke a shell automatically |
| env / envFrom | Explicit names versus importing keys; explicit env takes precedence on collisions |
| containerPort | Documents a listener and supplies a port name; it does not start or expose a server |
| requests | Scheduling reservation; CPU utilization autoscaling uses requests as its denominator |
| limits | CPU throttling and memory enforcement; a memory breach can cause OOM termination |
| fsGroup | Volume group handling; does not change the image files' owner or override every CSI driver |
| readOnlyRootFilesystem | Protects image filesystem; mounted writable volumes remain writable |
| emptyDir | Survives a container restart, disappears with the Pod; memory medium consumes memory |
| persistentVolumeClaim | Reuses separately provisioned storage through claimName; not shown in this ephemeral example |

Pod-level security supplies defaults; a container may override applicable fields. Capabilities and readOnlyRootFilesystem belong at container level. Non-root execution needs permission to access the chosen files and bind the listening port. Keep writable cache/temp paths on appropriate volumes. The [security-context guide](https://kubernetes.io/docs/tasks/configure-pod-container/security-context/) explains field placement and volume ownership.

## Ordering, readiness and scheduling

Ordinary init containers run sequentially before application containers. A native sidecar sits in initContainers with restartPolicy Always. Its startup probe, when present, must succeed before Kubernetes proceeds to the next init or application stage. A sidecar's readiness probe contributes to Pod readiness. In a Job, native sidecars can keep running alongside the worker without preventing Job completion; a normal long-running application container would prevent completion. See [native sidecars](https://kubernetes.io/docs/concepts/workloads/pods/sidecar-containers/).

The main startup probe postpones liveness and readiness probing until it succeeds. Readiness controls Service endpoint readiness; liveness restarts the container after repeated failure. Neither fixes a process listening on the wrong port.

A nodeSelector is mandatory matching. Preferred affinity adds a score and can still schedule on nodes without the preference. Required affinity must match; nodeSelector and required affinity both apply. Within nodeSelectorTerms, terms are OR; expressions inside a term are AND. Tolerating a taint permits placement but does not select a node. Avoid hardcoded node names for ordinary scheduling exercises.

Most Pod fields are immutable after creation. For a controller-managed Pod, change the controller's template and observe the rollout. For a standalone Pod needing a structural change, preserve the manifest and recreate that named Pod only when appropriate to the task. Editing the live child of a Deployment will not repair its desired state.
