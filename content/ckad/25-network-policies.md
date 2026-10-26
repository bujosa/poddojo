+++
id = "ckad-network-policies"
title = "NetworkPolicies: isolate, then allow"
module = "networking"
minutes = 50
needs = "cluster"
objectives = ["Explain directional isolation and additive policy behavior", "Combine namespace and Pod selectors without widening access", "Allow application traffic and UDP/TCP DNS after default deny", "Verify positive and negative connectivity from real Pods"]
domain = "networking"

[[quiz]]
q = "Two ingress policies select the same Pod. What traffic is permitted?"
choices = ["Only traffic allowed by both", "The union of their allowed traffic", "Only the newest policy", "None until policy priority is configured"]
answer = 1
why = "NetworkPolicies are additive allow-lists. A deny-all policy creates isolation but cannot override an allow rule in another policy."

[[quiz]]
q = "Where does a peer podSelector without a namespaceSelector select Pods?"
choices = ["All namespaces", "Only kube-system", "The policy namespace", "The caller namespace always"]
answer = 2
why = "A peer podSelector alone is limited to the namespace containing the policy. Put a namespaceSelector in the same peer to select Pods in other namespaces."

[[quiz]]
q = "How do namespaceSelector and podSelector in one from list entry combine?"
choices = ["OR", "AND", "OR, but only for the listed ports", "The namespaceSelector applies only to egress"]
answer = 1
why = "Both restrictions must match in the same peer. Writing two separate list entries changes the meaning to either peer, often allowing much more traffic than intended."

[[quiz]]
q = "Why can an HTTP request by IP work after egress isolation while the same request by name fails?"
choices = ["Egress rules apply only to Service IPs, not Pod IPs", "DNS traffic may not be allowed", "Name lookups go through kube-proxy, which the policy blocks", "TCP 80 automatically allows DNS"]
answer = 1
why = "Name resolution is separate traffic, usually UDP 53 with TCP 53 also needed. Allow the DNS destination and both protocols, as well as application traffic."

[[quiz]]
q = "What does podSelector: {} select at the top of a namespaced NetworkPolicy?"
choices = ["No Pods", "Every Pod in that namespace", "Every Pod in every namespace", "Only Pods without labels"]
answer = 1
why = "An empty label selector matches all Pods within the policy namespace. policyTypes then determines which traffic directions become isolated."

[lab]
summary = "Build default-deny boundaries, permit a precise frontend, then restore constrained DNS and application egress."
setup = ["kubectl create namespace policies-lab --dry-run=client -o yaml | kubectl apply -f -", "kubectl create namespace policies-other-lab --dry-run=client -o yaml | kubectl apply -f -", "kubectl -n policies-lab run backend --image=nginx:1.27-alpine --labels=app=backend --port=80 --dry-run=client -o yaml | kubectl apply -f -", "kubectl -n policies-lab wait --for=condition=Ready pod/backend --timeout=120s", "kubectl -n policies-lab run client --image=busybox:1.36 --restart=Never --labels=role=frontend --dry-run=client -o yaml --command -- sleep 14400 | kubectl apply -f -\nkubectl -n policies-lab wait --for=condition=Ready pod/client --timeout=120s", "kubectl -n policies-lab run observer --image=busybox:1.36 --restart=Never --labels=role=observer --dry-run=client -o yaml --command -- sleep 14400 | kubectl apply -f -\nkubectl -n policies-lab wait --for=condition=Ready pod/observer --timeout=120s", "kubectl -n policies-other-lab run outsider --image=busybox:1.36 --restart=Never --labels=role=frontend --dry-run=client -o yaml --command -- sleep 14400 | kubectl apply -f -\nkubectl -n policies-other-lab wait --for=condition=Ready pod/outsider --timeout=120s", "kubectl apply -f - <<'EOF'\napiVersion: v1\nkind: Service\nmetadata:\n  name: backend\n  namespace: policies-lab\nspec:\n  type: ClusterIP\n  selector: {app: backend}\n  ports:\n  - name: http\n    port: 80\n    targetPort: 80\nEOF", "kubectl -n policies-lab exec client -- wget -qO- -T 2 http://backend", "kubectl -n policies-other-lab run elsewhere --image=nginx:1.27-alpine --labels=app=elsewhere --port=80 --dry-run=client -o yaml | kubectl apply -f -\nkubectl -n policies-other-lab wait --for=condition=Ready pod/elsewhere --timeout=120s"]
cleanup = ["kubectl delete namespace policies-lab policies-other-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Isolate incoming traffic"
task = "In policies-lab create NetworkPolicy deny-ingress selecting all Pods, with policyTypes [Ingress] and no allowed ingress rules. Requests from the prepared client to backend must stop working."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: deny-ingress, namespace: policies-lab}
spec:
  podSelector: {}
  policyTypes: [Ingress]
  ingress: []
EOF
```'''
hints = ["An empty ingress list allows no incoming connections for selected Pods.", "Checks use backend Pod IPs so DNS cannot masquerade as isolation."]

[[lab.steps.checks]]
name = "all Pods have explicit ingress isolation"
run = '''kubectl -n policies-lab get networkpolicy deny-ingress -o jsonpath='{.spec.policyTypes[*]}''''
equals = "Ingress"

[[lab.steps.checks]]
name = "a previously reachable backend now rejects the client"
run = '''kubectl -n policies-lab get networkpolicy deny-ingress >/dev/null && kubectl -n policies-lab wait --for=condition=Ready pod/client --timeout=20s >/dev/null && ip=$(kubectl -n policies-lab get pod backend -o jsonpath='{.status.podIP}') && [ -n "$ip" ] && for i in $(seq 1 15); do good=0; for j in 1 2 3; do if kubectl -n policies-lab exec client -- wget -qO- -T 2 "http://$ip" >/dev/null 2>&1; then break; else good=$((good+1)); fi; done; [ "$good" = 3 ] && echo blocked && exit 0; sleep 2; done; exit 1'''
equals = "blocked"
timeout = 120

[[lab.steps]]
title = "Allow exactly the frontend peer"
task = "Create NetworkPolicy allow-frontend selecting app=backend. Allow incoming TCP 80 only from Pods with role=frontend in namespace policies-lab. Keep deny-ingress. The observer in policies-lab and the role=frontend outsider in policies-other-lab must remain blocked."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-frontend, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {app: backend}}
  policyTypes: [Ingress]
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          kubernetes.io/metadata.name: policies-lab
      podSelector:
        matchLabels:
          role: frontend
    ports:
    - {protocol: TCP, port: 80}
EOF
```'''
hints = ["Put namespaceSelector and podSelector in one from entry to require both.", "The immutable kubernetes.io/metadata.name namespace label identifies the namespace."]

[[lab.steps.checks]]
name = "the frontend can reach the backend"
run = '''ip=$(kubectl -n policies-lab get pod backend -o jsonpath='{.status.podIP}'); [ -n "$ip" ] && for i in $(seq 1 30); do v=$(kubectl -n policies-lab exec client -- wget -qO- -T 2 "http://$ip"); echo "$v" | grep -q "Welcome to nginx" && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 100

[[lab.steps.checks]]
name = "other labels and the other namespace stay excluded"
run = '''kubectl -n policies-lab get networkpolicy allow-frontend >/dev/null && ( kubectl -n policies-lab wait --for=condition=Ready pod/observer --timeout=20s >/dev/null && ip=$(kubectl -n policies-lab get pod backend -o jsonpath='{.status.podIP}') && [ -n "$ip" ] && for i in $(seq 1 15); do good=0; for j in 1 2 3; do if kubectl -n policies-lab exec observer -- wget -qO- -T 2 "http://$ip" >/dev/null 2>&1; then break; else good=$((good+1)); fi; done; [ "$good" = 3 ] && echo blocked && exit 0; sleep 2; done; exit 1 ) && ( kubectl -n policies-other-lab wait --for=condition=Ready pod/outsider --timeout=20s >/dev/null && ip=$(kubectl -n policies-lab get pod backend -o jsonpath='{.status.podIP}') && [ -n "$ip" ] && for i in $(seq 1 15); do good=0; for j in 1 2 3; do if kubectl -n policies-other-lab exec outsider -- wget -qO- -T 2 "http://$ip" >/dev/null 2>&1; then break; else good=$((good+1)); fi; done; [ "$good" = 3 ] && echo blocked && exit 0; sleep 2; done; exit 1 )'''
equals = '''blocked
blocked'''
timeout = 180

[[lab.steps.negatives]]
name = "lists the namespace and the label as two peers (OR), admitting the observer"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-frontend, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {app: backend}}
  policyTypes: [Ingress]
  ingress:
  - from:
    - namespaceSelector: {matchLabels: {kubernetes.io/metadata.name: policies-lab}}
    - podSelector: {matchLabels: {role: frontend}}
    ports:
    - {protocol: TCP, port: 80}
EOF
'''

[[lab.steps.negatives]]
name = "matches role=frontend in every namespace, admitting the outsider"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-frontend, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {app: backend}}
  policyTypes: [Ingress]
  ingress:
  - from:
    - namespaceSelector: {}
      podSelector: {matchLabels: {role: frontend}}
    ports:
    - {protocol: TCP, port: 80}
EOF
'''

[[lab.steps.alternatives]]
name = "a podSelector-only peer (it already means this namespace)"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-frontend, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {app: backend}}
  policyTypes: [Ingress]
  ingress:
  - from:
    - podSelector: {matchLabels: {role: frontend}}
    ports:
    - {protocol: TCP, port: 80}
EOF
'''

[[lab.steps]]
title = "Isolate the frontend egress independently"
task = "Create NetworkPolicy deny-client-egress selecting role=frontend in policies-lab, policyTypes [Egress], with no allowed egress rules. Even the authorized frontend must now fail to reach backend by IP."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: deny-client-egress, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {role: frontend}}
  policyTypes: [Egress]
  egress: []
EOF
```'''
hints = ["Ingress and egress isolation are independent.", "Keep the previous allow-frontend rule; repair the source side next."]

[[lab.steps.checks]]
name = "egress isolation selects the frontend"
run = '''kubectl -n policies-lab get networkpolicy deny-client-egress -o jsonpath='{.spec.podSelector.matchLabels.role}{" "}{.spec.policyTypes[*]}''''
equals = "frontend Egress"

[[lab.steps.checks]]
name = "allowed ingress is insufficient when source egress is blocked"
run = '''kubectl -n policies-lab get networkpolicy deny-client-egress >/dev/null && kubectl -n policies-lab wait --for=condition=Ready pod/client --timeout=20s >/dev/null && ip=$(kubectl -n policies-lab get pod backend -o jsonpath='{.status.podIP}') && [ -n "$ip" ] && for i in $(seq 1 15); do good=0; for j in 1 2 3; do if kubectl -n policies-lab exec client -- wget -qO- -T 2 "http://$ip" >/dev/null 2>&1; then break; else good=$((good+1)); fi; done; [ "$good" = 3 ] && echo blocked && exit 0; sleep 2; done; exit 1'''
equals = "blocked"
timeout = 120

[[lab.steps]]
title = "Restore DNS and the application path"
task = "Create NetworkPolicy allow-client-egress for role=frontend in policies-lab. Allow UDP and TCP 53 only to kube-system Pods labeled k8s-app=kube-dns, and TCP 80 only to app=backend Pods in policies-lab. Keep all earlier policies. Retrieve http://backend.policies-lab.svc.cluster.local from client; other destinations, such as the web server in policies-other-lab, must stay unreachable."
solution = '''```bash
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-client-egress, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {role: frontend}}
  policyTypes: [Egress]
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          kubernetes.io/metadata.name: kube-system
      podSelector:
        matchLabels:
          k8s-app: kube-dns
    ports:
    - {protocol: UDP, port: 53}
    - {protocol: TCP, port: 53}
  - to:
    - podSelector:
        matchLabels:
          app: backend
    ports:
    - {protocol: TCP, port: 80}
EOF
```'''
hints = ["Use separate egress rules for DNS and the application, so their ports and peers do not form an unintended cross-product.", "TCP DNS is required for responses that cannot be carried in a normal UDP exchange."]

[[lab.steps.checks]]
name = "DNS resolves and HTTP works with both directions isolated"
run = '''for i in $(seq 1 30); do v=$(kubectl -n policies-lab exec client -- wget -qO- -T 2 http://backend.policies-lab.svc.cluster.local); echo "$v" | grep -q "Welcome to nginx" && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 120

[[lab.steps.checks]]
name = "port 53 is open over both UDP and TCP, only towards kube-system"
run = '''v=$(kubectl -n policies-lab get networkpolicy allow-client-egress -o jsonpath='{.spec.egress[*].ports[?(@.port==53)].protocol}'); echo "$v" | grep -q UDP && echo "$v" | grep -q TCP && kubectl -n policies-lab get networkpolicy allow-client-egress -o jsonpath='{.spec.egress[*].to[*].namespaceSelector.matchLabels.kubernetes\.io/metadata\.name}''''
equals = "kube-system"

[[lab.steps.checks]]
name = "the client still cannot reach other web servers"
run = '''ip=$(kubectl -n policies-other-lab get pod elsewhere -o jsonpath='{.status.podIP}'); [ -n "$ip" ] || exit 1
kubectl -n policies-lab get networkpolicy allow-client-egress >/dev/null || exit 1
for i in $(seq 1 15); do
  if kubectl -n policies-lab exec client -- wget -qO- -T 2 "http://$ip" >/dev/null 2>&1; then sleep 2; else echo blocked; exit 0; fi
done; echo "client reaches $ip in policies-other-lab"'''
equals = "blocked"
timeout = 90

[[lab.steps.negatives]]
name = "allows TCP 80 to any destination instead of only the backend"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-client-egress, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {role: frontend}}
  policyTypes: [Egress]
  egress:
  - to:
    - namespaceSelector: {matchLabels: {kubernetes.io/metadata.name: kube-system}}
      podSelector: {matchLabels: {k8s-app: kube-dns}}
    ports:
    - {protocol: UDP, port: 53}
    - {protocol: TCP, port: 53}
  - ports:
    - {protocol: TCP, port: 80}
EOF
sleep 2
'''

[[lab.steps.negatives]]
name = "allows only UDP DNS"
run = '''
kubectl apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: allow-client-egress, namespace: policies-lab}
spec:
  podSelector: {matchLabels: {role: frontend}}
  policyTypes: [Egress]
  egress:
  - to:
    - namespaceSelector: {matchLabels: {kubernetes.io/metadata.name: kube-system}}
      podSelector: {matchLabels: {k8s-app: kube-dns}}
    ports:
    - {protocol: UDP, port: 53}
  - to:
    - podSelector: {matchLabels: {app: backend}}
    ports:
    - {protocol: TCP, port: 80}
EOF
sleep 2
'''
+++

## Isolation is a change to the default

Without NetworkPolicies selecting it, an ordinary Pod is non-isolated: Kubernetes networking allows its incoming and outgoing traffic, subject to other network controls. A NetworkPolicy selects Pods in its own namespace and describes allowed connections for ingress, egress, or both. It does not select Services or Deployments directly.

Selection causes isolation **per direction**. An ingress policy does not automatically isolate egress. An egress policy does not automatically isolate ingress. Once isolated, a connection in that direction must match an allowance in at least one applicable policy. Replies to an allowed connection are implicitly allowed; you do not need a reverse rule for every response packet.

```text
source Pod -- source egress rules -- network -- destination ingress rules -- destination Pod
                   must allow                         must allow
```

Both isolated sides must permit a new connection. Allowing the server's ingress cannot override the client's egress isolation. That is why this lab first restores server access and then deliberately breaks the source side.

## Policies are additive allow-lists

There is no first-match order or priority in the standard NetworkPolicy API. All applicable permissions are combined. A deny-all policy provides an empty starting set; another policy can add selected connections. It cannot subtract permissions from a broad allow policy that already selects the same Pod.

| Shape | Meaning |
|---|---|
| Top-level podSelector: {} | Select every Pod in this namespace |
| ingress: [] | No ingress allowance in this policy |
| ingress: [{}] | Allow all ingress to selected Pods |
| egress: [] | No egress allowance in this policy |
| egress: [{}] | Allow all egress from selected Pods |

The difference between an empty list and a list containing an empty rule is enormous. An empty peer selector also has different scope depending on where it appears. Read the entire YAML nesting, not just the braces.

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: deny-both
  namespace: policies-lab
spec:
  podSelector: {}
  policyTypes: [Ingress, Egress]
  ingress: []
  egress: []
```

Set policyTypes explicitly. Defaults can infer Ingress and infer Egress when egress rules are present, but an explicit deny-egress policy with no rules is easier to understand and safer to review when Egress is stated.

## Select a destination, then select its peers

At the top, podSelector selects the Pods being protected. In an ingress rule, `from` selects permitted **sources**. In an egress rule, `to` selects permitted **destinations**. The same selector vocabulary is used on both sides.

A peer podSelector by itself selects Pods in the policy's namespace. A namespaceSelector by itself selects Pods throughout matching namespaces. It matches **namespace labels**, not Pod labels. Kubernetes supplies the immutable `kubernetes.io/metadata.name` namespace label, which is useful when the task names a namespace rather than giving a team label.

```yaml
ingress:
- from:
  - namespaceSelector:
      matchLabels:
        kubernetes.io/metadata.name: policies-lab
    podSelector:
      matchLabels:
        role: frontend
  ports:
  - protocol: TCP
    port: 80
```

Here namespaceSelector **AND** podSelector must match. Only frontend Pods in policies-lab are peers. The port restriction is also required: this rule allows those peers on TCP 80, not every port.

## The classic AND versus OR trap

Compare the previous rule with this one:

```yaml
ingress:
- from:
  - namespaceSelector:
      matchLabels:
        kubernetes.io/metadata.name: policies-lab
  - podSelector:
      matchLabels:
        role: frontend
```

The second dash makes two alternative peers. This permits every Pod in policies-lab through the namespace peer, plus matching frontend Pods in the policy namespace through the Pod peer. It no longer requires a frontend label for all callers. With a different namespace selection, it can accidentally admit an entire remote namespace as well.

List entries are alternatives. Fields within one peer are combined restrictions. Multiple rules are also alternatives. Peers and ports within a single rule jointly restrict the connection, so splitting rules can widen permissions. Test a caller that should work **and** callers that should fail; a positive HTTP request alone cannot reveal an overbroad policy.

## IP ranges and their limits

An ipBlock peer specifies a CIDR, optionally excluding smaller ranges using except. For example, `cidr: 192.0.2.0/24` with `except: [192.0.2.128/25]` describes the lower half of that documentation address range. Use actual task-provided addresses in a real rule; these example addresses are not a reachable lab dependency.

ipBlock is most useful for external destinations. Pod IPs are ephemeral, and Service address translation can occur before or after policy processing depending on the network implementation. Avoid encoding a Service virtual IP as a portable substitute for selecting backend Pods. Standard NetworkPolicy also cannot match an HTTP path, DNS hostname, user identity or TLS certificate. It operates on peers and supported transport ports.

## Egress isolation includes DNS

Applications usually resolve a name before opening an HTTP connection. Default-deny egress blocks that lookup unless DNS is explicitly allowed. In this kind lab, CoreDNS Pods live in kube-system and carry `k8s-app=kube-dns`. Combine that Pod selector and namespace selector in one peer, and permit **UDP 53 and TCP 53**.

Use a separate egress rule for TCP 80 to backend Pods. Combining both DNS and backend destinations with all three ports in one rule would allow more combinations than required. Production clusters may use NodeLocal DNS or different labels, so inspect the actual resolver architecture rather than applying this destination blindly everywhere.

## Prove behavior with fresh connections

```bash
kubectl -n policies-lab get networkpolicies
kubectl -n policies-lab describe networkpolicy allow-frontend
kubectl -n policies-lab get pods --show-labels
kubectl get namespaces --show-labels
kubectl -n policies-lab exec client -- wget -qO- -T 2 http://backend
kubectl explain networkpolicy.spec.ingress
```

There is no built-in `kubectl create networkpolicy` generator; use an official example and adapt the selectors. PodDojo's kindnet enforces policies, but enforcement is a network-plugin capability, not an automatic consequence of API acceptance. A different cluster may store the policy while providing no enforcement.

Allow time for propagation. Repeated short wget requests test new connections; an existing long-lived connection is poor evidence immediately after a policy change. Test by Pod IP to separate HTTP reachability from DNS, then test by Service name to prove the complete path. A failed request is meaningful only if the source Pod and destination application are healthy.

## Exam tips

Sketch the traffic arrow and label each namespace before writing YAML. State policyTypes, inspect indentation, and test both allowed and excluded peers. When an application suddenly cannot reach anything after egress isolation, investigate DNS before changing server labels. Review the [official NetworkPolicy examples](https://kubernetes.io/docs/concepts/services-networking/network-policies/) for selector combinations and default-deny patterns.
