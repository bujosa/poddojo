+++
id = "ckad-volumes"
title = "Volumes: scratch space, claims and projected files"
module = "design-build"
minutes = 50
needs = "cluster"
objectives = [
  "Choose storage based on container, Pod and node lifecycles",
  "Use emptyDir and memory-backed scratch space",
  "Trigger delayed provisioning and reuse a persistent claim",
  "Project configuration and Pod metadata into files",
]
domain = "design-build"

[[quiz]]
q = "Which event normally removes an emptyDir's contents?"
choices = ["A single container restart", "Scaling its Deployment from one replica to two", "Removal of its Pod from the node", "Restarting all its containers after a liveness failure"]
answer = 2
why = "emptyDir belongs to the Pod's lifetime on the node. It can survive container restarts but is not durable across Pod replacement."

[[quiz]]
q = "Why is a new standard-class PVC Pending before this lab creates a consumer?"
choices = [
  "The class is broken",
  "WaitForFirstConsumer delays provisioning and binding",
  "It needs a matching PV created by hand first",
  "The class uses volumeBindingMode: Immediate",
]
answer = 1
why = "Delayed binding lets the scheduler and provisioner coordinate topology. Create a consuming Pod before diagnosing this expected state as failure."

[[quiz]]
q = "What does ReadWriteOnce mean?"
choices = [
  "Exactly one Pod",
  "Read-write mounting from one node, possibly by several Pods",
  "Read-write mounting from many nodes at once",
  "Read-only mounting anywhere",
]
answer = 1
why = "The scope is a node. ReadWriteOncePod is the distinct single-Pod mode and needs supported CSI storage."

[[quiz]]
q = "What does deleting a Pod that uses an independently created PVC normally do to that PVC?"
choices = ["Always deletes it", "Deletes it when the reclaimPolicy is Delete", "Leaves it available for reuse", "Releases its PV so another claim can bind it"]
answer = 2
why = "The claim has an independent lifecycle. Deleting the claim is a separate operation that can trigger the PV's reclaim policy."

[[quiz]]
q = "Why avoid hostPath for portable application persistence?"
choices = [
  "It is wiped whenever the Pod restarts",
  "It couples data and security exposure to a node's filesystem",
  "It counts against the Pod's memory limit",
  "It automatically uses network storage",
]
answer = 1
why = "Paths and data are node-specific, and mounts can expose sensitive host files. A claim provides a portable allocation contract, though backend durability still matters."

[lab]
summary = "Compare ephemeral scratch space with delayed provisioning and persistent claims, then project configuration into files."
setup = ["kubectl create namespace volumes-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace volumes-lab --ignore-not-found --wait=false 2>/dev/null || true"]

[[lab.steps]]
title = "Use disk and memory scratch volumes"
task = "Create Pod scratch in volumes-lab using container main with busybox:1.36. Mount emptyDir disk at /scratch and emptyDir memory with medium Memory and sizeLimit 32Mi at /ram. Write disk-data to /scratch/item and memory-data to /ram/item, then keep the container running."
hints = ["Both volume declarations go under spec.volumes.", "A shell command can initialize the files and then exec sleep 3600."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: scratch
  namespace: volumes-lab
spec:
  containers:
    - name: main
      image: busybox:1.36
      command: ["sh", "-c"]
      args: ["echo disk-data > /scratch/item; echo memory-data > /ram/item; exec sleep 3600"]
      volumeMounts:
        - name: disk
          mountPath: /scratch
        - name: memory
          mountPath: /ram
  volumes:
    - name: disk
      emptyDir: {}
    - name: memory
      emptyDir:
        medium: Memory
        sizeLimit: 32Mi
EOF
kubectl -n volumes-lab wait --for=condition=Ready pod/scratch --timeout=120s
```

The two emptyDirs have the same Pod lifetime but different storage media. Their contents are temporary even though the main process stays running.
'''

[[lab.steps.checks]]
name = "memory scratch is explicitly bounded"
run = "kubectl -n volumes-lab get pod scratch -o jsonpath='{.spec.volumes[?(@.name==\"memory\")].emptyDir.medium} {.spec.volumes[?(@.name==\"memory\")].emptyDir.sizeLimit}'"
equals = "Memory 32Mi"

[[lab.steps.checks]]
name = "disk scratch contains its data"
run = '''kubectl -n volumes-lab get pod/scratch -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec scratch -c main -- cat /scratch/item 2>/dev/null); [ "$v" = "disk-data" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "memory scratch contains its data"
run = '''kubectl -n volumes-lab get pod/scratch -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec scratch -c main -- cat /ram/item 2>/dev/null); [ "$v" = "memory-data" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "scratch sources and mounts are explicit"
run = "kubectl -n volumes-lab get pod scratch -o jsonpath='{.spec.volumes[?(@.name==\"disk\")].emptyDir} {.spec.containers[?(@.name==\"main\")].volumeMounts[?(@.mountPath==\"/scratch\")].name} {.spec.containers[?(@.name==\"main\")].volumeMounts[?(@.mountPath==\"/ram\")].name} {.spec.containers[?(@.name==\"main\")].image}'"
equals = "{} disk memory busybox:1.36"

[[lab.steps]]
title = "Request storage before a consumer exists"
task = "Create PVC records in volumes-lab requesting 64Mi, access mode ReadWriteOnce, storageClassName standard. Do not create a consumer yet. Observe that the claim remains Pending under WaitForFirstConsumer."
hints = ["Create a v1 PersistentVolumeClaim.", "Pending is expected here; inspect the standard class and the claim's events."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: records
  namespace: volumes-lab
spec:
  storageClassName: standard
  accessModes: ["ReadWriteOnce"]
  resources:
    requests:
      storage: 64Mi
EOF
kubectl get storageclass standard
kubectl -n volumes-lab describe pvc records
```

WaitForFirstConsumer deliberately leaves this claim unbound until a consuming Pod gives the scheduler topology information.
'''

[[lab.steps.checks]]
name = "claim requests the intended storage"
run = "kubectl -n volumes-lab get pvc records -o jsonpath='{.spec.storageClassName} {.spec.accessModes[0]} {.spec.resources.requests.storage}'"
equals = "standard ReadWriteOnce 64Mi"

[[lab.steps.checks]]
name = "claim awaits its first consumer"
run = '''kubectl -n volumes-lab get pvc records -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab get pvc records -o jsonpath='{.status.phase}' 2>/dev/null); [ "$v" = "Pending" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Trigger provisioning and write a record"
task = "Create Pod writer in volumes-lab with busybox:1.36, container main, mounting PVC records at /data. Write persistent-record to /data/record and keep it running. Wait for Ready and confirm the claim becomes Bound."
hints = ["Reference records through persistentVolumeClaim.claimName.", "Let the scheduler choose a node; do not set nodeName."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: writer
  namespace: volumes-lab
spec:
  containers:
    - name: main
      image: busybox:1.36
      command: ["sh", "-c"]
      args: ["echo persistent-record > /data/record; exec sleep 3600"]
      volumeMounts:
        - name: records
          mountPath: /data
  volumes:
    - name: records
      persistentVolumeClaim:
        claimName: records
EOF
kubectl -n volumes-lab wait --for=condition=Ready pod/writer --timeout=180s
```

Creating the consumer triggers scheduling and dynamic provisioning. The file is written onto the mounted claim, not the image's writable layer.
'''

[[lab.steps.checks]]
name = "claim binds after a consumer exists"
run = '''kubectl -n volumes-lab get pvc records -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab get pvc records -o jsonpath='{.status.phase}' 2>/dev/null); [ "$v" = "Bound" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "writer mounts the requested claim"
run = "kubectl -n volumes-lab get pod writer -o jsonpath='{.spec.volumes[?(@.name==\"records\")].persistentVolumeClaim.claimName} {.spec.containers[?(@.name==\"main\")].image} {.spec.containers[?(@.name==\"main\")].volumeMounts[?(@.name==\"records\")].mountPath}'"
equals = "records busybox:1.36 /data"

[[lab.steps.checks]]
name = "record is written on the claim"
run = '''kubectl -n volumes-lab get pod/writer -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec writer -c main -- cat /data/record 2>/dev/null); [ "$v" = "persistent-record" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Read from a replacement Pod"
task = "Delete only Pod writer, preserving PVC records. Create Pod reader in volumes-lab with busybox:1.36 and container main running sleep 3600. Mount the same claim read-only at /data. The existing /data/record must still contain persistent-record; do not rewrite it."
hints = ["A PVC outlives its consuming Pod.", "Use a read-only volumeMount to ensure the reader does not change the record."]
solution = '''
```bash
kubectl -n volumes-lab delete pod writer --wait=true
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: reader
  namespace: volumes-lab
spec:
  containers:
    - name: main
      image: busybox:1.36
      command: ["sleep", "3600"]
      volumeMounts:
        - name: records
          mountPath: /data
          readOnly: true
  volumes:
    - name: records
      persistentVolumeClaim:
        claimName: records
EOF
kubectl -n volumes-lab wait --for=condition=Ready pod/reader --timeout=180s
```

The replacement reads the same claim without permission to rewrite it. This demonstrates persistence across Pod deletion while avoiding a false result from reseeding data.
'''

[[lab.steps.checks]]
name = "original writer is removed"
run = "kubectl -n volumes-lab get pod writer --ignore-not-found -o name"
equals = ""

[[lab.steps.checks]]
name = "reader uses the same claim read-only"
run = "kubectl -n volumes-lab get pod reader -o jsonpath='{.spec.volumes[?(@.name==\"records\")].persistentVolumeClaim.claimName} {.spec.containers[?(@.name==\"main\")].volumeMounts[?(@.name==\"records\")].readOnly} {.spec.containers[?(@.name==\"main\")].image}'"
equals = "records true busybox:1.36"

[[lab.steps.checks]]
name = "record survives Pod replacement"
run = '''kubectl -n volumes-lab get pod/reader -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec reader -c main -- cat /data/record 2>/dev/null); [ "$v" = "persistent-record" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Project multiple sources into one directory"
task = "Create ConfigMap settings with message=hello-projected and dummy Secret sample with note=training-only in volumes-lab. Create Pod projection, busybox:1.36, container main running sleep 3600. Mount a projected volume read-only at /etc/bundle combining those keys and a downward API file pod-name containing its Pod name."
hints = [
  "A projected volume's sources can include configMap, secret and downwardAPI.",
  "These values are training examples, not credentials.",
]
solution = '''
```bash
kubectl -n volumes-lab create configmap settings --from-literal=message=hello-projected --dry-run=client -o yaml | kubectl apply -f -
kubectl -n volumes-lab create secret generic sample --from-literal=note=training-only --dry-run=client -o yaml | kubectl apply -f -
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: projection
  namespace: volumes-lab
spec:
  containers:
    - name: main
      image: busybox:1.36
      command: ["sleep", "3600"]
      volumeMounts:
        - name: bundle
          mountPath: /etc/bundle
          readOnly: true
  volumes:
    - name: bundle
      projected:
        sources:
          - configMap:
              name: settings
          - secret:
              name: sample
          - downwardAPI:
              items:
                - path: pod-name
                  fieldRef:
                    fieldPath: metadata.name
EOF
kubectl -n volumes-lab wait --for=condition=Ready pod/projection --timeout=120s
```

Projection combines three sources into one read-only directory. Configuration values and Pod identity arrive as files without rebuilding the image.
'''

[[lab.steps.checks]]
name = "ConfigMap becomes a readable file"
run = '''kubectl -n volumes-lab get pod/projection -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec projection -c main -- cat /etc/bundle/message 2>/dev/null); [ "$v" = "hello-projected" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "dummy Secret becomes a readable file"
run = '''kubectl -n volumes-lab get pod/projection -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec projection -c main -- cat /etc/bundle/note 2>/dev/null); [ "$v" = "training-only" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "downward API exposes the Pod name"
run = '''kubectl -n volumes-lab get pod/projection -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n volumes-lab exec projection -c main -- cat /etc/bundle/pod-name 2>/dev/null); [ "$v" = "projection" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "configuration sources share a projected volume"
run = "kubectl -n volumes-lab get pod projection -o jsonpath='{.spec.volumes[?(@.name==\"bundle\")].projected.sources[*].configMap.name} {.spec.volumes[?(@.name==\"bundle\")].projected.sources[*].secret.name} {.spec.containers[?(@.name==\"main\")].volumeMounts[?(@.name==\"bundle\")].readOnly}'"
equals = "settings sample true"
+++

## Storage has a lifecycle independent of a process

A container's writable layer is convenient for incidental files, but it is the wrong place for data that another container or a replacement instance must retain. Kubernetes volumes give a Pod explicit storage sources and give each container explicit mount points.

```text
storage source → Pod volume name → container volumeMount
                     |
                     └──────────→ another container's mount
```

The volume declaration belongs under Pod spec.volumes. Each participating container mounts that volume through volumeMounts. A mount can hide files already present at that path in the image, so choose the mount directory carefully.

The critical question is **what must survive?** A container restart, replacement of the entire Pod, or loss of the node are different events. Different storage backends provide different guarantees. A persistent claim is an API-level storage contract, not a backup strategy.

## emptyDir: temporary data for one Pod

An emptyDir is created for a Pod and survives container restarts inside that Pod. It disappears when the Pod is removed from its node. Two containers mounting the same emptyDir can exchange files without sharing their image layers.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: scratch
  namespace: volumes-lab
spec:
  containers:
    - name: main
      image: busybox:1.36
      command: ["sleep", "3600"]
      volumeMounts:
        - name: disk
          mountPath: /scratch
        - name: memory
          mountPath: /ram
  volumes:
    - name: disk
      emptyDir: {}
    - name: memory
      emptyDir:
        medium: Memory
        sizeLimit: 32Mi
```

The default medium uses storage backing the node's ephemeral filesystem. Memory uses tmpfs. Memory-backed files consume memory and can contribute to container memory-limit pressure; sizeLimit is not a reservation of additional memory. Disk-backed emptyDir can contribute to local ephemeral storage pressure and eviction.

Use emptyDir for caches, generated configuration, scratch space and shared intermediate files. Do not expect data to survive deleting the Pod and recreating a new Pod with the same name.

## hostPath couples the workload to a node

A hostPath mounts a path from the node's filesystem. In kind, that means the filesystem of the **node container**, not automatically the laptop's ordinary filesystem. A replacement on another node may see different files.

HostPath can expose sensitive node files or permit host modification. It also bypasses the portable storage allocation model. Avoid it for ordinary application persistence and do not use it to explore or modify a learner's host filesystem. This lab uses emptyDir and the provided StorageClass instead.

## PV, PVC and StorageClass form a contract

| Object | Scope | Responsibility |
|---|---|---|
| PersistentVolume, PV | Cluster | Represents a provisioned storage resource |
| PersistentVolumeClaim, PVC | Namespace | Requests size, access mode and storage class |
| StorageClass | Cluster | Selects a provisioner and allocation behavior |
| Pod volume | Namespace through its Pod | References the claim for mounting |

A developer usually creates a PVC and references it by claimName. An administrator or dynamic provisioner supplies a matching PV. A claim binds to a volume; a StorageClass is not itself a disk.

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: records
  namespace: volumes-lab
spec:
  storageClassName: standard
  accessModes: ["ReadWriteOnce"]
  resources:
    requests:
      storage: 64Mi
```

```bash
kubectl -n volumes-lab get pvc records
kubectl get storageclass standard -o yaml
kubectl -n volumes-lab describe pvc records
```

For static provisioning, the administrator creates a PV explicitly and coordinates storage class, capacity, modes and any topology requirements. Any manually authored cluster-scoped PV in PodDojo must have a dojo- prefixed name and dedicated cleanup. This lab creates no static PV; its dynamic provisioner chooses the generated PV name.

## Pending can be the correct initial state

The lab's standard class uses local-path provisioning with WaitForFirstConsumer. Binding and provisioning wait until a Pod uses the claim so scheduling can choose an appropriate node and storage topology together. A newly created claim can legitimately remain Pending.

Create the consuming Pod before treating that state as a storage failure:

```yaml
# Within a Pod spec:
containers:
  - name: main
    image: busybox:1.36
    command: ["sleep", "3600"]
    volumeMounts:
      - name: records
        mountPath: /data
volumes:
  - name: records
    persistentVolumeClaim:
      claimName: records
```

The Pod and PVC must be in the same namespace. Avoid setting nodeName to bypass scheduling for this exercise: WaitForFirstConsumer relies on scheduler participation. If placement is necessary, express a suitable scheduling constraint instead.

Local-path storage persists across Pod replacement on its storage node. It is not shared network storage and does not make node loss harmless. The [StorageClass documentation](https://kubernetes.io/docs/concepts/storage/storage-classes/) explains why delayed binding matters for topology-constrained storage.

## Access modes describe allowed attachment patterns

ReadWriteOnce permits read-write mounting from one node, potentially by multiple Pods on that node. It does **not** mean exactly one Pod. ReadOnlyMany permits read-only mounting from multiple nodes; ReadWriteMany permits read-write mounting from multiple nodes, when the backend supports it.

ReadWriteOncePod restricts use to a single Pod and is supported for CSI volumes with the necessary driver support. Do not assume the lab's local-path provisioner supports every access mode. Request a mode the backend can deliver rather than changing the field until an incompatible system appears to work.

Access modes describe storage capabilities rather than replacing filesystem permissions. Use a read-only volumeMount when a particular container must not write.

Reclaim policy determines what happens to the underlying volume after its claim is deleted. Delete normally removes dynamically provisioned storage; Retain preserves it for manual recovery. Deleting only a Pod does not delete its independently created PVC. Inspect policy before removing a claim containing valuable data.

## Projection assembles read-only configuration

A projected volume combines multiple configuration sources into one directory. Supported sources include ConfigMaps, Secrets and downward API fields; a projected ServiceAccount token is another important source. Each source contributes named files.

```yaml
# A volume entry inside a Pod:
name: bundle
projected:
  sources:
    - configMap:
        name: settings
    - secret:
        name: sample
    - downwardAPI:
        items:
          - path: pod-name
            fieldRef:
              fieldPath: metadata.name
```

Mount this as read-only. ConfigMap and Secret volumes are configuration delivery mechanisms, not application data stores. A dummy Secret is sufficient for this lab; real secrets require careful handling. Updates propagate eventually rather than instantly, and mounts using subPath do not receive the usual projected-file updates. Applications may need to reread files.

Ephemeral volumes are a broader family. CSI ephemeral volumes are managed by supporting CSI drivers for a Pod. Generic ephemeral volumes use a claim template and ordinary provisioning, with the generated claim's lifecycle tied to the Pod. Those mechanisms differ from an independently created PVC you intend to reuse after Pod deletion.

## Exam tips

Trace source, volume name and mount path separately. Check events for a Pending claim, then ask whether a first consumer exists. Prove persistence by reading a file from a replacement Pod. Match the access mode to the backend, and distinguish Pod deletion, claim deletion and storage reclamation before acting.
