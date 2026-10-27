+++
id = "ref-kubectl-speed"
title = "kubectl speed reference"
order = 1
summary = "Generate manifests, inspect precise fields, and complete common exam operations without changing your lab context."
+++

## Start with explicit scope

PodDojo supplies its own KUBECONFIG and Docker context. Keep those environment settings. Use an explicit namespace on each operation; aliases save keystrokes without switching the user's context. The examples use a disposable namespace. Commands that emit YAML do not create their generated object.

```bash
kubectl create namespace ref-practice --dry-run=client -o yaml | kubectl apply -f -
alias k=kubectl
alias kn='kubectl -n ref-practice'
# Optional interactive bash completion, if installed:
source <(kubectl completion bash)
complete -o default -F __start_kubectl k
```

Generate, edit, validate, apply, then observe. Client dry-run constructs an object locally; it does not prove the server accepts its schema, admission settings or runtime behavior. Server dry-run sends admission requests without persisting the resource; admission webhooks must support dry-run.

```bash
kubectl -n ref-practice create deployment web --image=nginx:1.27-alpine --dry-run=client -o yaml > web.yaml
kubectl apply --dry-run=server -f web.yaml
kubectl apply -f web.yaml
kubectl -n ref-practice rollout status deployment/web --timeout=120s
```

## Imperative generators

These are the built-in generation paths for common exam resources. Add redirection when you want a file. Secrets shown here contain practice data only; TLS and registry forms require your own prepared inputs. Shell placeholders are explained directly below the block.

```bash
kubectl -n ref-practice run client --image=busybox:1.36 --restart=Never --dry-run=client -o yaml --command -- sleep 3600
kubectl -n ref-practice create deployment web --image=nginx:1.27-alpine --replicas=2 --dry-run=client -o yaml
kubectl -n ref-practice create job report --image=busybox:1.36 --dry-run=client -o yaml -- echo report
kubectl -n ref-practice create cronjob report --image=busybox:1.36 --schedule='*/10 * * * *' --dry-run=client -o yaml -- echo report
kubectl -n ref-practice create job report-now --from=cronjob/report --dry-run=client -o yaml
kubectl -n ref-practice create configmap settings --from-literal=MODE=practice --dry-run=client -o yaml
kubectl -n ref-practice create secret generic identity --from-literal=ACCOUNT=practice --dry-run=client -o yaml
kubectl -n ref-practice create secret tls web-tls --cert=server.crt --key=server.key --dry-run=client -o yaml
kubectl -n ref-practice create secret docker-registry pull-auth --from-file=.dockerconfigjson=registry-config.json --dry-run=client -o yaml
kubectl -n ref-practice create serviceaccount reader --dry-run=client -o yaml
kubectl -n ref-practice create role pod-reader --verb=get,list,watch --resource=pods --dry-run=client -o yaml
kubectl -n ref-practice create rolebinding read-pods --role=pod-reader --serviceaccount=ref-practice:reader --dry-run=client -o yaml
kubectl create clusterrole dojo-pod-reader --verb=get,list --resource=pods --dry-run=client -o yaml
kubectl create clusterrolebinding dojo-read-pods --clusterrole=dojo-pod-reader --serviceaccount=ref-practice:reader --dry-run=client -o yaml
kubectl -n ref-practice create quota budget --hard=requests.cpu=1,requests.memory=1Gi,pods=10 --dry-run=client -o yaml
kubectl -n ref-practice create poddisruptionbudget web --selector=app=web --min-available=1 --dry-run=client -o yaml
kubectl create priorityclass dojo-practice --value=1000 --description='Practice priority' --dry-run=client -o yaml
kubectl -n ref-practice create service clusterip web --tcp=80:8080 --dry-run=client -o yaml
kubectl -n ref-practice create service clusterip peer --clusterip=None --tcp=8080:8080 --dry-run=client -o yaml
kubectl -n ref-practice create service nodeport edge --tcp=80:8080 --node-port=30080 --dry-run=client -o yaml
kubectl -n ref-practice create service loadbalancer public --tcp=80:8080 --dry-run=client -o yaml
kubectl -n ref-practice create service externalname alias --external-name=web.ref-practice.svc.cluster.local --dry-run=client -o yaml
kubectl -n ref-practice expose deployment web --port=80 --target-port=80 --dry-run=client -o yaml
kubectl -n ref-practice create ingress web --class=traefik --rule='web.dojo.test/app*=web:80' --dry-run=client -o yaml
kubectl -n ref-practice autoscale deployment web --min=2 --max=5 --cpu=70% --dry-run=client -o yaml
```

The `*` in this Ingress generator selects Prefix matching; inspect the output. `expose`, `autoscale` and `--from` require the referenced object to exist. Generated Service selectors need inspection. ClusterRoleBinding grants cluster-wide scope: prefer a namespaced RoleBinding when possible. DaemonSets, StatefulSets, NetworkPolicies, LimitRanges, PVCs and CRDs have no dedicated `kubectl create <kind>` generator. Start from a YAML skeleton and use `kubectl create --dry-run=client -f file.yaml -o yaml`; do not invent generator subcommands. See the [kubectl command reference](https://kubernetes.io/docs/reference/generated/kubectl/kubectl-commands) for supported generators.

## Inspect exactly the fields you need

```bash
kubectl explain pod.spec.containers --recursive
kubectl explain deployment.spec.strategy --api-version=apps/v1
kubectl -n ref-practice get pods --show-labels
kubectl -n ref-practice get pods -o jsonpath='{range .items[*]}{.metadata.name}{"\t"}{.status.podIP}{"\n"}{end}'
kubectl -n ref-practice get pods -o custom-columns='NAME:.metadata.name,NODE:.spec.nodeName,IMAGE:.spec.containers[*].image'
kubectl -n ref-practice get pods --sort-by=.metadata.creationTimestamp
kubectl -n ref-practice get events --sort-by=.metadata.creationTimestamp
```

Quote JSONPath and custom-column expressions so the shell does not expand brackets. JSONPath supports filters and ranges, not a general-purpose scripting language. Escape literal dots inside map keys: `{.metadata.labels.kubernetes\.io/hostname}`. Sorting is ascending; use `tail` when you want recent entries.

## Change and investigate

```bash
kubectl -n ref-practice set image deployment/web nginx=nginx:1.27-alpine
kubectl -n ref-practice rollout history deployment/web
kubectl -n ref-practice rollout undo deployment/web --to-revision=1
kubectl -n ref-practice rollout status deployment/web --timeout=120s
kubectl -n ref-practice scale deployment/web --replicas=3
kubectl -n ref-practice label pod/client role=frontend --overwrite
kubectl -n ref-practice annotate deployment/web owner=practice --overwrite
kubectl -n ref-practice exec client -- sh -c 'id; printenv'
kubectl -n ref-practice logs deployment/web --all-containers=true --tail=50
kubectl -n ref-practice logs pod/client --previous
kubectl -n ref-practice port-forward service/web 18080:80
kubectl -n ref-practice debug pod/client --image=busybox:1.36 --target=client -- sleep 600
kubectl auth can-i list pods -n ref-practice --as=system:serviceaccount:ref-practice:reader
```

Use the actual container name with `set image` and `-c`. Port-forward occupies the terminal until interrupted. An ephemeral debug container stays in Pod history and cannot be removed individually. `--target` requests visibility into the target container's processes when supported by the runtime. `--previous` reads the last terminated instance, not a deleted Pod.

## Fast YAML editing

In Vim, enter `:set expandtab shiftwidth=2 softtabstop=2 autoindent`, then `:set paste` before a large paste; restore `:set nopaste` afterwards. Use `:set number`, `gg`, `/field`, `dd`, `u` and `:wq` to navigate and recover quickly. Spaces matter; tabs inside indentation break YAML. Save generated manifests before experimenting, validate the final file, and wait for convergence before collecting evidence.
