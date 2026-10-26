+++
id = "ckad-configmaps"
title = "ConfigMaps: configuration without rebuilding images"
module = "config-security"
minutes = 40
domain = "config-security"
needs = "cluster"
objectives = [
  "Create ConfigMaps from literals, files and env-files",
  "Inject selected keys and prefixed groups into container environments",
  "Mount selected configuration files and explain subPath behavior",
  "Distinguish live volume projection from startup environment snapshots",
  "Use immutable, versioned configuration for predictable rollouts",
]

[[quiz]]
q = "A ConfigMap used by envFrom changes. What happens to an already running container?"
choices = ["Its environment changes immediately", "It restarts automatically", "Its environment remains a startup snapshot", "It updates after the kubelet sync period, like a volume"]
answer = 2
why = "The kubelet constructs the environment when starting a container. Updating a ConfigMap does not rewrite an existing process environment or change a Deployment template. Recreate the Pod, usually through a rollout, to consume the new values."

[[quiz]]
q = "Which mount does NOT receive automatic ConfigMap updates?"
choices = ["A directory projection", "A single file mounted using subPath", "A directory projection using items", "A readOnly directory projection"]
answer = 1
why = "A subPath mount binds the selected file rather than following the projection's later atomic replacements. The original file stays mounted until the Pod is recreated. Selecting files with items alone does not disable projection updates."

[[quiz]]
q = "What does --from-file=settings=app.conf create?"
choices = ["A key app.conf containing settings", "A key settings containing the file's contents", "A key settings containing the path app.conf", "Keys parsed from the key=value lines in app.conf"]
answer = 1
why = "The optional name before the equals sign chooses the ConfigMap key. File creation stores the whole file as one value; --from-env-file instead parses separate KEY=value entries."

[[quiz]]
q = "What is a sensible way to change an immutable ConfigMap used by a Deployment?"
choices = ["Set immutable back to false", "Patch data with --force", "Create a new named ConfigMap and update the Pod template reference", "Use kubectl edit, which bypasses the immutable flag"]
answer = 2
why = "Immutability cannot be reversed, and data cannot be changed on that object. A new versioned object plus a template change gives the Deployment an explicit rollout and leaves the old configuration available for rollback."

[lab]
summary = "Create three configuration sources, inject environments and files, then observe which consumers see an update."
setup = ["kubectl create namespace cm-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace cm-lab --ignore-not-found --wait=false || true"]

[[lab.steps]]
title = "Create configuration from three source formats"
task = """
In `cm-lab`, create `app-settings` with `MODE=training`, `file-settings` with key `app.conf` containing exactly `color=blue` and a trailing newline, and `env-settings` with `REGION=east` and `LOG_LEVEL=info`. Use literals, a file and an env-file respectively.
"""
hints = ["Use create configmap with --from-literal, --from-file or --from-env-file.", "Pipe --dry-run=client -o yaml into apply for repeatable creation."]
solution = '''
```bash
kubectl -n cm-lab create configmap app-settings --from-literal=MODE=training --dry-run=client -o yaml | kubectl apply -f -
printf 'color=blue\n' > app.conf
printf 'REGION=east\nLOG_LEVEL=info\n' > settings.env
kubectl -n cm-lab create configmap file-settings --from-file=app.conf --dry-run=client -o yaml | kubectl apply -f -
kubectl -n cm-lab create configmap env-settings --from-env-file=settings.env --dry-run=client -o yaml | kubectl apply -f -
```
File contents remain one value; env-file entries become separate keys.
'''

[[lab.steps.checks]]
name = "literal configuration is training"
run = "kubectl -n cm-lab get cm app-settings -o jsonpath='{.data.MODE}'"
equals = "training"
[[lab.steps.checks]]
name = "file content is stored under app.conf"
run = '''kubectl -n cm-lab get cm file-settings -o jsonpath='{.data.app\.conf}' '''
equals = "color=blue"
[[lab.steps.checks]]
name = "env-file entries became separate keys"
run = "kubectl -n cm-lab get cm env-settings -o jsonpath='{.data.REGION} {.data.LOG_LEVEL}'"
equals = "east info"

[[lab.steps]]
title = "Inject a selected key and prefixed environment"
task = """
Create Pod `env-reader` in `cm-lab`, image `busybox:1.36`, sleeping for 3600 seconds. Set `APP_MODE` from `app-settings` key `MODE` using `valueFrom`. Import `env-settings` using `envFrom` with prefix `CFG_`. Wait until Ready.
"""
hints = ["configMapKeyRef belongs under env[].valueFrom.", "prefix and configMapRef are siblings in an envFrom entry."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: env-reader
  namespace: cm-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    env:
    - name: APP_MODE
      valueFrom:
        configMapKeyRef:
          name: app-settings
          key: MODE
    envFrom:
    - prefix: CFG_
      configMapRef:
        name: env-settings
EOF
kubectl -n cm-lab wait --for=condition=Ready pod/env-reader --timeout=120s
```
'''
[[lab.steps.checks]]
name = "the process receives selected and prefixed values"
run = '''kubectl -n cm-lab get pod env-reader >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n cm-lab exec env-reader -- sh -c 'printf "%s %s %s" "$APP_MODE" "$CFG_REGION" "$CFG_LOG_LEVEL"' 2>/dev/null); [ "$v" = 'training east info' ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the environment uses ConfigMap references and a prefix"
run = "kubectl -n cm-lab get pod env-reader -o jsonpath='{.spec.containers[0].env[0].valueFrom.configMapKeyRef.name} {.spec.containers[0].env[0].valueFrom.configMapKeyRef.key} {.spec.containers[0].envFrom[0].prefix} {.spec.containers[0].envFrom[0].configMapRef.name}'"
equals = "app-settings MODE CFG_ env-settings"

[[lab.steps]]
title = "Mount a directory and a subPath snapshot"
task = """
Create `file-reader` in `cm-lab` using `busybox:1.36` and `sleep 3600`. Project only `file-settings` key `app.conf` as `settings.conf` with `items`. Mount the volume at `/config` and the same file using `subPath: settings.conf` at `/snapshot.conf`. Both files must initially contain `color=blue`.
"""
hints = ["items maps a ConfigMap key to a relative filename.", "Mount the same named volume twice; only the single-file mount needs subPath."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: file-reader
  namespace: cm-lab
spec:
  containers:
  - name: app
    image: busybox:1.36
    command: ["sleep", "3600"]
    volumeMounts:
    - name: settings
      mountPath: /config
      readOnly: true
    - name: settings
      mountPath: /snapshot.conf
      subPath: settings.conf
      readOnly: true
  volumes:
  - name: settings
    configMap:
      name: file-settings
      items:
      - key: app.conf
        path: settings.conf
EOF
kubectl -n cm-lab wait --for=condition=Ready pod/file-reader --timeout=120s
```
'''
[[lab.steps.checks]]
name = "both mounted paths expose the selected file"
run = '''kubectl -n cm-lab get pod file-reader >/dev/null 2>&1 || exit 1; for i in $(seq 1 30); do v=$(kubectl -n cm-lab exec file-reader -- sh -c 'cat /config/settings.conf /snapshot.conf' 2>/dev/null); [ "$v" = "$(printf 'color=blue\ncolor=blue')" ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 90
[[lab.steps.checks]]
name = "the volume selects and renames the configuration key"
run = "kubectl -n cm-lab get pod file-reader -o jsonpath='{.spec.volumes[?(@.name==\"settings\")].configMap.items[0].key} {.spec.volumes[?(@.name==\"settings\")].configMap.items[0].path}'"
equals = "app.conf settings.conf"
[[lab.steps.checks]]
name = "the snapshot path uses a single-file subPath mount"
run = "kubectl -n cm-lab get pod file-reader -o jsonpath='{.spec.containers[0].volumeMounts[?(@.mountPath==\"/snapshot.conf\")].subPath}'"
equals = "settings.conf"

[[lab.steps]]
title = "Observe update propagation"
task = """
Change `file-settings` to `color=green` and `app-settings` to `MODE=exam`. Keep both Pods running without replacement. Wait for `/config/settings.conf` to become green; `/snapshot.conf` must remain blue and `APP_MODE` must remain training.
"""
hints = ["Patch the ConfigMaps, not the Pods.", "Projection updates are eventual; reopening the directory-mounted file lets you see the replacement."]
solution = '''
```bash
kubectl -n cm-lab patch cm file-settings --type=merge -p '{"data":{"app.conf":"color=green\n"}}'
kubectl -n cm-lab patch cm app-settings --type=merge -p '{"data":{"MODE":"exam"}}'
for i in $(seq 1 90); do
  v=$(kubectl -n cm-lab exec file-reader -- cat /config/settings.conf)
  [ "$v" = 'color=green' ] && break
  sleep 2
done
[ "$v" = 'color=green' ]
```
The mounted directory follows projection updates. Neither an existing environment nor a subPath mount follows them.
'''
[[lab.steps.checks]]
name = "the API stores both new values"
run = '''a=$(kubectl -n cm-lab get cm app-settings -o jsonpath='{.data.MODE}') && b=$(kubectl -n cm-lab get cm file-settings -o jsonpath='{.data.app\.conf}') && [ "$a" = exam ] && [ "$b" = color=green ] && echo ok'''
equals = "ok"
[[lab.steps.checks]]
name = "directory projection updates while the two snapshots stay unchanged"
run = '''[ "$(kubectl -n cm-lab get cm app-settings -o jsonpath='{.data.MODE}')" = exam ] || exit 1; for i in $(seq 1 90); do a=$(kubectl -n cm-lab exec file-reader -- cat /config/settings.conf 2>/dev/null); b=$(kubectl -n cm-lab exec file-reader -- cat /snapshot.conf 2>/dev/null); c=$(kubectl -n cm-lab exec env-reader -- printenv APP_MODE 2>/dev/null); [ "$a" = color=green ] && [ "$b" = color=blue ] && [ "$c" = training ] && echo ok && exit 0; sleep 2; done; exit 1'''
equals = "ok"
timeout = 210

[[lab.steps]]
title = "Publish an immutable configuration version"
task = "Create ConfigMap `release-v1` in `cm-lab` with `MODE=exam` and `immutable: true`. Leave the mutable maps available for comparison."
hints = ["immutable is a top-level field, alongside data."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: release-v1
  namespace: cm-lab
immutable: true
data:
  MODE: exam
EOF
```
'''
[[lab.steps.checks]]
name = "the versioned map is immutable and contains the release value"
run = "kubectl -n cm-lab get cm release-v1 -o jsonpath='{.immutable} {.data.MODE}'"
equals = "true exam"
+++

## Configuration is an input, not an image layer

An image packages executable code and defaults. Deployment-specific values belong outside it: a region, a logging level, or a configuration file should not require another image build. A ConfigMap is a namespaced API object that stores those non-confidential inputs. The kubelet can turn its keys into environment variables or files when constructing a Pod. The application still decides how to interpret them.

```text
ConfigMap in API ── kubelet ──┬── environment at container startup
                             └── projected files inside the container
image + configuration + command = running application
```

This separation lets the same pinned image run in training and production. It does not make configuration safe automatically: a wrong database hostname can break an otherwise healthy application. Keep configuration changes reviewable, validate them, and plan how the application reloads them. Never put passwords in a ConfigMap; use a Secret and appropriate access controls.

## Three creation paths, one data model

Use the creation method that matches the source you already have:

| Input | Flag | Result |
|---|---|---|
| One setting | `--from-literal=MODE=training` | One key and string value |
| A complete file | `--from-file=app.conf` | Filename becomes key; whole content becomes value |
| Explicit file key | `--from-file=settings=app.conf` | Key is `settings`, independent of filename |
| Environment file | `--from-env-file=settings.env` | Each `KEY=value` line becomes a key |

```bash
kubectl -n cm-lab create configmap app-settings \
  --from-literal=MODE=training --dry-run=client -o yaml
```

Dry-run prints a manifest without creating the object. Redirect it into a file for editing, or pipe it into `kubectl apply -f -`. An env-file is parsed as data, not executed as a shell script: avoid `export`, expansion, and shell quoting assumptions. A trailing newline in a source file is part of its value. Prefer `printf` when exact bytes matter.

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: app-settings
  namespace: cm-lab
data:
  MODE: training
  app.conf: |
    color=blue
```

Quote values such as `"true"` or `"8080"` because `data` values must be strings. Binary content belongs in base64-encoded `binaryData`; the same key cannot appear in both fields. ConfigMaps are limited to 1 MiB and are not a general file store. Pod references normally resolve in the Pod's own namespace, so creating a map elsewhere does not satisfy a local reference.

## Selected variables versus bulk imports

`env[].valueFrom.configMapKeyRef` chooses one key and lets you rename it. This is useful when an application expects `APP_MODE` but the shared map calls the setting `MODE`. `envFrom[].configMapRef` imports all keys, with an optional `prefix`, such as `CFG_`. Prefixes prevent accidental collisions when several sources contain names like `HOST` or `PORT`.

```yaml
env:
- name: APP_MODE
  valueFrom:
    configMapKeyRef:
      name: app-settings
      key: MODE
envFrom:
- prefix: CFG_
  configMapRef:
    name: env-settings
```

Explicit `env` entries win over conflicting imported names; for multiple `envFrom` sources, later sources win. Use simple uppercase identifiers for portability across shells and tools. Kubernetes can accept names that a shell cannot conveniently expand, so API acceptance alone is not a good naming policy.

A missing required ConfigMap or selected key prevents container startup. Read `kubectl describe pod` and its events when you see `CreateContainerConfigError`. `optional: true` deliberately permits absence; it is not a general repair for a misspelled dependency. Existing processes do not gain new environment values when you update the map.

There are also two different kinds of variable expansion to recognize. Kubernetes can substitute `$(APP_MODE)` in container `command` and `args` using the container environment. A shell launched with `sh -c` instead understands `$APP_MODE`. Do not copy Kubernetes substitution syntax into an ordinary shell command and assume it means the same thing: shells interpret `$()` as command substitution. Prefer an argument list when no shell features are needed, and invoke a shell explicitly for pipelines or redirections. ConfigMap file contents are not templates that Kubernetes expands; a literal `$HOST` in `app.conf` remains literal unless the application itself interprets it.

## Files, items and subPath

A ConfigMap volume normally exposes every key as a filename. `items` narrows the projection and maps keys to relative paths. Referencing a nonexistent required key fails the projection. Mounting a volume over an existing image directory hides the image's files there; choose an application-specific directory rather than masking a directory full of dependencies.

```yaml
volumes:
- name: settings
  configMap:
    name: file-settings
    items:
    - key: app.conf
      path: settings.conf
```

The container mounts `settings` through `volumeMounts`. A directory mount at `/config` exposes `/config/settings.conf`. A mount at `/snapshot.conf` with `subPath: settings.conf` places only that file at that path, leaving neighboring image files visible. That convenience has a lifecycle cost: **subPath mounts do not receive automatic ConfigMap updates**. Normal projected ConfigMap files are read-only; if a program needs to edit configuration, copy it into a writable volume first.

## Updates have three different clocks

Changing an API object, refreshing a projected file, and reloading an application are separate events. Ordinary directory projections update eventually after kubelet synchronization and its configured change-detection behavior. Do not assume an immediate refresh or grade with a fixed short sleep. Reopen the filename to read the replacement; a process holding an old file descriptor can keep reading old content.

| Consumer | After a mutable map changes |
|---|---|
| Existing environment | Keeps startup values |
| Normal projected directory | Eventually exposes updated files |
| A subPath-mounted file | Keeps its original mounted file |
| Application's parsed settings | Changes only if the application reloads |

Updating a ConfigMap does not itself change a Deployment's Pod template. For environment consumers, `kubectl rollout restart deployment/NAME` recreates Pods using current data. For predictable releases, use a new configuration name in the template: that reference change triggers a rollout and records which version each release intended to consume.

## Immutable configuration and exam tips

Set top-level `immutable: true` when a configuration version should never change. You cannot turn it back off or edit its data. Publish `release-v2`, update the workload reference, and retain the previous version if rollback needs it. Avoid deleting and recreating a same-named map as a substitute: existing mounted consumers can remain attached to old content and the release history becomes ambiguous.

During the exam, generate YAML imperatively, then add the precise reference or mount structure. Use `kubectl explain pod.spec.containers.envFrom` and inspect the live Pod manifest when field placement is unclear. Verify both the API data and the value the process actually reads. A successful `apply` proves that the API accepted your configuration, not that the application used it.
