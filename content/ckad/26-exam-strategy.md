+++
id = "ckad-exam-strategy"
title = "Exam strategy: turn knowledge into points"
module = "exam-prep"
minutes = 25
needs = "none"
objectives = ["Establish the target host, context and namespace for every task", "Allocate two hours using task weight and confidence", "Generate and verify resources under time pressure", "Plan the final week with PodDojo mock exams"]

[[quiz]]
q = "A task gives a different target host or context. What is the first action?"
choices = ["Reuse the previous task namespace", "Follow the task target instructions, then confirm namespace", "Delete resources with the same name everywhere", "Start applying manifests and fix the target later"]
answer = 1
why = "A correct object on the wrong target earns no useful result. Select the supplied host or context before setting the namespace and making changes."

[[quiz]]
q = "A low-weight task has consumed eight minutes without progress. What is the best next move?"
choices = ["Keep going until solved", "Flag it, record the obstacle and take an easier task", "Restart the exam desktop", "Guess that it has passed"]
answer = 1
why = "Time has an opportunity cost. Preserve useful work, mark the task and collect accessible points before returning with the remaining budget."

[[quiz]]
q = "Which workflow best balances speed and correctness?"
choices = ["Type every manifest from memory", "Generate a skeleton, add required fields, apply and verify", "Paste any blog example unchanged", "Only inspect resources without changing them"]
answer = 1
why = "Imperative generators reduce typing, but inspection and verification catch wrong namespaces, missing fields and workloads that never become ready."

[[quiz]]
q = "Why reserve time at the end?"
choices = ["The final tasks always have more weight", "To recheck flagged tasks, target namespaces and actual outcomes", "The timer pauses for verification", "Only the final object is graded"]
answer = 1
why = "A quick final pass can catch incomplete rollouts, typoed names and tasks applied to the wrong namespace. The clock continues running, so reserve that time deliberately."

[[quiz]]
q = "Which terminal paste shortcut is normally used in the exam Linux remote desktop?"
choices = ["Command+V in every application", "Ctrl+Shift+V in the terminal", "Ctrl+W", "The Insert key is required"]
answer = 1
why = "The Linux terminal normally uses Ctrl+Shift+C and Ctrl+Shift+V; other desktop applications use Ctrl+C and Ctrl+V. Practice the provided desktop, since host clipboard behavior and key mappings can differ."

[[quiz]]
q = "What should you trust for exam rules shortly before the appointment?"
choices = ["A remembered task count from last year", "The current official CKAD page, candidate handbook and allowed-resources rules", "An unofficial question dump", "The PodDojo pass result alone"]
answer = 1
why = "Versions, delivery details, permitted resources and retake terms can change. Mock exams train execution but do not replace the current official policies or guarantee a real-exam result."
+++

## Train execution, not recognition

The CKAD is an online, remotely proctored performance exam delivered through PSI. You solve approximately 15–20 command-line tasks in two hours using a remote desktop. The pass mark is 66%, and the standard exam purchase includes one free retake, subject to eligibility and scheduling terms. These are planning facts, not permanent promises: check the [official CKAD page](https://training.linuxfoundation.org/certification/certified-kubernetes-application-developer-ckad/), [candidate handbook](https://docs.linuxfoundation.org/tc-docs/certification/lf-handbook2), and [exam FAQ](https://docs.linuxfoundation.org/tc-docs/certification/faq-cka-ckad-cks) before booking and again before exam day.

Your job is to produce working resource state. Recognizing which answer looks plausible is not enough. Practice turning a written requirement into commands, YAML and a short verification sequence without opening a worked solution. A mock score is useful feedback, but repeated practice against a memorized task is weaker evidence than solving a fresh variation.

## Establish the target before doing work

Expect tasks across several cluster environments. Follow the target instructions on **each** task. Current official instructions describe designated SSH hosts reached from the base desktop. Other task wording may supply a kubeconfig context. Do not assume your previous terminal is still attached to the correct environment. Read the [current technical instructions](https://docs.linuxfoundation.org/tc-docs/certification/tips-cka-and-ckad) for the actual delivery model.

The following is an **exam-only illustration**, not a PodDojo lab procedure. In PodDojo, the application supplies its protected kubeconfig; never switch or modify that configuration. In the real exam, if a task explicitly supplies a context, select that supplied context first:

```bash
alias k=kubectl
kubectl config use-context TASK_CONTEXT
kubectl config set-context --current --namespace=TASK_NAMESPACE
kubectl config current-context
kubectl get pods -n TASK_NAMESPACE
```

Replace the placeholders with task-provided values. When instructed to use a designated host, SSH to that host instead, run kubectl there, and exit back to the base before moving to another host; do not accumulate nested SSH sessions. Aliases belong to the shell where they are defined. Explicit `-n` flags are often safer than relying entirely on a remembered default namespace.

## Read the entire task before typing

Extract the target, namespace, resource names, images, required ports, labels and any constraints about preserving existing state. A request to repair one field is not a request to rebuild the whole application. Existing selectors, names and ownership relationships often matter.

```text
target -> namespace -> desired state -> smallest change -> independent verification
```

Translate prose into a short acceptance checklist. For a Service repair, that might be: correct selector, correct Service port, correct targetPort, ready endpoints and a successful request. The checklist prevents a fast but incomplete repair from feeling finished merely because kubectl accepted the command.

## Spend minutes where they earn points

At the start, scan the task list and identify high-weight work you can complete quickly. Use the platform's flag or review feature for uncertain tasks. Do not solve strictly in numerical order if a small unfamiliar detail is consuming the time needed for several familiar tasks.

| Phase | Suggested budget | Goal |
|---|---|---|
| Orientation and triage | 5 minutes | Understand targets and identify quick wins |
| First pass | 65 minutes | Finish familiar tasks with verification |
| Second pass | 35 minutes | Resolve flagged and longer tasks |
| Final review | 15 minutes | Verify outcomes and recover omissions |

Sixteen tasks across 120 minutes average 7.5 minutes each **including review**. With a review reserve, many first-pass tasks need to take roughly four to six minutes. Use weights as a guide, not a guarantee of difficulty. If you cannot name your next useful diagnostic action, flag the task and move on. Leave valid partial work when appropriate, but do not assume any particular partial-credit algorithm.

## Generate the boring parts

Start imperatively where kubectl has a reliable generator. Add the fields the generator does not express, then apply the result. This reduces transcription mistakes without hiding the desired state.

```bash
kubectl -n practice create deployment web --image=nginx:1.27-alpine --replicas=2 --dry-run=client -o yaml > web.yaml
kubectl explain deployment.spec.template.spec.containers.readinessProbe
kubectl apply --dry-run=server -f web.yaml
kubectl apply -f web.yaml
kubectl -n practice rollout status deployment/web --timeout=90s
```

A dry run proves API acceptance, not a working application. For NetworkPolicy and complex multi-container Pods, start from an official YAML example and reduce it to the task's fields. Know which values belong in Pod metadata and which belong in the controller's Pod template. Changing a Deployment's own labels does not change existing Pod labels.

```yaml
spec:
  template:
    metadata:
      labels:
        app: web
    spec:
      containers:
      - name: web
        image: nginx:1.27-alpine
```

Keep indentation visible. Sibling fields align; list items start with a dash. Do not copy tabs into YAML. Keep API version and kind intact when adapting an example, then inspect the nested path for every field you add.

## Survive the editor and remote clipboard

Practice one editor until its basic commands are automatic. In vim, `i` enters insert mode, Escape returns to normal mode, `:wq` writes and quits, and `:q!` abandons unsaved changes. `:set number` helps locate an error; `:set expandtab shiftwidth=2 tabstop=2` helps maintain spaces. If autoindent mangles pasted YAML, use `:set paste` before pasting and `:set nopaste` afterwards.

The remote Linux terminal normally uses Ctrl+Shift+C and Ctrl+Shift+V; other desktop applications use Ctrl+C and Ctrl+V. A Mac host's usual shortcuts are not a reliable guide. The official environment prohibits the Insert key, so learn `i` rather than relying on it. Test the remote desktop and keyboard layout before the timed appointment. Do not assume the host clipboard is available or permitted; use the provided environment and follow its instructions.

## Verify the result from another angle

After creating a Job, wait for completion and inspect its output. After changing a Deployment, inspect its rollout and ready replicas. After repairing a Service, inspect EndpointSlices and send a request. After a security change, test both the allowed action and a relevant denied one. The acceptance command should measure what the task requested, rather than repeat the mutation that supposedly accomplished it.

Watch for false confidence: Running is not Ready; a saved manifest is not an applied resource; a successful apply is not a successful rollout. Read events and logs when status disagrees with your expectation. Use short timeouts during diagnosis so one failed request does not silently consume several minutes.

## Use documentation as a tool

The permitted resources include kubernetes.io/docs through the exam environment's browser. Consult the [current allowed-resources rules](https://docs.linuxfoundation.org/tc-docs/certification/certification-resources-allowed) for exact scope and exceptions. Do not assume an external search result, forum or AI assistant is allowed because a documentation page links to it. Learn the official page titles for probes, NetworkPolicies, volumes and Ingress so you can reach a relevant example quickly.

## The final week with PodDojo

Spend the first two days repairing your weakest domains with lesson labs and quizzes. On day three, take **CKAD Mock Exam 1** in PodDojo with the full 120-minute timer and no solutions. Record where time went and which checks failed. On day four, redo those skills from a blank namespace, changing names or values to defeat memorization.

Use day five for another available PodDojo mock, or reset Mock Exam 1 after reviewing the gaps. Aim for a comfortable margin above 66% while verifying each task. On day six, rehearse imperative generation, YAML editing, documentation navigation and the remote environment checklist. On the final day, review short notes, confirm identification and system requirements, and rest. Avoid replacing deliberate practice with last-minute feature collecting.

## Exam tips

Confirm the target every time, finish quick valuable tasks first, and reserve review time. Verify real state after every change. PodDojo's fixed lab cluster builds the execution habits; the official task panel tells you which host, context and rules apply on the actual day.
