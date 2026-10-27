+++
id = "ref-workloads"
title = "Workload controllers"
order = 3
summary = "Choose and configure Deployments, batch controllers, DaemonSets, StatefulSets, and autoscaling with stable APIs."
+++

## Controller selection

Create namespace ref-practice before applying these independent examples. A controller reconciles desired state; editing one generated Pod rarely fixes the workload. Keep selectors stable and ensure the Pod template satisfies them. Inspect ownership with `kubectl get pod NAME -o yaml` before changing anything.

| Workload | Use when | Important contract |
|---|---|---|
| Deployment | Replicas are interchangeable | Revisioned template, rolling update or recreation |
| Job | Work should finish successfully | Completion count, retries, deadline |
| CronJob | Jobs should be created on a schedule | Schedule, overlap, missed-start handling |
| DaemonSet | One agent per eligible node | Node eligibility, taints, update strategy |
| StatefulSet | Identity and storage belong to each ordinal | Stable names, governing Service, claim templates |
| HPA | Replica count follows observed demand | Metrics API and appropriate resource requests |

## Deployment strategy and recovery

```yaml
apiVersion: apps/v1
kind: Deployment
metadata: {name: frontend, namespace: ref-practice}
spec:
  replicas: 3
  revisionHistoryLimit: 5
  progressDeadlineSeconds: 120
  minReadySeconds: 5
  strategy:
    type: RollingUpdate
    rollingUpdate: {maxSurge: 1, maxUnavailable: 0}
  selector: {matchLabels: {app: frontend}}
  template:
    metadata: {labels: {app: frontend}}
    spec:
      containers:
      - name: web
        image: nginx:1.27-alpine
        ports: [{name: http, containerPort: 80}]
        resources:
          requests: {cpu: 100m, memory: 32Mi}
          limits: {cpu: 200m, memory: 64Mi}
        readinessProbe:
          httpGet: {path: /, port: http}
          periodSeconds: 2
```

Surge allows extra Pods; unavailable limits how many desired replicas may be unavailable. Percentage surge rounds up; percentage unavailability rounds down. Both cannot be zero. Tight quotas must accommodate surge, including terminating Pods that may still consume resources. Recreate replaces the old replicas before bringing up the new ones; set `strategy: {type: Recreate}` and remove rollingUpdate fields. Read the [Deployment documentation](https://kubernetes.io/docs/concepts/workloads/controllers/deployment/) when availability settings interact.

```bash
kubectl -n ref-practice set image deployment/frontend web=nginx:1.26-alpine
kubectl -n ref-practice rollout status deployment/frontend --timeout=120s
kubectl -n ref-practice rollout history deployment/frontend
kubectl -n ref-practice rollout history deployment/frontend --revision=1
kubectl -n ref-practice rollout undo deployment/frontend --to-revision=1
kubectl -n ref-practice rollout pause deployment/frontend
kubectl -n ref-practice rollout resume deployment/frontend
```

Only template changes produce a new revision; scaling alone does not. Rollback restores a retained template, not external ConfigMap contents, Service selectors or replica count. Kubernetes reports a progress deadline failure; it does not automatically roll back a Deployment. Diagnose before restoring a known-good revision.

## Job and CronJob

```yaml
apiVersion: batch/v1
kind: Job
metadata: {name: batch-report, namespace: ref-practice}
spec:
  completions: 3
  parallelism: 2
  backoffLimit: 1
  activeDeadlineSeconds: 90
  ttlSecondsAfterFinished: 3600
  template:
    spec:
      restartPolicy: Never
      containers:
      - name: report
        image: busybox:1.36
        command: [sh, -ec, 'echo report-complete']
---
apiVersion: batch/v1
kind: CronJob
metadata: {name: scheduled-report, namespace: ref-practice}
spec:
  schedule: '*/15 * * * *'
  timeZone: Etc/UTC
  concurrencyPolicy: Forbid
  startingDeadlineSeconds: 60
  successfulJobsHistoryLimit: 2
  failedJobsHistoryLimit: 1
  suspend: false
  jobTemplate:
    spec:
      backoffLimit: 1
      activeDeadlineSeconds: 90
      template:
        spec:
          restartPolicy: Never
          containers:
          - name: report
            image: busybox:1.36
            command: [echo, scheduled-report]
```

completions counts successes; parallelism bounds concurrent workers. `restartPolicy` must be Never or OnFailure for Job Pods. Never leaves failed attempts easier to inspect; OnFailure may restart a container inside the same Pod. backoffLimit bounds retries, while activeDeadlineSeconds bounds elapsed execution time. A Job's Pod template is generally immutable: create a replacement Job when the work changes. Indexed Jobs use completionMode Indexed and expose a completion index for partitioning work.

CronJob policies are Allow, Forbid and Replace; they apply to Jobs from the same CronJob, not every Job in a namespace. startingDeadlineSeconds concerns a missed scheduled start, not execution duration. suspend prevents future scheduling but does not stop existing Jobs. History limits retain finished Jobs, while TTL independently deletes them after completion. Scheduling is approximate: design work to tolerate duplicate or missed execution. See [CronJob behavior](https://kubernetes.io/docs/concepts/workloads/controllers/cron-jobs/).

```bash
kubectl -n ref-practice create job report-now --from=cronjob/scheduled-report
kubectl -n ref-practice wait --for=condition=complete job/report-now --timeout=120s
kubectl -n ref-practice logs job/report-now
kubectl -n ref-practice describe job/report-now
```

## DaemonSet and StatefulSet minimums

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata: {name: node-agent, namespace: ref-practice}
spec:
  selector: {matchLabels: {app: node-agent}}
  updateStrategy: {type: RollingUpdate, rollingUpdate: {maxUnavailable: 1}}
  template:
    metadata: {labels: {app: node-agent}}
    spec:
      nodeSelector: {kubernetes.io/os: linux}
      containers:
      - {name: agent, image: 'busybox:1.36', command: [sleep, '3600']}
---
apiVersion: v1
kind: Service
metadata: {name: journal-headless, namespace: ref-practice}
spec:
  clusterIP: None
  selector: {app: journal}
  ports: [{name: peer, port: 8080, targetPort: 8080}]
---
apiVersion: apps/v1
kind: StatefulSet
metadata: {name: journal, namespace: ref-practice}
spec:
  serviceName: journal-headless
  replicas: 2
  selector: {matchLabels: {app: journal}}
  template:
    metadata: {labels: {app: journal}}
    spec:
      containers:
      - name: journal
        image: busybox:1.36
        command: [sh, -ec, 'test -f /data/identity || hostname > /data/identity; sleep 3600']
        volumeMounts: [{name: data, mountPath: /data}]
  volumeClaimTemplates:
  - metadata: {name: data}
    spec:
      storageClassName: standard
      accessModes: [ReadWriteOnce]
      resources: {requests: {storage: 64Mi}}
```

The sample StatefulSet does not serve HTTP; the governing Service provides its network identity. Claims become data-journal-0 and data-journal-1. Default claim retention preserves PVCs on deletion or scale-down; check the configured retention policy before cleanup. Default OrderedReady startup waits for each predecessor to become ready. DaemonSets count eligible nodes rather than a replicas field; inspect desiredNumberScheduled and numberReady.

## HPA with autoscaling/v2

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata: {name: frontend, namespace: ref-practice}
spec:
  scaleTargetRef: {apiVersion: apps/v1, kind: Deployment, name: frontend}
  minReplicas: 2
  maxReplicas: 5
  metrics:
  - type: Resource
    resource:
      name: cpu
      target: {type: Utilization, averageUtilization: 70}
  behavior:
    scaleDown: {stabilizationWindowSeconds: 120}
```

Utilization is usage relative to CPU requests, not limits or total node capacity. Resource metrics need a working metrics.k8s.io provider; PodDojo does not install metrics-server by default. This manifest can be valid while targets show unknown. Investigate `kubectl describe hpa frontend` and resource requests before changing thresholds. Multiple metrics choose the largest desired replica recommendation, subject to safety handling of unavailable metrics. See [horizontal autoscaling](https://kubernetes.io/docs/concepts/workloads/autoscaling/horizontal-pod-autoscale/).
