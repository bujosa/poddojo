+++
id = "ckad-kubectl-at-exam-speed"
title = "kubectl at exam speed"
module = "foundations"
minutes = 45
needs = "cluster"
objectives = [
  "Generate manifests using imperative commands and client dry-run",
  "Discover fields and inspect precise resource state",
  "Update workload templates, replicas and metadata",
  "Use namespace and editor habits that prevent exam mistakes",
]

[[quiz]]
q = "What does client dry-run prove?"
choices = [
  "The image is available",
  "The workload is Ready",
  "The client can generate the object without persisting it",
  "Admission will accept it",
]
answer = 2
why = "Client dry-run produces a scaffold. Server admission and runtime behavior still need their own validation."

[[quiz]]
q = "Set image reports that container api was not found. What should you inspect?"
choices = ["The Deployment's container names", "The Deployment's labels", "The image repository and tag", "The ReplicaSet's selector"]
answer = 0
why = "The assignment uses CONTAINER=IMAGE. A Deployment called api may contain a container called nginx."

[[quiz]]
q = "You label a Deployment owner=learner. Are its existing Pods automatically relabeled?"
choices = [
  "Yes, labels always cascade",
  "Yes, after the next rollout",
  "No; Deployment metadata and Pod template metadata are different",
  "Only Pods created after the label is added",
]
answer = 2
why = "To affect Pods, edit spec.template.metadata.labels. Changing the template causes replacement Pods to be created."

[[quiz]]
q = "Why is force deletion risky?"
choices = [
  "It also deletes the Pod's PersistentVolumeClaims",
  "It also deletes the Pod's ReplicaSet",
  "It removes the API object without confirming process termination",
  "It bypasses RBAC checks on the delete",
]
answer = 2
why = "A process can outlive force deletion temporarily. Replacements can overlap with an old instance, which matters especially for shared or stateful work."

[lab]
summary = "Generate reusable YAML, create resources, and make targeted changes at exam speed."
setup = ["kubectl create namespace speed-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace speed-lab --ignore-not-found --wait=false 2>/dev/null || true", "rm -f web.yaml settings.yaml || true"]

[[lab.steps]]
title = "Generate two manifests"
task = "Save web.yaml describing Pod web in speed-lab with nginx:1.27-alpine and label app=web. Save settings.yaml describing ConfigMap settings in speed-lab with MODE=practice. Do not create either object yet."
hints = ["Use --dry-run=client -o yaml and shell redirection.", "The checks decode the files with kubectl's client dry-run."]
solution = '''
```bash
kubectl -n speed-lab run web --image=nginx:1.27-alpine --labels=app=web --dry-run=client -o yaml > web.yaml
kubectl -n speed-lab create configmap settings --from-literal=MODE=practice --dry-run=client -o yaml > settings.yaml
```

Client dry-run prints resource documents without persisting them. Including namespace metadata makes the saved artifacts unambiguous.
'''

[[lab.steps.checks]]
name = "saved Pod has the requested identity and image"
run = "kubectl create --dry-run=client -f web.yaml -o jsonpath='{.kind} {.metadata.namespace} {.metadata.name} {.spec.containers[0].image} {.metadata.labels.app}'"
equals = "Pod speed-lab web nginx:1.27-alpine web"

[[lab.steps.checks]]
name = "saved ConfigMap contains MODE"
run = "kubectl create --dry-run=client -f settings.yaml -o jsonpath='{.kind} {.metadata.namespace} {.metadata.name} {.data.MODE}'"
equals = "ConfigMap speed-lab settings practice"

[[lab.steps]]
title = "Apply and observe"
task = "Apply both files. Pod web must become Ready and ConfigMap settings must contain MODE=practice in speed-lab."
hints = ["Use kubectl apply -f once for each file.", "Wait for the Pod's Ready condition."]
solution = '''
```bash
kubectl apply -f settings.yaml
kubectl apply -f web.yaml
kubectl -n speed-lab wait --for=condition=Ready pod/web --timeout=120s
```

Applying creates the live objects from the saved specifications. Readiness is observed separately because starting a container takes time.
'''

[[lab.steps.checks]]
name = "live Pod is Ready"
run = '''kubectl -n speed-lab get pod web -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n speed-lab get pod web -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "live configuration has the expected value"
run = "kubectl -n speed-lab get configmap settings -o jsonpath='{.data.MODE}'"
equals = "practice"

[[lab.steps]]
title = "Generate a controller and expose it"
task = "Create Deployment api with nginx:1.27-alpine, one replica, and expose it with a ClusterIP Service named api on port 80 targeting port 80. Wait for rollout completion."
hints = ["create deployment makes a controller, while run makes a Pod.", "expose derives its selector from the Deployment."]
solution = '''
```bash
kubectl -n speed-lab create deployment api --image=nginx:1.27-alpine
kubectl -n speed-lab expose deployment api --name=api --port=80 --target-port=80
kubectl -n speed-lab rollout status deployment/api --timeout=120s
```

The Deployment maintains replicas, while the Service selects their Pod labels. Expose derives the initial selector from the controller.
'''

[[lab.steps.checks]]
name = "Deployment uses the requested image"
run = "kubectl -n speed-lab get deployment api -o jsonpath='{.spec.template.spec.containers[0].image} {.spec.replicas}'"
equals = "nginx:1.27-alpine 1"

[[lab.steps.checks]]
name = "Service selects api on port 80"
run = "kubectl -n speed-lab get service api -o jsonpath='{.spec.type} {.spec.selector.app} {.spec.ports[0].port} {.spec.ports[0].targetPort}'"
equals = "ClusterIP api 80 80"

[[lab.steps]]
title = "Tune an existing controller"
task = "Set MODE=exam on api's nginx container. Request 50m CPU and 32Mi memory, limit 200m CPU and 64Mi memory. Scale to two Ready replicas. Label the Deployment owner=learner and annotate it training=timed."
hints = ["kubectl set env and set resources update the Pod template.", "Use scale, label and annotate on the Deployment itself."]
solution = '''
```bash
kubectl -n speed-lab set env deployment/api MODE=exam
kubectl -n speed-lab set resources deployment/api --requests=cpu=50m,memory=32Mi --limits=cpu=200m,memory=64Mi
kubectl -n speed-lab scale deployment/api --replicas=2
kubectl -n speed-lab label deployment/api owner=learner --overwrite
kubectl -n speed-lab annotate deployment/api training=timed --overwrite
kubectl -n speed-lab rollout status deployment/api --timeout=120s
```

These commands change the controller's desired state. Template changes roll out replacement Pods; metadata on the Deployment remains distinct from Pod labels.
'''

[[lab.steps.checks]]
name = "template contains MODE=exam"
run = "kubectl -n speed-lab get deploy api -o jsonpath='{.spec.template.spec.containers[0].env[?(@.name==\"MODE\")].value}'"
equals = "exam"

[[lab.steps.checks]]
name = "requests and limits are set"
run = "kubectl -n speed-lab get deploy api -o jsonpath='{.spec.template.spec.containers[0].resources.requests.cpu} {.spec.template.spec.containers[0].resources.requests.memory} {.spec.template.spec.containers[0].resources.limits.cpu} {.spec.template.spec.containers[0].resources.limits.memory}'"
equals = "50m 32Mi 200m 64Mi"

[[lab.steps.checks]]
name = "desired and Ready replicas reach two"
run = '''kubectl -n speed-lab get deploy api -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n speed-lab get deploy api -o jsonpath='{.spec.replicas} {.status.readyReplicas}' 2>/dev/null); [ "$v" = "2 2" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "Deployment metadata records ownership and training"
run = "kubectl -n speed-lab get deploy api -o jsonpath='{.metadata.labels.owner} {.metadata.annotations.training}'"
equals = "learner timed"
+++

## Speed comes from a repeatable workflow

The CKAD rewards correct outcomes under a time limit. The fastest command is useful only when it targets the right resource and namespace. Build a small loop: read the requirement, generate a starting object, change the necessary fields, apply it, then inspect the result.

```text
requirement → generator → editable YAML → apply → observe
                  ↑                         |
                  └──── fix the cause ───────┘
```

Generators save typing; they do not decide the right workload or add every field a task requires. A Pod generator cannot infer your volume mounts, security context or readiness probe. Use generated YAML as a scaffold and the API schema as your guide.

## Learn a compact generator vocabulary

These commands create resources immediately unless you add dry-run output options. The examples all target this lesson's namespace. Do not run all of them blindly; choose the resource the task requests.

```bash
kubectl -n speed-lab run scratch --image=busybox:1.36 --restart=Never -- sleep 3600
kubectl -n speed-lab create deployment web --image=nginx:1.27-alpine --replicas=2
kubectl -n speed-lab create service clusterip web --tcp=80:80
kubectl -n speed-lab create job report --image=busybox:1.36 -- echo done
kubectl -n speed-lab create cronjob report --image=busybox:1.36 --schedule='*/5 * * * *' -- echo done
kubectl -n speed-lab create configmap settings --from-literal=MODE=practice
kubectl -n speed-lab create secret generic credentials --from-literal=sample=training-only
```

The Secret value above is dummy training data. Real credentials should not be typed into saved shell history or committed manifests. Base64 in a Secret manifest is encoding, not encryption.

A generated Service has selectors that you must verify against the target Pods. Generating a Deployment and Service with the same name commonly produces matching app labels, but different names or edited templates can break that assumption.

## Generate YAML before committing to an object

Append **--dry-run=client -o yaml** to print the object without saving it in the API. Shell redirection writes the result to a file:

```bash
kubectl -n speed-lab run web --image=nginx:1.27-alpine \
  --labels=app=web --port=80 --dry-run=client -o yaml > web.yaml
kubectl apply -f web.yaml
```

Client dry-run builds the object locally. It does not prove that admission policy permits it or that its image exists. Server dry-run exercises server validation and admission without persisting the result, but requires API access and permission.

The essential structure is short enough to recognize immediately:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: web
  namespace: speed-lab
  labels:
    app: web
spec:
  containers:
    - name: web
      image: nginx:1.27-alpine
      ports:
        - containerPort: 80
```

A containerPort is descriptive metadata. It does not create a Service, publish a node port, or make a non-listening application accept connections. Always distinguish object configuration from process behavior.

## Ask the schema instead of guessing

```bash
kubectl explain pod.spec.containers
kubectl explain pod.spec.containers.resources
kubectl explain deployment.spec.template --recursive
kubectl api-resources
```

Explain shows types, descriptions and nesting. Recursive output is helpful for discovering field paths, but can be very long. Start near the field you need. A Deployment wraps a Pod template under spec.template; its containers are therefore under spec.template.spec.containers. A bare Pod has containers directly under spec.

Case and indentation matter. Field names such as imagePullPolicy are exact. YAML indentation expresses structure; a valid field at the wrong nesting level is still an invalid object. Use spaces rather than tabs, and validate by asking the API to accept the manifest.

## Inspect only the information you need

| Output | Best use | Example |
|---|---|---|
| wide | Fast placement and address inspection | get pods -o wide |
| yaml | Full editable structure and defaults | get pod web -o yaml |
| jsonpath | One exact field or selected values | get pod web -o jsonpath=... |
| custom-columns | A readable comparison across objects | get pods -o custom-columns=... |

```bash
kubectl -n speed-lab get pods -l app=web -o wide
kubectl -n speed-lab get pod web -o jsonpath='{.spec.containers[0].image}'
kubectl -n speed-lab get pods -o custom-columns='NAME:.metadata.name,NODE:.spec.nodeName,PHASE:.status.phase'
kubectl -n speed-lab get pods -l 'app in (web,api),tier!=cache'
```

Selectors combine requirements with logical AND. Set membership is useful when several values are acceptable. A label query selects objects with matching metadata; it does not search their names or environment variables. JSONPath output does not automatically add a newline, so an empty result may look like a missing command.

## Change the right object

```bash
kubectl -n speed-lab set image deployment/api nginx=nginx:1.27-alpine
kubectl -n speed-lab set env deployment/api MODE=exam
kubectl -n speed-lab set resources deployment/api --requests=cpu=50m,memory=32Mi --limits=cpu=200m,memory=64Mi
kubectl -n speed-lab scale deployment/api --replicas=2
kubectl -n speed-lab expose deployment/api --name=api --port=80 --target-port=80
kubectl -n speed-lab label deployment/api owner=learner --overwrite
kubectl -n speed-lab annotate deployment/api training=timed --overwrite
```

Set image needs the **container name**, which may differ from the Deployment name. Set env and set resources update the workload's Pod template and can trigger rollouts. Scaling changes the desired replica count. Labeling the Deployment's metadata does not automatically label its Pods; change template metadata when a Pod selector must see a new label.

Forced deletion with **--force --grace-period=0** removes the API object without waiting for confirmation that its processes stopped. Use it only when a task and the situation justify that consequence. It is not a general speed trick for stateful applications or shared machines.

## Small shell and editor habits

On an exam shell that supports aliases, these conventions reduce repetition:

```bash
alias k=kubectl
export do='--dry-run=client -o yaml'
export now='--force --grace-period=0'
k -n speed-lab run draft --image=nginx:1.27-alpine $do > draft.yaml
```

These variables intentionally expand into separate arguments; this pattern assumes an ordinary shell, not every command launcher. PodDojo console commands may run in separate shells, so do not rely on an alias or export surviving the next command. The worked lab solutions use complete commands.

In vim, **:set expandtab shiftwidth=2 tabstop=2** helps produce YAML with two-space indentation. Paste carefully and inspect the surrounding structure. Interactive editing is useful in an exam, but saved heredocs make these labs reproducible.

## Keep files and live state aligned

After a quick imperative change, remember that an older saved manifest may still describe the previous state. Reapplying it can reverse your change. For a task that asks for both a file and a live workload, update both deliverables intentionally. Use one manifest as the source of truth once the initial scaffold exists.

A full live-object export includes server-managed fields and status. It is useful for inspection, but do not blindly copy every field into a new object. For recreation, keep the required metadata and specification, and remove identity fields such as UID and resourceVersion. A fresh generator is often safer than cleaning a large export under time pressure.

Read error messages as navigation hints. A validation error names an invalid field path; a conflict may mean a different writer changed the object; an immutable-field error means another patch will not solve the same problem. Choose the next operation from that evidence. Deletion and recreation are appropriate only when replacing that particular object is acceptable, and a controller's template is usually the right update target for managed Pods.

## Exam tips

Put the namespace in every command. Inspect container names before set image, check Service selectors after expose, and wait for rollout completion before declaring success. A generated file and a live object are separate deliverables: if a task asks for a file, save it; if it asks for a running workload, apply and verify it.
