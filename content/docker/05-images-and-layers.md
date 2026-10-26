+++
id = "docker-images-and-layers"
title = "Images, layers and immutable identity"
module = "images"
minutes = 40
needs = "docker"
objectives = ["Explain shared image layers and per-container copy-on-write storage", "Distinguish image IDs, mutable tags and registry digests", "Inspect image metadata, history and multi-platform manifests", "Archive and restore named lesson images without broad cleanup"]

[[quiz]]
q = "Two containers start from the same local image. What do they share?"
choices = ["Read-only image layers, with a separate writable layer per container", "One writable root filesystem", "A PID namespace by default", "One writable layer, until one of them writes a file"]
answer = 0
why = "Image layers are reused. A write in one container changes its own upper layer and does not rewrite the image or the other container filesystem."

[[quiz]]
q = "What happens when a registry owner moves a tag?"
choices = ["Already running containers change their root filesystem", "Your local copy updates on its next `docker run`", "The digest changes but still identifies the old bytes", "Future pulls can resolve the tag to different content"]
answer = 3
why = "A tag is a mutable reference. Existing containers retain their selected image content, while a later pull may retrieve a different image for the same tag."

[[quiz]]
q = "Why does deleting a large file in a later Dockerfile RUN not remove its bytes from earlier layers?"
choices = ["Docker ignores file deletion", "The later layer masks the file but does not rewrite the immutable earlier layer", "Deletion takes effect only after `docker image prune`", "The build cache keeps a copy that is still counted in the image"]
answer = 1
why = "Deletion is represented in a later filesystem changeset. The original blob still contains the file, so avoid copying secrets or temporary payloads into earlier image layers."

[[quiz]]
q = "Which command preserves image layers and image configuration for later restoration?"
choices = ["docker export", "docker image history", "docker image save", "docker cp"]
answer = 2
why = "save creates an image archive that load can restore. Container export produces a filesystem archive and does not preserve the original image history and configuration in the same way."

[[quiz]]
q = "Why can one tag run on both an ARM laptop and an x86 server?"
choices = ["An image index can reference separate platform-specific manifests", "One manifest holds both architectures' binaries in one layer", "The registry rebuilds the image for the requesting CPU on pull", "The layers contain portable bytecode any CPU can execute"]
answer = 0
why = "A multi-platform index points to images for different OS/architecture combinations. Docker selects a matching platform; the individual binaries still target a particular architecture."

[lab]
summary = "Give Alpine a lesson-owned tag, demonstrate copy-on-write, and round-trip an image archive."
setup = ["docker rm -f dojo-layer-a dojo-layer-b 2>/dev/null || true", "docker image rm dojo/layers:source dojo/layers:restored 2>/dev/null || true", "rm -f dojo-layers.tar"]
cleanup = ["docker rm -f dojo-layer-a dojo-layer-b 2>/dev/null || true", "docker image rm dojo/layers:source dojo/layers:restored 2>/dev/null || true", "rm -f dojo-layers.tar || true"]

[[lab.steps]]
title = "Pull and name immutable content"
task = "Pull **alpine:3.20** and tag the selected local image **dojo/layers:source**. Inspect its history and metadata. The lesson tag must point to the same image ID as the pulled base."
hints = ["docker image tag creates another reference without copying all layers.", "docker image inspect -f can read .Id."]
solution = '''
```bash
docker pull alpine:3.20
docker image tag alpine:3.20 dojo/layers:source
docker image history dojo/layers:source
docker image inspect -f '{{.Id}} {{.Os}}/{{.Architecture}}' dojo/layers:source
```

The new tag belongs to the lesson. The public base stays available after lesson cleanup.
'''

[[lab.steps.checks]]
name = "the lesson tag identifies the pulled Alpine image"
run = "id=$(docker image inspect -f '{{.Id}}' dojo/layers:source) && test -n \"$id\" && test \"$id\" = \"$(docker image inspect -f '{{.Id}}' alpine:3.20)\""

[[lab.steps]]
title = "Prove each writable layer is private"
task = "Start **dojo-layer-a** and **dojo-layer-b** from **dojo/layers:source**. Write **only A** followed by a newline to **/tmp/dojo-layer.txt** in A. Keep both running; B must not have that file."
hints = ["Run sleep 3600 in both containers.", "Use docker exec on A and compare the filesystems; do not create a shared volume."]
solution = '''
```bash
docker run -d --name dojo-layer-a dojo/layers:source sleep 3600
docker run -d --name dojo-layer-b dojo/layers:source sleep 3600
docker exec dojo-layer-a sh -c 'printf "%s\n" "only A" > /tmp/dojo-layer.txt'
docker diff dojo-layer-a
```

A changeset is attached to A alone. B and the image remain unchanged.
'''

[[lab.steps.checks]]
name = "A has its private change"
run = "test \"$(docker inspect -f '{{.State.Running}}' dojo-layer-a)\" = true && test \"$(docker inspect -f '{{.Image}}' dojo-layer-a)\" = \"$(docker image inspect -f '{{.Id}}' dojo/layers:source)\" && docker exec dojo-layer-a cat /tmp/dojo-layer.txt"
equals = "only A"

[[lab.steps.checks]]
name = "B runs the same image but has no private A file"
run = "test \"$(docker inspect -f '{{.State.Running}}' dojo-layer-b)\" = true && test \"$(docker inspect -f '{{.Image}}' dojo-layer-b)\" = \"$(docker image inspect -f '{{.Id}}' dojo/layers:source)\" && docker exec dojo-layer-b test ! -e /tmp/dojo-layer.txt"

[[lab.steps]]
title = "Create an offline image archive"
task = "Tag **dojo/layers:source** additionally as **dojo/layers:restored**, save that tag to **dojo-layers.tar** in the lab folder, then remove only the **dojo/layers:restored** tag. Keep the source tag and both containers."
hints = ["docker image save -o FILE IMAGE writes an image archive to the client.", "Removing one of two tags normally removes only that reference."]
solution = '''
```bash
docker image tag dojo/layers:source dojo/layers:restored
docker image save -o dojo-layers.tar dojo/layers:restored
docker image rm dojo/layers:restored
```

The archive includes image metadata and layers, not the private file written in container A.
'''

[[lab.steps.checks]]
name = "a readable archive exists and its restore tag is absent"
run = "test -s dojo-layers.tar && test -z \"$(docker image ls --filter reference=dojo/layers:restored --format '{{.Repository}}:{{.Tag}}')\" && tar -tf dojo-layers.tar | grep -Eq '(^|/)(manifest.json|index.json)$'"

[[lab.steps]]
title = "Restore the saved reference"
task = "Load **dojo-layers.tar**. The restored **dojo/layers:restored** reference must identify exactly the same image as **dojo/layers:source**. Inspect its metadata rather than expecting container A changes to appear in it."
hints = ["docker image load -i FILE restores the tags stored by save.", "Compare .Id values, not table sizes rounded for humans."]
solution = '''
```bash
docker image load -i dojo-layers.tar
docker image inspect -f '{{.Id}}' dojo/layers:restored
```

An image archive carries image content. A container writable layer would require a separate, deliberate workflow.
'''

[[lab.steps.checks]]
name = "the restored tag identifies the original image"
run = "id=$(docker image inspect -f '{{.Id}}' dojo/layers:restored) && test -n \"$id\" && test \"$id\" = \"$(docker image inspect -f '{{.Id}}' dojo/layers:source)\""
+++

## Images are content plus configuration

An image is a collection of immutable filesystem changesets and configuration. The configuration includes the default command, environment, working directory, user and other metadata. It is more than a tar file of an application directory: the runtime needs both the files and instructions for starting them.

```text
container A writable layer       container B writable layer
             \                     /
              shared application layer
              shared dependency layer
              shared base filesystem layers
```

A running container gets its own writable layer above those shared layers. Reading an unchanged file uses the underlying image. Changing an existing file requires a private writable copy; creating a file adds it to the container layer. Removing a file hides it from the merged view without rewriting the image underneath.

This is copy-on-write. It explains why a second container starts without duplicating the whole image and why an emergency edit inside one container does not repair other replicas. It also explains why removing the container discards its edits while removing an image tag does not immediately remove content still referenced elsewhere.

## The merged filesystem hides implementation details

A union filesystem presents multiple layers as one directory tree. On Linux, OverlayFS can combine lower read-only directories with an upper writable directory. Docker's classic `overlay2` storage driver implements this familiar model.

Docker Engine 29 uses the containerd image store by default on **fresh installations**. Upgraded installations can retain classic storage drivers such as `overlay2`; containerd uses snapshotters rather than those classic drivers. Inspect your engine instead of assuming every Docker 29 installation reports the same backend:

```bash
docker info --format '{{.Driver}}'
docker info
```

The backend difference does not change the learner-facing image/container model. See the [Docker 29 storage explanation](https://docs.docker.com/engine/storage/containerd/). Do not browse or edit the daemon's private storage directories to investigate layers: they belong to the engine and, on Desktop or an SSH machine, may not even live on your computer.

Copy-on-write has a cost. Updating a small portion of a large image file can require copying that file into the writable layer. Frequently changing application data belongs in a suitable volume or external service, not in an ever-growing container layer. Volumes are separate from this root-filesystem mechanism.

## Pull transfers missing content

```bash
docker pull alpine:3.20
docker image ls alpine
docker image inspect -f '{{.Id}} {{.Os}}/{{.Architecture}}' alpine:3.20
docker image history alpine:3.20
```

Pull resolves the reference, retrieves manifests and configuration, and downloads content the selected engine lacks. Shared layers can be reused across images. If your context uses SSH, the remote daemon stores those layers. A later run uses local content unless its pull policy or an explicit pull requests something else.

`history` helps associate layers with build instructions. Its size column is not a precise accounting of total host disk usage: blobs can be shared, compressed or stored in both compressed and unpacked forms. Metadata-only instructions may appear without meaningful filesystem bytes. New build tooling can also change the presentation of build history.

## Tags, IDs and digests answer different questions

| Identifier | Example shape | Meaning |
|---|---|---|
| Tag | `alpine:3.20` | A convenient, potentially mutable reference |
| Local image ID | `sha256:…` | A content hash identifying the local image configuration |
| Registry digest | `repository@sha256:…` | Content-addressed reference to a manifest or image index |
| Container ID | Another hexadecimal ID | An instance, not its image |

A tag can move to different content. Tagging locally creates an additional reference; it does not copy every layer:

```bash
docker image tag alpine:3.20 dojo/layers:source
docker image inspect -f '{{.Id}}' dojo/layers:source
```

The local image ID is not generally interchangeable with a registry manifest digest. A registry reference by digest makes the selected manifest immutable, but you must obtain that real digest from trusted registry metadata. Do not invent a hash or copy a digest from an unrelated platform and expect it to work.

Inspect repository digest information when available:

```bash
docker image inspect -f '{{json .RepoDigests}}' alpine:3.20
```

Locally built or merely retagged images may not have a repository digest for your new repository. That absence does not mean their local image ID is missing.

## Architecture is part of the image contract

An image index, often called a multi-arch manifest list, points to platform-specific manifests. The same tag can therefore select Linux/amd64 on one host and Linux/arm64 on another. The executable bytes are still architecture-specific; there is no universal translation baked into a tag.

```bash
docker manifest inspect alpine:3.20
# If the buildx plugin is installed:
docker buildx imagetools inspect alpine:3.20
```

These inspect registry metadata; they need registry access even when an image already exists locally. A local `image inspect` describes locally available content, whereas a registry index can describe several platforms. Buildx `--platform` lets a build target a platform deliberately, but running a foreign binary requires native support or a configured emulation mechanism. An `exec format error` is a useful architecture clue.

## Save/load preserves an image, not a running machine

```bash
docker image save -o dojo-layers.tar dojo/layers:source
docker image load -i dojo-layers.tar
```

The archive contains image layers, configuration and saved references. The CLI writes or reads the archive on your computer even when the engine is remote. This makes it useful for transfer without a registry. It does not capture running processes, container logs, writable-layer edits or separate volumes.

`docker export` exports a container filesystem view instead. Importing that filesystem does not restore the same original image layering and configuration contract. For ordinary image transport, choose save/load. For application data, choose a deliberate backup method appropriate to the data store.

Deleting a file in a later Dockerfile layer does not remove its bytes from an earlier immutable layer. Never copy credentials into an image and then delete them as a supposed security measure. Keep build inputs small and prevent unwanted inputs from entering the context.

## Clean references, not your neighbor's disk

A dangling image has no normal repository tag pointing to it; it may appear as `<none>`. A new build can leave an older untagged image behind. Dangling does not mean it is safe to delete every untagged object on a shared engine.

In PodDojo, remove only explicit lesson-owned references:

```bash
docker image rm dojo/layers:source dojo/layers:restored
```

A shared layer remains while another image or container needs it. Image removal is reference-aware, so a successful untag operation need not recover the size shown in an image table. Do not remove public base images or run broad pruning commands during these lessons.

## Why this matters for Kubernetes

Kubernetes also resolves image references and depends on platform-compatible content:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-layer-reader
  namespace: images-lab
spec:
  containers:
    - name: reader
      image: alpine:3.20
      imagePullPolicy: IfNotPresent
      command: ["sleep", "3600"]
```

This is a translation example. A locally built Docker image is not automatically present in kind's node image stores; load lesson images into the lab cluster before using them. For CKAD, distinguish image retrieval failures from application exits, and remember that changing a tag does not rewrite an existing container's filesystem.
