+++
id = "ckad-lab-cluster-and-architecture"
title = "The lab cluster and Kubernetes architecture"
module = "foundations"
minutes = 35
needs = "cluster"
requires = ["two-nodes"]
objectives = [
  "Explain the control-plane and node components",
  "Trace desired state through a reconciliation loop",
  "Discover namespaced and cluster-scoped resources",
  "Run a Pod using PodDojo's isolated lab configuration",
]

[[quiz]]
q = "Which component chooses a node for an unscheduled Pod?"
choices = ["kubelet", "kube-scheduler", "etcd", "kube-proxy"]
answer = 1
why = "The scheduler records a node assignment after evaluating placement constraints. The kubelet on that node then realizes the assigned Pod."

[[quiz]]
q = "Why does deleting a Deployment-owned Pod usually cause another Pod to appear?"
choices = [
  "The kubelet restarts the deleted Pod on the same node",
  "The Pod's restartPolicy: Always recreates it",
  "The ReplicaSet reconciles the desired replica count",
  "The scheduler reschedules the same Pod object",
]
answer = 2
why = "Deleting an instance does not remove the controller's desired state. A bare Pod has no such replacement controller."

[[quiz]]
q = "Where does PodDojo direct kubectl commands?"
choices = [
  "Your personal default cluster",
  "Any context chosen by the lesson",
  "Its own kubeconfig and kind-poddojo lab context",
  "Whatever KUBECONFIG your shell exports",
]
answer = 2
why = "PodDojo supplies KUBECONFIG and guards the lab destination. Keep that environment rather than switching contexts or editing a personal configuration."

[[quiz]]
q = "A successful create request proves which fact?"
choices = ["The application is ready", "The API accepted the object", "The image was downloaded", "The Pod has a node"]
answer = 1
why = "Creation is an API event. Scheduling, pulling and readiness happen asynchronously and must be inspected separately."

[lab]
summary = "Discover the lab's API and nodes, label your namespace, and observe a running Pod."
setup = ["kubectl create namespace arch-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = [
  "kubectl delete namespace arch-lab --ignore-not-found --wait=false 2>/dev/null || true",
  "rm -f nodes.txt namespaces.txt resources.txt || true",
]

[[lab.steps]]
title = "Discover the cluster"
task = "Save node names to nodes.txt, namespace names to namespaces.txt, and API resource discovery to resources.txt in the lab folder. Inspect node roles and whether Pods are namespaced."
hints = ["Use kubectl get with -o name.", "Use kubectl api-resources for discovery."]
solution = '''
```bash
kubectl get nodes -o name > nodes.txt
kubectl get namespaces -o name > namespaces.txt
kubectl api-resources > resources.txt
kubectl get nodes -o wide
```

The API discovery is read-only. Saving the observations gives you a concrete inventory without changing any shared cluster component.
'''

[[lab.steps.checks]]
name = "node inventory contains the lab worker"
run = "cat nodes.txt"
contains = "poddojo-worker"

[[lab.steps.checks]]
name = "namespace inventory includes this lab"
run = "cat namespaces.txt"
contains = "arch-lab"

[[lab.steps.checks]]
name = "discovery includes Pods"
run = "cat resources.txt"
matches = '''(?m)^pods\s'''

[[lab.steps]]
title = "Declare and label your namespace"
task = "Ensure namespace arch-lab exists using an idempotent create/apply workflow, label it purpose=architecture, and inspect its YAML. Preparation may already have created the namespace."
hints = ["Namespace is cluster-scoped.", "Use kubectl label namespace with --overwrite."]
solution = '''
```bash
kubectl create namespace arch-lab --dry-run=client -o yaml | kubectl apply -f -
kubectl label namespace arch-lab purpose=architecture --overwrite
kubectl get namespace arch-lab -o yaml
```

The label belongs to the Namespace object itself, so no namespace flag is needed for this cluster-scoped resource.
'''

[[lab.steps.checks]]
name = "namespace has its purpose label"
run = "kubectl get namespace arch-lab -o jsonpath='{.metadata.labels.purpose}'"
equals = "architecture"

[[lab.steps]]
title = "Run and observe a Pod"
task = "Create Pod explorer in arch-lab, using nginx:1.27-alpine, with label app=explorer. Wait for Ready and inspect its node placement."
hints = ["kubectl run creates a bare Pod.", "Use kubectl wait --for=condition=Ready with a timeout."]
solution = '''
```bash
kubectl -n arch-lab run explorer --image=nginx:1.27-alpine --labels=app=explorer
kubectl -n arch-lab wait --for=condition=Ready pod/explorer --timeout=120s
kubectl -n arch-lab get pod explorer -o wide
```

The create request records intent; the Ready wait observes the Pod's readiness condition. With no readiness probe, this confirms container startup rather than testing nginx's HTTP response.
'''

[[lab.steps.checks]]
name = "explorer uses the requested image and label"
run = "kubectl -n arch-lab get pod explorer -o jsonpath='{.spec.containers[0].image} {.metadata.labels.app}'"
equals = "nginx:1.27-alpine explorer"

[[lab.steps.checks]]
name = "explorer becomes Ready"
run = '''kubectl -n arch-lab get pod explorer -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n arch-lab get pod explorer -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"
+++

## A cluster is a system of control loops

Docker starts containers on a machine. Kubernetes adds an API that describes what should run, then continuously works to make reality match that description. You submit objects such as Pods and Deployments; you do not normally log into a node and start their processes yourself.

The important distinction is **desired state versus observed state**. An object has a specification, usually under spec, and many objects expose observations under status. Creating a Pod successfully means the API accepted the request. It does not mean the image has downloaded, the process has started, or the application is ready.

```text
you → kubectl → API server ↔ etcd
                   ↑ ↓
               controllers
                   ↓
scheduler → node assignment → kubelet → runtime → containers
                   ↑                            |
                   └──── observations/status ───┘
```

This is a feedback system, not a script with a fixed ending. Temporary failures cause retries. Deleting a managed Pod often causes a replacement to appear because the controller still sees a requirement for that replica. Deleting a bare Pod has a different outcome: there is no workload controller requesting another one.

## Meet the control plane

The control plane manages cluster state. It is useful to identify each component when interpreting symptoms, even though CKAD focuses on applications rather than administering these components.

| Component | Responsibility | What it does not do |
|---|---|---|
| kube-apiserver | Serves the Kubernetes API; authenticates, authorizes and validates requests | Run application containers |
| etcd | Stores the cluster's API data durably | Schedule Pods or store application database files |
| kube-scheduler | Selects a suitable node for an unscheduled Pod | Download its image |
| kube-controller-manager | Runs controllers that reconcile objects | Execute every application process itself |

The scheduler considers resource requests, constraints, taints and other scheduling rules. It records a node assignment. A Pod that has no suitable node can stay Pending even though every node is healthy. Conversely, a Pod assigned to a node can still fail to pull its image. Distinguish those situations before changing YAML.

Controllers watch the API and make further API requests. A Deployment controller manages ReplicaSets; a ReplicaSet controller creates or removes Pods. Kubernetes components generally communicate through the API rather than each keeping an unrelated private interpretation of your manifest.

## Meet the node

A node is a worker machine from Kubernetes' point of view. It may be a physical machine, VM, or, in kind, a container acting as a node.

| Component | Responsibility |
|---|---|
| kubelet | Watches Pods assigned to its node and asks the runtime to realize them |
| Container runtime | Pulls images and manages containers through the Container Runtime Interface |
| kube-proxy | Implements Service traffic forwarding rules on nodes in this lab |
| Network plugin | Connects Pod networks and provides cluster networking behavior |

The kubelet reports Pod and node status back to the API. It can restart a failed container according to the Pod's restart policy. That restart happens within the existing Pod; replacing a lost Pod on another node requires a controller.

Do not assume Kubernetes uses the Docker daemon for application containers. PodDojo's kind nodes use containerd internally. Docker runs the **node containers**, while containerd inside each node runs the **application containers**. This separation explains why building an image with Docker does not automatically put it into every node's image store.

## The PodDojo lab and kind

Kind means Kubernetes IN Docker. PodDojo provides a cluster named poddojo with one control-plane node and one worker. Both are real Kubernetes nodes, although both are backed by containers on your selected Docker machine. The control-plane node normally has a scheduling taint, so ordinary applications usually land on the worker.

```bash
kubectl get nodes -o wide
kubectl get namespaces
kubectl api-resources
kubectl get pods -n kube-system
```

These commands inspect rather than modify the cluster. Wide output adds useful columns such as node addresses or Pod placement. API discovery lists resource names, short names, API groups, and whether a resource is namespaced. Use discovery when you remember the concept but not the spelling. `kubectl get nodes -o name` and `kubectl get namespaces -o name` emit one resource identity per line; append `> nodes.txt` or `> namespaces.txt` to save each inventory. Similarly, redirect `kubectl api-resources` into `resources.txt`. Redirection writes files in the lab folder without changing cluster objects.

Nodes are cluster-scoped. Pods are namespaced. A request for Pods in arch-lab limits the result to one namespace, whereas adding a namespace to a node request does not make nodes belong to it. Namespaces separate names and policy scope; they are not, by themselves, a complete security or network isolation boundary.

## Kubeconfig and contexts without surprises

A kubeconfig contains cluster endpoints, user authentication references and contexts. A context combines a cluster, a user and an optional default namespace. On an exam workstation, always follow the task's instructions about the cluster you must use.

PodDojo deliberately isolates its lab: commands receive KUBECONFIG pointing to PodDojo's own file, whose lab context is kind-poddojo. The app guards that destination. Your personal kubeconfig and Docker context are outside this exercise. Keep the environment supplied by PodDojo and specify lesson namespaces per command.

```bash
kubectl config current-context
kubectl -n arch-lab get pods
```

The first command is read-only. Do not copy instructions that overwrite kubeconfig files or change a machine's default context. A correct object on the wrong cluster is still the wrong result, and on a shared machine it can damage someone else's work.

## Imperative and declarative workflows

An imperative command asks for an operation now:

```bash
kubectl create namespace arch-lab
kubectl -n arch-lab run explorer --image=nginx:1.27-alpine
```

A declarative manifest records the intended object:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: explorer
  namespace: arch-lab
  labels:
    app: explorer
spec:
  containers:
    - name: web
      image: nginx:1.27-alpine
      ports:
        - containerPort: 80
```

Apply a saved manifest with kubectl apply. Reapplying the same desired state is useful for repeatable work; it is not a promise that every field of every existing resource can be changed. Pods have substantial immutability restrictions.

A useful bridge is generation: add **--dry-run=client -o yaml** to kubectl run. Client dry-run prints an object without creating it. Add the namespace before saving or applying so the file documents where it belongs.

Labels are queryable identifiers such as `purpose=architecture`. Set this lesson's label with `kubectl label namespace arch-lab purpose=architecture --overwrite`; the overwrite flag allows replacing an existing value. Read it back with `kubectl get namespace arch-lab -o yaml`. Annotations hold descriptive metadata. Neither changes an application's behavior by magic: a controller or tool must interpret a value for it to have further effect.

## Exam tips

Read the requested resource kind, namespace, name and image before typing. Check object creation separately from readiness. Use kubectl describe pod to distinguish scheduling, image and process problems, then inspect the relevant part of the system. The architecture is a troubleshooting map: API acceptance, scheduling, node execution and controller reconciliation are different stages.

The [Kubernetes architecture documentation](https://kubernetes.io/docs/concepts/architecture/) is the reference for the control-plane and node responsibilities introduced here.
