+++
id = "docker-tags-digests-registries"
title = "Tags, digests and registries"
module = "images"
minutes = 40
needs = "docker"
objectives = ["Parse image references into registry, repository, tag and digest", "Publish an image to a local registry on the selected machine", "Pull and run the exact manifest identified by a digest", "Distinguish content identity from signing and vulnerability evidence"]

[[quiz]]
q = "Which part selects the registry in localhost:15000/dojo/demo:v1?"
choices = ["localhost:15000", "dojo", "demo", "v1"]
answer = 0
why = "The hostname and optional port select the registry. dojo/demo is the repository path; v1 is a tag within that repository."

[[quiz]]
q = "A colleague pushes different content under the same v1 tag. What remains immutable?"
choices = ["The v1 tag", "The manifest digest of the old content", "Every local container using v1 changes", "The registry hostname"]
answer = 1
why = "Tags are movable references. A digest identifies bytes; changing a manifest changes its digest. Existing containers keep their original image."

[[quiz]]
q = "Your Docker daemon is remote over SSH. Where does localhost point during docker push localhost:15000/dojo/demo:v1?"
choices = ["The laptop running the CLI", "The registry container itself", "The Docker daemon machine", "The SSH gateway in every setup"]
answer = 2
why = "The daemon performs the registry transfer. Its localhost is the selected Docker machine. curl runs locally and instead needs PODDOJO_HOST to reach that machine."

[[quiz]]
q = "Does a digest prove an image comes from a trusted publisher?"
choices = ["Yes, SHA-256 is a publisher signature", "Only for Alpine images", "No; identity, signatures and vulnerability evidence answer different questions", "Yes, if its tag is v1"]
answer = 2
why = "A digest detects changed content. A verified signature binds that digest to a trusted identity under a policy. An SBOM inventories components; neither by itself establishes that software is vulnerability-free."

[lab]
summary = "Run a disposable registry, publish a dojo image, and launch its exact digest without relying on a mutable tag."
setup = []
cleanup = ["docker rm -f dojo-registry dojo-digest-run 2>/dev/null || true", "docker image rm localhost:15000/dojo/registry-demo:v1 dojo/registry-demo:v1 2>/dev/null || true", "docker volume rm dojo-registry-data 2>/dev/null || true"]

[[lab.steps]]
title = "Start the registry"
task = "Run **dojo-registry** from **registry:2**, publishing host port **15000** to container port **5000**. Store registry data in named volume **dojo-registry-data** mounted at **/var/lib/registry**. Wait until its HTTP API responds. This intentionally unauthenticated HTTP registry is only for this disposable lab; port 15000 must be reachable through PODDOJO_HOST."
hints = ["Use --mount type=volume rather than a remote host path.", "The API base path is /v2/."]
solution = '''```bash
docker volume create dojo-registry-data
docker run -d --name dojo-registry -p 15000:5000 --mount type=volume,src=dojo-registry-data,dst=/var/lib/registry registry:2
for i in $(seq 1 30); do
  if curl -fsS --max-time 2 "http://$PODDOJO_HOST:15000/v2/"; then break; fi
  sleep 2
done
curl -fsS --max-time 2 "http://$PODDOJO_HOST:15000/v2/"
```'''

[[lab.steps.checks]]
name = "registry is running with the requested publication and persistent storage"
run = "test \"$(docker inspect -f '{{.State.Running}}' dojo-registry)\" = true && docker inspect -f '{{range (index .HostConfig.PortBindings \"5000/tcp\")}}{{println .HostPort}}{{end}}' dojo-registry | grep -qx 15000 && test \"$(docker inspect -f '{{range .Mounts}}{{if eq .Destination \"/var/lib/registry\"}}{{.Name}}{{end}}{{end}}' dojo-registry)\" = dojo-registry-data && echo ok"
equals = "ok"

[[lab.steps.checks]]
name = "registry API responds"
run = "for i in $(seq 1 30); do if curl -fsS --max-time 2 \"http://$PODDOJO_HOST:15000/v2/\" >/dev/null; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Give the image a destination"
task = "Pull **alpine:3.20**. Give its image both names **dojo/registry-demo:v1** and **localhost:15000/dojo/registry-demo:v1**. Tagging alone must not upload anything."
hints = ["docker tag takes an existing source and a destination reference.", "Compare image IDs: tags do not copy the layers."]
solution = '''```bash
docker pull alpine:3.20
docker tag alpine:3.20 dojo/registry-demo:v1
docker tag dojo/registry-demo:v1 localhost:15000/dojo/registry-demo:v1
```'''

[[lab.steps.checks]]
name = "both dojo references resolve to the source image"
run = "a=$(docker image inspect -f '{{.Id}}' alpine:3.20) && b=$(docker image inspect -f '{{.Id}}' dojo/registry-demo:v1) && c=$(docker image inspect -f '{{.Id}}' localhost:15000/dojo/registry-demo:v1) && test \"$a\" = \"$b\" && test \"$b\" = \"$c\" && echo ok"
equals = "ok"

[[lab.steps]]
title = "Publish to the registry"
task = "Push **localhost:15000/dojo/registry-demo:v1**. Confirm the registry catalog contains **dojo/registry-demo** and its tag list contains **v1**."
hints = ["The daemon, not curl, uploads the image.", "The tag-list endpoint is /v2/dojo/registry-demo/tags/list."]
solution = '''```bash
docker push localhost:15000/dojo/registry-demo:v1
curl -fsS "http://$PODDOJO_HOST:15000/v2/_catalog"
curl -fsS "http://$PODDOJO_HOST:15000/v2/dojo/registry-demo/tags/list"
```'''

[[lab.steps.checks]]
name = "registry serves the published repository and v1 tag"
run = "curl -fsS --max-time 3 \"http://$PODDOJO_HOST:15000/v2/_catalog\" | grep -q '\"dojo/registry-demo\"' && curl -fsS --max-time 3 \"http://$PODDOJO_HOST:15000/v2/dojo/registry-demo/tags/list\" | grep -q '\"v1\"' && echo ok"
equals = "ok"

[[lab.steps]]
title = "Run by immutable identity"
task = "Find the pushed repository digest, explicitly pull that digest reference, and run it as **dojo-digest-run**, detached, with **sleep 3600**. Use **localhost:15000/dojo/registry-demo@sha256:\u2026** as the container image reference."
hints = ["RepoDigests contains repository@digest strings; select the local registry entry.", "An image ID is not the registry manifest digest."]
solution = '''```bash
ref=$(docker image inspect -f '{{range .RepoDigests}}{{println .}}{{end}}' localhost:15000/dojo/registry-demo:v1 | grep '^localhost:15000/dojo/registry-demo@sha256:' | head -n 1)
test -n "$ref"
docker pull "$ref"
docker run -d --name dojo-digest-run "$ref" sleep 3600
```'''

[[lab.steps.checks]]
name = "running container uses the published repository digest"
run = '''ref=$(docker inspect -f '{{.Config.Image}}' dojo-digest-run) && printf '%s
' "$ref" | grep -Eq '^localhost:15000/dojo/registry-demo@sha256:[0-9a-f]{64}$' && test "$(docker inspect -f '{{.State.Running}}' dojo-digest-run)" = true && test "$(docker inspect -f '{{.Image}}' dojo-digest-run)" = "$(docker image inspect -f '{{.Id}}' localhost:15000/dojo/registry-demo:v1)" && echo ok'''
equals = "ok"
+++

## A name is an address, not the image itself

An image reference tells the engine where to find content and which version to request. Learn to read it from left to right before debugging a failed pull. The general form is `registry/namespace/repository:tag@digest`; namespace is a convention within the repository path, and there may be several path components. A tag or a digest can appear alone. When both appear, the digest selects the content.

```text
localhost:15000 / dojo / registry-demo : v1
└ registry ───┘   └ repository path ─┘   └ tag
localhost:15000/dojo/registry-demo@sha256:<manifest hash>
```

Without a registry hostname, `alpine:3.20` resolves through Docker Hub as `docker.io/library/alpine:3.20`. A first path component containing a dot or colon, or the special name `localhost`, identifies a registry. Thus `dojo/demo:v1` names a repository on Docker Hub, while `localhost:15000/dojo/demo:v1` names our lab registry. Repository names are lowercase. Never assume a short name selects your company's private registry.

| Part | Meaning | What can change? |
|---|---|---|
| Registry | Service storing and serving manifests and blobs | Its availability and access rules |
| Repository | Collection of related image versions | Its tags and retained content |
| Tag | Human-friendly pointer, such as `v1` | The manifest it references |
| Digest | Hash identifying manifest or index bytes | Changing bytes produces a different digest |

## Tags are convenient pointers

A tag is not a promise of immutability. Even `3.20` can be republished to include security fixes. A patch-looking tag is more specific than a floating major version, but the registry can still move it unless registry policy prevents that. Pinning a version tag makes intent clearer; pinning a digest fixes content identity. Docker does not update an existing container when a tag moves.

`docker tag` adds a local reference to an existing image. It does not rebuild, duplicate the image's layers, log you in, or upload anything. Likewise, removing one local tag does not necessarily remove the shared image content: another tag or container may still reference it. To transfer content you need `push` or `pull`.

```bash
docker pull alpine:3.20
docker tag alpine:3.20 dojo/registry-demo:v1
docker tag dojo/registry-demo:v1 localhost:15000/dojo/registry-demo:v1
docker push localhost:15000/dojo/registry-demo:v1
docker image inspect localhost:15000/dojo/registry-demo:v1
```

For a release, record the digest printed by the successful push. Treat promotion as moving a tested identity through environments, rather than rebuilding separately and hoping the resulting bytes match. Use tags for discoverability and digests for reproducibility. Updating a digest deliberately is part of patch management; a permanently frozen vulnerable image is still vulnerable.

## What the registry actually stores

A registry stores content-addressed blobs and manifests. A manifest describes the image configuration and filesystem layers. A multi-platform index points to platform-specific manifests, allowing one reference to support both an ARM laptop and an x86 server. The engine chooses a compatible platform. An index digest and one of its child manifest digests identify different objects, so they need not match.

A local image ID usually identifies the image configuration, not the registry manifest. Do not take the value of `docker image inspect -f '{{.Id}}'` and paste it after a repository's `@` as though it were a registry digest. Inspect `RepoDigests` or use the digest returned by the registry operation. The relationship is explained in [Docker's digest documentation](https://docs.docker.com/dhi/explore/security-concepts/digests/).

```bash
docker image inspect -f '{{range .RepoDigests}}{{println .}}{{end}}' \
  localhost:15000/dojo/registry-demo:v1
# Copy the complete repository@sha256:... reference from that output:
# docker pull "$ref"
# docker run -d --name dojo-pinned "$ref" sleep 3600
```

A digest is not an offline guarantee. A registry can delete content, credentials can expire, and a node may lack the image. Content identity and content availability are separate operational requirements.

## The remote-machine distinction

The CLI sends API requests to the selected engine. The engine performs the push and pull. If the machine is remote, `localhost:15000` in the image reference means the remote engine's loopback address. This is why our push works without uploading through a local registry on your laptop.

```text
CLI on your computer ── Docker API over SSH ──> remote engine
                                                │ push / pull
                                                ▼
                                      localhost:15000 registry
curl on your computer ── PODDOJO_HOST:15000 ──────┘
```

By contrast, `curl` is an ordinary command running in the PodDojo lab terminal on your computer. It reaches the published registry using `http://$PODDOJO_HOST:15000`. A container's own localhost is yet another network namespace; do not substitute it for the engine's loopback address.

The lab publishes port 15000 so the API is reachable from either machine arrangement. It intentionally uses unauthenticated HTTP and disposable data. Keep it on a trusted lab network and reset the lab afterwards. Real registries need TLS, access control and retention policies. Docker treats loopback registries specially; an arbitrary remote HTTP registry is not automatically trusted. Do not change the daemon's registry security settings for this exercise.

## Inspect the API, then consider trust

The Distribution HTTP API exposes `/v2/` as a version probe, `/v2/_catalog` for repositories, and `/v2/<repository>/tags/list` for tags. Our tiny catalog fits in one response. Large registries may paginate, filter results according to authorization, or restrict catalog access; absence from one catalog response is not universal proof a repository does not exist. See the [Distribution API specification](https://distribution.github.io/distribution/spec/api/).

```bash
curl -fsS "http://$PODDOJO_HOST:15000/v2/"
curl -fsS "http://$PODDOJO_HOST:15000/v2/_catalog"
curl -fsS "http://$PODDOJO_HOST:15000/v2/dojo/registry-demo/tags/list"
```

A digest answers “which bytes?” A signature, verified against an approved identity and policy, answers “who endorsed these bytes?” An SBOM inventories packaged components and versions so scanners can connect them to vulnerability information. Provenance records describe a build's inputs and process. None of these is interchangeable, and having an SBOM is not the same as passing a vulnerability policy. Prefer evidence attached to the exact digest you deploy.

## Why this matters for Kubernetes

Kubernetes uses image references in workload YAML. It does not build images from Dockerfiles during Pod creation, and a registry reachable from your laptop may be unreachable from the nodes. Private repositories usually require an `imagePullSecrets` Secret in the Pod's namespace. A local registry's `localhost` reference also means the node's localhost when a node pulls; our Docker lab reference is not automatically suitable for kind.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-pinned
  namespace: image-lab
spec:
  containers:
    - name: app
      # Replace this placeholder with a real published digest before applying.
      image: registry.example.com/dojo/app@sha256:<verified-digest>
      imagePullPolicy: IfNotPresent
```

At exam speed, generate the skeleton with `kubectl -n image-lab run dojo-pinned --image=alpine:3.20 --dry-run=client -o yaml`, then adjust the requested image. `Always` controls pull resolution behavior; it does not make a mutable tag immutable. An explicitly specified pull policy stays explicit when you change the image. Diagnose `ImagePullBackOff` through Pod events before changing commands or probes: the application has not started yet.
