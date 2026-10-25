+++
id = "ckad-jobs-and-cronjobs"
title = "Jobs and CronJobs: completion, retries and schedules"
module = "design-build"
minutes = 45
needs = "cluster"
objectives = [
  "Control Job completion, parallelism, retries and deadlines",
  "Distinguish execution timeouts from finished-Job cleanup",
  "Configure CronJob schedules, concurrency and retention",
  "Create an immediate Job from a CronJob template",
]
domain = "design-build"

[[quiz]]
q = "What does completions=3, parallelism=2 request?"
choices = [
  "Two successes using three containers each",
  "Three successful Pod completions with up to two active attempts",
  "Three nodes",
  "Exactly two running Pods at all times",
]
answer = 1
why = "Completions sets the success target; parallelism controls concurrent active work. Scheduling and reconciliation timing can temporarily reduce observed concurrency."

[[quiz]]
q = "Which field stops a Job that runs too long?"
choices = ["ttlSecondsAfterFinished", "successfulJobsHistoryLimit", "activeDeadlineSeconds", "schedule"]
answer = 2
why = "The active deadline limits execution duration. TTL begins after completion and controls removal rather than runtime."

[[quiz]]
q = "Does concurrencyPolicy Forbid stop a manually created Job from overlapping?"
choices = [
  "Yes, across the cluster",
  "Yes, across the namespace",
  "No; it governs scheduled Jobs from that CronJob",
  "Only if the image is busybox",
]
answer = 2
why = "The policy coordinates a CronJob's scheduled executions. Manual Jobs and other CronJobs are outside that coordination."

[[quiz]]
q = "What is true when restartPolicy is Never?"
choices = [
  "No Job retries can happen",
  "A failed container stays stopped in its Pod; the Job may create another Pod",
  "The Job can never complete",
  "Every failure is ignored",
]
answer = 1
why = "Never disables in-Pod container restarts, not the Job controller's replacement attempts. backoffLimit still controls the Job retry budget."

[lab]
summary = "Compare completion and concurrency, observe a deadline failure, then test and enable a CronJob."
setup = ["kubectl create namespace jobs-lab --dry-run=client -o yaml | kubectl apply -f -"]
cleanup = ["kubectl delete namespace jobs-lab --ignore-not-found --wait=false 2>/dev/null || true"]

[[lab.steps]]
title = "Complete three units of work"
task = "Create Job batch in jobs-lab with completions 3, parallelism 2, backoffLimit 2, activeDeadlineSeconds 120 and ttlSecondsAfterFinished 3600. Its worker uses busybox:1.36, restartPolicy Never, and prints item-complete before sleeping two seconds and exiting successfully."
hints = ["These execution controls belong in the Job spec.", "The Pod restart policy belongs under template.spec."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: batch/v1
kind: Job
metadata:
  name: batch
  namespace: jobs-lab
spec:
  completions: 3
  parallelism: 2
  backoffLimit: 2
  activeDeadlineSeconds: 120
  ttlSecondsAfterFinished: 3600
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: worker
          image: busybox:1.36
          command: ["sh", "-c"]
          args: ["echo item-complete; sleep 2"]
EOF
kubectl -n jobs-lab wait --for=condition=complete job/batch --timeout=150s
```

Three successes are required, with at most two active attempts targeted concurrently. The runtime deadline and cleanup TTL control different lifecycle stages.
'''

[[lab.steps.checks]]
name = "completion and retry settings are correct"
run = "kubectl -n jobs-lab get job batch -o jsonpath='{.spec.completions} {.spec.parallelism} {.spec.backoffLimit} {.spec.activeDeadlineSeconds} {.spec.ttlSecondsAfterFinished} {.spec.template.spec.restartPolicy}'"
equals = "3 2 2 120 3600 Never"

[[lab.steps.checks]]
name = "three successful completions are recorded"
run = '''kubectl -n jobs-lab get job batch -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n jobs-lab get job batch -o jsonpath='{.status.succeeded} {.status.conditions[?(@.type=="Complete")].status}' 2>/dev/null); [ "$v" = "3 True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "worker reports its output"
run = "kubectl -n jobs-lab logs job/batch -c worker"
equals = "item-complete"

[[lab.steps.checks]]
name = "batch worker uses the required image"
run = "kubectl -n jobs-lab get job batch -o jsonpath='{.spec.template.spec.containers[?(@.name==\"worker\")].image}'"
equals = "busybox:1.36"

[[lab.steps]]
title = "Bound a task's duration"
task = "Create Job deadline in jobs-lab with busybox:1.36 running sleep 3600, restartPolicy Never, backoffLimit 0, activeDeadlineSeconds 5 and ttlSecondsAfterFinished 3600. Observe a Failed condition with reason DeadlineExceeded."
hints = ["The image need not start before the active deadline can expire.", "Wait for condition=failed on the Job, not Ready on a Pod."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: batch/v1
kind: Job
metadata:
  name: deadline
  namespace: jobs-lab
spec:
  backoffLimit: 0
  activeDeadlineSeconds: 5
  ttlSecondsAfterFinished: 3600
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: worker
          image: busybox:1.36
          command: ["sleep", "3600"]
EOF
kubectl -n jobs-lab wait --for=condition=failed job/deadline --timeout=120s
```

The Job deadline bounds the whole active execution window. It can expire even while startup is delayed, and is independent of retry exhaustion.
'''

[[lab.steps.checks]]
name = "deadline and cleanup limits are configured"
run = "kubectl -n jobs-lab get job deadline -o jsonpath='{.spec.activeDeadlineSeconds} {.spec.backoffLimit} {.spec.ttlSecondsAfterFinished} {.spec.template.spec.restartPolicy} {.spec.template.spec.containers[?(@.name==\"worker\")].image}'"
equals = "5 0 3600 Never busybox:1.36"

[[lab.steps.checks]]
name = "Job fails because its deadline expires"
run = '''kubectl -n jobs-lab get job deadline -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n jobs-lab get job deadline -o jsonpath='{.status.conditions[?(@.type=="Failed")].reason}' 2>/dev/null); [ "$v" = "DeadlineExceeded" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps]]
title = "Prepare a safe schedule"
task = "Create CronJob heartbeat in jobs-lab, every ten minutes in Etc/UTC, initially suspended. Use concurrencyPolicy Forbid, startingDeadlineSeconds 60, success history 1 and failure history 2. Its worker must use busybox:1.36 to print heartbeat, with restartPolicy Never and Job backoffLimit 1."
hints = ["Use schedule: '*/10 * * * *'.", "The Job retry limit is inside jobTemplate.spec."]
solution = '''
```bash
kubectl apply -f - <<'EOF'
apiVersion: batch/v1
kind: CronJob
metadata:
  name: heartbeat
  namespace: jobs-lab
spec:
  schedule: "*/10 * * * *"
  timeZone: Etc/UTC
  suspend: true
  concurrencyPolicy: Forbid
  startingDeadlineSeconds: 60
  successfulJobsHistoryLimit: 1
  failedJobsHistoryLimit: 2
  jobTemplate:
    spec:
      backoffLimit: 1
      template:
        spec:
          restartPolicy: Never
          containers:
            - name: worker
              image: busybox:1.36
              command: ["echo", "heartbeat"]
EOF
```

The outer specification controls scheduling; the nested Job specification controls each run. Suspension makes preparation independent of the current minute.
'''

[[lab.steps.checks]]
name = "schedule and timezone are explicit"
run = "kubectl -n jobs-lab get cronjob heartbeat -o jsonpath='{.spec.schedule} {.spec.timeZone} {.spec.suspend}'"
equals = "*/10 * * * * Etc/UTC true"

[[lab.steps.checks]]
name = "concurrency and retention are configured"
run = "kubectl -n jobs-lab get cronjob heartbeat -o jsonpath='{.spec.concurrencyPolicy} {.spec.startingDeadlineSeconds} {.spec.successfulJobsHistoryLimit} {.spec.failedJobsHistoryLimit}'"
equals = "Forbid 60 1 2"

[[lab.steps.checks]]
name = "Job template uses the intended execution settings"
run = "kubectl -n jobs-lab get cronjob heartbeat -o jsonpath='{.spec.jobTemplate.spec.backoffLimit} {.spec.jobTemplate.spec.template.spec.restartPolicy} {.spec.jobTemplate.spec.template.spec.containers[?(@.name==\"worker\")].image}'"
equals = "1 Never busybox:1.36"

[[lab.steps]]
title = "Test without waiting for the clock"
task = "Create Job heartbeat-now from CronJob heartbeat in jobs-lab. Wait for it to complete and confirm its output is heartbeat. Leave the CronJob suspended for this test."
hints = ["kubectl create job accepts --from=cronjob/heartbeat.", "Suspension does not prevent a manual template copy."]
solution = '''
```bash
kubectl -n jobs-lab create job heartbeat-now --from=cronjob/heartbeat
kubectl -n jobs-lab wait --for=condition=complete job/heartbeat-now --timeout=120s
```

A manual Job copies the template without waiting for the scheduler. Its success and logs test the executable behavior of that template.
'''

[[lab.steps.checks]]
name = "manual test completes"
run = '''kubectl -n jobs-lab get job heartbeat-now -o name >/dev/null 2>&1 || exit 1; for i in $(seq 1 45); do v=$(kubectl -n jobs-lab get job heartbeat-now -o jsonpath='{.status.conditions[?(@.type=="Complete")].status}' 2>/dev/null); [ "$v" = "True" ] && echo ok && exit 0; sleep 2; done; printf '%s\n' "$v"; exit 1'''
timeout = 120
equals = "ok"

[[lab.steps.checks]]
name = "copied template produces heartbeat"
run = "kubectl -n jobs-lab logs job/heartbeat-now -c worker"
equals = "heartbeat"

[[lab.steps]]
title = "Enable the tested schedule"
task = "Unsuspend CronJob heartbeat in jobs-lab, preserving the ten-minute schedule and Forbid policy. You do not need to wait for a scheduled execution."
hints = ["Patch spec.suspend to false.", "Do not modify or rerun the completed manual Job."]
solution = '''
```bash
kubectl -n jobs-lab patch cronjob heartbeat --type=merge -p '{"spec":{"suspend":false}}'
```

Unsuspending permits future scheduled Jobs. It leaves the already completed manual test unchanged and retains the configured concurrency policy.
'''

[[lab.steps.checks]]
name = "tested schedule is enabled"
run = "kubectl -n jobs-lab get cronjob heartbeat -o jsonpath='{.spec.suspend} {.spec.schedule} {.spec.concurrencyPolicy}'"
equals = "false */10 * * * * Forbid"
+++

## Finite work needs a completion contract

A web server is healthy while it keeps serving. A batch task is healthy when it finishes correctly. Jobs express that distinction: their controller tracks successful Pod completions and retries failures within configured limits.

```text
CronJob schedule → Job → Pod attempt → exit code
                   ↑         |
                   └─ retry failure, count success
```

A Job does not guarantee that an external side effect happens exactly once. A process can update a database and crash before reporting success; another attempt can repeat the operation. Design batch commands to tolerate retries, for example by using a unique work key or an atomic update. Kubernetes retries execution, not application transactions.

A quick one-shot task is easy to generate:

```bash
kubectl -n jobs-lab create job sample --image=busybox:1.36 \
  --dry-run=client -o yaml -- echo complete
```

Inspect the generated restart policy and command before applying. A Job's template is a Pod template, so ordinary container fields still live under template.spec.containers.

## Completions and parallelism answer different questions

For a normal fixed-completion Job, completions is the number of successful Pods required. Parallelism is the desired maximum number of active Pods at a time. Both default to one in the ordinary single-task case.

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: batch
  namespace: jobs-lab
spec:
  completions: 3
  parallelism: 2
  backoffLimit: 2
  activeDeadlineSeconds: 120
  ttlSecondsAfterFinished: 3600
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: worker
          image: busybox:1.36
          command: ["sh", "-c"]
          args: ["echo item-complete; sleep 2"]
```

This requests three successful completions, with up to two active attempts at once. Parallelism is a control target, not a promise that exactly two Pods will be running at every instant. Scheduling capacity, controller timing and failures affect observations.

Repeated identical commands do not automatically receive different work items. For explicit index-based partitioning, Indexed completion mode gives each Pod a completion index. A work-queue design can instead have workers claim items from a queue. Do not assume parallelism alone divides your dataset.

## Retrying inside a Pod versus replacing a Pod

Jobs allow Pod restartPolicy Never or OnFailure. With Never, a failed container leaves a failed Pod and the Job controller can create another Pod. With OnFailure, the kubelet can restart a failed container inside the existing Pod.

| Control | Scope | Purpose |
|---|---|---|
| restartPolicy | Pod/container execution | Decide whether a failed container restarts locally |
| backoffLimit | Job | Bound failure retries |
| activeDeadlineSeconds | Job | Bound total active duration |
| ttlSecondsAfterFinished | Finished Job | Delay automatic cleanup after success or failure |

Never often makes debugging clearer because each failed attempt remains as a separate Pod. With OnFailure, inspect previous container logs for earlier crashes. Retry accounting can include container restarts, so backoffLimit should not be interpreted as an exact promise about how many Pod objects you will see.

When a Job reaches its active deadline, Kubernetes terminates active work and marks the Job failed with DeadlineExceeded. A deadline can stop a Job before its retry budget is exhausted. It includes time the Job spends active, not just useful CPU work. Set it generously enough for normal scheduling and startup.

TTL is a cleanup control, not an execution timeout. It starts after the Job finishes and permits the TTL controller to delete the Job and its dependents. A very short TTL can erase logs and status before you inspect or grade them. This lab uses an hour.

## Inspect the controller's result

```bash
kubectl -n jobs-lab wait --for=condition=complete job/batch --timeout=120s
kubectl -n jobs-lab get pods -l batch.kubernetes.io/job-name=batch
kubectl -n jobs-lab logs job/batch
kubectl -n jobs-lab describe job batch
```

A completed task's Pod does not need to remain Ready. Successful completion, zero exit status and expected output are better evidence. Logs from job/name select a Pod; for parallel work, inspect individual Pod names when you need every worker's output.

Do not blindly retry a whole failed Job without understanding its partial effects. Many Job fields, especially the Pod template, are immutable in normal use. To change the command for a new run, create a new named Job or deliberately delete and recreate only the lesson's existing Job.

## Cron syntax and the extra template level

A CronJob is a schedule plus a Job template. Its five cron fields are:

```text
minute  hour  day-of-month  month  day-of-week
*/10    *     *             *      *          every ten minutes
0       2     *             *      *          daily at 02:00
```

Quote schedules in YAML because an asterisk has YAML syntax meaning. Use spec.timeZone, such as Etc/UTC, to make the intended timezone explicit. Do not embed a timezone prefix into the schedule string.

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: heartbeat
  namespace: jobs-lab
spec:
  schedule: "*/10 * * * *"
  timeZone: Etc/UTC
  suspend: true
  concurrencyPolicy: Forbid
  successfulJobsHistoryLimit: 1
  failedJobsHistoryLimit: 2
  jobTemplate:
    spec:
      backoffLimit: 1
      template:
        spec:
          restartPolicy: Never
          containers:
            - name: worker
              image: busybox:1.36
              command: ["echo", "heartbeat"]
```

Notice the nesting: CronJob spec contains jobTemplate.spec, which contains template.spec. Place Job deadlines and retry limits in the Job spec, not beside the schedule.

## Concurrency, history and suspension

Allow permits overlapping scheduled Jobs. Forbid skips a new scheduled execution when the same CronJob's earlier Job is still active. Replace terminates the current scheduled Job and replaces it. These policies do not coordinate independent CronJobs or prevent you from manually creating overlapping Jobs.

History limits retain a number of finished successful or failed Jobs. TTL is time-based cleanup and can remove a Job before a history limit would. Neither is a durable audit archive.

Suspend prevents future scheduled execution; already running Jobs continue. Unsuspending can make missed executions eligible, subject to startingDeadlineSeconds and controller rules. That field limits how late a scheduled start may be, whereas activeDeadlineSeconds limits a Job's duration after it starts.

Test the template immediately without waiting for the clock:

```bash
kubectl -n jobs-lab create job heartbeat-now --from=cronjob/heartbeat
```

The new Job copies the current template. Later CronJob edits do not rewrite it. Schedules are approximate and can miss or duplicate executions under unusual conditions, reinforcing the need for idempotent work. See the [CronJob documentation](https://kubernetes.io/docs/concepts/workloads/controllers/cron-jobs/) for scheduling semantics.

## Read counters as evidence, not a work ledger

A successful completion counter records Kubernetes' view of finished attempts. It does not prove that an external payment, upload or database change occurred exactly once. Keep application-level records when duplicate processing would matter.

When a Job fails, inspect both the controller's condition reason and the individual Pod's termination reason. A retry budget failure, a deadline expiration and an invalid command need different fixes. Increasing backoffLimit cannot repair a misspelled executable, and increasing a deadline cannot provide missing credentials. Preserve enough failed history to diagnose the cause before removing the evidence.

## Exam tips

Separate scheduling, runtime deadline, retry budget and cleanup. Use a manual Job to test a CronJob template quickly. Verify the required spec even if no schedule has fired, and preserve finished evidence long enough to inspect success.
