+++
id = "ckad-security-contexts"
title = "Security contexts: identity, privileges and writable paths"
module = "config-security"
minutes = 40
domain = "config-security"
needs = "cluster"
objectives = [
  "Distinguish Pod defaults from container-specific security settings",
  "Set process identity and shared-volume group access",
  "Disable privilege escalation and unnecessary Linux capabilities",
  "Combine a read-only root filesystem with explicit writable volumes",
  "Build and verify a workload accepted by the restricted Pod Security Standard",
]

[[quiz]]
q = "Where does fsGroup belong?"
choices = ["The container's securityContext", "The Pod's securityContext", "The emptyDir volume settings", "The PersistentVolumeClaim spec"]
answer = 1
why = "fsGroup is a Pod-level setting for supplemental group membership and supported volume access. It is not a container-level field and does not change ownership of files baked into the image."

[[quiz]]
q = "A Pod sets runAsUser: 1000, but one container sets runAsUser: 2000. Which UID does that container use?"
choices = ["0", "1000", "2000", "The image's USER"]
answer = 2
why = "Where both levels support a setting, a container-level value overrides the Pod default for that container. Pod-level volume settings such as fsGroup still apply at their own scope."

[[quiz]]
q = "Does runAsNonRoot: true automatically choose a non-root user?"
choices = ["Yes, it always selects UID 1000", "No, it asserts non-root execution; the image or runAsUser must provide a verifiable nonzero UID", "Yes, it creates a user account", "No, it only changes file permissions"]
answer = 1
why = "runAsNonRoot is a safety assertion. If the runtime would run as UID 0, or Kubernetes cannot verify a named image user as non-root, startup can fail. Setting an appropriate numeric runAsUser makes the intended identity explicit."

[[quiz]]
q = "An application needs to write temporary files, but readOnlyRootFilesystem is true. What should you do?"
choices = ["Make the container privileged", "Mount a writable emptyDir at the required temporary path", "Add the DAC_OVERRIDE capability", "Set runAsUser: 0"]
answer = 1
why = "The root filesystem can remain read-only while a separately mounted volume is writable. Provide only the paths the application needs, with ownership compatible with its runtime UID and groups."

[[quiz]]
q = "Which statement about the restricted Pod Security Standard is accurate for Linux Pods?"
choices = ["It requires a read-only root filesystem in every case", "It permits privileged containers", "It requires dropping ALL capabilities and allows adding back only NET_BIND_SERVICE", "It allows adding NET_ADMIN when runAsNonRoot is true"]
answer = 2
why = "Restricted limits privilege escalation, identity, seccomp, capabilities and other fields. A read-only root filesystem is valuable additional hardening but is not itself a universal Restricted requirement. Admission policy and RBAC answer different questions."

[lab]
summary = "Run as a non-root identity, verify Linux process restrictions, and restore only the writable storage a hardened application needs."
setup = ["kubectl create namespace security-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace security-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Set identity and shared-volume group access"
task = """
Create Pod `identity` in `security-lab`, image `busybox:1.36`, running `sleep 3600`. Set Pod-level `runAsUser: 1000`, `runAsGroup: 3000`, `runAsNonRoot: true`, and `fsGroup: 2000`. Mount an `emptyDir` at `/data`. Verify UID 1000, primary GID 3000, supplemental group 2000, and group ownership 2000 on `/data`.
"""
hints = ["These four fields belong under spec.securityContext.", "kubectl exec identity -- id shows the process identity; stat -c %g shows a directory's group."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: identity
  namespace: security-lab
spec:
  securityContext:
    runAsUser: 1000
    runAsGroup: 3000
    runAsNonRoot: true
    fsGroup: 2000
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    volumeMounts:
    - name: data
      mountPath: /data
  volumes:
  - name: data
    emptyDir: {}
EOF
kubectl -n security-lab wait --for=condition=Ready pod/identity --timeout=120s
kubectl -n security-lab exec identity -- id
```
'''
[[lab.steps.checks]]
name = "the process has the requested user and primary group"
run = '''kubectl -n security-lab get pod identity >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n security-lab exec identity -- sh -c 'printf "%s %s" "$(id -u)" "$(id -g)"' 2>/dev/null); [ "$v" = '1000 3000' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the shared group is present and owns the volume"
run = '''kubectl -n security-lab get pod identity >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do if kubectl -n security-lab exec identity -- sh -c 'case " $(id -G) " in *" 2000 "*) [ "$(stat -c %g /data)" = 2000 ];; *) exit 1;; esac' 2>/dev/null; then echo ok; exit 0; fi; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Harden a second container and inspect the runtime"
task = """
Create Pod `hardened` in `security-lab`, image `busybox:1.36`, running `sleep 3600` as UID 1000 and GID 3000 with non-root enforcement. Use Pod seccomp `RuntimeDefault`. On the container, set `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true`, `privileged: false`, and drop `ALL` capabilities with none added. Verify runtime flags without attempting to change privileges or write to the root filesystem.
"""
hints = ["Privilege escalation, capabilities and root-filesystem controls belong to the container's securityContext.", "Inspect /proc/1/status for NoNewPrivs, Seccomp and CapEff; /proc/mounts records the root mount's flags."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: hardened
  namespace: security-lab
spec:
  securityContext:
    runAsUser: 1000
    runAsGroup: 3000
    runAsNonRoot: true
    seccompProfile:
      type: RuntimeDefault
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      privileged: false
      capabilities:
        drop: ["ALL"]
EOF
kubectl -n security-lab wait --for=condition=Ready pod/hardened --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the declared container controls and seccomp profile are enabled"
run = "kubectl -n security-lab get pod hardened -o jsonpath='{.spec.securityContext.seccompProfile.type} {.spec.containers[0].securityContext.allowPrivilegeEscalation} {.spec.containers[0].securityContext.readOnlyRootFilesystem} {.spec.containers[0].securityContext.capabilities.drop[*]}'"
equals = "RuntimeDefault false true ALL"
[[lab.steps.checks]]
name = "the runtime process cannot gain privileges and has no effective capabilities"
run = '''kubectl -n security-lab get pod hardened >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do if kubectl -n security-lab exec hardened -- sh -c 'awk "/^NoNewPrivs:/ {n=\$2} /^Seccomp:/ {s=\$2} /^CapEff:/ {c=\$2} END {exit !(n==1 && s==2 && c ~ /^0+$/)}" /proc/1/status' 2>/dev/null; then echo ok; exit 0; fi; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the container root mount is read-only"
run = '''kubectl -n security-lab get pod hardened >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do if kubectl -n security-lab exec hardened -- sh -c 'awk '\''$2=="/" {if ($4 ~ /(^|,)ro(,|$)/) found=1} END {exit !found}'\'' /proc/mounts' 2>/dev/null; then echo ok; exit 0; fi; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Allow one writable path under restricted admission"
task = """
Label `security-lab` with `pod-security.kubernetes.io/enforce=restricted` and `pod-security.kubernetes.io/enforce-version=v1.37`. Create Pod `writable` using `busybox:1.36` with the same hardening as `hardened`, plus `fsGroup: 2000` and a writable `emptyDir` mounted at `/work`. Its startup command must write `ready` followed by a newline into `/work/ready`, then sleep 3600 seconds. Keep the root filesystem read-only.
"""
hints = ["A writable volume is separate from the image root filesystem.", "The fsGroup permits this non-root process to create the marker inside the emptyDir."]
solution = '''
```bash
kubectl label namespace security-lab pod-security.kubernetes.io/enforce=restricted pod-security.kubernetes.io/enforce-version=v1.37 --overwrite
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: writable
  namespace: security-lab
spec:
  securityContext:
    runAsUser: 1000
    runAsGroup: 3000
    runAsNonRoot: true
    fsGroup: 2000
    seccompProfile:
      type: RuntimeDefault
  containers:
  - name: app
    image: busybox:1.36
    command: ["sh", "-c", "printf 'ready\\n' > /work/ready; exec sleep 3600"]
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      privileged: false
      capabilities:
        drop: ["ALL"]
    volumeMounts:
    - name: work
      mountPath: /work
  volumes:
  - name: work
    emptyDir: {}
EOF
kubectl -n security-lab wait --for=condition=Ready pod/writable --timeout=120s
```
'''
[[lab.steps.checks]]
name = "restricted admission is enabled for this lesson namespace"
run = '''kubectl get namespace security-lab -o jsonpath='{.metadata.labels.pod-security\.kubernetes\.io/enforce} {.metadata.labels.pod-security\.kubernetes\.io/enforce-version}' '''
equals = "restricted v1.37"
[[lab.steps.checks]]
name = "the non-root application successfully writes only its volume marker"
run = '''kubectl -n security-lab get pod writable >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n security-lab exec writable -- sh -c 'printf "%s %s %s" "$(id -u)" "$(id -g)" "$(cat /work/ready)"' 2>/dev/null); [ "$v" = '1000 3000 ready' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "writable storage is an emptyDir and root hardening remains enabled"
run = '''r=$(kubectl -n security-lab get pod writable -o jsonpath='{.spec.containers[0].securityContext.readOnlyRootFilesystem}'); e=$(kubectl -n security-lab get pod writable -o go-template='{{range .spec.volumes}}{{if eq .name "work"}}{{if ne .emptyDir nil}}emptyDir{{end}}{{end}}{{end}}'); [ "$r $e" = 'true emptyDir' ] && echo ok'''
equals = "ok"
+++

## Security has several independent dimensions

Running as a non-root user is useful, but it does not answer every security question. What files can the process write? Which kernel capabilities remain? Can executing a special binary grant more privilege? Which system calls can it invoke? A security context makes these choices explicit rather than inheriting every image or runtime default.

```text
Pod securityContext ── defaults: UID, GID, non-root, seccomp
        │             shared-volume access: fsGroup
        ├── container A securityContext → local overrides and privileges
        └── container B securityContext → local overrides and privileges
```

The Pod is the shared execution envelope, but each container still has its own process and filesystem settings. Use Pod-level defaults for a common identity and container-level fields for controls that apply to that specific container. A container-level value overrides a Pod default where both levels support the field. Fields do not become valid everywhere simply because both objects are called `securityContext`.

## Identity: user, primary group and volume group

`runAsUser` sets the numeric process UID. `runAsGroup` sets its primary GID. `runAsNonRoot: true` asserts that the process must not run as UID 0; it does not create a user or automatically select UID 1000. An image that declares a username rather than a numeric UID can fail this verification if Kubernetes cannot establish that it is non-root. Explicit numeric identity avoids that ambiguity, provided the application supports it.

```yaml
spec:
  securityContext:
    runAsUser: 1000
    runAsGroup: 3000
    runAsNonRoot: true
    fsGroup: 2000
```

`fsGroup` gives processes a supplemental group and arranges compatible group access on supported volumes. It is a Pod-level setting. It does not make every file in the image writable or replace the process's primary group. Volume type and storage-driver support determine ownership behavior; CSI drivers can handle group permissions themselves. Large ownership changes can affect startup time, so investigate storage behavior rather than assuming every permission delay is a broken image.

```bash
kubectl -n security-lab exec identity -- id
kubectl -n security-lab exec identity -- stat -c '%u %g %a' /data
```

`id` can also reveal supplemental memberships inherited from image group configuration. Do not assume the displayed group list contains only the primary GID and fsGroup. Verify the required group is present rather than requiring one exact ordering of all groups.

## Prevent gaining privilege and remove capabilities

`allowPrivilegeEscalation: false` sets the Linux no-new-privileges behavior, preventing exec-based gains such as set-user-ID elevation. This is separate from choosing a non-root UID. It does not erase capabilities already granted, so combine it with a minimal capability set.

Linux capabilities split traditionally root-only operations into narrower permissions. Kubernetes capability names omit the `CAP_` prefix. Start with `drop: ["ALL"]`, and add only a capability that the application demonstrably needs. For a process that truly needs the capability to bind a low port, the field is `add: ["NET_BIND_SERVICE"]`; first consider serving on a high container port and exposing the desired Service port instead. Runtime settings can affect whether low-port binding needs that capability at all.

```yaml
securityContext:
  allowPrivilegeEscalation: false
  privileged: false
  capabilities:
    drop: ["ALL"]
```

`privileged: true` removes major isolation protections and grants extensive host-level access. It is not a reasonable repair for an application permission error. It also interacts with other controls: privileged execution or powerful capabilities can defeat assumptions behind privilege-escalation restrictions. Keep privileged mode disabled for ordinary application workloads.

## Read-only root, explicit writable storage

`readOnlyRootFilesystem: true` makes the container's image root filesystem read-only. It reduces the places an attacker or accidental write can modify, but it does not make all mounted volumes read-only. This is precisely how a hardened application can still write a cache, temporary files or generated runtime data.

| Need | Appropriate storage decision |
|---|---|
| Immutable application files | Read-only image root |
| Pod-lifetime scratch data | Writable `emptyDir` at the exact path |
| Durable application data | Suitable persistent storage |
| Delivered configuration | Read-only ConfigMap or Secret projection |

Mount an `emptyDir` at `/tmp`, `/work`, or another path the actual application needs. Combine it with correct UID and group access. An `emptyDir` survives individual container restarts within the Pod but disappears with the Pod. If a process expects to write a PID file or cache under the image filesystem, either configure its location or supply an appropriate volume. Avoid making the whole container writable just to accommodate one directory.

## Seccomp limits the kernel interface

`seccompProfile.type: RuntimeDefault` asks the runtime to apply its default system-call filtering profile. The exact allowed set is runtime-dependent. It is preferable to silently leaving seccomp unconfined, but it does not substitute for dropping capabilities, using a non-root identity, or limiting writable storage. `Localhost` profiles require files installed on the node; do not assume they exist during the exam.

For Linux containers, `/proc/1/status` exposes useful evidence: `NoNewPrivs: 1`, `Seccomp: 2` for filtering mode, and an all-zero `CapEff` for no effective capabilities. These observations complement API inspection. A field in accepted YAML states intent; runtime evidence checks that the resulting process has the intended restrictions. Read mount flags from `/proc/mounts` rather than performing a write in an automated read-only grading check.

## Restricted is a policy profile, not a magic field

The Pod Security Standards define Privileged, Baseline and Restricted profiles. Pod Security Admission can enforce a profile using namespace labels. For Linux workloads, Restricted includes non-root execution, disabling privilege escalation, an approved seccomp profile, dropping all capabilities, and allowing only `NET_BIND_SERVICE` to be added back. It also constrains volume types, host access and other security-sensitive fields. Init and ephemeral containers matter too.

A read-only root filesystem is additional hardening; Restricted does not universally require it. An `emptyDir` is compatible with Restricted. Policy admission checks the manifest and does not guarantee the process will start: a compliant non-root process can still fail because a file is inaccessible. Label changes do not evict existing Pods, so test compliance by creating a new workload after enabling enforcement.

## Exam tips

Use `kubectl explain pod.spec.securityContext` and `kubectl explain pod.spec.containers.securityContext` to avoid field-scope errors. Set identity first, add required volume access, then apply privilege and filesystem controls. Inspect events for admission rejection versus runtime startup failure. Verify UID, GID, groups, requested JSON fields and the actual writable path; Pod readiness alone does not prove the requested hardening is present.
