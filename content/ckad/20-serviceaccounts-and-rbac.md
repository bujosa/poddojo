+++
id = "ckad-serviceaccounts-and-rbac"
title = "ServiceAccounts, RBAC and admission"
module = "config-security"
minutes = 50
domain = "config-security"
needs = "cluster"
objectives = [
  "Separate authentication, authorization and admission in an API request",
  "Assign ServiceAccounts and choose whether to mount API credentials",
  "Grant narrowly scoped permissions with roles and bindings",
  "Test authorization by impersonating a ServiceAccount",
  "Recognize Pod Security Admission and ValidatingAdmissionPolicy responsibilities",
]

[[quiz]]
q = "A valid ServiceAccount token identifies the caller, but the caller cannot list Pods. Which stage answers whether listing is permitted?"
choices = ["Authentication", "Authorization", "Scheduling", "Container image verification"]
answer = 1
why = "Authentication establishes identity; authorization evaluates that identity's requested action. Admission can then default, mutate or reject eligible write requests based on object policy. Having a token alone grants no general workload permissions."

[[quiz]]
q = "A RoleBinding in namespace red references a ClusterRole allowing get on Pods. What scope does that binding grant?"
choices = ["Every namespace", "Only namespace red for those namespaced resources", "Only cluster-scoped resources", "Only the namespace containing the ServiceAccount"]
answer = 1
why = "A RoleBinding limits the grant to its own namespace even when it references reusable rules from a ClusterRole. The subject's namespace identifies the account; it does not move the binding's permission scope."

[[quiz]]
q = "Which apiGroups value selects core resources such as Pods and ConfigMaps?"
choices = ["v1", "core/v1", "An empty string", "kubernetes.io"]
answer = 2
why = "RBAC uses API groups, not apiVersion strings. Core resources use apiGroups: [\"\"], Deployments use apps, and Jobs use batch. Resource names are normally lowercase plurals."

[[quiz]]
q = "What does automountServiceAccountToken: false on a Pod do?"
choices = ["Deletes its ServiceAccount", "Removes all RBAC grants", "Disables automatic API token projection for that Pod", "Prevents explicit projected token volumes"]
answer = 2
why = "The Pod still has its assigned identity and any RBAC grants, but automatic credential injection is disabled. You can explicitly project a token if needed. A Pod-level setting overrides the ServiceAccount's default setting."

[[quiz]]
q = "kubectl auth can-i create pods returns yes, but a privileged Pod is rejected in a restricted namespace. Why?"
choices = ["Authorization is broken", "Admission can reject object content even when the action is authorized", "Restricted namespaces have no API", "ServiceAccounts cannot create Pods"]
answer = 1
why = "can-i evaluates authorization, not every admission rule. Pod Security Admission can reject the Pod's security settings, just as quota admission can reject missing resources. A successful permission test does not promise every object will be accepted."

[lab]
summary = "Give a dedicated workload identity limited ConfigMap access, add a narrow cluster-level grant, and distinguish token delivery from admission policy."
setup = ["kubectl create namespace rbac-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete clusterrolebinding dojo-rbac-discovery --ignore-not-found || true", "kubectl delete clusterrole dojo-rbac-discovery --ignore-not-found || true", "kubectl delete namespace rbac-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Create an identity without automatic credentials"
task = """
In `rbac-lab`, create ServiceAccount `reader` with `automountServiceAccountToken: false`. Create Pod `identity-client` using that account, image `busybox:1.36` and `sleep 3600`. The Pod must not receive an automatic API credential volume.
"""
hints = ["The ServiceAccount's automountServiceAccountToken is top-level.", "Assign it through spec.serviceAccountName on the Pod."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: ServiceAccount
metadata:
  name: reader
  namespace: rbac-lab
automountServiceAccountToken: false
---
apiVersion: v1
kind: Pod
metadata:
  name: identity-client
  namespace: rbac-lab
spec:
  serviceAccountName: reader
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
EOF
kubectl -n rbac-lab wait --for=condition=Ready pod/identity-client --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the ServiceAccount disables automatic token projection"
run = "kubectl -n rbac-lab get sa reader -o jsonpath='{.automountServiceAccountToken}'"
equals = "false"
[[lab.steps.checks]]
name = "the assigned workload has no default API credential directory"
run = '''kubectl -n rbac-lab get pod identity-client >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do sa=$(kubectl -n rbac-lab get pod identity-client -o jsonpath='{.spec.serviceAccountName}' 2>/dev/null); if [ "$sa" = reader ] && kubectl -n rbac-lab exec identity-client -- test ! -e /var/run/secrets/kubernetes.io/serviceaccount/token 2>/dev/null; then echo ok; exit 0; fi; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Grant namespace-local ConfigMap reads"
task = """
Create Role `config-reader` in `rbac-lab`, granting only `get`, `list`, and `watch` on core `configmaps`. Bind it to ServiceAccount `rbac-lab:reader` using RoleBinding `config-reader`. Test impersonation: the account must be able to get/list/watch ConfigMaps in `rbac-lab`, but must not get Secrets there or list ConfigMaps in `default`.
"""
hints = ["kubectl create role takes --verb and --resource.", "kubectl create rolebinding takes --role and --serviceaccount=NAMESPACE:NAME."]
solution = '''
```bash
kubectl -n rbac-lab create role config-reader --verb=get,list,watch --resource=configmaps --dry-run=client -o yaml | kubectl apply -f -
kubectl -n rbac-lab create rolebinding config-reader --role=config-reader --serviceaccount=rbac-lab:reader --dry-run=client -o yaml | kubectl apply -f -
kubectl -n rbac-lab auth can-i list configmaps --as=system:serviceaccount:rbac-lab:reader
```
'''
[[lab.steps.checks]]
name = "the grant allows ConfigMap reads while preserving namespace and Secret boundaries"
run = '''kubectl -n rbac-lab get rolebinding config-reader >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do g=$(kubectl -n rbac-lab auth can-i get configmaps --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); l=$(kubectl -n rbac-lab auth can-i list configmaps --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); w=$(kubectl -n rbac-lab auth can-i watch configmaps --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); s=$(kubectl -n rbac-lab auth can-i get secrets --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); o=$(kubectl -n default auth can-i list configmaps --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); [ "$g $l $w $s $o" = 'yes yes yes no no' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Grant one cluster-scoped read action"
task = """
Create ClusterRole `dojo-rbac-discovery` granting only `get` on core `namespaces`. Bind it to `rbac-lab:reader` with ClusterRoleBinding `dojo-rbac-discovery`. The account must gain get access to namespaces without gaining delete access. These two dojo-prefixed cluster objects have dedicated cleanup.
"""
hints = ["A namespace object is cluster-scoped; a namespace RoleBinding cannot grant access to it.", "Use create clusterrole and create clusterrolebinding."]
solution = '''
```bash
kubectl create clusterrole dojo-rbac-discovery --verb=get --resource=namespaces --dry-run=client -o yaml | kubectl apply -f -
kubectl create clusterrolebinding dojo-rbac-discovery --clusterrole=dojo-rbac-discovery --serviceaccount=rbac-lab:reader --dry-run=client -o yaml | kubectl apply -f -
kubectl auth can-i get namespaces --as=system:serviceaccount:rbac-lab:reader
```
'''
[[lab.steps.checks]]
name = "the cluster grant allows reading but not deleting namespaces"
run = '''kubectl get clusterrolebinding dojo-rbac-discovery >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do r=$(kubectl auth can-i get namespaces --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); d=$(kubectl auth can-i delete namespaces --as=system:serviceaccount:rbac-lab:reader 2>/dev/null); [ "$r $d" = 'yes no' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Explicitly project a short-lived token"
task = """
Create `token-client` in `rbac-lab`, using account `reader`, `automountServiceAccountToken: false`, `busybox:1.36` and `sleep 3600`. Explicitly project a ServiceAccount token with `expirationSeconds: 600` and path `token`, mounted read-only at `/var/run/dojo-token`. Also request a ten-minute token using `kubectl create token`, discarding its output rather than exposing it in the terminal.
"""
hints = ["Use volumes[].projected.sources[].serviceAccountToken.", "A requested duration can be adjusted by the API server; projected tokens are rotated by the kubelet."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: token-client
  namespace: rbac-lab
spec:
  serviceAccountName: reader
  automountServiceAccountToken: false
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    volumeMounts:
    - name: api-token
      mountPath: /var/run/dojo-token
      readOnly: true
  volumes:
  - name: api-token
    projected:
      sources:
      - serviceAccountToken:
          path: token
          expirationSeconds: 600
EOF
kubectl -n rbac-lab wait --for=condition=Ready pod/token-client --timeout=120s
kubectl -n rbac-lab create token reader --duration=10m >/dev/null
```
Never log or commit the returned bearer token.
'''
[[lab.steps.checks]]
name = "an explicit projected token is present without printing it"
run = '''kubectl -n rbac-lab get pod token-client >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do kubectl -n rbac-lab exec token-client -- test -s /var/run/dojo-token/token 2>/dev/null && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the requested projection has a ten-minute lifetime"
run = "kubectl -n rbac-lab get pod token-client -o jsonpath='{.spec.serviceAccountName} {.spec.automountServiceAccountToken} {.spec.volumes[?(@.name==\"api-token\")].projected.sources[0].serviceAccountToken.expirationSeconds}'"
equals = "reader false 600"

[[lab.steps]]
title = "Enforce Pod Security Admission for future Pods"
task = """
Label only `rbac-lab` with `pod-security.kubernetes.io/enforce=restricted` and `pod-security.kubernetes.io/enforce-version=v1.37`. Existing Pods are not evicted by this label. Create `admitted-client` using `busybox:1.36`, `sleep 3600`, UID 1000, `runAsNonRoot: true`, RuntimeDefault seccomp, disabled privilege escalation and all capabilities dropped. Wait for Ready.
"""
hints = ["Use kubectl label namespace with --overwrite.", "Put identity and seccomp at Pod level; privilege escalation and capabilities belong on the container."]
solution = '''
```bash
kubectl label namespace rbac-lab pod-security.kubernetes.io/enforce=restricted pod-security.kubernetes.io/enforce-version=v1.37 --overwrite
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: admitted-client
  namespace: rbac-lab
spec:
  serviceAccountName: reader
  securityContext:
    runAsUser: 1000
    runAsNonRoot: true
    seccompProfile:
      type: RuntimeDefault
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    securityContext:
      allowPrivilegeEscalation: false
      capabilities:
        drop: ["ALL"]
EOF
kubectl -n rbac-lab wait --for=condition=Ready pod/admitted-client --timeout=120s
```
'''
[[lab.steps.checks]]
name = "restricted admission is pinned to the requested policy version"
run = '''kubectl get namespace rbac-lab -o jsonpath='{.metadata.labels.pod-security\.kubernetes\.io/enforce} {.metadata.labels.pod-security\.kubernetes\.io/enforce-version}' '''
equals = "restricted v1.37"
[[lab.steps.checks]]
name = "a compliant new workload is admitted and becomes Ready"
run = '''kubectl -n rbac-lab get pod admitted-client >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n rbac-lab get pod admitted-client -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null); [ "$v" = True ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
+++

## Three gates before a workload can exist

A request to create a Pod passes through several responsibilities. **Authentication** determines who sent it. **Authorization** asks whether that identity may perform the requested action on the resource. **Admission** evaluates an eligible write and can supply defaults, mutate fields or reject its content. Schema validation and persistence are also part of request processing; this diagram isolates the security questions:

```text
credentials → authentication → authorization → admission → accepted object
                 who?             may?          acceptable?
```

A missing or invalid credential often produces an authentication failure. A recognized identity lacking permission receives Forbidden. Admission can also reject an authorized request, for example because its Pod is privileged or exceeds quota. Read the full error rather than assuming every rejection is an RBAC problem. Ordinary reads do not pass through mutating and validating admission in the same way as object writes.

## ServiceAccounts identify workloads

A ServiceAccount is a namespaced identity. Every namespace normally has a `default` account, and Pods use it unless `spec.serviceAccountName` selects another. An account named `reader` in `rbac-lab` authenticates as `system:serviceaccount:rbac-lab:reader`. Another namespace's `reader` is a different identity.

```bash
kubectl -n rbac-lab create serviceaccount reader --dry-run=client -o yaml
kubectl -n rbac-lab create token reader --duration=10m >/dev/null
```

The second command uses TokenRequest to obtain a bearer token; its requested lifetime is subject to server policy. Discarding output is useful when practicing the operation without exposing a credential. For a real consumer, deliver the returned token through an approved confidential channel rather than a log or repository.

Modern in-cluster credentials use projected, time-bound tokens that the kubelet rotates. They replace the old habit of expecting an automatically generated long-lived token Secret for every ServiceAccount. Applications must reread rotating token files rather than caching a bearer string indefinitely. The token's audience determines the intended recipient; requesting an external audience does not automatically make that token valid for the Kubernetes API.

Set `automountServiceAccountToken: false` on an account or Pod when API access is unnecessary. Pod-level configuration takes precedence. This controls automatic credential delivery, not RBAC permission, and explicit projection remains possible. The lab shows that distinction without printing a token. An explicitly projected token alone also does not supply every client input: an API client may need a CA bundle and server address.

## Roles describe actions; bindings connect identities

All four RBAC objects use `rbac.authorization.k8s.io/v1`:

| Rules | Binding | Effective scope |
|---|---|---|
| Role | RoleBinding | Binding's namespace |
| ClusterRole | RoleBinding | Binding's namespace for namespaced resources |
| ClusterRole | ClusterRoleBinding | Cluster-wide, including applicable namespaced resources |

A ClusterRole is reusable rules, not an automatic cluster-wide grant. The binding determines where those rules are granted. Cluster-scoped resources such as namespaces require an appropriate cluster-level grant. A RoleBinding may name a ServiceAccount from another namespace; the subject namespace identifies the account, while the binding namespace determines the grant's scope.

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: config-reader
  namespace: rbac-lab
rules:
- apiGroups: [""]
  resources: ["configmaps"]
  verbs: ["get", "list", "watch"]
```

Use an empty string for the core API group, `apps` for Deployments, and `batch` for Jobs. These are group names, not `v1` or `apps/v1`. Resources are API resource names, normally plural and lowercase. Subresources are explicit: reading `pods` does not automatically grant reading `pods/log`; executing into a container uses `pods/exec` and is a powerful permission.

## Narrow grants are easier to reason about

Common verbs include `get`, `list`, `watch`, `create`, `update`, `patch` and `delete`. They are independent permissions: being able to read one object does not inherently permit listing the collection. RBAC grants are additive; there is no deny rule that cancels a broad grant elsewhere. Avoid wildcards and broad built-in administrative roles when the task asks for one narrow action.

```bash
kubectl -n rbac-lab create role config-reader \
  --verb=get,list,watch --resource=configmaps --dry-run=client -o yaml
kubectl -n rbac-lab create rolebinding config-reader \
  --role=config-reader --serviceaccount=rbac-lab:reader --dry-run=client -o yaml
```

The binding stores a `roleRef` with `apiGroup`, `kind` and `name`, and a `subjects` list. `roleRef` is immutable: changing the referenced role requires replacing the binding rather than patching that field in place. Permissions to create workloads, mount Secrets or bind powerful roles can enable escalation; least privilege involves more than hiding direct Secret reads.

For some tasks, `resourceNames` narrows a rule to named existing objects, such as one ConfigMap. It does not constrain object creation by a chosen name, because the authorization decision for collection-level create does not use that field in the required way. List and watch also need a matching name field selector when restricted with resourceNames. Avoid guessing from the appearance of a role: test the actual operation. A named-object read grant can pass `can-i get configmap/NAME` while a collection-wide listing remains forbidden. Also remember that editing an existing Role cannot remove permissions the same identity receives from an unrelated binding elsewhere.

## Verify the identity you actually mean

```bash
kubectl -n rbac-lab auth can-i list configmaps \
  --as=system:serviceaccount:rbac-lab:reader
```

Testing as your current administrator identity proves little about the workload. Impersonation tests require the caller to have permission to impersonate that identity; the lab administrator has this ability. A negative `can-i` answer returns a failing exit status, which matters in scripts. Test one required action plus nearby forbidden actions and the namespace boundary. Allow brief propagation time after binding changes.

## Admission evaluates the object itself

Admission controllers include ServiceAccount defaulting, LimitRanger, ResourceQuota and Pod Security Admission. Pod Security Admission implements the Pod Security Standards using namespace labels such as `pod-security.kubernetes.io/enforce=restricted`. `warn` reports violations to the requester; `audit` records them for auditing; `enforce` rejects violating Pods. Version labels pin the policy definition. Labeling a namespace does not evict its existing Pods.

`ValidatingAdmissionPolicy` provides declarative validation using CEL expressions within the API server. A policy defines matching and validation logic; a `ValidatingAdmissionPolicyBinding` determines where it applies and its validation actions. It can enforce organization-specific object constraints beyond RBAC. It validates content and does not replace identity checks or grant permissions. You need not install a webhook server to understand or use this mechanism.

## Exam tips

Write down identity, verb, resource, API group and scope before creating RBAC. Generate roles and bindings imperatively, then inspect their YAML. If `can-i` says yes but creation fails, investigate admission and schema errors. Keep cluster-scoped lab objects uniquely dojo-prefixed, and clean up bindings as well as roles so no accidental access remains after practice.
