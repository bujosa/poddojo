+++
id = "docker-container-networking"
title = "Container networking and published ports"
module = "data-network"
minutes = 40
needs = "docker"
objectives = ["Use user-defined bridge DNS and network aliases", "Distinguish container ports from published host ports", "Inspect network membership and attach a second network", "Choose bridge, host or none networking deliberately"]

[[quiz]]
q = "What does a user-defined bridge add compared with the default bridge?"
choices = ["A separate kernel", "Automatic DNS by container name and network alias", "A public IP for every container", "Automatic host-port publication"]
answer = 1
why = "User-defined bridges provide scoped DNS. The default bridge does not provide the same automatic name lookup; publishing host ports is a separate decision."

[[quiz]]
q = "Two containers share a user-defined network. How should a client reach nginx listening on port 80?"
choices = ["The daemon SSH port", "The published host port only", "The nginx container name or alias on port 80", "localhost:80 inside the client"]
answer = 2
why = "A shared network gives direct container-to-container reachability. localhost in the client means the client itself, not its peer."

[[quiz]]
q = "What does -p 18090:80 mean?"
choices = ["Host 80 forwards to container 18090", "Container 80 becomes reachable through host 18090", "Both ports must be identical", "The application starts listening on 18090 inside"]
answer = 1
why = "Publication maps a host socket to an existing listener in the container network namespace. It does not reconfigure the application listener."

[[quiz]]
q = "A container connects to two bridge networks. What is the safest assumption?"
choices = ["It automatically routes every packet between them", "It can communicate with peers on both attached networks", "Every other container joins both networks", "Its aliases become global DNS records"]
answer = 1
why = "Dual attachment gives that container access to both networks. It does not automatically turn the container into a router or make network-scoped aliases globally visible."

[lab]
summary = "Build a DNS-enabled bridge, reach a server by alias, publish it to the selected host, and attach a second network."
setup = []
cleanup = ["docker rm -f dojo-net-web dojo-net-client 2>/dev/null || true", "docker network rm dojo-net-front dojo-net-back 2>/dev/null || true"]

[[lab.steps]]
title = "Create an application network"
task = "Create the user-defined **bridge** network **dojo-net-front**. Inspect its driver and scope."
hints = ["docker network create defaults to the bridge driver.", "PodDojo labs pass an explicit subnet from 10.214.0.0/16 (here 10.214.1.0/24) so they work even when Docker's automatic address pools are used up."]
solution = '''```bash
docker network create --driver bridge --subnet 10.214.1.0/24 dojo-net-front
docker network inspect dojo-net-front
```'''

[[lab.steps.checks]]
name = "application network is a local bridge"
run = "docker network inspect -f '{{.Driver}} {{.Scope}}' dojo-net-front"
equals = "bridge local"

[[lab.steps]]
title = "Use name-based communication"
task = "Run **dojo-net-web** from **nginx:1.27-alpine** on **dojo-net-front** with network alias **dojo-http**. Run **dojo-net-client** from **busybox:1.36** on the same network with **sleep 3600**. From the client, fetch **http://dojo-http:80/** successfully. Do not publish a host port yet."
hints = ["--network-alias adds another name on that network.", "BusyBox includes wget; use wget -qO- from docker exec."]
solution = '''```bash
docker run -d --name dojo-net-web --network dojo-net-front --network-alias dojo-http nginx:1.27-alpine
docker run -d --name dojo-net-client --network dojo-net-front busybox:1.36 sleep 3600
for i in $(seq 1 30); do
  if docker exec dojo-net-client wget -qO- -T 2 http://dojo-http:80/; then break; fi
  sleep 2
done
```'''

[[lab.steps.checks]]
name = "client reaches the nginx welcome page through its network alias"
run = "for i in $(seq 1 30); do if docker exec dojo-net-client wget -qO- -T 2 http://dojo-http:80/ | grep -q 'Welcome to nginx'; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Publish a host port"
task = "Replace **dojo-net-web** with an equivalent nginx container, preserving network **dojo-net-front** and alias **dojo-http**, but publish **18090:80**. Reach it at **http://$PODDOJO_HOST:18090/**. Keep the client running."
hints = ["Port mappings are chosen when a container is created; recreate the server.", "The client continues using the container port and alias."]
solution = '''```bash
docker rm -f dojo-net-web
docker run -d --name dojo-net-web --network dojo-net-front --network-alias dojo-http -p 18090:80 nginx:1.27-alpine
for i in $(seq 1 30); do
  if curl -fsS --max-time 2 "http://$PODDOJO_HOST:18090/"; then break; fi
  sleep 2
done
```'''

[[lab.steps.checks]]
name = "published server responds on host port 18090"
run = "for i in $(seq 1 30); do if docker inspect -f '{{range (index .HostConfig.PortBindings \"80/tcp\")}}{{println .HostPort}}{{end}}' dojo-net-web | grep -qx 18090 && curl -fsS --max-time 2 \"http://$PODDOJO_HOST:18090/\" | grep -q 'Welcome to nginx'; then echo ok; exit 0; fi; sleep 2; done; exit 1"
equals = "ok"
timeout = 90

[[lab.steps]]
title = "Attach the client to two networks"
task = "Create bridge network **dojo-net-back**. Connect the existing **dojo-net-client** to it with alias **dojo-inspector**, retaining its original front-network attachment and ability to fetch **http://dojo-http:80/**. Leave the web container only on the front network."
hints = ["docker network connect attaches an existing container.", "Use --alias on network connect, rather than --network-alias."]
solution = '''```bash
docker network create --driver bridge --subnet 10.214.2.0/24 dojo-net-back
docker network connect --alias dojo-inspector dojo-net-back dojo-net-client
docker inspect -f '{{json .NetworkSettings.Networks}}' dojo-net-client
```'''

[[lab.steps.checks]]
name = "client is dual-attached with the requested back-network alias"
run = "test \"$(docker inspect -f '{{len .NetworkSettings.Networks}}' dojo-net-client)\" = 2 && docker inspect -f '{{with index .NetworkSettings.Networks \"dojo-net-back\"}}{{range .Aliases}}{{println .}}{{end}}{{end}}' dojo-net-client | grep -qx dojo-inspector && docker inspect -f '{{with index .NetworkSettings.Networks \"dojo-net-front\"}}{{.NetworkID}}{{end}}' dojo-net-client | grep -q . && test \"$(docker inspect -f '{{len .NetworkSettings.Networks}}' dojo-net-web)\" = 1 && echo ok"
equals = "ok"

[[lab.steps.checks]]
name = "dual-attached client still reaches the front-network web service"
run = "docker inspect -f '{{with index .NetworkSettings.Networks \"dojo-net-back\"}}{{.NetworkID}}{{end}}' dojo-net-client | grep -q . && docker exec dojo-net-client wget -qO- -T 3 http://dojo-http:80/ | grep -q 'Welcome to nginx' && echo ok"
equals = "ok"
+++

## Every container gets a network viewpoint

With ordinary bridge networking, each container has its own network namespace: interfaces, routes, listening sockets and loopback. Two programs in different containers can both listen on port 80 because they do not share one socket table. Inside a container, `localhost` means that container. It does not mean the Docker machine, your laptop, or a neighboring database container.

Docker connects a container interface to a bridge on the daemon host. For ordinary outgoing traffic, host routing and address translation let the container reach external destinations. That outbound ability does not mean an outside client can spontaneously reach every container port. Incoming publication is a separate configuration decision.

```text
client container ── dojo-net-front bridge ── web container :80
                                                ▲
computer ── PODDOJO_HOST:18090 ── host mapping ───┘
```

Always name both endpoints when describing a connection: source namespace and destination address plus port. “Port 80 works” is incomplete; it might work from one container but fail from your browser because nothing is published.


> **"all predefined address pools have been fully subnetted"?** Docker hands each new network a
> subnet from a small set of default pools (172.17–31.0.0/16 and 192.168.0.0/16 cut into /20s, about
> 31 networks). Machines with many Compose projects run out. Either remove networks you no longer
> need (`docker network ls`, then `docker network rm NAME`), pass an explicit `--subnet` as these
> labs do, or give the daemon bigger pools with `default-address-pools` in `daemon.json`. Kubernetes
> does not create a Docker bridge per application: the CNI plugin gives Pods addresses from the
> cluster's Pod ranges, which are finite too (configured CIDRs, CNI allocation and node limits).

## Prefer a user-defined bridge

The built-in network named `bridge` is a default attachment for containers without an explicit network selection. Containers there can communicate by IP under the usual bridge configuration, but do not get the same automatic name resolution that user-defined bridges provide. Avoid legacy links or hardcoded IP addresses as a substitute for a proper application network.

```bash
docker network create --driver bridge --subnet 10.214.1.0/24 dojo-net-front
docker run -d --name dojo-net-web --network dojo-net-front \
  --network-alias dojo-http nginx:1.27-alpine
docker run -d --name dojo-net-client --network dojo-net-front \
  busybox:1.36 sleep 3600
docker exec dojo-net-client wget -qO- http://dojo-http:80/
```

The server is discoverable by container name and by alias on that network. Docker's embedded DNS resolves the current address, so recreating a named container need not force configuration changes in its clients. An already-open connection or cached DNS record can still refer to the previous container until the client reconnects or resolves again. Applications need sensible retries.

| Network choice | Name resolution and isolation | Appropriate use |
|---|---|---|
| Default bridge | Shared default network; no automatic peer-name DNS equivalent | Simple experiments |
| User-defined bridge | Scoped DNS names and selectable membership | Related application services |
| Host | Shares the host networking namespace where supported | Special workloads needing that model |
| None | Only loopback, no normal external connectivity | Deliberately disconnected workloads |

A bridge network is local to one Docker daemon. Giving networks identical names on two servers does not connect them. Multi-host networking requires additional infrastructure; a plain bridge is not a cross-machine service mesh. The [bridge driver documentation](https://docs.docker.com/engine/network/drivers/bridge/) describes these boundaries.

## Publishing is for traffic through the host

`-p 18090:80` maps port 18090 on the selected Docker machine to port 80 in the container. Read it as `HOST:CONTAINER`. The application must actually listen at the container end, usually on all appropriate interfaces rather than only container loopback. `EXPOSE 80` in an image is documentation and metadata; it does not create a host mapping.

```bash
docker run -d --name dojo-published -p 18091:80 nginx:1.27-alpine
docker port dojo-published
docker inspect -f '{{json .NetworkSettings.Ports}}' dojo-published
curl -fsS "http://$PODDOJO_HOST:18091/"
```

Without an explicit host address, publication commonly binds all host interfaces. That makes the lab usable with a remote machine, but also means network access controls matter. For a service intended only for clients on the Docker machine, bind loopback explicitly, such as `-p 127.0.0.1:18091:80`. A remote laptop cannot directly reach that loopback binding merely because it has an SSH Docker context.

Peer containers sharing the bridge use `dojo-http:80`, not the published port 18090. Publication is not required for that internal connection. Conversely, a laptop browser cannot normally resolve Docker's network-scoped alias. Use the reachable host address supplied as `PODDOJO_HOST`. TCP is the default publishing protocol; UDP requires a mapping explicitly suffixed with `/udp`.

## Aliases belong to networks

An alias describes a role rather than a single container identity: `dojo-http` can remain your frontend endpoint while the backing container changes. Aliases are scoped to the network attachment. Do not assume an alias defined on the frontend network is visible to a container attached only to the backend network.

```bash
docker network create --subnet 10.214.2.0/24 dojo-net-back
docker network connect --alias dojo-inspector dojo-net-back dojo-net-client
docker network inspect dojo-net-back
docker inspect -f '{{json .NetworkSettings.Networks}}' dojo-net-client
```

A container may attach to two networks at once and communicate with peers on both. That is useful for a diagnostic client or an application connecting frontend and backend services. It expands that container's reach, so attach only what it needs. Dual attachment does not automatically configure the container as a router for every other peer. Routing tables and default gateways also matter if you need predictable outbound paths.

Network membership is not a full application authorization policy. Containers sharing a bridge can normally reach each other's listening ports without host publication. Keep databases off unnecessary networks, and require application authentication where appropriate. Removing a network fails while containers still have active endpoints; remove or disconnect the specifically named lab endpoints first.

## Host and none are different models

On Linux, host networking removes the separate network namespace: the container uses host interfaces and socket ports directly. A service can conflict with an existing host listener. Port publishing flags do not provide an extra translation layer in this mode. Docker Desktop support has platform and configuration requirements, so do not assume Linux host behavior everywhere.

With `--network none`, the container has loopback but no ordinary network interface for external connectivity. It can still perform local computation and use mounted files. It is useful for a task designed not to communicate, but it will not magically download packages or contact a registry from inside its running process. Image pulling happens through the engine before the process starts.

## Diagnose the path in layers

Start with `docker ps` and `docker inspect`: is the server running, on which networks, with which port bindings? Then test from a peer using the internal name and port. Finally test from outside using the host address and published port. A successful internal request with a failed external request points toward publication, host reachability or firewall rules, rather than immediately implicating the application.

Do not paste a container IP into permanent configuration. Inspect it to understand the path, then use DNS names. Do not “fix” connectivity by attaching every workload to every network. Make the smallest membership or listener change that meets the stated requirement.

## Why this matters for Kubernetes

Kubernetes containers in one Pod share a network namespace and can use localhost to reach one another. Different Pods have distinct addresses. Services provide stable discovery and routing for replaceable Pods; they are not the same thing as Docker aliases. A named Service port may target a differently numbered application port.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: dojo-web
  namespace: networking-lab
spec:
  selector:
    app: dojo-web
  ports:
    - port: 8080
      targetPort: 80
```

Generate a Service quickly with `kubectl -n networking-lab expose deployment dojo-web --port=8080 --target-port=80 --dry-run=client -o yaml`. This assumes the Deployment already exists. A `containerPort` declaration does not itself publish a Pod outside the cluster, just as `EXPOSE` does not publish a Docker port. For exam troubleshooting, trace client → DNS → Service selector → ready endpoints → application listener before changing unrelated YAML.
