+++
id = "ref-networking"
title = "Services, DNS, Ingress, and NetworkPolicy"
order = 5
summary = "Route traffic by the right ports and names, then verify both allowed and denied connections."
+++

## Service contract

A Service routes to backend endpoints, not to a container merely because containerPort appears in YAML. The process must listen on the Pod network address and intended port. This example targets the field-guide Pod from the Pod YAML reference, which has app=field-guide and a named port http at 8080. Namespace ref-practice and that Pod must exist first.

```yaml
apiVersion: v1
kind: Service
metadata: {name: web, namespace: ref-practice}
spec:
  type: ClusterIP
  selector: {app: field-guide}
  ports:
  - name: http
    protocol: TCP
    port: 80
    targetPort: http
```

| Setting | What the caller or backend uses |
|---|---|
| port: 80 | Service listener, such as web:80 |
| targetPort: http | Named container port resolved on each selected Pod; here 8080 |
| targetPort: 8080 | Numeric backend listener, independent of the port name |
| nodePort: 30080 | Additional listener on nodes for a NodePort/LoadBalancer Service |
| clusterIP: None | Headless Service: DNS returns backend addresses without a virtual Service IP |

ClusterIP is internal by default. NodePort adds a node-level port, normally from 30000–32767. LoadBalancer requests external load-balancer integration and may remain pending on kind. ExternalName returns a DNS alias and performs no traffic proxying, port translation or TLS hostname rewriting. A selectorless Service can use manually maintained discovery.k8s.io/v1 EndpointSlices; link them with kubernetes.io/service-name and correct named ports. Do not edit slices managed by a Service controller. See [Services](https://kubernetes.io/docs/concepts/services-networking/service/).

```bash
kubectl -n ref-practice get service web -o wide
kubectl -n ref-practice get pods -l app=field-guide --show-labels
kubectl -n ref-practice get endpointslices -l kubernetes.io/service-name=web -o yaml
kubectl -n ref-practice exec field-guide -c http -- wget -qO- -T 2 http://web/identity
kubectl -n ref-practice port-forward service/web 18080:80
```

Empty endpoints suggest a selector mismatch, no matching ready Pods, or a named target port that cannot be resolved. Existing endpoints with failing traffic suggest listener, port, policy or application problems. Readiness normally controls whether endpoints receive ordinary Service traffic; publishNotReadyAddresses changes that behavior deliberately.

## DNS names

For the lab's cluster.local domain, these names refer to Service web in ref-practice:

| Name | Where it is useful |
|---|---|
| web | Same namespace, using the Pod DNS search path |
| web.ref-practice | Other namespaces |
| web.ref-practice.svc.cluster.local | Explicit cluster Service FQDN |
| web.ref-practice.svc.cluster.local. | Absolute DNS name with a trailing dot |
| journal-0.journal-headless.ref-practice.svc.cluster.local | StatefulSet Pod with governing headless Service |

Custom clusters can use another domain. DNS search suffixes depend on the Pod's resolver configuration. ClusterFirst is the usual policy; hostNetwork workloads normally need ClusterFirstWithHostNet for cluster DNS. Inspect `/etc/resolv.conf` and use nslookup from the actual client. ExternalName does not make a certificate for the alias valid or adjust HTTP Host headers.

## Ingress: host, path and TLS

PodDojo prepares Traefik when a lab requires ingress-controller. The ready-made class is traefik. This manifest expects Service web on port 80 and TLS Secret web-tls in the same namespace. Supply a certificate whose DNS SAN covers web.dojo.test; do not reuse a private production key for practice.

```bash
kubectl -n ref-practice create secret tls web-tls --cert=server.crt --key=server.key --dry-run=client -o yaml | kubectl apply -f -
```

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata: {name: web, namespace: ref-practice}
spec:
  ingressClassName: traefik
  tls:
  - hosts: [web.dojo.test]
    secretName: web-tls
  rules:
  - host: web.dojo.test
    http:
      paths:
      - path: /identity
        pathType: Exact
        backend:
          service: {name: web, port: {number: 80}}
      - path: /
        pathType: Prefix
        backend:
          service: {name: web, port: {name: http}}
```

Exact matches only that case-sensitive path. Prefix matches path elements: /app matches /app and /app/cart, not /apple. The longest matching path wins; Exact wins a tie with Prefix. ImplementationSpecific delegates semantics to the controller. Ingress does not inherently remove a prefix: the backend normally receives the original path. TLS termination also needs the correct SNI hostname, not only an HTTP Host header. The [Ingress reference](https://kubernetes.io/docs/concepts/services-networking/ingress/) details matching.

```bash
kubectl -n dojo-ingress exec deploy/probe -- curl -sS --max-time 3 -H 'Host: web.dojo.test' http://traefik.dojo-ingress/identity
kubectl -n ref-practice get secret web-tls -o go-template='{{index .data "tls.crt" | base64decode}}' | kubectl -n dojo-ingress exec -i deploy/probe -- sh -c 'cat > /tmp/practice-ca.pem; curl -sS --max-time 3 --cacert /tmp/practice-ca.pem --connect-to web.dojo.test:443:traefik.dojo-ingress:443 https://web.dojo.test/identity'
```

The second command validates against a self-signed practice certificate used as the trust anchor. For a real CA-signed certificate, supply its trusted CA chain instead. Give the controller time to reconcile before judging a routing change.

## NetworkPolicy patterns

Policies require CNI enforcement; PodDojo's kindnet enforces them. They select Pods, not Services, and combine additively. A connection must satisfy both source egress isolation and destination ingress isolation when those directions are isolated. Return traffic for an allowed connection is implicitly allowed.

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: default-deny, namespace: ref-practice}
spec:
  podSelector: {}
  policyTypes: [Ingress, Egress]
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: web-from-edge, namespace: ref-practice}
spec:
  podSelector: {matchLabels: {app: field-guide}}
  policyTypes: [Ingress]
  ingress:
  - from:
    - namespaceSelector:
        matchLabels: {kubernetes.io/metadata.name: ref-edge}
      podSelector: {matchLabels: {role: proxy}}
    ports: [{protocol: TCP, port: 8080}]
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: web-dns, namespace: ref-practice}
spec:
  podSelector: {matchLabels: {app: field-guide}}
  policyTypes: [Egress]
  egress:
  - to:
    - namespaceSelector:
        matchLabels: {kubernetes.io/metadata.name: kube-system}
      podSelector: {matchLabels: {k8s-app: kube-dns}}
    ports:
    - {protocol: UDP, port: 53}
    - {protocol: TCP, port: 53}
```

A peer combining namespaceSelector and podSelector means AND. Separate list peers mean OR. A podSelector alone selects peers only in the policy's namespace; namespaceSelector alone selects every Pod in matching namespaces. An empty namespaceSelector matches all namespaces, while an empty top-level podSelector selects all Pods in this namespace. No ingress rules means no allowed ingress; ingress with one empty rule allows all ingress.

To permit same-namespace role=frontend clients, use `from: [{podSelector: {matchLabels: {role: frontend}}}]`. For egress to an application, use the same peer structure under egress.to and specify its destination port. DNS rules may need adaptation for NodeLocal DNS or different labels. A DNS allow rule permits lookup; it does not permit subsequent HTTP connections.

Test the full matrix: allowed client/port, wrong client, wrong namespace and wrong port. Confirm clients can execute commands and the server is healthy before interpreting a timeout as policy enforcement. Broad pre-existing policies can defeat a new narrow policy. See [NetworkPolicy semantics](https://kubernetes.io/docs/concepts/services-networking/network-policies/).
