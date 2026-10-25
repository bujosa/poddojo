+++
id = "docker-volumes-and-bind-mounts"
title = "Volumes, bind mounts and temporary data"
module = "data-network"
minutes = 40
needs = "docker"
objectives = ["Choose named volumes, bind mounts or tmpfs for a data lifecycle", "Prove data survives removal of its creating container", "Share a named volume with read-only consumers", "Explain why remote bind mounts use the daemon host filesystem"]

[[quiz]]
q = "Where is the source of a bind mount resolved for a remote Docker engine?"
choices = ["Inside the image", "On the computer running the CLI", "On the selected daemon host", "Inside the SSH connection cache"]
answer = 2
why = "The daemon creates the mount. A local laptop directory is not automatically uploaded to the server; a named volume avoids this host-path dependency."

[[quiz]]
q = "What happens to a named volume when its container is removed?"
choices = ["It is always deleted", "It remains until explicitly removed when no longer in use", "It becomes part of the image", "It is copied into the next image pull"]
answer = 1
why = "Named volumes have their own lifecycle. Removing a container removes its writable layer, while the named volume remains available for a replacement container."

[[quiz]]
q = "Two containers mount one volume, and the second uses readonly. What is protected?"
choices = ["The entire volume from every writer", "Only reads by the first container", "Writes through the second container mount", "All files on the host"]
answer = 2
why = "Read-only is a property of that mount. Another container with a writable mount can still change the shared data, and readers may see those changes."

[[quiz]]
q = "Which storage is designed to disappear when its container stops?"
choices = ["A named volume", "A bind mount", "An anonymous volume in every case", "A tmpfs mount"]
answer = 3
why = "tmpfs is a temporary memory-backed filesystem. Stopping the container discards it. Anonymous volumes are still volumes and commonly persist unless explicitly removed, for example by --rm."

[lab]
summary = "Write a named volume, replace its writer, share it safely, and distinguish temporary scratch space from durable data."
setup = []
cleanup = ["docker rm -f dojo-volume-writer dojo-volume-reader dojo-volume-peer dojo-volume-scratch 2>/dev/null || true", "docker volume rm dojo-volume-data 2>/dev/null || true"]

[[lab.steps]]
title = "Write outside the container layer"
task = "Create named volume **dojo-volume-data**. Run a one-shot container **dojo-volume-writer** from **alpine:3.20**, mount the volume at **/data**, and write exactly **durable dojo** followed by a newline into **/data/message.txt**. Leave the stopped writer for inspection."
hints = ["Use --mount type=volume,src=dojo-volume-data,dst=/data.", "The shell redirection must execute inside the container."]
solution = '''```bash
docker volume create dojo-volume-data
docker run --name dojo-volume-writer --mount type=volume,src=dojo-volume-data,dst=/data alpine:3.20 sh -c 'printf "durable dojo\n" > /data/message.txt' 
```'''

[[lab.steps.checks]]
name = "successful writer mounted the named volume at /data"
run = "test \"$(docker inspect -f '{{.State.Status}} {{.State.ExitCode}}' dojo-volume-writer)\" = 'exited 0' && test \"$(docker inspect -f '{{range .Mounts}}{{if eq .Destination \"/data\"}}{{.Type}} {{.Name}}{{end}}{{end}}' dojo-volume-writer)\" = 'volume dojo-volume-data' && echo ok"
equals = "ok"

[[lab.steps.checks]]
name = "writer stored the required message on its mounted volume"
run = "docker cp dojo-volume-writer:/data/message.txt - | tar -xO"
equals = "durable dojo"

[[lab.steps]]
title = "Replace the writer without losing its data"
task = "Remove **dojo-volume-writer**. Start **dojo-volume-reader** from **alpine:3.20**, running **sleep 3600**, with **dojo-volume-data** at **/data**. Verify **/data/message.txt** still contains **durable dojo**."
hints = ["Remove only the named container; do not remove the volume.", "The replacement attaches the same source volume."]
solution = '''```bash
docker rm dojo-volume-writer
docker run -d --name dojo-volume-reader --mount type=volume,src=dojo-volume-data,dst=/data alpine:3.20 sleep 3600
docker exec dojo-volume-reader cat /data/message.txt
```'''

[[lab.steps.checks]]
name = "replacement reads the surviving message after writer removal"
run = "test -z \"$(docker ps -a --filter name=^/dojo-volume-writer$ --format '{{.Names}}')\" && test \"$(docker inspect -f '{{range .Mounts}}{{if eq .Destination \"/data\"}}{{.Name}}{{end}}{{end}}' dojo-volume-reader)\" = dojo-volume-data && test \"$(docker exec dojo-volume-reader cat /data/message.txt)\" = 'durable dojo' && echo ok"
equals = "ok"

[[lab.steps]]
title = "Share the volume with a read-only peer"
task = "Start **dojo-volume-peer** from **alpine:3.20**, running **sleep 3600**. Mount **dojo-volume-data** read-only at **/shared**. Both containers must be able to read the same message."
hints = ["Append readonly to the --mount fields.", "Inspect .Mounts: RW should be false for /shared."]
solution = '''```bash
docker run -d --name dojo-volume-peer --mount type=volume,src=dojo-volume-data,dst=/shared,readonly alpine:3.20 sleep 3600
docker exec dojo-volume-peer cat /shared/message.txt
```'''

[[lab.steps.checks]]
name = "peer has a read-only shared mount and reads the same data"
run = "test \"$(docker inspect -f '{{range .Mounts}}{{if eq .Destination \"/shared\"}}{{.Type}} {{.Name}} {{.RW}}{{end}}{{end}}' dojo-volume-peer)\" = 'volume dojo-volume-data false' && test \"$(docker exec dojo-volume-peer cat /shared/message.txt)\" = 'durable dojo' && test \"$(docker exec dojo-volume-reader cat /data/message.txt)\" = 'durable dojo' && echo ok"
equals = "ok"

[[lab.steps]]
title = "Provide temporary scratch space"
task = "Run **dojo-volume-scratch** from **alpine:3.20** with a read-only root filesystem and a **tmpfs** mount at **/scratch**, limited to **16 MiB**. Write **temporary dojo** to **/scratch/message.txt** and keep the container running with sleep 3600."
hints = ["Use --read-only and --mount type=tmpfs,dst=/scratch,tmpfs-size=16777216.", "Write to the tmpfs before using exec sleep to make sleep PID 1."]
solution = '''```bash
docker run -d --name dojo-volume-scratch --read-only --mount type=tmpfs,dst=/scratch,tmpfs-size=16777216 alpine:3.20 sh -c 'printf "temporary dojo\n" > /scratch/message.txt; exec sleep 3600' 
```'''

[[lab.steps.checks]]
name = "read-only root has a bounded tmpfs with the temporary message"
run = "for i in $(seq 1 15); do if test \"$(docker inspect -f '{{.HostConfig.ReadonlyRootfs}}' dojo-volume-scratch)\" = true && docker exec dojo-volume-scratch sh -c 'test \"$(stat -f -c %T /scratch)\" = tmpfs && test \"$(cat /scratch/message.txt)\" = \"temporary dojo\"' && docker exec dojo-volume-scratch df -k /scratch | awk 'END {print $2}' | grep -qx 16384 && echo ok; then exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 60
+++

## Decide how long the data must live

A container's writable layer belongs to that container. Restarting the same container usually preserves the layer, which can hide a bad storage design during development. Removing and replacing the container removes that layer. An application image should be replaceable without destroying business data, so put durable state on storage with an independent lifecycle.

A mount makes another filesystem visible at a path inside the container. The application continues using ordinary file operations; it does not need to know that `/data` is a Docker volume. Mounting storage does not copy every future write into the image. A new image build will not contain the database that happened to run beside it.

```text
container root filesystem       Docker-managed volume
  /bin, /etc, /app                dojo-volume-data
  /data ───────────────────────> message.txt
remove the container            volume and file remain
```

| Storage | Who chooses the source? | Typical lifetime | Useful for |
|---|---|---|---|
| Writable layer | Engine, per container | Container removal | Disposable files |
| Named volume | You name it; Docker manages it | Explicit volume removal | Database state, shared files |
| Anonymous volume | Docker generates its name | Until removal, with lifecycle exceptions | Short-lived attached storage |
| Bind mount | You select a daemon-host path | Host file lifecycle | Development source and configuration |
| tmpfs | Kernel memory-backed mount | Container stop | Scratch files and temporary secrets |

## Named volumes make replacement routine

Create a volume with `docker volume create dojo-volume-data`. Attach it using the explicit `--mount` syntax. The fields say what you mean: storage type, source name and destination path. `dst`, `target` and `destination` are accepted destination aliases. Missing named volumes are created automatically when attached, but creating them first makes their independent identity easy to see.

```bash
docker volume create dojo-volume-data
docker run --name dojo-volume-writer \
  --mount type=volume,src=dojo-volume-data,dst=/data \
  alpine:3.20 sh -c 'printf "durable dojo\n" > /data/message.txt'
docker volume inspect dojo-volume-data
docker inspect -f '{{json .Mounts}}' dojo-volume-writer
```

The redirection belongs inside the container's quoted shell command. Writing `docker run ... echo hello > /data/message.txt` redirects the CLI's output on your computer instead. Quoting determines which shell sees the operator; a mount does not change the local terminal's filesystem.

Remove the stopped writer, attach the same volume to a replacement, and the file is still there. Docker refuses ordinary volume removal while a container still references it, even if that container is stopped. Remove the specific consumers first, then the named lab volume. `docker rm -v` and `--rm` do not delete explicitly named volumes. See [Docker's volume lifecycle documentation](https://docs.docker.com/engine/storage/volumes/) for the anonymous-volume exceptions.

## Anonymous does not mean temporary

An anonymous volume has a generated name rather than a human-selected one. For example, `--mount type=volume,dst=/data` asks Docker to allocate it. A Dockerfile `VOLUME` declaration can also cause an anonymous volume when the destination is not otherwise mounted. Recreating a container does not automatically select the old anonymous volume, so data can remain on disk while the new application sees an empty volume.

An ordinary container removal may leave anonymous volumes behind. `--rm` removes the container's associated anonymous volumes on exit, while named volumes remain. Anonymous volumes can be reused by their generated names, but named volumes are easier to inspect, document and clean up deliberately. This lab uses named storage throughout so no random resource needs broad cleanup.

A further surprise: mounting over existing image files hides those files. With an empty named volume, Docker normally copies existing files at the target into the volume on first attachment. Use `volume-nocopy` when you specifically do not want that initialization. A bind mount does not perform the same copy-up. Avoid diagnosing a hidden image directory as a failed image build.

## Bind mounts belong to the daemon machine

A bind mount exposes a particular host file or directory. With a local Linux daemon that might feel like a direct connection to your terminal's filesystem. With a remote SSH context, however, the path is resolved on the **remote host**. Docker Desktop provides selected host-file integration into its Linux VM, but that convenience is not a general remote file-transfer service.

```bash
# Only if this exact path already exists on the selected daemon host:
docker run -d --name dojo-bind-example \
  --mount type=bind,src=/srv/dojo-example,dst=/config,readonly \
  alpine:3.20 sleep 3600
```

This is a syntax example, not a lab prerequisite. Do not create or overwrite arbitrary directories on a shared server. With ordinary `--mount type=bind`, a missing source path causes an error instead of silently giving you the intended files. Relative local source assumptions are especially dangerous when the engine is remote. Our exercises use volumes so the same commands work across both machine arrangements.

## Sharing and read-only consumers

A volume can be mounted into several containers at once, at different destinations. A writer might use `/data`, while a reader uses `/shared`. `readonly` prevents writes **through that reader's mount**; it does not freeze the volume globally. Another container can keep writing through its own writable mount.

```bash
docker run -d --name dojo-volume-peer \
  --mount type=volume,src=dojo-volume-data,dst=/shared,readonly \
  alpine:3.20 sleep 3600
```

Sharing a filesystem does not provide application-level locking or transactional consistency. Two unrelated database processes must not casually share one database data directory. File ownership still matters: the numeric UID and GID seen inside the container must have the necessary access. A volume attachment does not automatically make its files writable by every non-root user.

## tmpfs is deliberately disposable

A tmpfs mount provides temporary storage without persisting it as a named volume. Its contents disappear when the container stops. A `--read-only` container can still write to explicitly writable volumes and tmpfs mounts, so provide scratch paths intentionally rather than making the whole root filesystem writable.

```bash
docker run -d --name dojo-volume-scratch --read-only \
  --mount type=tmpfs,dst=/scratch,tmpfs-size=16777216 \
  alpine:3.20 sleep 3600
```

The size is an upper bound, not an allocation made immediately at startup. tmpfs consumes memory resources; depending on host configuration, pages can reach swap. Do not equate “memory-backed” with a universal guarantee that sensitive bytes can never touch disk. It is unsuitable for data that must survive restarts. Bound its size and the container's memory use together.

## Why this matters for Kubernetes

Kubernetes separates a volume declaration from each container's `volumeMounts`. An `emptyDir` survives a container restart within a Pod but is deleted with that Pod. A PersistentVolumeClaim refers to storage with a different lifecycle; its eventual deletion behavior also depends on the underlying PersistentVolume reclaim policy. Docker named volumes are a useful analogy, not a Kubernetes API object.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-scratch
  namespace: storage-lab
spec:
  containers:
    - name: app
      image: alpine:3.20
      command: ["sleep", "3600"]
      volumeMounts:
        - name: scratch
          mountPath: /scratch
  volumes:
    - name: scratch
      emptyDir:
        medium: Memory
        sizeLimit: 16Mi
```

Add `readOnly: true` to a consumer's `volumeMounts` entry when needed. Generate a starting Pod with `kubectl -n storage-lab run dojo-scratch --image=alpine:3.20 --dry-run=client -o yaml`, then add volumes. Use `kubectl explain pod.spec.volumes` to avoid inventing fields. Under exam pressure, ask first whether the data must survive container restart, Pod replacement, or storage deletion; those are three different requirements.
