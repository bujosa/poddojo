+++
id = "ckad-images-for-kubernetes"
title = "Build and deliver images to Kubernetes"
module = "design-build"
minutes = 45
needs = "cluster"
objectives = [
  "Build versioned Docker images and load them into kind",
  "Choose image pull policies deliberately",
  "Distinguish image tags from digests",
  "Roll out a new image and diagnose pull failures",
]
domain = "design-build"

[[quiz]]
q = "Why can a kind Pod fail to find an image built by Docker?"
choices = [
  "containerd cannot run images built by Docker",
  "The node runtime has a separate image store",
  "kind nodes can only pull from Docker Hub",
  "Local images must use the latest tag",
]
answer = 1
why = "Docker runs the kind nodes, while containerd inside them runs Pods. Load the image into the named kind cluster or publish it to a reachable registry."

[[quiz]]
q = "Which policy fits a locally loaded dojo/greeter:1.0 image?"
choices = ["Always, because it avoids the registry", "IfNotPresent", "Unset, because a versioned tag defaults to Always", "Always, with an imagePullSecret for the local store"]
answer = 1
why = "IfNotPresent can use the node's loaded image. Always performs registry resolution and may fail for a tag that only exists locally."

[[quiz]]
q = "You change an existing Deployment's image tag. Does its defaulted pull policy automatically change?"
choices = ["Yes, every time", "Yes, if the new tag is latest", "No; set the policy explicitly when needed", "Yes, the API server recomputes defaults on every update"]
answer = 2
why = "Pull policy defaulting happens at creation. Editing the image reference later does not recompute an already recorded policy."

[[quiz]]
q = "What does an image digest provide that a mutable tag does not?"
choices = ["A signature proving who built the image", "Registry authentication", "Content identity", "Automatic updates when the image is rebuilt"]
answer = 2
why = "A digest identifies content. A tag can be reassigned, so the same text can resolve to different bytes over time."

[lab]
summary = "Build, load and release a local image, then repair an existing workload with a bad image reference."
setup = [
  "kubectl create namespace images-lab --dry-run=client -o yaml | kubectl apply -f -",
  '''
kubectl apply -f - <<'EOF'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: broken
  namespace: images-lab
spec:
  replicas: 1
  selector:
    matchLabels:
      app: broken
  template:
    metadata:
      labels:
        app: broken
    spec:
      containers:
        - name: web
          image: dojo/greeter:missing
          imagePullPolicy: IfNotPresent
EOF
''',
]
cleanup = [
  "kubectl delete namespace images-lab --ignore-not-found --wait=false 2>/dev/null || true",
  "docker image rm dojo/greeter:1.0 dojo/greeter:1.1 2>/dev/null || true",
]

[lab.files]
"app/Dockerfile" = '''
FROM nginx:1.27-alpine
ARG VERSION=1.0
LABEL io.poddojo.version=$VERSION
COPY greeting.txt /usr/share/nginx/html/index.html
'''
"app/greeting.txt" = '''
hello from 1.0
'''

[[lab.steps]]
title = "Build and load version 1.0"
task = "Build dojo/greeter:1.0 from app with build argument VERSION=1.0, then load it into kind cluster poddojo. Keep the image for the next step."
hints = ["The Dockerfile and greeting.txt are already supplied in app.", "Use kind load docker-image with --name poddojo."]
solution = '''
```bash
docker build -t dojo/greeter:1.0 --build-arg VERSION=1.0 app
kind load docker-image dojo/greeter:1.0 --name poddojo
```

The build produces a versioned local image. Loading copies it into kind's node image stores so kubelets can use it without a registry copy.
'''

[[lab.steps.checks]]
name = "version 1.0 is built and labeled"
run = "docker image inspect dojo/greeter:1.0 --format '{{index .Config.Labels \"io.poddojo.version\"}}'"
equals = "1.0"

[[lab.steps]]
title = "Run the loaded image"
task = "Create Deployment greeter in images-lab with one replica, container web, image dojo/greeter:1.0 and imagePullPolicy IfNotPresent. Label its Pods app=greeter. Wait for rollout completion; the web index must contain hello from 1.0."
hints = [
  "A Deployment wraps the Pod spec under spec.template.spec.",
  "Inspect /usr/share/nginx/html/index.html in the running web container.",
]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: apps/v1
kind: Deployment
metadata:
  name: greeter
  namespace: images-lab
spec:
  replicas: 1
  selector:
    matchLabels:
      app: greeter
  template:
    metadata:
      labels:
        app: greeter
    spec:
      containers:
        - name: web
          image: dojo/greeter:1.0
          imagePullPolicy: IfNotPresent
EOF
kubectl -n images-lab rollout status deployment/greeter --timeout=120s
```

The explicit pull policy permits use of the loaded image. The content check verifies the application bytes rather than only an image reference.
'''

[[lab.steps.checks]]
name = "template uses the loaded version and policy"
run = "kubectl -n images-lab get deploy greeter -o jsonpath='{.spec.template.spec.containers[?(@.name==\"web\")].image} {.spec.template.spec.containers[?(@.name==\"web\")].imagePullPolicy}'"
equals = "dojo/greeter:1.0 IfNotPresent"

[[lab.steps.checks]]
name = "version 1.0 content is present"
run = '''kubectl -n images-lab get deployment/greeter -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n images-lab exec deployment/greeter -c web -- cat /usr/share/nginx/html/index.html 2>/dev/null); [ "$v" = "hello from 1.0" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Build and roll out version 1.1"
task = "Change app/greeting.txt to hello from 1.1, build dojo/greeter:1.1 with VERSION=1.1, and load it into poddojo. Update greeter's web container to 1.1. Wait for the rollout and verify the new content."
hints = ["Use a new tag rather than overwriting 1.0.", "kubectl set image expects the container name web."]
solution = '''
```bash
printf '%s\n' 'hello from 1.1' > app/greeting.txt
docker build -t dojo/greeter:1.1 --build-arg VERSION=1.1 app
kind load docker-image dojo/greeter:1.1 --name poddojo
kubectl -n images-lab set image deployment/greeter web=dojo/greeter:1.1
kubectl -n images-lab rollout status deployment/greeter --timeout=120s
```

A new tag makes the release visible in the template and avoids silently relying on a cache entry for the previous version.
'''

[[lab.steps.checks]]
name = "version 1.1 image is built"
run = "docker image inspect dojo/greeter:1.1 --format '{{index .Config.Labels \"io.poddojo.version\"}}'"
equals = "1.1"

[[lab.steps.checks]]
name = "template points to version 1.1"
run = "kubectl -n images-lab get deploy greeter -o jsonpath='{.spec.template.spec.containers[?(@.name==\"web\")].image}'"
equals = "dojo/greeter:1.1"

[[lab.steps.checks]]
name = "new application content reaches a running Pod"
run = '''kubectl -n images-lab get deployment/greeter -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n images-lab exec deployment/greeter -c web -- cat /usr/share/nginx/html/index.html 2>/dev/null); [ "$v" = "hello from 1.1" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Repair an image reference"
task = "Preparation created Deployment broken with a nonexistent image tag. Inspect its Pods and events, then change its web container to dojo/greeter:1.1, retaining IfNotPresent. It must reach one available replica and contain hello from 1.1."
hints = ["describe pod shows the image-pull failure reason.", "Fix the Deployment template rather than editing a managed Pod."]
solution = '''
```bash
kubectl -n images-lab get pods -l app=broken
kubectl -n images-lab describe pods -l app=broken
kubectl -n images-lab set image deployment/broken web=dojo/greeter:1.1
kubectl -n images-lab rollout status deployment/broken --timeout=120s
```

Fixing the Deployment template creates replacement Pods with a usable reference. The controller, not an individual managed Pod, owns the desired image.
'''

[[lab.steps.checks]]
name = "bad image reference is repaired"
run = "kubectl -n images-lab get deploy broken -o jsonpath='{.spec.template.spec.containers[?(@.name==\"web\")].image} {.spec.template.spec.containers[?(@.name==\"web\")].imagePullPolicy}'"
equals = "dojo/greeter:1.1 IfNotPresent"

[[lab.steps.checks]]
name = "repaired Deployment becomes available"
run = '''kubectl -n images-lab get deploy broken -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n images-lab get deploy broken -o jsonpath='{.status.availableReplicas}' 2>/dev/null); [ "$v" = "1" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "repaired workload has the expected content"
run = '''kubectl -n images-lab get deployment/broken -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n images-lab exec deployment/broken -c web -- cat /usr/share/nginx/html/index.html 2>/dev/null); [ "$v" = "hello from 1.1" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"
+++

## Three image stores, one application

A Docker image packages application files and runtime defaults. Kubernetes references that image in a Pod specification, but the kubelet asks the **node's container runtime** to obtain it. A local Docker build is not automatically visible to that runtime.

In PodDojo, Docker runs the kind node containers. Each node runs containerd internally and has its own image store. Loading an image into kind bridges those stores:

```text
lab files → docker build → selected Docker engine
                                 |
                      kind load docker-image
                                 ↓
                control-plane store + worker store
                                 |
                          kubelet starts Pod
```

This distinction applies even when everything physically runs on one laptop. On a remote Docker context, the selected engine is on that server; PodDojo supplies the environment so commands consistently target the lab machine.

## Build a small, inspectable application

The lab supplies an app directory with a Dockerfile and greeting.txt. Its web server returns a static versioned greeting. The base image has an explicit tag rather than an unspecified moving default.

```dockerfile
FROM nginx:1.27-alpine
ARG VERSION=1.0
LABEL io.poddojo.version=$VERSION
COPY greeting.txt /usr/share/nginx/html/index.html
```

```bash
docker build -t dojo/greeter:1.0 --build-arg VERSION=1.0 app
kind load docker-image dojo/greeter:1.0 --name poddojo
```

The final argument to build is the **build context**, not the output image name. COPY sources are resolved relative to that context. A changed greeting invalidates the relevant build layer; there is no need to disable all caching. Keep credentials and unnecessary files out of the context.

The [kind quick start](https://kind.sigs.k8s.io/docs/user/quick-start/#loading-an-image-into-your-cluster) documents loading locally built images and choosing an appropriate pull policy. In this lab, load each new version before asking Kubernetes to start it.

## Specify image and pull policy together

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: greeter
  namespace: images-lab
spec:
  replicas: 1
  selector:
    matchLabels:
      app: greeter
  template:
    metadata:
      labels:
        app: greeter
    spec:
      containers:
        - name: web
          image: dojo/greeter:1.0
          imagePullPolicy: IfNotPresent
          ports:
            - containerPort: 80
```

IfNotPresent allows the node to use the image loaded into its store. Without an explicit registry hostname, an unavailable image may be looked up in the default public registry; the fact that its name starts with dojo does not make that namespace a registry service.

| Policy | Node behavior | Useful situation |
|---|---|---|
| IfNotPresent | Uses a local image if available; otherwise pulls | Loaded development images, immutable version tags |
| Always | Resolves the image through the registry on each container start; cached layers can still be reused | Registry-backed mutable tags |
| Never | Uses local images only and fails if absent | Strictly preloaded environments |

Always does not mean downloading every layer every time. A runtime can reuse cached content matching the resolved digest. It still needs the relevant registry interaction, so Always is a poor match for a local-only tag with no registry copy.

If omitted at creation, a non-latest explicit tag normally defaults to IfNotPresent. An omitted tag or the special latest tag normally defaults to Always; a digest reference defaults to IfNotPresent. The default is recorded when the object is created and does not automatically change just because you later edit its image reference. Specify the policy when the task depends on it. These details are described in the [image documentation](https://kubernetes.io/docs/concepts/containers/images/).

## Tags identify a name; digests identify content

A tag is a registry reference that can be moved to different image content. A digest is a content-addressed identifier. Reusing the same tag for different builds can leave different nodes running different cached bytes, especially with IfNotPresent.

```yaml
# Shape only: replace DIGEST with the real published SHA-256 value.
image: registry.example.test/team/greeter@sha256:DIGEST
```

The example is deliberately a placeholder, not an image to run. Obtain a real digest from the registry or tooling and copy it exactly. When a reference contains both a tag and a digest, the digest selects the content. Version tags make human communication easier; digests make content identity stronger.

Do not put a Docker image's local image ID into a Pod spec as though it were automatically a registry manifest digest. Multi-platform indexes and individual platform manifests can have different digests. Use an actual pullable registry reference when deploying by digest.

## Release a new version rather than replacing history

Change the greeting, build dojo/greeter:1.1, and load that version. Then update the Deployment's template:

```bash
kubectl -n images-lab set image deployment/greeter web=dojo/greeter:1.1
kubectl -n images-lab rollout status deployment/greeter --timeout=120s
kubectl -n images-lab get pods -l app=greeter -o wide
```

Changing the image triggers a Deployment rollout. Pushing new bytes to a tag does not itself change the Pod template or restart running containers. A new tag makes the intended change explicit and avoids relying on a stale local cache.

Readiness is necessary but does not prove the release contains the right application bytes. In this lab, inspect the served index file in a new container. In a production application, a version endpoint or a real request is a better behavioral signal than image metadata alone.

## Diagnose ImagePullBackOff systematically

ImagePullBackOff means the runtime could not obtain the image and Kubernetes is delaying retries. It is a symptom, not a Pod phase or a single root cause.

```bash
kubectl -n images-lab describe pod POD_NAME
kubectl -n images-lab get events --sort-by=.metadata.creationTimestamp
kubectl -n images-lab get deployment broken -o yaml
```

Read the event message. Common causes are a misspelled repository or tag, missing registry credentials, network failure, and requesting an image platform unavailable for the node. Locally loaded images can also fail when policy forces a registry lookup or the image was not loaded on the scheduled node.

For private registries, imagePullSecrets references a Secret of an appropriate registry credential type in the **same namespace** as the Pod. Place the reference under Pod spec, or configure the ServiceAccount so new Pods receive it. Credentials in an application's env variables do not authenticate kubelet image pulls. Never paste real credentials into lesson files.

## Verify the execution platform too

An image must support the node's operating system and CPU architecture. Building against the selected Docker engine usually produces a compatible platform for that engine, but copying an image from another machine can introduce an architecture mismatch. Inspect the build output and runtime events rather than assuming every tag supports every platform.

A registry can contain a multi-platform index that points to separate manifests. That is why the same human-readable tag can work across machines while individual platform images differ. For this lab, build on the selected engine and load the resulting image into its own kind cluster.

## Exam tips

Check spelling, pull policy, node availability and events before changing unrelated fields. Fix a controller's template rather than one of its Pods. Wait for the rollout and verify application content. Distinguish “Docker built it,” “kind loaded it,” and “the new Pod ran it”: each is a separate milestone.
