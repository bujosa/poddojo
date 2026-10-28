+++
id = "ref-packaging"
title = "Helm 4, Kustomize, and Docker packaging"
order = 7
summary = "Render and upgrade packages, compose overlays, and build non-root images that kind can actually run."
+++

## Helm 4: inspect, render, install, upgrade

A chart is a versioned package of templates and defaults; a release is one installed instance in one namespace. Names and ownership matter. Inspect the release before applying changes so you preserve user-supplied values and history. The examples use a local chart and namespace ref-practice, avoiding a remote chart repository.

```bash
kubectl create namespace ref-practice --dry-run=client -o yaml | kubectl apply -f -
helm version --short
helm create "$PODDOJO_LAB_DIR/dojo-ref-chart"
helm show chart "$PODDOJO_LAB_DIR/dojo-ref-chart"
helm show values "$PODDOJO_LAB_DIR/dojo-ref-chart"
cat > "$PODDOJO_LAB_DIR/ref-values.yaml" <<'EOF'
replicaCount: 2
image:
  repository: nginx
  tag: 1.27-alpine
service:
  port: 80
autoscaling:
  enabled: false
EOF
helm lint "$PODDOJO_LAB_DIR/dojo-ref-chart" -f "$PODDOJO_LAB_DIR/ref-values.yaml"
helm template reference "$PODDOJO_LAB_DIR/dojo-ref-chart" -n ref-practice -f "$PODDOJO_LAB_DIR/ref-values.yaml" > "$PODDOJO_LAB_DIR/ref-rendered.yaml"
helm install reference "$PODDOJO_LAB_DIR/dojo-ref-chart" -n ref-practice -f "$PODDOJO_LAB_DIR/ref-values.yaml" --wait --timeout=180s
helm list -n ref-practice
helm status reference -n ref-practice
helm get values reference -n ref-practice -o yaml
helm get values reference -n ref-practice --all -o yaml
helm get manifest reference -n ref-practice
helm history reference -n ref-practice
helm upgrade reference "$PODDOJO_LAB_DIR/dojo-ref-chart" -n ref-practice --reuse-values --set replicaCount=3 --rollback-on-failure --wait --timeout=180s
helm rollback reference 1 -n ref-practice --wait --timeout=180s
```

Helm 4's --rollback-on-failure restores the previous successful release when an upgrade fails. It replaces the older --atomic spelling in new examples. Helm 4 supports wait strategies: bare --wait uses watcher; an omitted wait flag defaults to hookOnly. --wait-for-jobs additionally waits for ordinary Jobs when waiting is enabled. Check the installed command help when switching Helm versions. These details are specified by [Helm upgrade](https://helm.sh/docs/helm/helm_upgrade/).

| Input or operation | Effect |
|---|---|
| -f values.yaml | Merge a YAML values file; later files win |
| --set key=value | Set a value with type inference |
| --set-string image.tag=1.27-alpine | Force a string override |
| --reuse-values | Merge overrides into the last release's values |
| --reset-values | Start from the new chart defaults |
| helm get values | User-supplied values; --all includes computed defaults |
| helm uninstall reference -n ref-practice | Remove the release and ordinary managed resources |
| uninstall --keep-history | Retain release history; unsuitable when a task requires full removal |

helm template does not prove runtime readiness or validate every server capability. Inspect generated container ports when customizing starter charts: a Service port change must not accidentally change an application's actual listening port. Direct kubectl edits drift from Helm's stored desired state. CRDs, retained objects and hook resources can have separate lifecycle behavior; inspect chart policies rather than assuming uninstall removes every object.

## Kustomize: build a reusable overlay

Kustomize transforms ordinary Kubernetes resources rather than templating arbitrary strings. The following creates a small base and an overlay that adds names, configuration and replica count without editing the base later.

```bash
mkdir -p "$PODDOJO_LAB_DIR/ref-kustomize/base" "$PODDOJO_LAB_DIR/ref-kustomize/overlays/practice"
cat > "$PODDOJO_LAB_DIR/ref-kustomize/base/kustomization.yaml" <<'EOF'
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources: [app.yaml]
EOF
cat > "$PODDOJO_LAB_DIR/ref-kustomize/base/app.yaml" <<'EOF'
apiVersion: apps/v1
kind: Deployment
metadata: {name: worker}
spec:
  replicas: 1
  selector: {matchLabels: {app: worker}}
  template:
    metadata: {labels: {app: worker}}
    spec:
      containers:
      - name: worker
        image: busybox:1.35
        command: [sh, -ec, 'echo "$MODE"; sleep 3600']
        envFrom: [{configMapRef: {name: settings}}]
EOF
cat > "$PODDOJO_LAB_DIR/ref-kustomize/overlays/practice/kustomization.yaml" <<'EOF'
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources: [../../base]
namespace: ref-practice
namePrefix: practice-
nameSuffix: -v1
images:
- {name: busybox, newTag: '1.36'}
replicas:
- {name: worker, count: 2}
labels:
- pairs: {stage: practice}
  includeTemplates: true
  includeSelectors: false
configMapGenerator:
- name: settings
  literals: [MODE=practice]
patches:
- target: {kind: Deployment, name: worker}
  patch: |-
    - op: add
      path: /spec/template/spec/containers/0/resources
      value:
        requests: {cpu: 20m, memory: 16Mi}
EOF
kubectl kustomize "$PODDOJO_LAB_DIR/ref-kustomize/overlays/practice"
kubectl apply -k "$PODDOJO_LAB_DIR/ref-kustomize/overlays/practice"
```

| kustomization field | Purpose |
|---|---|
| resources | Files or bases to compose; use instead of deprecated bases |
| namespace / namePrefix / nameSuffix | Set namespace and transform resource names/references |
| images / replicas | Change image references or controller replica counts |
| labels / annotations | Add metadata; label options control templates and selectors |
| configMapGenerator / secretGenerator | Generate from literals, files or envs |
| generatorOptions | Generator labels, annotations and hash settings |
| patches | Targeted strategic-merge or JSON patches |
| replacements | Copy a source field into selected targets |
| components | Reusable optional configuration fragments |

Generated ConfigMaps and Secrets normally get a content hash suffix. Built-in name references are rewritten so a template references the generated name; changed content then changes the template and triggers a rollout. Disabling hashes removes that behavior. Do not accidentally extend an immutable Deployment selector while adding an organizational label. Render with the same kubectl version used for apply because bundled Kustomize versions differ. See [Kustomize with kubectl](https://kubernetes.io/docs/tasks/manage-kubernetes-objects/kustomization/).

## Dockerfile and kind workflow

Use a small pinned base, only needed build context files, explicit ownership, and a non-root numeric runtime UID. COPY is preferable to ADD when no archive or remote-fetch behavior is needed. Multi-stage builds keep compilers and intermediate files out of the final stage. A .dockerignore prevents accidental inclusion of credentials and unrelated workspace data. Build arguments and image layers are not secret storage. These are [Docker build practices](https://docs.docker.com/build/building/best-practices/).

```bash
mkdir -p "$PODDOJO_LAB_DIR/ref-image"
printf 'original practice page\n' > "$PODDOJO_LAB_DIR/ref-image/index.html"
cat > "$PODDOJO_LAB_DIR/ref-image/Dockerfile" <<'EOF'
FROM busybox:1.36
WORKDIR /www
COPY --chown=1000:1000 index.html /www/index.html
USER 1000:1000
EXPOSE 8080
ENTRYPOINT ["httpd"]
CMD ["-f", "-p", "8080", "-h", "/www"]
EOF
docker build -t dojo/reference:v1 "$PODDOJO_LAB_DIR/ref-image"
docker tag dojo/reference:v1 dojo/reference:exam
docker image inspect dojo/reference:exam --format '{{.Config.User}}'
docker save -o "$PODDOJO_LAB_DIR/dojo-reference.tar" dojo/reference:exam
kind load docker-image dojo/reference:exam --name poddojo
# Alternative import of the locally saved archive:
kind load image-archive "$PODDOJO_LAB_DIR/dojo-reference.tar" --name poddojo
```

EXPOSE documents a port; it does not publish one or change the listener. Exec-form ENTRYPOINT/CMD preserve signal delivery without an extra shell. A read-only-root Pod still needs writable volumes for any application cache or temporary files. Loading into kind is separate from docker build: each node has its own image store. Use the exact loaded tag with imagePullPolicy IfNotPresent, and prefer a new tag when rebuilding. The lab's supplied DOCKER_CONTEXT selects the machine; do not switch it or push practice images to a registry.
