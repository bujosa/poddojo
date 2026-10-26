+++
id = "ckad-workload-resources"
title = "Choose the right workload resource"
module = "design-build"
minutes = 55
needs = "cluster"
requires = ["two-nodes"]
objectives = [
  "Choose controllers from application lifecycle requirements",
  "Compare Deployment replicas with StatefulSet identities",
  "Run a DaemonSet on eligible nodes",
  "Distinguish finite Jobs, scheduled Jobs and bare Pods",
]
domain = "design-build"

[[quiz]]
q = "Which workload suits interchangeable HTTP replicas and rolling updates?"
choices = ["StatefulSet", "Deployment", "CronJob", "DaemonSet"]
answer = 1
why = "A Deployment manages ReplicaSets and template revisions. Stateless replicas normally do not need ordinal identities or per-replica claims."

[[quiz]]
q = "What does a StatefulSet provide?"
choices = [
  "Automatic database replication",
  "A stable virtual IP shared by its Pods",
  "Stable ordinal identity and association with persistent claims",
  "One Pod on each node",
]
answer = 2
why = "StatefulSet supplies Kubernetes identity and storage mechanics; the application must implement replication and data consistency itself."

[[quiz]]
q = "Why might a DaemonSet have only one Pod in a two-node cluster?"
choices = [
  "DaemonSets have replicas=1 by default",
  "The other node may be ineligible because of taints or selectors",
  "DaemonSets skip nodes already running a Pod with the same labels",
  "The DaemonSet's updateStrategy is OnDelete",
]
answer = 1
why = "One per node means one per eligible node. Inspect constraints and tolerations before deciding coverage is wrong."

[[quiz]]
q = "What happens when a CronJob is suspended?"
choices = [
  "Existing Jobs are killed",
  "Every missed run starts when it is resumed",
  "New scheduled Jobs stop; existing Jobs continue",
  "Existing Jobs pause until it is resumed",
]
answer = 2
why = "Suspend controls future scheduling. Existing Job objects retain their own execution lifecycle."

[lab]
summary = "Compare a bare Pod with Deployment, StatefulSet, DaemonSet, Job and CronJob controllers."
setup = ["kubectl create namespace workloads-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace workloads-lab --ignore-not-found --wait=false 2>/dev/null || true"]

[[lab.steps]]
title = "Compare a Pod with a Deployment"
task = "Create bare Pod manual from busybox:1.36 running sleep 3600. Create Deployment web from nginx:1.27-alpine with two replicas. Both belong in workloads-lab and must become Ready."
hints = ["run creates a bare Pod; create deployment creates a controller.", "Wait for the Pod and the Deployment separately."]
solution = '''
```bash
kubectl -n workloads-lab run manual --image=busybox:1.36 --command -- sleep 3600
kubectl -n workloads-lab create deployment web --image=nginx:1.27-alpine --replicas=2
kubectl -n workloads-lab wait --for=condition=Ready pod/manual --timeout=120s
kubectl -n workloads-lab rollout status deployment/web --timeout=120s
```

The bare Pod has no workload owner, while the Deployment delegates replica maintenance to a ReplicaSet. Their running containers can look similar despite different replacement behavior.
'''

[[lab.steps.checks]]
name = "bare Pod uses busybox and is Ready"
run = '''kubectl -n workloads-lab get pod manual -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get pod manual -o jsonpath='{.spec.containers[0].image} {.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = "busybox:1.36 True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "Deployment has two Ready replicas"
run = '''kubectl -n workloads-lab get deploy web -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get deploy web -o jsonpath='{.spec.template.spec.containers[0].image} {.spec.replicas} {.status.readyReplicas}' 2>/dev/null); [ "$v" = "nginx:1.27-alpine 2 2" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "manual Pod has no controller owner"
run = "kubectl -n workloads-lab get pod manual -o jsonpath='{.metadata.ownerReferences}'"
equals = ""

[[lab.steps]]
title = "Give a workload stable identity"
task = "Create headless Service store selecting app=store on TCP port 80. Create one-replica StatefulSet store using that governing Service, Pods labeled app=store, container web from nginx:1.27-alpine, and claim template data requesting 64Mi ReadWriteOnce storage from standard. Mount it at /data. Wait for store-0 to be Ready."
hints = ["Set serviceName: store on the StatefulSet.", "volumeClaimTemplates is a sibling of template."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata:
  name: store
  namespace: workloads-lab
spec:
  clusterIP: None
  selector:
    app: store
  ports:
    - port: 80
      targetPort: 80
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: store
  namespace: workloads-lab
spec:
  serviceName: store
  replicas: 1
  selector:
    matchLabels:
      app: store
  template:
    metadata:
      labels:
        app: store
    spec:
      containers:
        - name: web
          image: nginx:1.27-alpine
          volumeMounts:
            - name: data
              mountPath: /data
  volumeClaimTemplates:
    - metadata:
        name: data
      spec:
        storageClassName: standard
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 64Mi
EOF
kubectl -n workloads-lab rollout status statefulset/store --timeout=180s
```

The StatefulSet gives the Pod an ordinal identity and a dedicated claim. Its headless Service provides the governing DNS domain.
'''

[[lab.steps.checks]]
name = "headless governing Service is configured"
run = "kubectl -n workloads-lab get service store -o jsonpath='{.spec.clusterIP} {.spec.selector.app} {.spec.ports[0].port}'"
equals = "None store 80"

[[lab.steps.checks]]
name = "ordinal Pod has its persistent claim"
run = '''kubectl -n workloads-lab get pod store-0 -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get pod store-0 -o jsonpath='{.spec.containers[?(@.name=="web")].image} {.spec.volumes[?(@.name=="data")].persistentVolumeClaim.claimName} {.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = "nginx:1.27-alpine data-store-0 True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "per-instance claim is Bound"
run = '''kubectl -n workloads-lab get pvc data-store-0 -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get pvc data-store-0 -o jsonpath='{.spec.storageClassName} {.spec.accessModes[0]} {.spec.resources.requests.storage} {.status.phase}' 2>/dev/null); [ "$v" = "standard ReadWriteOnce 64Mi Bound" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "StatefulSet names its governing Service"
run = "kubectl -n workloads-lab get statefulset store -o jsonpath='{.kind} {.spec.serviceName} {.spec.replicas}'"
equals = "StatefulSet store 1"

[[lab.steps]]
title = "Run on both lab nodes"
task = "Create DaemonSet agent in workloads-lab with container agent from busybox:1.36 running sleep 3600. Label its Pods app=agent and tolerate node-role.kubernetes.io/control-plane:NoSchedule. Both lab nodes must have Ready instances."
hints = ["Use an Exists toleration for the control-plane taint key.", "A DaemonSet has no replicas field."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: agent
  namespace: workloads-lab
spec:
  selector:
    matchLabels:
      app: agent
  template:
    metadata:
      labels:
        app: agent
    spec:
      tolerations:
        - key: node-role.kubernetes.io/control-plane
          operator: Exists
          effect: NoSchedule
      containers:
        - name: agent
          image: busybox:1.36
          command: ["sleep", "3600"]
EOF
kubectl -n workloads-lab rollout status daemonset/agent --timeout=120s
```

The toleration makes the control-plane node eligible. The DaemonSet creates one instance per eligible node without a replicas field.
'''

[[lab.steps.checks]]
name = "DaemonSet covers two nodes"
run = '''kubectl -n workloads-lab get ds agent -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get ds agent -o jsonpath='{.status.desiredNumberScheduled} {.status.numberReady}' 2>/dev/null); [ "$v" = "2 2" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "agent template uses busybox"
run = "kubectl -n workloads-lab get ds agent -o jsonpath='{.spec.template.spec.containers[?(@.name==\"agent\")].image}'"
equals = "busybox:1.36"

[[lab.steps]]
title = "Complete finite work"
task = "Create Job report in workloads-lab using busybox:1.36. It must print report-complete and complete successfully."
hints = ["create job accepts the command after --.", "Wait for the Job Complete condition."]
solution = '''
```bash
kubectl -n workloads-lab create job report --image=busybox:1.36 -- echo report-complete
kubectl -n workloads-lab wait --for=condition=complete job/report --timeout=120s
```

The Job records completion rather than trying to keep the successful process running. Its logs preserve the finite task's result.
'''

[[lab.steps.checks]]
name = "Job completes"
run = '''kubectl -n workloads-lab get job report -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get job report -o jsonpath='{.status.conditions[?(@.type=="Complete")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "completed work has the expected output"
run = "kubectl -n workloads-lab logs job/report"
equals = "report-complete"

[[lab.steps.checks]]
name = "report uses the required image"
run = "kubectl -n workloads-lab get job report -o jsonpath='{.spec.template.spec.containers[0].image}'"
equals = "busybox:1.36"

[[lab.steps]]
title = "Describe scheduled work safely"
task = "Create suspended CronJob heartbeat in workloads-lab, scheduled every five minutes, with busybox:1.36 printing heartbeat and restartPolicy Never. Leave it suspended. Create a manual Job heartbeat-check from this CronJob and confirm it completes with output heartbeat."
hints = ["Use batch/v1 with spec.schedule and spec.suspend.", "The Pod spec is inside jobTemplate.spec.template.spec."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: batch/v1
kind: CronJob
metadata:
  name: heartbeat
  namespace: workloads-lab
spec:
  schedule: "*/5 * * * *"
  suspend: true
  jobTemplate:
    spec:
      template:
        spec:
          restartPolicy: Never
          containers:
            - name: main
              image: busybox:1.36
              command: ["echo", "heartbeat"]
EOF
```

```bash
kubectl -n workloads-lab create job heartbeat-check --from=cronjob/heartbeat
kubectl -n workloads-lab wait --for=condition=complete job/heartbeat-check --timeout=120s
```

Suspension prevents clock-driven runs during the exercise. The manual Job tests the same template immediately and leaves inspectable completion evidence.
'''

[[lab.steps.checks]]
name = "CronJob has the requested schedule and suspension"
run = "kubectl -n workloads-lab get cronjob heartbeat -o jsonpath='{.spec.schedule} {.spec.suspend} {.spec.jobTemplate.spec.template.spec.restartPolicy}'"
equals = "*/5 * * * * true Never"

[[lab.steps.checks]]
name = "scheduled template uses busybox"
run = "kubectl -n workloads-lab get cronjob heartbeat -o jsonpath='{.spec.jobTemplate.spec.template.spec.containers[0].image}'"
equals = "busybox:1.36"

[[lab.steps.checks]]
name = "manual CronJob test completes"
run = '''kubectl -n workloads-lab get job heartbeat-check -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n workloads-lab get job heartbeat-check -o jsonpath='{.status.conditions[?(@.type=="Complete")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "template produces the required output"
run = "kubectl -n workloads-lab logs job/heartbeat-check"
equals = "heartbeat"
+++

## Choose the controller from the application's contract

A Pod describes a group of containers. A workload controller describes how a set of Pods should behave over time. Choosing the correct controller is an application design decision: should the work run forever, finish once, recur on a schedule, have stable identity, or run on every eligible node?

```text
continuous interchangeable replicas → Deployment → ReplicaSet → Pods
stable numbered identities         → StatefulSet → Pods + claims
node-local instance                → DaemonSet   → Pod per eligible node
finite work                        → Job         → successful Pods
scheduled finite work              → CronJob     → Jobs → Pods
unmanaged experiment               → bare Pod
```

You can make several controllers start the same container image, but that does not make them interchangeable. The controller determines replacement, scaling, completion and identity semantics. Read the workload requirement before reaching for the generator you remember best.

| Requirement | First choice | Key distinction |
|---|---|---|
| Stateless HTTP replicas | Deployment | Interchangeable replicas and rolling updates |
| Stable database member identities | StatefulSet | Ordinal names and per-instance claims |
| Node-local log agent | DaemonSet | Placement follows eligible nodes |
| One finite conversion | Job | Tracks successful completions |
| Nightly report | CronJob | Creates Jobs on a schedule |
| Temporary manual experiment | Bare Pod | No controller replaces it |

## Deployment and ReplicaSet

A Deployment maintains a desired replica count through ReplicaSets and manages revisions of a Pod template. A ReplicaSet ensures the required number of matching Pods exists; a Deployment adds rollout behavior over those sets.

```bash
kubectl -n workloads-lab create deployment web --image=nginx:1.27-alpine --replicas=2
kubectl -n workloads-lab rollout status deployment/web --timeout=120s
kubectl -n workloads-lab get replicasets
```

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  namespace: workloads-lab
spec:
  replicas: 2
  selector:
    matchLabels:
      app: web
  template:
    metadata:
      labels:
        app: web
    spec:
      containers:
        - name: web
          image: nginx:1.27-alpine
```

The selector must match the template's labels. For an existing apps/v1 Deployment, the selector is immutable. Avoid overlapping selectors between unrelated controllers. Changing a template field creates a rollout; changing only the replica count scales the workload without a new template revision.

Do not edit an owned ReplicaSet to perform an ordinary Deployment update. Change the Deployment, which is the source of intent. Likewise, deleting an individual managed Pod tests replacement, not permanent application removal.

## StatefulSet: identity and storage follow the ordinal

A StatefulSet gives Pods stable numbered names such as store-0 and store-1. With a governing headless Service, those identities have stable DNS names. Replacement may change the Pod UID or IP, but the ordinal name and its associated persistent claim remain.

A headless Service uses clusterIP: None. It publishes endpoint DNS rather than a single virtual Service IP. StatefulSet serviceName names that governing Service, which you create separately. Its selector must match the StatefulSet's Pod labels.

```yaml
# Fields inside a StatefulSet spec:
serviceName: store
replicas: 1
selector:
  matchLabels:
    app: store
template:
  metadata:
    labels:
      app: store
  spec:
    containers:
      - name: web
        image: nginx:1.27-alpine
        volumeMounts:
          - name: data
            mountPath: /data
volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      storageClassName: standard
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 64Mi
```

The claim for ordinal zero is named data-store-0. It is different from a single shared claim mounted by every replica. Default ordered behavior creates lower ordinals before higher ones and waits for readiness; applications can depend on those sequencing rules.

StatefulSet does not automatically make a database replicated, consistent or highly available. The application must implement those capabilities. By default, persistent claims are retained when a StatefulSet is deleted or scaled down; retention policy can alter that behavior. In this lab, namespace cleanup removes the namespaced claims.

## DaemonSet: follow nodes rather than replicas

A DaemonSet runs one Pod on each eligible node. Eligibility depends on node selectors, affinity, taints and tolerations. You do not scale it with a desired replicas field.

The lab has two nodes, but ordinary application Pods cannot normally schedule onto its tainted control-plane node. To demonstrate two DaemonSet instances, the lab explicitly tolerates the control-plane NoSchedule taint. A toleration permits placement; it does not force placement or select a node.

Typical DaemonSets provide node-level logging, monitoring or networking. For a harmless lab example, each instance sleeps. Compare desiredNumberScheduled, currentNumberScheduled and numberReady when diagnosing coverage. A healthy instance on one node does not prove every required node is covered.

## Job: successful completion is the goal

A Job creates Pods until its completion requirement is satisfied, subject to failure and deadline settings. Its Pod template must use Never or OnFailure. Always would restart even successfully completed containers, conflicting with finite work.

```bash
kubectl -n workloads-lab create job report --image=busybox:1.36 -- echo report-complete
kubectl -n workloads-lab wait --for=condition=complete job/report --timeout=120s
kubectl -n workloads-lab logs job/report
```

Completed Pods are useful evidence: inspect logs and exit codes. Do not wait for a finished task's Pod to become Ready. Job success is a controller condition. Deleting a Job normally cascades to its owned Pods, whereas history limits and TTL can remove completed work later.

## CronJob: a schedule creates Jobs

A CronJob contains a schedule and a Job template. Each scheduled execution creates a Job, which creates Pods. Changes to a CronJob affect future Jobs rather than rewriting work already running.

```bash
kubectl -n workloads-lab create cronjob heartbeat --image=busybox:1.36 \
  --schedule='*/5 * * * *' -- echo heartbeat
```

Suspending a CronJob prevents new scheduled Jobs; it does not stop existing Jobs. The lab starts its CronJob suspended to avoid time-dependent background work. You can still create a manual Job from the template to test execution immediately.

Concurrency and history settings belong to the CronJob; completion, parallelism and retry settings belong inside its Job template. Keep those nesting levels straight when editing YAML.

## Bare Pods and exam decisions

A bare Pod is appropriate for a direct Pod task or a temporary experiment. Its kubelet may restart containers, but no workload controller replaces the Pod after deletion. A Pod phase, a container restart policy and a controller replacement policy answer different questions.

## Follow ownership when troubleshooting

Inspect a Pod's ownerReferences to find the object that will replace it. A Deployment-owned application normally has a ReplicaSet as the Pod's immediate owner; the ReplicaSet then points to the Deployment. That chain explains why changing a child object is often temporary.

Labels describe selection, while owner references describe controller relationships and cleanup ownership. They are related but different mechanisms. A Pod with app=web is not automatically proof that the intended Deployment owns it. Check the workload kind, selector, template and observed readiness together when confirming a task.

## Exam tips

Map the nouns in the requirement to controller behavior: replicas, ordinal identity, every node, completion or schedule. Verify the chosen kind as well as its running containers. Inspect ownership to understand replacement, and inspect selectors before blaming networking or the scheduler.
