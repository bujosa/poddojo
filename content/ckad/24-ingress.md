+++
id = "ckad-ingress"
title = "Ingress: hosts, paths and TLS"
module = "networking"
minutes = 40
needs = "cluster"
requires = ["ingress-controller"]
objectives = ["Distinguish an Ingress resource from its controller and class", "Build host and path rules using the v1 API", "Configure a default backend and TLS Secret reference", "Explain controller-specific behavior and the Gateway API model"]
domain = "networking"

[[quiz]]
q = "What makes an Ingress resource forward HTTP traffic?"
choices = ["Creating the resource alone", "A running compatible controller that handles its class", "A LoadBalancer Service for each backend", "The TLS Secret alone"]
answer = 1
why = "The API stores the desired route. A controller watches the class and configures a data plane; object creation does not install that controller."

[[quiz]]
q = "An Ingress backend references port 8080. What must that number identify?"
choices = ["A nodePort", "The backend Service port", "The container port regardless of the Service", "An external load-balancer port"]
answer = 1
why = "Ingress backends reference a Service in the same namespace and one of its exposed ports, by number or name. That Service maps onward to targetPort."

[[quiz]]
q = "Which request matches Prefix /tea but not Exact /tea?"
choices = ["/tea", "/tea/cup", "/teapot", "/Tea"]
answer = 1
why = "Prefix matches complete slash-separated path elements. It includes /tea/cup, while /teapot is not a match. Both types are case-sensitive, so /Tea matches neither; Exact requires the complete path."

[[quiz]]
q = "What does a TLS section with secretName guarantee without a controller?"
choices = ["A public certificate is issued", "A DNS record is created", "The desired TLS configuration is stored", "Every backend now speaks HTTPS"]
answer = 2
why = "The TLS section references a Secret. Certificate issuance, listener configuration and public reachability require additional components; normal Ingress TLS termination does not imply encrypted backend connections."

[[quiz]]
q = "How do portable Ingress annotations behave?"
choices = ["All controllers implement the same rewrite keys", "Annotations are controller-specific extensions", "Unknown annotations make the API reject the Ingress", "They are standardized under the networking.k8s.io prefix"]
answer = 1
why = "The core API standardizes hosts, paths and backends. Rewrite, authentication and timeout annotations depend on the chosen controller and should not be copied across implementations blindly."

[lab]
summary = "Route real HTTP and HTTPS traffic through the lab's Traefik controller: a class, host and path rules to distinct backends, a default backend and TLS."
setup = [
'''kubectl create namespace ingress-lab --dry-run=client -o yaml | kubectl apply -f -''',
'''kubectl apply -f - <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: echo
  namespace: ingress-lab
data:
  default.conf.template: |
    server {
      listen 80;
      location / { default_type text/plain; return 200 "${BACKEND} $request_uri\n"; }
    }
EOF''',
'''for app in web status fallback; do
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: $app
  namespace: ingress-lab
spec:
  replicas: 1
  selector:
    matchLabels: {app: $app}
  template:
    metadata:
      labels: {app: $app}
    spec:
      containers:
      - name: echo
        image: nginx:1.27-alpine
        env:
        - {name: BACKEND, value: $app}
        ports:
        - {name: http, containerPort: 80}
        readinessProbe:
          httpGet: {path: /, port: http}
          periodSeconds: 2
        volumeMounts:
        - {name: templates, mountPath: /etc/nginx/templates}
      volumes:
      - name: templates
        configMap: {name: echo}
---
apiVersion: v1
kind: Service
metadata:
  name: $app
  namespace: ingress-lab
spec:
  selector: {app: $app}
  ports:
  - {name: http, port: 80, targetPort: http}
EOF
done
kubectl -n ingress-lab rollout status deployment/web --timeout=120s
kubectl -n ingress-lab rollout status deployment/status --timeout=120s
kubectl -n ingress-lab rollout status deployment/fallback --timeout=120s'''
]
cleanup = ["kubectl delete namespace ingress-lab --ignore-not-found --wait=false || true", "kubectl delete ingressclass dojo-traefik --ignore-not-found || true", "rm -rf shop.key shop.crt shop.cnf wrong || true"]

[[lab.steps]]
title = "Declare who handles these routes"
task = "The lab cluster runs a Traefik Ingress controller (namespace dojo-ingress) that serves every IngressClass whose controller is traefik.io/ingress-controller. Create cluster-scoped IngressClass dojo-traefik with spec.controller traefik.io/ingress-controller. Do not mark it as the cluster default."
solution = '''```bash
kubectl get ingressclass
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: dojo-traefik
spec:
  controller: traefik.io/ingress-controller
EOF
```'''
hints = ["IngressClass is cluster-scoped; `kubectl get ingressclass traefik -o yaml` shows the shape.", "Its controller string identifies an implementation; it is not an image name."]

[[lab.steps.checks]]
name = "the class names the Traefik controller"
run = '''kubectl get ingressclass dojo-traefik -o jsonpath='{.spec.controller}''''
equals = "traefik.io/ingress-controller"

[[lab.steps.checks]]
name = "the class is not the cluster default"
run = '''kubectl get ingressclass dojo-traefik -o name >/dev/null && kubectl get ingressclass dojo-traefik -o jsonpath='{.metadata.annotations.ingressclass\.kubernetes\.io/is-default-class}' && echo checked'''
equals = "checked"

[[lab.steps.negatives]]
name = "points the class at another controller"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: dojo-traefik
spec:
  controller: k8s.io/ingress-nginx
EOF
'''
undo = '''kubectl delete ingressclass dojo-traefik --ignore-not-found'''

[[lab.steps.negatives]]
name = "marks the class as the cluster default"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: dojo-traefik
  annotations:
    ingressclass.kubernetes.io/is-default-class: "true"
spec:
  controller: traefik.io/ingress-controller
EOF
'''
undo = '''kubectl delete ingressclass dojo-traefik --ignore-not-found'''

[[lab.steps]]
title = "Route a host and a path prefix"
task = "In ingress-lab create Ingress shop using class dojo-traefik. Route host shop.dojo.test and Prefix path /shop to Service web, Service port 80. Then send a request through the controller: `kubectl -n dojo-ingress exec deploy/probe -- curl -s -H 'Host: shop.dojo.test' http://traefik.dojo-ingress/shop/cart` must answer `web /shop/cart`. Other hosts, and paths such as /shopping, must not reach web."
solution = '''```bash
kubectl -n ingress-lab create ingress shop --class=dojo-traefik --rule="shop.dojo.test/shop*=web:80" --dry-run=client -o yaml | kubectl apply -f -
sleep 3
kubectl -n dojo-ingress exec deploy/probe -- curl -s -H 'Host: shop.dojo.test' http://traefik.dojo-ingress/shop/cart
```'''
hints = ["kubectl create ingress uses a trailing * in its rule syntax to generate Prefix.", "The echo backends answer with their name and the path they received."]

[[lab.steps.checks]]
name = "the Ingress uses class dojo-traefik"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.ingressClassName}''''
equals = "dojo-traefik"

[[lab.steps.checks]]
name = "shop.dojo.test/shop and /shop/cart reach web"
run = '''r() { kubectl -n dojo-ingress exec deploy/probe -- curl -sS --max-time 3 -H "Host: $1" "http://traefik.dojo-ingress$2" 2>&1; }
for i in $(seq 1 20); do a=$(r shop.dojo.test /shop); b=$(r shop.dojo.test /shop/cart); [ "$a|$b" = "web /shop|web /shop/cart" ] && echo ok && exit 0; sleep 2; done; echo "/shop -> $a; /shop/cart -> $b"'''
equals = "ok"
timeout = 90

[[lab.steps.checks]]
name = "/shopping and other hosts do not reach web"
run = '''r() { kubectl -n dojo-ingress exec deploy/probe -- curl -sS --max-time 3 -H "Host: $1" "http://traefik.dojo-ingress$2" 2>&1; }
[ "$(r shop.dojo.test /shop)" = "web /shop" ] || { echo "the route does not work yet"; exit 1; }
a=$(r shop.dojo.test /shopping); b=$(r other.dojo.test /shop)
case "$a$b" in *web*) echo "/shopping -> $a; other.dojo.test/shop -> $b" ;; *) echo ok ;; esac'''
equals = "ok"
timeout = 60

[[lab.steps.negatives]]
name = "uses Exact instead of Prefix"
run = '''kubectl -n ingress-lab create ingress shop --class=dojo-traefik --rule="shop.dojo.test/shop=web:80" --dry-run=client -o yaml | kubectl apply -f -
sleep 3'''

[[lab.steps.negatives]]
name = "leaves out the host, so every host reaches web"
run = '''kubectl -n ingress-lab delete ingress shop --ignore-not-found
kubectl -n ingress-lab create ingress shop --class=dojo-traefik --rule="/shop*=web:80"
sleep 3'''
undo = '''kubectl -n ingress-lab delete ingress shop --ignore-not-found'''

[[lab.steps.alternatives]]
name = "declares the same route in YAML with the named Service port"
run = '''kubectl -n ingress-lab delete ingress shop --ignore-not-found
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: shop
  namespace: ingress-lab
spec:
  ingressClassName: dojo-traefik
  rules:
  - host: shop.dojo.test
    http:
      paths:
      - path: /shop
        pathType: Prefix
        backend:
          service:
            name: web
            port: {name: http}
EOF
sleep 3'''
undo = '''kubectl -n ingress-lab delete ingress shop --ignore-not-found'''

[[lab.steps]]
title = "Add an exact path and a fallback"
task = "Keep the /shop Prefix route. On the same host add Exact path /health to Service status port 80: /health must reach status, /health/deep must not. Also configure spec.defaultBackend as Service fallback port 80 (Traefik, like several controllers, only uses defaultBackend from an Ingress without rules, so here it is graded on the spec)."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: shop
  namespace: ingress-lab
spec:
  ingressClassName: dojo-traefik
  defaultBackend:
    service:
      name: fallback
      port: {number: 80}
  rules:
  - host: shop.dojo.test
    http:
      paths:
      - path: /shop
        pathType: Prefix
        backend:
          service:
            name: web
            port: {number: 80}
      - path: /health
        pathType: Exact
        backend:
          service:
            name: status
            port: {number: 80}
EOF
sleep 3
kubectl -n dojo-ingress exec deploy/probe -- curl -s -H 'Host: shop.dojo.test' http://traefik.dojo-ingress/health
kubectl -n dojo-ingress exec deploy/probe -- curl -s -H 'Host: shop.dojo.test' http://traefik.dojo-ingress/health/deep
```'''
hints = ["defaultBackend belongs directly under spec, alongside rules.", "Exact /health matches only /health; /health/deep matches no rule."]

[[lab.steps.checks]]
name = "/health reaches status and /shop/cart still reaches web"
run = '''r() { kubectl -n dojo-ingress exec deploy/probe -- curl -sS --max-time 3 -H "Host: $1" "http://traefik.dojo-ingress$2" 2>&1; }
for i in $(seq 1 20); do a=$(r shop.dojo.test /health); b=$(r shop.dojo.test /shop/cart); [ "$a|$b" = "status /health|web /shop/cart" ] && echo ok && exit 0; sleep 2; done; echo "/health -> $a; /shop/cart -> $b"'''
equals = "ok"
timeout = 90

[[lab.steps.checks]]
name = "/health/deep and other hosts reach neither status nor web"
run = '''r() { kubectl -n dojo-ingress exec deploy/probe -- curl -sS --max-time 3 -H "Host: $1" "http://traefik.dojo-ingress$2" 2>&1; }
[ "$(r shop.dojo.test /health)" = "status /health" ] || { echo "/health is not routed to status yet"; exit 1; }
a=$(r shop.dojo.test /health/deep); b=$(r other.dojo.test /health)
case "$a$b" in *status*|*web*) echo "/health/deep -> $a; other.dojo.test/health -> $b" ;; *) echo ok ;; esac'''
equals = "ok"
timeout = 60

[[lab.steps.checks]]
name = "unmatched requests have a default backend"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.defaultBackend.service.name}{" "}{.spec.defaultBackend.service.port.number}{.spec.defaultBackend.service.port.name}''''
equals = "fallback 80"

[[lab.steps.negatives]]
name = "uses Prefix for /health"
run = '''kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: shop
  namespace: ingress-lab
spec:
  ingressClassName: dojo-traefik
  defaultBackend:
    service:
      name: fallback
      port: {number: 80}
  rules:
  - host: shop.dojo.test
    http:
      paths:
      - {path: /shop, pathType: Prefix, backend: {service: {name: web, port: {number: 80}}}}
      - {path: /health, pathType: Prefix, backend: {service: {name: status, port: {number: 80}}}}
EOF
sleep 3'''

[[lab.steps.negatives]]
name = "adds a / catch-all under the host instead of a default backend"
run = '''kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: shop
  namespace: ingress-lab
spec:
  ingressClassName: dojo-traefik
  rules:
  - host: shop.dojo.test
    http:
      paths:
      - {path: /shop, pathType: Prefix, backend: {service: {name: web, port: {number: 80}}}}
      - {path: /health, pathType: Exact, backend: {service: {name: status, port: {number: 80}}}}
      - {path: /, pathType: Prefix, backend: {service: {name: fallback, port: {number: 80}}}}
EOF
sleep 3'''

[[lab.steps]]
title = "Terminate TLS"
task = "Create TLS Secret shop-tls in ingress-lab from a locally generated self-signed certificate for shop.dojo.test. Add a TLS entry to shop using that host and Secret, preserving both paths and the default backend. HTTPS requests for shop.dojo.test must then present your certificate: `kubectl -n dojo-ingress exec -i deploy/probe -- sh -c 'cat > /tmp/ca.pem && curl -s --cacert /tmp/ca.pem --connect-to shop.dojo.test:443:traefik.dojo-ingress:443 https://shop.dojo.test/shop' < shop.crt` answers `web /shop`. This step requires the openssl CLI on the lab terminal host."
solution = '''```bash
cat > shop.cnf <<'EOF'
[req]
distinguished_name = dn
x509_extensions = san
prompt = no
[dn]
CN = shop.dojo.test
[san]
subjectAltName = DNS:shop.dojo.test
EOF
openssl req -x509 -nodes -newkey rsa:2048 -days 1 -keyout shop.key -out shop.crt -config shop.cnf
kubectl -n ingress-lab create secret tls shop-tls --cert=shop.crt --key=shop.key --dry-run=client -o yaml | kubectl apply -f -
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: shop
  namespace: ingress-lab
spec:
  ingressClassName: dojo-traefik
  defaultBackend:
    service:
      name: fallback
      port: {number: 80}
  tls:
  - hosts: [shop.dojo.test]
    secretName: shop-tls
  rules:
  - host: shop.dojo.test
    http:
      paths:
      - path: /shop
        pathType: Prefix
        backend:
          service:
            name: web
            port: {number: 80}
      - path: /health
        pathType: Exact
        backend:
          service:
            name: status
            port: {number: 80}
EOF
sleep 3
kubectl -n dojo-ingress exec -i deploy/probe -- sh -c 'cat > /tmp/ca.pem && curl -s --cacert /tmp/ca.pem --connect-to shop.dojo.test:443:traefik.dojo-ingress:443 https://shop.dojo.test/shop' < shop.crt
```'''
hints = ["kubectl create secret tls reads PEM files and creates kubernetes.io/tls data keys.", "Without a matching tls entry the controller serves its own default certificate, which your CA file will not trust."]

[[lab.steps.checks]]
name = "shop-tls is a kubernetes.io/tls Secret"
run = '''kubectl -n ingress-lab get secret shop-tls -o jsonpath='{.type}''''
equals = "kubernetes.io/tls"

[[lab.steps.checks]]
name = "HTTPS for shop.dojo.test presents the shop-tls certificate and routes /shop"
run = '''kubectl -n ingress-lab get secret shop-tls >/dev/null 2>&1 || { echo "no Secret shop-tls"; exit 1; }
for i in $(seq 1 20); do
  v=$(kubectl -n ingress-lab get secret shop-tls -o go-template='{{index .data "tls.crt" | base64decode}}' | kubectl -n dojo-ingress exec -i deploy/probe -- sh -c 'cat > /tmp/ca.pem && curl -sS --max-time 3 --cacert /tmp/ca.pem --connect-to shop.dojo.test:443:traefik.dojo-ingress:443 https://shop.dojo.test/shop/tls' 2>&1)
  [ "$v" = "web /shop/tls" ] && echo ok && exit 0; sleep 2
done; echo "$v"'''
equals = "ok"
timeout = 90

[[lab.steps.checks]]
name = "plain HTTP routing still works"
run = '''kubectl -n dojo-ingress exec deploy/probe -- curl -sS --max-time 3 -H "Host: shop.dojo.test" http://traefik.dojo-ingress/health 2>&1'''
equals = "status /health"

[[lab.steps.negatives]]
name = "creates the Secret but never references it from the Ingress"
run = '''openssl req -x509 -nodes -newkey rsa:2048 -days 1 -keyout shop.key -out shop.crt -subj /CN=shop.dojo.test -addext subjectAltName=DNS:shop.dojo.test 2>/dev/null
kubectl -n ingress-lab create secret tls shop-tls --cert=shop.crt --key=shop.key --dry-run=client -o yaml | kubectl apply -f -
sleep 3'''
undo = '''kubectl -n ingress-lab delete secret shop-tls --ignore-not-found
rm -f shop.key shop.crt'''

[[lab.steps.negatives]]
name = "references a certificate issued for another host name"
run = '''mkdir -p wrong
openssl req -x509 -nodes -newkey rsa:2048 -days 1 -keyout wrong/tls.key -out wrong/tls.crt -subj /CN=www.dojo.test -addext subjectAltName=DNS:www.dojo.test 2>/dev/null
kubectl -n ingress-lab create secret tls shop-tls --cert=wrong/tls.crt --key=wrong/tls.key --dry-run=client -o yaml | kubectl apply -f -
kubectl -n ingress-lab patch ingress shop --type=merge -p '{"spec":{"tls":[{"hosts":["shop.dojo.test"],"secretName":"shop-tls"}]}}'
sleep 3'''
undo = '''kubectl -n ingress-lab patch ingress shop --type=json -p '[{"op":"remove","path":"/spec/tls"}]'
kubectl -n ingress-lab delete secret shop-tls --ignore-not-found
rm -rf wrong'''
+++

## A routing declaration and the process that implements it

A Service provides a stable transport endpoint. An Ingress describes how incoming HTTP or HTTPS requests should reach Services based on hostnames and paths. It lets several applications share an entry point instead of assigning a separate external load balancer to each Service.

```text
browser -> controller listener -> host/path rule -> Service port -> Pod
                         |
                         +-> TLS certificate from a Secret
```

The resource is configuration. The **Ingress controller** is software that watches the API and configures listeners and routes. Creating an Ingress without a compatible controller stores an object but does not create a working proxy. This lab installs one for you: Traefik, in namespace `dojo-ingress`, serving every IngressClass whose controller is `traefik.io/ingress-controller` (a ready-made class `traefik` exists). The lab grades real requests, so a rule that looks right but routes wrongly does not pass.

An IngressClass connects a resource to a controller implementation. Its `spec.controller` contains an implementation identifier. An Ingress uses `spec.ingressClassName` to reference the class by name. A default IngressClass may handle resources without that field, but relying on a default hides an important dependency. Read the task's class requirement and set it explicitly.

## Start with the current API

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: shop
  namespace: ingress-lab
spec:
  ingressClassName: dojo-traefik
  rules:
  - host: shop.dojo.test
    http:
      paths:
      - path: /shop
        pathType: Prefix
        backend:
          service:
            name: web
            port:
              number: 80
```

The backend names a Service in the **same namespace**, and its port is a **Service port**. It is neither the container's targetPort nor the Service's nodePort. You may instead use `port: {name: http}` if the Service exposes a port named http. Do not set name and number together.

The v1 API requires a pathType. Older examples using `extensions/v1beta1`, flat `serviceName` or flat `servicePort` are not valid substitutes. Use `kubectl explain ingress.spec.rules.http.paths.backend` when your memory and an old blog disagree.

## Match hosts and paths precisely

The HTTP Host header, or the HTTP/2 authority, determines host matching. Pointing a browser at the controller IP without the expected hostname may select the default backend. Creating an Ingress does not create a public DNS record. DNS must independently point the hostname at the controller's entry point.

| pathType | Meaning | /shop examples |
|---|---|---|
| Exact | Complete, case-sensitive path | /shop matches; /shop/cart does not |
| Prefix | Slash-separated path element prefix | /shop and /shop/cart match; /shopping does not |
| ImplementationSpecific | Controller chooses semantics | Read that controller's documentation |

Prefix is not arbitrary character-prefix matching, and Exact is not a regular expression. When several paths match, the longest matching path wins; when equal-length paths tie, Exact takes precedence over Prefix. The `/` Prefix is a convenient catch-all path within a host rule.

A wildcard host such as `*.dojo.test` matches a single DNS label, for example `shop.dojo.test`, not the bare `dojo.test` or `a.shop.dojo.test`. An omitted host permits matching without a specific virtual host. These distinctions become important when troubleshooting a request that reaches a controller but is routed to an unexpected application.

## Test through the controller

A route is only proven by a request. Inside the lab cluster the controller answers on `traefik.dojo-ingress` (HTTP on 80, HTTPS on 443), and Deployment `probe` in `dojo-ingress` is a curl client you can exec into. The Host header selects the rule, so you do not need DNS:

```bash
kubectl -n dojo-ingress exec deploy/probe -- curl -s -H 'Host: shop.dojo.test' http://traefik.dojo-ingress/shop/cart
kubectl -n dojo-ingress exec deploy/probe -- curl -s -H 'Host: other.dojo.test' http://traefik.dojo-ingress/shop
```

The lab's backends answer with their own name and the path they received (`web /shop/cart`), so you can see which rule matched. HTTPS is also published on your machine: `curl -sk -H 'Host: shop.dojo.test' https://$PODDOJO_HOST:30443/shop`. On the exam, the equivalent is a `curl` from the exam terminal or a temporary Pod (`kubectl run tmp --rm -it --image=curlimages/curl:8.13.0 --restart=Never -- curl ...`).

## Generate, inspect, apply

```bash
kubectl -n ingress-lab create ingress shop --class=dojo-traefik --rule='shop.dojo.test/shop*=web:80' --dry-run=client -o yaml
kubectl -n ingress-lab get ingress shop -o yaml
kubectl -n ingress-lab describe ingress shop
kubectl get ingressclass
```

In `kubectl create ingress` rule syntax, the trailing star generates a Prefix path. It is not stored as a wildcard in the path itself. A rule without that star generates Exact. Quote the argument so the shell cannot expand it. For a complex resource, generate a skeleton, save it, and edit the YAML. Inspect output before applying it rather than guessing which generator defaults were selected.

A declarative manifest is usually clearer for multiple routes, TLS and a default backend. Keep the file as a reproducible record of the intended state. `kubectl apply` updates that declaration; it does not validate whether an absent controller will implement it.

## Handle unmatched traffic deliberately

`spec.defaultBackend` names the backend for requests that do not match a rule. It sits directly under spec, beside rules. It is different from adding a `/` Prefix route under one host: the host-specific route still requires that host to match. A controller may provide its own default response when the resource does not specify a backend, commonly a not-found page.

```yaml
spec:
  defaultBackend:
    service:
      name: fallback
      port:
        number: 80
```

Support for `defaultBackend` varies by controller. Traefik, for example, only uses it from an Ingress that has no rules, as the controller's cluster-wide fallback, and answers its own 404 otherwise; that is why the lab grades this field on the spec. A default backend Service still needs working endpoints. A route can be syntactically correct while the backend is unavailable. Follow failures through class selection, host, path, Service port, endpoint readiness and finally the application's response. Do not treat an empty ADDRESS column as proof that a rule is malformed; reporting depends on the controller and environment.

## TLS configuration has two sides

The TLS section lists hosts and the name of a Secret in the Ingress namespace. A normal TLS Secret has type `kubernetes.io/tls` and keys `tls.crt` and `tls.key`. Supply a valid certificate and its matching private key. The certificate must cover the hostname clients use.

```yaml
spec:
  tls:
  - hosts:
    - shop.dojo.test
    secretName: shop-tls
```

A reference does not issue or renew a certificate. Controllers often terminate HTTPS and send HTTP to backends; encrypting the backend hop requires separate controller-specific configuration. Likewise, redirects from HTTP to HTTPS are not standardized by the TLS stanza alone. The lab uses a self-signed certificate and verifies it with `curl --cacert shop.crt`: if the Ingress does not reference the Secret, or the certificate names another host, the controller presents its own default certificate and verification fails. Self-signed certificates practice the wiring; they do not establish browser trust. Never put a real private key into source control or display Secret data while debugging.

## Controllers and the path toward Gateway API

The community **ingress-nginx** project retired in March 2026. Its retirement does not remove the Ingress API or retire every NGINX product. Select a maintained implementation appropriate to the environment, such as Traefik, HAProxy's controller, or Contour with Envoy. Controller-specific annotations can configure rewrites, timeouts or authentication, but their meanings are not portable across implementations. See the [official retirement announcement](https://kubernetes.io/blog/2025/11/11/ingress-nginx-retirement/).

Gateway API is the successor direction for richer, role-oriented traffic management. A GatewayClass identifies an implementation; a Gateway defines listeners and infrastructure attachment; an HTTPRoute attaches host/path rules to a Gateway and references backends. These are separate resources with clearer ownership boundaries. Gateway API requires its APIs and a compatible controller to be installed; creating an Ingress does not automatically create a Gateway. For CKAD, recognize the model while practicing the resource explicitly requested by the task.

## Exam tips

Use the required class, namespace, hostname, pathType and Service port exactly. Inspect the stored spec after applying, then send a request with the expected Host header to prove the route. Avoid copying rewrite annotations from an unrelated controller. The [kubectl generator reference](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_create/kubectl_create_ingress/) is useful when reconstructing imperative syntax under time pressure.
