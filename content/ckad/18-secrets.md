+++
id = "ckad-secrets"
title = "Secrets: delivery, types and real protection"
module = "config-security"
minutes = 45
domain = "config-security"
needs = "cluster"
objectives = [
  "Choose appropriate Secret types and creation commands",
  "Explain data, stringData and the limits of base64 encoding",
  "Consume Secret values through environment references and file projections",
  "Configure imagePullSecrets and immutable Secret versions",
  "Identify encryption at rest and RBAC as essential protection layers",
]

[[quiz]]
q = "A Secret manifest contains base64 text in data. What protection does base64 provide?"
choices = ["Encryption with a cluster key", "One-way hashing", "Encoding only; anyone with the value can decode it", "Access control based on the Secret type"]
answer = 2
why = "Base64 represents bytes as text and is trivially reversible. Confidentiality depends on API access controls, encryption at rest, secure transport and careful handling by workloads and operators."

[[quiz]]
q = "Which field accepts plain strings on write but is not returned as such on a Secret read?"
choices = ["binaryData", "stringData", "plainText", "envFrom"]
answer = 1
why = "The API merges stringData into data, encoding the values for storage and output. If a key exists in both, stringData wins on that write. It is a convenience for input, not a separate storage mechanism."

[[quiz]]
q = "Which Secret type and key are used for modern registry credentials?"
choices = ["Opaque with imagePassword", "kubernetes.io/tls with tls.key", "kubernetes.io/dockerconfigjson with .dockerconfigjson", "kubernetes.io/basic-auth with registry"]
answer = 2
why = "The docker-registry generator produces kubernetes.io/dockerconfigjson with a .dockerconfigjson document. Reference it using imagePullSecrets in the same namespace as the Pod. It is used by the kubelet, not automatically injected into the application."

[[quiz]]
q = "What does YAML defaultMode: 0400 mean for a projected Secret file?"
choices = ["Owner read permission only", "World-readable", "Owner write permission only", "Four hundred replicas"]
answer = 0
why = "0400 is an octal permission mode. Its decimal equivalent is 256, which is what JSON and many API outputs show. File ownership and fsGroup can affect whether a non-root process can read the projection."

[[quiz]]
q = "Why can permission to create Pods expose Secrets even without direct get permission on Secrets?"
choices = ["All Secrets are public", "A Pod creator can arrange for a workload to mount available namespace Secrets", "ServiceAccounts always have cluster-admin", "Base64 bypasses authentication"]
answer = 1
why = "A workload can consume a namespace Secret through the kubelet even if its ServiceAccount cannot directly read that Secret through the API. Control workload creation as well as direct Secret API access; RBAC on Secret reads alone is insufficient."

[lab]
summary = "Use disposable training data to create typed Secrets, consume files and variables, and attach registry credentials without contacting a private registry."
setup = ["kubectl create namespace secrets-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace secrets-lab --ignore-not-found --wait=false || true", "rm -f \"$PODDOJO_LAB_DIR/training.key\" \"$PODDOJO_LAB_DIR/training.crt\" || true"]

[[lab.steps]]
title = "Create generic and basic-auth Secrets"
task = """
In `secrets-lab`, create Opaque Secret `app-secret` with `message=training-only`. Create `login-secret` of type `kubernetes.io/basic-auth`, with `username=student` and `password=unused-training-value`. These are deliberately public, nonfunctional exercise values; never substitute real credentials.
"""
hints = ["create secret generic defaults to type Opaque.", "Use stringData for readable YAML input and type for the basic-auth contract."]
solution = '''
```bash
kubectl -n secrets-lab create secret generic app-secret --from-literal=message=training-only --dry-run=client -o yaml | kubectl apply -f -
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Secret
metadata:
  name: login-secret
  namespace: secrets-lab
type: kubernetes.io/basic-auth
stringData:
  username: student
  password: unused-training-value
EOF
```
'''
[[lab.steps.checks]]
name = "the generic Secret has the requested training value"
run = '''t=$(kubectl -n secrets-lab get secret app-secret -o jsonpath='{.type}') && v=$(kubectl -n secrets-lab get secret app-secret -o jsonpath='{.data.message}' | base64 -d) && [ "$t" = Opaque ] && [ "$v" = training-only ] && echo ok'''
equals = "ok"
[[lab.steps.checks]]
name = "the typed login Secret has both required values"
run = '''t=$(kubectl -n secrets-lab get secret login-secret -o jsonpath='{.type}') && u=$(kubectl -n secrets-lab get secret login-secret -o jsonpath='{.data.username}' | base64 -d) && p=$(kubectl -n secrets-lab get secret login-secret -o jsonpath='{.data.password}' | base64 -d) && [ "$t" = kubernetes.io/basic-auth ] && [ "$u" = student ] && [ "$p" = unused-training-value ] && echo ok'''
equals = "ok"

[[lab.steps]]
title = "Consume a Secret as an environment variable and a restricted file"
task = """
Create `secret-reader` in `secrets-lab` using `busybox:1.36`, sleeping 3600 seconds. Set `MESSAGE` using `secretKeyRef` to `app-secret/message`. Mount the same Secret at `/run/app-secret`, read-only, with `defaultMode: 0400` (decimal 256). Read `/run/app-secret/message` and the variable; both must equal `training-only`.
"""
hints = ["Secret volumes use secretName; env references use secretKeyRef.name.", "This exercise container runs as root so it can read the owner-only projection."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: secret-reader
  namespace: secrets-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    env:
    - name: MESSAGE
      valueFrom:
        secretKeyRef:
          name: app-secret
          key: message
    volumeMounts:
    - name: credentials
      mountPath: /run/app-secret
      readOnly: true
  volumes:
  - name: credentials
    secret:
      secretName: app-secret
      defaultMode: 0400
EOF
kubectl -n secrets-lab wait --for=condition=Ready pod/secret-reader --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the environment and file contain the training value"
run = '''kubectl -n secrets-lab get pod secret-reader >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n secrets-lab exec secret-reader -- sh -c 'printf "%s %s" "$MESSAGE" "$(cat /run/app-secret/message)"' 2>/dev/null); [ "$v" = 'training-only training-only' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the projected file grants only owner read access"
run = '''kubectl -n secrets-lab get pod secret-reader >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n secrets-lab exec secret-reader -- stat -L -c '%a' /run/app-secret/message 2>/dev/null); [ "$v" = 400 ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the environment uses a Secret key reference"
run = "kubectl -n secrets-lab get pod secret-reader -o jsonpath='{.spec.containers[0].env[0].valueFrom.secretKeyRef.name} {.spec.containers[0].env[0].valueFrom.secretKeyRef.key}'"
equals = "app-secret message"

[[lab.steps]]
title = "Create TLS and registry credential formats"
task = """
Create `training-tls` in `secrets-lab` with a newly generated self-signed certificate for `dojo.example.com` and its matching key, using local lab files `training.crt` and `training.key`. Create `pull-secret` for registry `registry.example.invalid` with dummy username `student` and dummy password `unused-training-value`. No external login or push is required. The TLS solution needs the local `openssl` CLI.
"""
hints = ["openssl req -x509 -newkey with -nodes is non-interactive when -subj is supplied.", "kubectl create secret tls takes --cert and --key; docker-registry takes --docker-server, --docker-username and --docker-password."]
solution = '''
```bash
umask 077
openssl req -x509 -newkey rsa:2048 -nodes -keyout training.key -out training.crt -days 1 -subj '/CN=dojo.example.com' 2>/dev/null
kubectl -n secrets-lab create secret tls training-tls --cert=training.crt --key=training.key --dry-run=client -o yaml | kubectl apply -f -
kubectl -n secrets-lab create secret docker-registry pull-secret --docker-server=registry.example.invalid --docker-username=student --docker-password=unused-training-value --dry-run=client -o yaml | kubectl apply -f -
```
These generated credentials belong only to this disposable exercise.
'''
[[lab.steps.checks]]
name = "the TLS Secret contains a certificate and matching public key"
run = '''[ "$(kubectl -n secrets-lab get secret training-tls -o jsonpath='{.type}')" = kubernetes.io/tls ] || exit 1; c=$(kubectl -n secrets-lab get secret training-tls -o jsonpath='{.data.tls\.crt}' | base64 -d | openssl x509 -pubkey -noout 2>/dev/null); k=$(kubectl -n secrets-lab get secret training-tls -o jsonpath='{.data.tls\.key}' | base64 -d | openssl pkey -pubout 2>/dev/null); [ -n "$c" ] && [ "$c" = "$k" ] && echo ok'''
equals = "ok"
[[lab.steps.checks]]
name = "the registry Secret has the expected type and registry entry"
run = '''[ "$(kubectl -n secrets-lab get secret pull-secret -o jsonpath='{.type}')" = kubernetes.io/dockerconfigjson ] || exit 1; kubectl -n secrets-lab get secret pull-secret -o jsonpath='{.data.\.dockerconfigjson}' | base64 -d'''
contains = "registry.example.invalid"

[[lab.steps]]
title = "Freeze the app Secret and reference image-pull credentials"
task = """
Mark `app-secret` immutable. Create `pull-client` in `secrets-lab`, image `busybox:1.36`, sleeping 3600 seconds, with `imagePullSecrets` referencing `pull-secret`. The image is public and does not use the fake registry entry; this step verifies the reference, not private-registry authentication.
"""
hints = ["Patch top-level immutable, not data.immutable.", "imagePullSecrets is a Pod-spec list of objects with name fields."]
solution = '''
```bash
kubectl -n secrets-lab patch secret app-secret --type=merge -p '{"immutable":true}'
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: pull-client
  namespace: secrets-lab
spec:
  imagePullSecrets:
  - name: pull-secret
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
EOF
kubectl -n secrets-lab wait --for=condition=Ready pod/pull-client --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the application Secret is immutable"
run = "kubectl -n secrets-lab get secret app-secret -o jsonpath='{.immutable}'"
equals = "true"
[[lab.steps.checks]]
name = "the image-pull reference is attached to the client Pod"
run = "kubectl -n secrets-lab get pod pull-client -o jsonpath='{.spec.imagePullSecrets[*].name}'"
contains = "pull-secret"
+++

## A delivery object, not a vault by itself

A Secret delivers confidential bytes to workloads without putting them into an image. Like a ConfigMap, it is namespaced and can become environment variables or projected files. Unlike a ConfigMap, it has types for credential formats and receives special handling by components such as the kubelet. None of that means a base64 value is encrypted. Anyone who obtains the encoded value can recover its original bytes.

```text
author's input → API access controls → Secret → kubelet → selected container
                     │                           │
              encrypted storage?          file or environment
```

Treat each arrow as a possible exposure point. A Secret manifest, a terminal transcript, a debug print, or an application log can leak the same credential. The lab uses conspicuously fake values and a disposable certificate. In real work, do not paste credentials into source control or shell arguments that may be recorded in history or process listings.

## Types describe how consumers interpret data

| Type | Conventional keys | Intended consumer |
|---|---|---|
| `Opaque` | Application-defined | Your application |
| `kubernetes.io/tls` | `tls.crt`, `tls.key` | TLS endpoint or controller |
| `kubernetes.io/dockerconfigjson` | `.dockerconfigjson` | Kubelet registry authentication |
| `kubernetes.io/basic-auth` | `username`, `password` | Basic authentication integration |

The API validates aspects of built-in types, such as required keys and registry JSON structure; a type label does not prove a credential works. A TLS consumer still needs a usable certificate chain and private key. A registry entry still needs valid permissions on the correct registry. Generic application Secrets remain `Opaque` unless you specify another type.

```bash
kubectl -n secrets-lab create secret generic app-secret \
  --from-file=message=message.txt --dry-run=client -o yaml
kubectl -n secrets-lab create secret tls training-tls \
  --cert=training.crt --key=training.key --dry-run=client -o yaml
```

`--from-file` avoids placing the value itself in command arguments. Protect the source file as well. `kubectl create secret docker-registry` generates the registry document for you; `create secret generic --from-file=.dockerconfigjson=... --type=kubernetes.io/dockerconfigjson` imports an existing one.

## data and stringData are two input representations

`data` maps keys to base64-encoded byte strings. `stringData` accepts ordinary strings on write and is merged into `data`; it is not returned as a separate readable storage field. If both specify a key in one request, `stringData` takes precedence. It is convenient for examples, but has caveats with server-side apply ownership, so prefer a consistent generator or managed delivery workflow.

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: app-secret
  namespace: secrets-lab
type: Opaque
stringData:
  message: training-only
```

For exact manual encoding, `printf %s VALUE` avoids adding a newline. An accidentally encoded newline can break authentication even when the displayed text looks correct. Secrets have a 1 MiB size limit; they are not a way to distribute large archives. Read only the required key during diagnosis and avoid dumping entire production Secret objects.

## Environment and file consumption

Use `env[].valueFrom.secretKeyRef` to select and rename a key. `envFrom[].secretRef` imports all keys and also supports a prefix. Explicit environment entries override imported collisions. Environment values are startup snapshots: updating the Secret does not rewrite a running process's environment or trigger a Deployment rollout.

```yaml
env:
- name: MESSAGE
  valueFrom:
    secretKeyRef:
      name: app-secret
      key: message
```

A volume references `secret.secretName`, then a container mounts the volume. `items` can select and rename specific keys. The default file permission is `0644`; choose a tighter `defaultMode` where appropriate. YAML `0400` means owner-read only, while JSON uses decimal `256`. The mode applies to projected files, not the volume directory. Check effective access with the actual runtime UID and groups; a non-root application cannot necessarily read a root-owned `0400` file. Using `fsGroup` can change effective ownership and permissions.

Normal Secret projections refresh eventually, but application reload behavior remains the application's responsibility. A file mounted with `subPath` does not receive automatic Secret updates. Prefer directory projections when rotation should be visible, and remember that an open descriptor can still refer to old bytes. Missing required Secrets or keys prevent startup; inspect events instead of weakening a reference with `optional: true` by habit.

## Image pull credentials act before your application starts

`imagePullSecrets` belongs in the Pod spec and references Secrets in the same namespace. The kubelet uses matching registry credentials while obtaining images. These values do not automatically become environment variables or files in the application. A ServiceAccount can supply image-pull references to newly admitted Pods as well.

```yaml
spec:
  imagePullSecrets:
  - name: pull-secret
  containers:
  - name: app
    image: busybox:1.36
```

When an image pull fails, distinguish a missing Secret, a wrong registry hostname, invalid authentication, and an image tag that does not exist. `kubectl describe pod` exposes events such as retrieval or pull failures. A cached public image succeeding does not prove private-registry credentials are valid; the lab deliberately checks only reference wiring.

In a multi-container Pod, reference a Secret only from the containers that need it. Defining a Pod volume does not require mounting it into every sidecar. A logging container that only reads application logs should not automatically receive the application's database credentials. This is a practical reason to prefer an explicit key or narrow file projection over indiscriminately importing every Secret key. Inspect the Pod template in its owning Deployment as well as the current Pod: repairing only a disposable Pod can leave the next replacement using the old reference. ServiceAccount changes likewise do not retroactively rewrite the image-pull references of already admitted Pods.

## Protection, rotation and exam tips

An immutable Secret has top-level `immutable: true`. Data cannot be edited afterward and immutability cannot be reversed. Rotate by creating a new named version, changing the workload reference, and replacing its Pods. Coordinate credential rotation with the external service so both the producer and consumer agree during transition.

Real protection requires restricted API access and encryption at rest configured for Secret storage, alongside TLS in transit. RBAC should limit reads, listing and watching: list access can expose values just as get access can. Also restrict workload creation, since a Pod creator can mount namespace Secrets indirectly. Container compromise, node access and careless logging remain relevant even with encrypted storage.

In the exam, select the correct type, namespace, key and consumer field first. Generate the manifest, inspect the Pod's events, and verify only the requested value or filename. Do not confuse `imagePullSecrets` with `secretKeyRef`, base64 with encryption, or a successful API write with successful application authentication.
