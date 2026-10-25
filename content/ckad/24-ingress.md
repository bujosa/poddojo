+++
id = "ckad-ingress"
title = "Ingress: hosts, paths and TLS"
module = "networking"
minutes = 40
needs = "cluster"
objectives = ["Distinguish an Ingress resource from its controller and class", "Build host and path rules using the v1 API", "Configure a default backend and TLS Secret reference", "Explain controller-specific behavior and the Gateway API model"]
domain = "networking"

[[quiz]]
q = "What makes an Ingress resource forward HTTP traffic?"
choices = ["Creating the resource alone", "A running compatible controller that handles its class", "A kubelet on the client", "The TLS Secret alone"]
answer = 1
why = "The API stores the desired route. A controller watches the class and configures a data plane; object creation does not install that controller."

[[quiz]]
q = "An Ingress backend references port 8080. What must that number identify?"
choices = ["A nodePort", "The backend Service port", "The container port regardless of the Service", "An external load-balancer port"]
answer = 1
why = "Ingress backends reference a Service in the same namespace and one of its exposed ports, by number or name. That Service maps onward to targetPort."

[[quiz]]
q = "Which request matches Prefix /tea but not Exact /tea?"
choices = ["/tea", "/tea/cup", "/teapot", "/coffee"]
answer = 1
why = "Prefix matches complete slash-separated path elements. It includes /tea/cup, while /teapot is not a match. Exact requires the complete, case-sensitive path."

[[quiz]]
q = "What does a TLS section with secretName guarantee without a controller?"
choices = ["A public certificate is issued", "A DNS record is created", "The desired TLS configuration is stored", "Every backend now speaks HTTPS"]
answer = 2
why = "The TLS section references a Secret. Certificate issuance, listener configuration and public reachability require additional components; normal Ingress TLS termination does not imply encrypted backend connections."

[[quiz]]
q = "How do portable Ingress annotations behave?"
choices = ["All controllers implement the same rewrite keys", "Annotations are controller-specific extensions", "They replace pathType in v1", "They install the selected controller"]
answer = 1
why = "The core API standardizes hosts, paths and backends. Rewrite, authentication and timeout annotations depend on the chosen controller and should not be copied across implementations blindly."

[lab]
summary = "Declare portable HTTP routes and TLS without installing an Ingress controller."
setup = ["kubectl create namespace ingress-lab --dry-run=client -o yaml | kubectl apply -f -", "kubectl apply -f - <<'EOF'\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n  namespace: ingress-lab\nspec:\n  replicas: 2\n  selector:\n    matchLabels: {app: web}\n  template:\n    metadata:\n      labels: {\"app\": \"web\"}\n    spec:\n      containers:\n      - name: web\n        image: nginx:1.27-alpine\n        ports:\n        - name: http\n          containerPort: 80\n        readinessProbe:\n          httpGet: {path: /, port: http}\n          periodSeconds: 2\nEOF\nkubectl -n ingress-lab rollout status deployment/web --timeout=120s", "kubectl apply -f - <<'EOF'\napiVersion: v1\nkind: Service\nmetadata:\n  name: web\n  namespace: ingress-lab\nspec:\n  type: ClusterIP\n  selector: {app: web}\n  ports:\n  - name: http\n    port: 80\n    targetPort: http\nEOF", "kubectl apply -f - <<'EOF'\napiVersion: v1\nkind: Service\nmetadata:\n  name: fallback\n  namespace: ingress-lab\nspec:\n  type: ClusterIP\n  selector: {app: web}\n  ports:\n  - name: http\n    port: 80\n    targetPort: http\nEOF"]
cleanup = ["kubectl delete namespace ingress-lab --ignore-not-found --wait=false || true", "kubectl delete ingressclass dojo-traefik --ignore-not-found || true", "rm -f shop.key shop.crt shop.cnf || true"]

[[lab.steps]]
title = "Declare who would handle these routes"
task = "Create cluster-scoped IngressClass dojo-traefik with spec.controller traefik.io/ingress-controller. Do not install a controller or mark this class as the cluster default."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: dojo-traefik
spec:
  controller: traefik.io/ingress-controller
EOF
```'''
hints = ["IngressClass is cluster-scoped and needs the dojo- prefix.", "Its controller string identifies an implementation; it is not an image name."]

[[lab.steps.checks]]
name = "the class names the Traefik controller"
run = '''kubectl get ingressclass dojo-traefik -o jsonpath='{.spec.controller}''''
equals = "traefik.io/ingress-controller"

[[lab.steps]]
title = "Route a host and a path prefix"
task = "In ingress-lab create Ingress shop using class dojo-traefik. Route host shop.dojo.test and Prefix path /shop to Service web, Service port 80. No address or successful HTTP request is required."
solution = '''```bash
kubectl -n ingress-lab create ingress shop --class=dojo-traefik --rule="shop.dojo.test/shop*=web:80" --dry-run=client -o yaml | kubectl apply -f -
```'''
hints = ["kubectl create ingress uses a trailing * in its rule syntax to generate Prefix.", "The * is generator syntax; the stored path should be /shop."]

[[lab.steps.checks]]
name = "host and class are set"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.ingressClassName}{" "}{.spec.rules[0].host}''''
equals = "dojo-traefik shop.dojo.test"

[[lab.steps.checks]]
name = "the prefix routes to the Service port"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.rules[0].http.paths[?(@.path=="/shop")].pathType}{" "}{.spec.rules[0].http.paths[?(@.path=="/shop")].backend.service.name}{" "}{.spec.rules[0].http.paths[?(@.path=="/shop")].backend.service.port.number}''''
equals = "Prefix web 80"

[[lab.steps]]
title = "Add an exact path and a fallback"
task = "Keep the /shop Prefix route. Add Exact path /health on the same host to web:80. Configure spec.defaultBackend as Service fallback port 80."
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
            name: web
            port: {number: 80}
EOF
```'''
hints = ["defaultBackend belongs directly under spec, alongside rules.", "The backend uses networking.k8s.io/v1 nested service.name and service.port fields."]

[[lab.steps.checks]]
name = "health is an Exact route to web"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.rules[0].http.paths[?(@.path=="/health")].pathType}{" "}{.spec.rules[0].http.paths[?(@.path=="/health")].backend.service.name}{" "}{.spec.rules[0].http.paths[?(@.path=="/health")].backend.service.port.number}''''
equals = "Exact web 80"

[[lab.steps.checks]]
name = "unmatched requests have a default backend"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.defaultBackend.service.name}{" "}{.spec.defaultBackend.service.port.number}''''
equals = "fallback 80"

[[lab.steps]]
title = "Describe TLS termination"
task = "Create TLS Secret shop-tls in ingress-lab from a locally generated self-signed certificate for shop.dojo.test. Add a TLS entry to shop using that host and Secret. Preserve both paths and the default backend. This step requires the openssl CLI on the lab terminal host."
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
            name: web
            port: {number: 80}
EOF
```'''
hints = ["kubectl create secret tls reads PEM files and creates kubernetes.io/tls data keys.", "The certificate is a throwaway lab certificate; no external certificate authority or controller is needed."]

[[lab.steps.checks]]
name = "TLS references the requested host and Secret"
run = '''kubectl -n ingress-lab get ingress shop -o jsonpath='{.spec.tls[0].hosts[0]}{" "}{.spec.tls[0].secretName}''''
equals = "shop.dojo.test shop-tls"

[[lab.steps.checks]]
name = "the referenced TLS Secret has both required entries"
run = '''t=$(kubectl -n ingress-lab get secret shop-tls -o jsonpath='{.type}'); c=$(kubectl -n ingress-lab get secret shop-tls -o jsonpath='{.data.tls\.crt}'); k=$(kubectl -n ingress-lab get secret shop-tls -o jsonpath='{.data.tls\.key}'); [ "$t" = kubernetes.io/tls ] && [ -n "$c" ] && [ -n "$k" ] && echo ok'''
equals = "ok"
+++

## A routing declaration and the process that implements it

A Service provides a stable transport endpoint. An Ingress describes how incoming HTTP or HTTPS requests should reach Services based on hostnames and paths. It lets several applications share an entry point instead of assigning a separate external load balancer to each Service.

```text
browser -> controller listener -> host/path rule -> Service port -> Pod
                         |
                         +-> TLS certificate from a Secret
```

The resource is configuration. The **Ingress controller** is software that watches the API and configures listeners and routes. Creating an Ingress without a compatible controller stores an object but does not create a working proxy. PodDojo deliberately has no controller installed, so this lab grades the desired specification rather than external reachability.

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

A default backend Service still needs working endpoints. A route can be syntactically correct while the backend is unavailable. Follow failures through class selection, host, path, Service port, endpoint readiness and finally the application's response. Do not treat an empty ADDRESS column as proof that a rule is malformed; reporting depends on the controller and environment.

## TLS configuration has two sides

The TLS section lists hosts and the name of a Secret in the Ingress namespace. A normal TLS Secret has type `kubernetes.io/tls` and keys `tls.crt` and `tls.key`. Supply a valid certificate and its matching private key. The certificate must cover the hostname clients use.

```yaml
spec:
  tls:
  - hosts:
    - shop.dojo.test
    secretName: shop-tls
```

A reference does not issue or renew a certificate. Controllers often terminate HTTPS and send HTTP to backends; encrypting the backend hop requires separate controller-specific configuration. Likewise, redirects from HTTP to HTTPS are not standardized by the TLS stanza alone. The lab uses a self-signed certificate to practice object wiring, not to establish browser trust. Never put a real private key into source control or display Secret data while debugging.

## Controllers and the path toward Gateway API

The community **ingress-nginx** project retired in March 2026. Its retirement does not remove the Ingress API or retire every NGINX product. Select a maintained implementation appropriate to the environment, such as Traefik, HAProxy's controller, or Contour with Envoy. Controller-specific annotations can configure rewrites, timeouts or authentication, but their meanings are not portable across implementations. See the [official retirement announcement](https://kubernetes.io/blog/2025/11/11/ingress-nginx-retirement/).

Gateway API is the successor direction for richer, role-oriented traffic management. A GatewayClass identifies an implementation; a Gateway defines listeners and infrastructure attachment; an HTTPRoute attaches host/path rules to a Gateway and references backends. These are separate resources with clearer ownership boundaries. Gateway API requires its APIs and a compatible controller to be installed; creating an Ingress does not automatically create a Gateway. For CKAD, recognize the model while practicing the resource explicitly requested by the task.

## Exam tips

Use the required class, namespace, hostname, pathType and Service port exactly. Inspect stored spec after applying and test traffic only when a controller is actually available. Avoid copying rewrite annotations from an unrelated controller. The [kubectl generator reference](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_create/kubectl_create_ingress/) is useful when reconstructing imperative syntax under time pressure.
