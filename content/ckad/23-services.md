+++
id = "ckad-services"
title = "Services: stable discovery and exposure"
module = "networking"
minutes = 45
needs = "cluster"
objectives = ["Choose ClusterIP, NodePort, LoadBalancer, ExternalName or headless discovery", "Map Service ports to named container ports", "Diagnose selectors, EndpointSlices and readiness", "Test Service DNS and HTTP from a client Pod"]
domain = "networking"

[[quiz]]
q = "A ClusterIP Service exposes port 8080 with targetPort http; its Pods define http as port 80. Where does a client connect through the Service?"
choices = ["The Pod IP on port 8080", "The Service IP on port 8080", "The node IP on port 8080", "The Service IP on port 80"]
answer = 1
why = "Clients use the Service port. The named targetPort resolves independently against each selected Pod, allowing a backend to listen on a different port."

[[quiz]]
q = "A selector matches running Pods but EndpointSlices have no ready backend. What should you inspect next?"
choices = ["The Service's sessionAffinity", "Pod readiness and named port resolution", "kube-proxy's iptables rules", "The Service type"]
answer = 1
why = "Running is a Pod phase, not proof of readiness. An unready Pod is normally excluded from usable Service backends; unresolved named ports can also prevent endpoints from being usable."

[[quiz]]
q = "What does a headless Service with a selector normally return through DNS?"
choices = ["One stable virtual IP", "The node IP only", "Addresses of its eligible backend Pods", "A CNAME to the Service's cluster IP"]
answer = 2
why = "clusterIP: None removes the virtual IP. DNS exposes backend addresses, so the client handles endpoint choice and connections."

[[quiz]]
q = "A LoadBalancer Service stays pending in this kind cluster. What does this mean?"
choices = ["The selector is necessarily wrong", "No load-balancer implementation has assigned an external address", "Its NodePorts were not allocated", "The selector matches no ready Pods yet"]
answer = 1
why = "A LoadBalancer Service needs an implementation to provision or advertise an external address. Pending is expected in this lab and does not by itself diagnose backend health."

[[quiz]]
q = "What does an ExternalName Service provide?"
choices = ["A DNS CNAME alias", "A proxy that rewrites HTTP Host headers", "A selected set of local Pods", "A reserved node port"]
answer = 0
why = "ExternalName creates a DNS alias to spec.externalName. It does not proxy traffic, choose Pods, or adapt HTTP hostnames and TLS certificate names."

[lab]
summary = "Connect stable names to healthy backends, repair broken routing and publish the assigned NodePort."
setup = ["kubectl create namespace services-lab --dry-run=client -o yaml | kubectl apply -f -", "kubectl apply -f - <<'EOF'\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n  namespace: services-lab\nspec:\n  replicas: 2\n  selector:\n    matchLabels: {app: web}\n  template:\n    metadata:\n      labels: {\"app\": \"web\"}\n    spec:\n      containers:\n      - name: web\n        image: nginx:1.27-alpine\n        ports:\n        - name: http\n          containerPort: 80\n        readinessProbe:\n          httpGet: {path: /, port: http}\n          periodSeconds: 2\nEOF\nkubectl -n services-lab rollout status deployment/web --timeout=120s", "kubectl -n services-lab run client --image=busybox:1.36 --restart=Never --labels=role=frontend --dry-run=client -o yaml --command -- sleep 14400 | kubectl apply -f -\nkubectl -n services-lab wait --for=condition=Ready pod/client --timeout=120s", "kubectl apply -f - <<'EOF'\napiVersion: v1\nkind: Service\nmetadata:\n  name: broken\n  namespace: services-lab\nspec:\n  type: ClusterIP\n  selector: {app: missing}\n  ports:\n  - name: http\n    port: 80\n    targetPort: wrong\nEOF"]
cleanup = ["kubectl delete namespace services-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Give the web Pods a stable address"
task = "In services-lab create ClusterIP Service web selecting app=web, with TCP port 80 and targetPort http. Confirm HTTP from the prepared busybox:1.36 client."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: services-lab
spec:
  type: ClusterIP
  selector: {app: web}
  ports:
  - name: http
    port: 80
    targetPort: http
EOF
```'''
hints = ["Use kubectl expose deployment web --port=80 --target-port=http.", "The client is already running: kubectl -n services-lab exec client -- wget -qO- -T 2 http://web."]

[[lab.steps.checks]]
name = "web maps port 80 to the named backend port"
run = '''kubectl -n services-lab get service web -o jsonpath='{.spec.type}{" "}{.spec.selector.app}{" "}{.spec.ports[0].port}{" "}{.spec.ports[0].targetPort}''''
equals = "ClusterIP web 80 http"

[[lab.steps.checks]]
name = "HTTP works through Service DNS"
run = '''for i in $(seq 1 30); do v=$(kubectl -n services-lab exec client -- wget -qO- -T 2 http://web.services-lab.svc.cluster.local); echo "$v" | grep -q "Welcome to nginx" && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 100

[[lab.steps]]
title = "Repair a Service with no usable backends"
task = "The existing broken Service must serve the same app=web Pods on port 80, forwarding to their named port http. Repair its selector and targetPort; leave the Deployment unchanged."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata:
  name: broken
  namespace: services-lab
spec:
  type: ClusterIP
  selector: {app: web}
  ports:
  - name: http
    port: 80
    targetPort: http
EOF
```'''
hints = ["Compare kubectl get pods --show-labels with the selector.", "Then compare targetPort with the named container ports; endpoints alone do not prove a listening application."]

[[lab.steps.checks]]
name = "broken has a ready EndpointSlice backend"
run = '''for i in $(seq 1 30); do v=$(kubectl -n services-lab get endpointslices -l kubernetes.io/service-name=broken -o jsonpath='{range .items[*].endpoints[?(@.conditions.ready==true)]}{.addresses[0]}{end}'); [ -n "$v" ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps.checks]]
name = "the repaired Service serves HTTP"
run = '''for i in $(seq 1 30); do v=$(kubectl -n services-lab exec client -- wget -qO- -T 2 http://broken); echo "$v" | grep -q "Welcome to nginx" && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 100

[[lab.steps]]
title = "Publish the assigned NodePort"
task = "Create NodePort Service public-web selecting app=web, port 80, targetPort http and nodePort 30080. Retrieve its page at http://$PODDOJO_HOST:30080."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata:
  name: public-web
  namespace: services-lab
spec:
  type: NodePort
  selector: {app: web}
  ports:
  - name: http
    port: 80
    targetPort: http
    nodePort: 30080
EOF
```'''
hints = ["nodePort belongs in the same ports entry as port and targetPort.", "30080 is already published by the lab cluster; do not change Docker networking."]

[[lab.steps.checks]]
name = "the assigned node port and backend are correct"
run = '''kubectl -n services-lab get service public-web -o jsonpath='{.spec.type}{" "}{.spec.selector.app}{" "}{.spec.ports[0].port}{" "}{.spec.ports[0].targetPort}{" "}{.spec.ports[0].nodePort}''''
equals = "NodePort web 80 http 30080"

[[lab.steps.checks]]
name = "the published machine port serves HTTP"
run = '''for i in $(seq 1 30); do v=$(curl -fsS --max-time 3 "http://$PODDOJO_HOST:30080"); echo "$v" | grep -q "Welcome to nginx" && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 120

[[lab.steps]]
title = "Discover Pods without a virtual IP"
task = "Create headless Service web-headless selecting app=web, port 80 and targetPort http. Resolve web-headless.services-lab.svc.cluster.local from client."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata:
  name: web-headless
  namespace: services-lab
spec:
  type: ClusterIP
  selector: {app: web}
  ports:
  - name: http
    port: 80
    targetPort: http
  clusterIP: None
EOF
```'''
hints = ["clusterIP: None is the literal string None.", "A headless Service still needs selectors and ready endpoints for this DNS behavior."]

[[lab.steps.checks]]
name = "the headless selector and port are configured"
run = '''kubectl -n services-lab get service web-headless -o jsonpath='{.spec.clusterIP}{" "}{.spec.selector.app}{" "}{.spec.ports[0].port}{" "}{.spec.ports[0].targetPort}''''
equals = "None web 80 http"

[[lab.steps.checks]]
name = "headless DNS resolves from the client"
run = '''kubectl -n services-lab get service web-headless >/dev/null && for i in $(seq 1 30); do v=$(kubectl -n services-lab exec client -- nslookup web-headless.services-lab.svc.cluster.local); echo "$v" | grep -q "Name:" && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Describe two different external integrations"
task = "Create ExternalName Service docs-alias pointing to kubernetes.io. Also create LoadBalancer Service cloud-web selecting app=web with port 80 and targetPort http. An external address is not required in kind."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata: {name: docs-alias, namespace: services-lab}
spec:
  type: ExternalName
  externalName: kubernetes.io
EOF
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Service
metadata:
  name: cloud-web
  namespace: services-lab
spec:
  type: LoadBalancer
  selector: {app: web}
  ports:
  - name: http
    port: 80
    targetPort: http
EOF
```'''
hints = ["ExternalName needs externalName, not a selector.", "Pending is expected without a load-balancer controller; do not install one."]

[[lab.steps.checks]]
name = "the external DNS alias is correct"
run = '''kubectl -n services-lab get service docs-alias -o jsonpath='{.spec.type}{" "}{.spec.externalName}''''
equals = "ExternalName kubernetes.io"

[[lab.steps.checks]]
name = "the load balancer exposes the correct backend"
run = '''kubectl -n services-lab get service cloud-web -o jsonpath='{.spec.type}{" "}{.spec.selector.app}{" "}{.spec.ports[0].port}{" "}{.spec.ports[0].targetPort}''''
equals = "LoadBalancer web 80 http"
+++

## A stable name for changing processes

A Deployment replaces Pods during rollouts, rescheduling and scaling. Each replacement can have a different IP. Putting those IPs in application configuration makes every rollout a client migration. A Service gives callers a durable name and, usually, a virtual IP while Kubernetes continually updates the set of destinations behind it.

A Service is not a process listening inside your application container. It is an API object that describes discovery and forwarding. A cluster networking implementation programs the data plane. The application still must listen on the destination port and accept the request.

```text
client -> web.services-lab.svc.cluster.local:80
       -> Service virtual IP:80
       -> selected, ready Pod IP:80
```

Selection is based on **Pod labels**, not Deployment names. A Service named web can select Pods named anything. Conversely, naming a Deployment web does not automatically make every Service named web select it. Inspect the selector and the actual Pod labels whenever the wiring is unclear.

## Three ports with different jobs

| Field | Who uses it | Meaning |
|---|---|---|
| port | Client inside the cluster | Port on the Service address |
| targetPort | Forwarding implementation | Destination port on a selected Pod |
| nodePort | Client reaching a node | Externally reachable port reserved on nodes |

The port numbers can all differ. A Service with port 8080 and targetPort 80 accepts connections on its own address at 8080 and forwards to port 80 on a Pod. Merely declaring containerPort does not open a socket or reconfigure nginx. It documents an application port and enables named port references.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: services-lab
spec:
  selector:
    app: web
  ports:
  - name: http
    port: 8080
    targetPort: http
```

The selected Pod declares `ports: [{name: http, containerPort: 80}]`. Using targetPort http decouples the Service from the backend's numeric port. Different selected Pods can resolve that name to different numbers during an application migration. Name the port consistently in every Pod template. The Service port name and the container port name are separate fields, even when both happen to be http.

If omitted, `targetPort` defaults to the Service's `port`, and `protocol` defaults to TCP. A Service exposing several ports requires a unique name on each entry. A headless Service is still type ClusterIP; `Headless` is not a valid `spec.type` value.

## Choose the right Service shape

| Shape | Discovery and reachability | Typical use |
|---|---|---|
| ClusterIP | Internal virtual IP and DNS | Application-to-application traffic |
| NodePort | ClusterIP plus a port on node addresses | Explicit node-level entry point |
| LoadBalancer | External address supplied by an implementation | Managed external exposure |
| ExternalName | DNS CNAME to another hostname | A discovery alias |
| Headless | clusterIP: None; DNS returns backend addresses | Direct endpoint discovery |

ClusterIP is the default. NodePort normally allocates from 30000–32767, although administrators can change that range. A LoadBalancer Service normally also allocates node ports, but implementations can support disabling that allocation. It does not mean Kubernetes itself contains a universal cloud load balancer.

In PodDojo, NodePort **30080** is published on the chosen Docker machine. Use `http://$PODDOJO_HOST:30080`, including when the machine is reached over SSH. A random node port is not automatically published through kind's container boundary. Avoid choosing another number and expecting the same host access.

LoadBalancer will normally show a pending external address in this lab. Its ClusterIP can still work. ExternalName is different: DNS returns an alias, and no Service proxy redirects traffic. An HTTP client may send the alias as its Host header, or validate TLS against it, while the upstream expects another hostname. DNS indirection does not solve that mismatch.

A headless Service has no virtual IP and does not provide normal Service load balancing. With a selector, eligible Pod addresses are published for clients to choose. Set `spec.clusterIP: None` when creating it; changing an allocated ClusterIP to None is rejected as an immutable-field change. Stateful workloads often build stable discovery on headless Services.

## Create quickly, then inspect

```bash
kubectl -n services-lab expose deployment web --port=80 --target-port=http
kubectl -n services-lab expose deployment web --name=preview --port=80 --target-port=http --dry-run=client -o yaml
kubectl -n services-lab get services -o wide
kubectl -n services-lab get pods --show-labels
kubectl explain service.spec.ports
```

`kubectl expose` derives a selector from the workload. Inspect generated YAML before using it for a workload with complex labels. Generating a skeleton saves typing; it does not remove the need to check port semantics.

The usual DNS name is `service.namespace.svc.cluster.local` in a cluster using the default cluster domain. Inside the same namespace, `web` is enough. From another namespace, prefer `web.services-lab` or the fully qualified name. `cluster.local` is configurable, so do not assume every production cluster uses it. DNS resolves a destination; it does not prove the application is reachable.

## Follow the request through EndpointSlices

EndpointSlices record backend addresses, ports and conditions. Prefer inspecting them over relying on the legacy Endpoints API. One Service may have several slices; inspect all slices selected by its Service label.

```bash
kubectl -n services-lab get endpointslices -l kubernetes.io/service-name=web -o yaml
kubectl -n services-lab describe service web
kubectl -n services-lab describe pod client
kubectl -n services-lab exec client -- wget -qO- -T 2 http://web
```

Use a persistent `busybox:1.36` test Pod so every probe starts from the same network location. A successful request from your laptop is not equivalent to one from an application namespace. The network path, DNS configuration and NetworkPolicies can differ.

When there are no usable endpoints, work in order. First compare selector key/value pairs against Pod labels in the **same namespace**; Services do not select Pods across namespaces. Next inspect readiness: Running Pods can still be unready. Finally verify named port resolution and the actual listening port. A wrong numeric targetPort can leave apparently healthy endpoints while every connection fails. A wrong named targetPort may prevent usable endpoint port information altogether.

Normally only ready backends receive traffic. `publishNotReadyAddresses: true` deliberately overrides that readiness filtering, often for peer discovery; it does not make an unhealthy application ready. The lab uses ordinary readiness filtering. Resolve its headless name with `kubectl -n services-lab exec client -- nslookup web-headless.services-lab.svc.cluster.local`, then compare the returned addresses with the backend Pod IPs.

If endpoints and ports are correct, test the Pod IP directly from the client. Then compare Service-IP access with DNS-name access. A failure only by name points toward DNS; a failure on both paths may involve policy or forwarding. An HTTP 404 means a server answered, which is different from a connection timeout. Read the failure before changing resources.

## Exam tips

Write down the caller's namespace, Service port and required exposure type before editing. Check selectors, EndpointSlices and one real request after every repair. Preserve existing labels unless the task explicitly asks to change them: fixing a selector is usually safer than relabeling an entire application. Allow a short convergence interval after changing selectors or readiness. The [official Service documentation](https://kubernetes.io/docs/concepts/services-networking/service/) is the reference for less common fields.
