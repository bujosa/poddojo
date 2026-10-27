+++
id = "docker-terminal-yaml-vim"
title = "The terminal, YAML and vim"
module = "start"
minutes = 35
needs = "docker"
objectives = [
  "Navigate directories and create, inspect and filter text files safely",
  "Use redirection, quoted here-documents, variables and exit status in the shell",
  "Read and repair YAML mappings, lists, indentation and string values",
  "Edit and save YAML with vim, or use nano as a fallback",
  "Recognize the structure of a Kubernetes manifest and a multi-document file",
]

[[quiz]]
q = "You want to add one line to notes.txt while keeping its existing lines. Which command does that?"
choices = [
  "printf '%s\\n' ready > notes.txt",
  "printf '%s\\n' ready >> notes.txt",
  "printf '%s\\n' ready | cat notes.txt",
  "printf '%s\\n' ready < notes.txt",
]
answer = 1
why = "The shell opens >> for appending. A single > truncates an existing file before the command runs. A pipe sends output to another command's input; cat notes.txt reads the named file instead of that input. The < operator supplies input from a file."

[[quiz]]
q = "A here-document begins with cat > notes.txt <<'EOF' and contains $HOME. What is written for that text?"
choices = [
  "The value of the HOME environment variable",
  "The literal characters $HOME",
  "An empty string because HOME is not assigned locally",
  "The characters EOF surrounding the value of HOME",
]
answer = 1
why = "Quoting the delimiter disables variable and command expansion in the here-document body. The terminating EOF goes alone on a line without quotes or indentation. With an unquoted delimiter, $HOME would expand."

[[quiz]]
q = "grep '^ready$' notes.txt returns exit status 1. In grep '^ready$' notes.txt && printf 'found' || printf 'missing', what runs next?"
choices = [
  "Only printf 'found', because grep finished running",
  "Only printf 'missing', because grep did not succeed",
  "Both printf commands, because the operators form a pipeline",
  "Neither printf command, because status 1 stops every shell",
]
answer = 1
why = "Status zero means success. grep returns 1 when no lines match, so && skips the first printf and || runs the second. Status 1 does not automatically stop an ordinary interactive shell. grep uses a larger status for an error."

[[quiz]]
q = "A ConfigMap must have metadata.name and data.mode. Where should the data: line begin in the lesson's two-space layout?"
choices = [
  "At column 1, alongside metadata:",
  "With two spaces, alongside metadata's name:",
  "With four spaces, beneath metadata's name:",
  "With one tab, to separate it from metadata:",
]
answer = 0
why = "metadata and data are separate top-level mapping keys. Indenting data under metadata changes the structure, even if the YAML still parses. Tabs cannot be used for YAML indentation; use spaces."

[[quiz]]
q = "A ConfigMap data entry must preserve the string 08. Which YAML value makes that intent explicit?"
choices = [
  'count: "08"',
  "count: 08",
  "count: 8",
  "count: false",
]
answer = 0
why = "Quote numeric-looking identifiers when they must remain strings. Plain scalars are resolved into types according to the parser and schema; quoting avoids both type surprises and loss of a meaningful leading zero. ConfigMap data values must be strings."

[[quiz]]
q = "You are typing in vim's Insert mode and want to save the file and leave. Which sequence works?"
choices = [
  "Esc, then :wq and Enter",
  "Esc, then :q! and Enter",
  "Type :wq and Enter without leaving Insert mode",
  "Esc, then :q and Enter after making changes",
]
answer = 0
why = "Esc returns to Normal mode; : opens a command, and :wq writes then quits. :q! discards unsaved changes. :q refuses to quit a modified buffer. In Insert mode, typing :wq just inserts those characters into the file."

[lab]
summary = "Build a tiny text workspace, repair YAML, save a pipeline result and write two manifests. No Docker, Kubernetes or YAML parser is required."
files = { "broken.yaml" = """apiVersion: v1
kind: ConfigMap
metadata:
\tname: dojo-terminal
  data:
    mode: practice
""" }
setup = ['''test -n "$PODDOJO_LAB_DIR" && mkdir -p "$PODDOJO_LAB_DIR"''']
cleanup = ['''if test -n "$PODDOJO_LAB_DIR"; then
  rm -f "$PODDOJO_LAB_DIR/practice/notes.txt" "$PODDOJO_LAB_DIR/practice/scores.txt" "$PODDOJO_LAB_DIR/practice/highest.txt" "$PODDOJO_LAB_DIR/broken.yaml" "$PODDOJO_LAB_DIR/bundle.yaml"
  rmdir "$PODDOJO_LAB_DIR/practice" 2>/dev/null || true
fi || true''']

[[lab.steps]]
title = "Create a workspace with literal text"
task = """
Work inside **`$PODDOJO_LAB_DIR`**. Create a directory named **`practice`**.
Use quoted here-documents to create these two files with the following lines.

**`practice/notes.txt`** (the dollar sign and HOME must stay literal):
```text
Terminal practice
Keep YAML indentation consistent.
$HOME stays literal here.
```

**`practice/scores.txt`**:
```text
# practice scores
8
10
3
```
"""
hints = [
  "Quote directory expansions: cd \"$PODDOJO_LAB_DIR\". mkdir -p practice creates the directory if needed.",
  "Start each file with cat > practice/NAME <<'EOF'; finish with EOF alone on its own line.",
]
solution = """
```bash
test -n "$PODDOJO_LAB_DIR"
cd "$PODDOJO_LAB_DIR"
mkdir -p practice
cat > practice/notes.txt <<'EOF'
Terminal practice
Keep YAML indentation consistent.
$HOME stays literal here.
EOF
cat > practice/scores.txt <<'EOF'
# practice scores
8
10
3
EOF
```
Quoting EOF preserves the literal dollar sign; > creates or replaces each file.
"""

[[lab.steps.checks]]
name = "practice contains all three literal note lines"
run = '''test -d "$PODDOJO_LAB_DIR/practice" && awk '
  { sub(/[ \t]+$/, "") }
  NR == 1 { if ($0 != "Terminal practice") bad = 1 }
  NR == 2 { if ($0 != "Keep YAML indentation consistent.") bad = 1 }
  NR == 3 { if ($0 != "$HOME stays literal here.") bad = 1 }
  END { exit (bad || NR != 3) }
' "$PODDOJO_LAB_DIR/practice/notes.txt"'''

[[lab.steps.checks]]
name = "scores has the header and exactly 8, 10 and 3 in order"
run = '''awk '
  { sub(/[ \t]+$/, "") }
  NR == 1 { if ($0 != "# practice scores") bad = 1 }
  NR == 2 { if ($0 != "8") bad = 1 }
  NR == 3 { if ($0 != "10") bad = 1 }
  NR == 4 { if ($0 != "3") bad = 1 }
  END { exit (bad || NR != 4) }
' "$PODDOJO_LAB_DIR/practice/scores.txt"'''

[[lab.steps.negatives]]
name = "an unquoted here-document expands HOME instead of preserving it"
run = '''test -n "$PODDOJO_LAB_DIR"
mkdir -p "$PODDOJO_LAB_DIR/practice"
cat > "$PODDOJO_LAB_DIR/practice/notes.txt" <<EOF
Terminal practice
Keep YAML indentation consistent.
$HOME stays literal here.
EOF
printf '# practice scores\n8\n10\n3\n' > "$PODDOJO_LAB_DIR/practice/scores.txt"'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/practice/notes.txt" "$PODDOJO_LAB_DIR/practice/scores.txt"'''

[[lab.steps.alternatives]]
name = "printf produces the same text with harmless trailing spaces"
run = '''test -n "$PODDOJO_LAB_DIR"
mkdir -p "$PODDOJO_LAB_DIR/practice"
printf '%s\n' 'Terminal practice  ' 'Keep YAML indentation consistent.' '$HOME stays literal here.' > "$PODDOJO_LAB_DIR/practice/notes.txt"
printf '# practice scores\n8\n10\n3  \n' > "$PODDOJO_LAB_DIR/practice/scores.txt"'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/practice/notes.txt" "$PODDOJO_LAB_DIR/practice/scores.txt"'''

[[lab.steps]]
title = "Repair indentation and nesting"
task = """
Open the supplied **`broken.yaml`** with vim (or nano). It has a tab before `name` and puts `data` inside `metadata`.
Repair it to represent exactly this single ConfigMap:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-terminal
data:
  mode: practice
```

Use unquoted keys and the shown two-space block layout. String values may be plain, single-quoted or double-quoted;
mapping key order, comments and trailing spaces do not matter. Do not add other fields or duplicate keys.
The grader reads this small, defined layout with awk; it is not a general YAML parser or Kubernetes validator.
"""
hints = [
  "In vim, Esc then :set list and Enter makes tabs visible as ^I; :set nolist hides those markers.",
  "data: must start at column 1; name: and mode: each start with exactly two spaces under their own parent.",
]
solution = """
```bash
test -n "$PODDOJO_LAB_DIR"
cat > "$PODDOJO_LAB_DIR/broken.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-terminal
data:
  mode: practice
EOF
```
The automated solution replaces the file non-interactively. In the exam, edit the same lines in your editor.
"""

[[lab.steps.checks]]
name = "the repaired mapping has no tabs, duplicates or misplaced fields"
run = '''awk '
  function scalar(s) {
    if (s ~ /^"[^"]*"$/ || s ~ /^\047[^\047]*\047$/) return substr(s, 2, length(s) - 2)
    return s
  }
  index($0, "\t") { bad = 1 }
  {
    sub(/ +#.*$/, ""); sub(/ +$/, "")
    if ($0 ~ /^ *$/ || $0 ~ /^ *#/) next
    if ($0 ~ /^[^ ]/) {
      parent = ""
      if ($0 ~ /^(metadata|data):$/) {
        parent = $0; sub(/:$/, "", parent)
        if (++seen[parent] != 1) bad = 1
        next
      }
      if ($0 ~ /^(apiVersion|kind): +/) {
        key = $0; sub(/:.*/, "", key)
        value = $0; sub(/^[^:]+: +/, "", value)
        if (++seen[key] != 1 || scalar(value) != (key == "kind" ? "ConfigMap" : "v1")) bad = 1
        next
      }
      bad = 1; next
    }
    if ($0 ~ /^  name: +/ && parent == "metadata") {
      value = $0; sub(/^  name: +/, "", value)
      if (++seen["name"] != 1 || scalar(value) != "dojo-terminal") bad = 1
      next
    }
    if ($0 ~ /^  mode: +/ && parent == "data") {
      value = $0; sub(/^  mode: +/, "", value)
      if (++seen["mode"] != 1 || scalar(value) != "practice") bad = 1
      next
    }
    bad = 1
  }
  END { exit (bad || seen["apiVersion"] != 1 || seen["kind"] != 1 || seen["metadata"] != 1 || seen["name"] != 1 || seen["data"] != 1 || seen["mode"] != 1) }
' "$PODDOJO_LAB_DIR/broken.yaml"'''

[[lab.steps.negatives]]
name = "replacing tabs with spaces leaves data nested under metadata"
run = '''cat > "$PODDOJO_LAB_DIR/broken.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-terminal
  data:
    mode: practice
EOF'''
undo = '''printf 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n\tname: dojo-terminal\n  data:\n    mode: practice\n' > "$PODDOJO_LAB_DIR/broken.yaml"'''

[[lab.steps.negatives]]
name = "correct nesting still contains tab indentation"
run = '''printf 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n\tname: dojo-terminal\ndata:\n  mode: practice\n' > "$PODDOJO_LAB_DIR/broken.yaml"'''
undo = '''printf 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n\tname: dojo-terminal\n  data:\n    mode: practice\n' > "$PODDOJO_LAB_DIR/broken.yaml"'''

[[lab.steps.negatives]]
name = "a correct-looking mode is followed by a duplicate with another value"
run = '''cat > "$PODDOJO_LAB_DIR/broken.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-terminal
data:
  mode: practice
  mode: production
EOF'''
undo = '''printf 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n\tname: dojo-terminal\n  data:\n    mode: practice\n' > "$PODDOJO_LAB_DIR/broken.yaml"'''

[[lab.steps.alternatives]]
name = "reordered mappings with comments, quoted strings and trailing spaces"
run = '''cat > "$PODDOJO_LAB_DIR/broken.yaml" <<'EOF'
# The order of mapping keys is not significant.
kind: "ConfigMap"
data:
  mode: 'practice'  
metadata:
  name: "dojo-terminal" # a comment
apiVersion: 'v1'
EOF'''
undo = '''printf 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n\tname: dojo-terminal\n  data:\n    mode: practice\n' > "$PODDOJO_LAB_DIR/broken.yaml"'''

[[lab.steps]]
title = "Save the highest score with a pipeline"
task = """
Use a pipeline to read **`practice/scores.txt`**, discard its comment line and sort the scores **numerically**.
Save only the highest value, **`10`**, as the single line in **`practice/highest.txt`**.
Replace any old contents; the output file must not contain the other scores or the header.
"""
hints = [
  "grep -v '^#' keeps lines that do not start with #. sort -n compares numbers instead of text.",
  "After ascending numeric sort, tail -n 1 selects the highest score. Use > for the output file.",
]
solution = """
```bash
test -n "$PODDOJO_LAB_DIR"
cd "$PODDOJO_LAB_DIR"
grep -v '^#' practice/scores.txt | sort -n | tail -n 1 > practice/highest.txt
```
Each pipe carries text to the next command. The final redirection saves that command's output.
"""

[[lab.steps.checks]]
name = "highest.txt contains exactly one score, 10"
run = '''awk '{ sub(/[ \t]+$/, ""); if ($0 != "10") bad = 1 } END { exit (bad || NR != 1) }' "$PODDOJO_LAB_DIR/practice/highest.txt"'''

[[lab.steps.negatives]]
name = "text sorting incorrectly chooses 8 instead of 10"
run = '''grep -v '^#' "$PODDOJO_LAB_DIR/practice/scores.txt" | sort | tail -n 1 > "$PODDOJO_LAB_DIR/practice/highest.txt"'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/practice/highest.txt"'''

[[lab.steps.negatives]]
name = "appending a second result leaves two score lines"
run = '''printf '10\n10\n' > "$PODDOJO_LAB_DIR/practice/highest.txt"'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/practice/highest.txt"'''

[[lab.steps.alternatives]]
name = "descending numeric sort and head select the same value"
run = '''grep -v '^#' "$PODDOJO_LAB_DIR/practice/scores.txt" | sort -nr | head -n 1 > "$PODDOJO_LAB_DIR/practice/highest.txt"'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/practice/highest.txt"'''

[[lab.steps]]
title = "Write two YAML documents"
task = """
Create **`bundle.yaml`** containing exactly two ConfigMap documents separated by **`---`** on its own line:

1. `apiVersion: v1`, `kind: ConfigMap`, `metadata.name: dojo-greeting`, and `data.message` equal to the string `hello terminal`.
2. `apiVersion: v1`, `kind: ConfigMap`, `metadata.name: dojo-counter`, and `data.count` equal to the string `08`.

Use the same two-space block layout and unquoted keys as step 2, with no extra fields or duplicate keys.
Quote `08` with either single or double quotes so it remains a string. The other string values may be plain or quoted.
Comments, mapping key order, trailing spaces and an optional initial `---` are accepted.
"""
hints = [
  "Each document needs its own apiVersion, kind, metadata and data mappings; the separator resets the document.",
  "metadata and data begin at column 1. Their name, message or count entries begin with two spaces.",
]
solution = """
```bash
test -n "$PODDOJO_LAB_DIR"
cat > "$PODDOJO_LAB_DIR/bundle.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-greeting
data:
  message: hello terminal
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-counter
data:
  count: "08"
EOF
```
The separator creates two independent objects. Quotes preserve the counter's string type and leading zero.
"""

[[lab.steps.checks]]
name = "two complete documents have the requested names, data and string types"
run = '''awk '
  function scalar(s) {
    if (s ~ /^"[^"]*"$/ || s ~ /^\047[^\047]*\047$/) return substr(s, 2, length(s) - 2)
    return s
  }
  BEGIN { doc = 1 }
  index($0, "\t") { bad = 1 }
  {
    sub(/ +#.*$/, ""); sub(/ +$/, "")
    if ($0 ~ /^ *$/ || $0 ~ /^ *#/) next
    if ($0 == "---") {
      if (doc == 1 && !lines && !initial) { initial = 1; next }
      if (doc != 1 || !lines) bad = 1
      doc++; parent = ""; lines = 0; next
    }
    lines++
    if ($0 ~ /^[^ ]/) {
      parent = ""
      if ($0 ~ /^(metadata|data):$/) {
        parent = $0; sub(/:$/, "", parent)
        if (++seen[doc, parent] != 1) bad = 1
        next
      }
      if ($0 ~ /^(apiVersion|kind): +/) {
        key = $0; sub(/:.*/, "", key)
        value = $0; sub(/^[^:]+: +/, "", value)
        if (++seen[doc, key] != 1 || scalar(value) != (key == "kind" ? "ConfigMap" : "v1")) bad = 1
        next
      }
      bad = 1; next
    }
    if ($0 ~ /^  name: +/ && parent == "metadata") {
      value = $0; sub(/^  name: +/, "", value)
      if (++seen[doc, "name"] != 1 || scalar(value) != (doc == 1 ? "dojo-greeting" : "dojo-counter")) bad = 1
      next
    }
    if (parent == "data" && (($0 ~ /^  message: +/ && doc == 1) || ($0 ~ /^  count: +/ && doc == 2))) {
      value = $0; sub(/^  [^:]+: +/, "", value)
      if (++seen[doc, "value"] != 1) bad = 1
      if (doc == 1 && scalar(value) != "hello terminal") bad = 1
      if (doc == 2 && value !~ /^"08"$/ && value !~ /^\04708\047$/) bad = 1
      next
    }
    bad = 1
  }
  END {
    if (doc != 2 || !lines) bad = 1
    for (d = 1; d <= 2; d++)
      if (seen[d, "apiVersion"] != 1 || seen[d, "kind"] != 1 || seen[d, "metadata"] != 1 || seen[d, "name"] != 1 || seen[d, "data"] != 1 || seen[d, "value"] != 1) bad = 1
    exit bad
  }
' "$PODDOJO_LAB_DIR/bundle.yaml"'''

[[lab.steps.negatives]]
name = "both objects are written without a document separator"
run = '''cat > "$PODDOJO_LAB_DIR/bundle.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-greeting
data:
  message: hello terminal
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-counter
data:
  count: "08"
EOF'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/bundle.yaml"'''

[[lab.steps.negatives]]
name = "the second document leaves the numeric-looking string unquoted"
run = '''cat > "$PODDOJO_LAB_DIR/bundle.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-greeting
data:
  message: hello terminal
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-counter
data:
  count: 08
EOF'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/bundle.yaml"'''

[[lab.steps.negatives]]
name = "a third document is appended to two otherwise correct objects"
run = '''cat > "$PODDOJO_LAB_DIR/bundle.yaml" <<'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-greeting
data:
  message: hello terminal
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-counter
data:
  count: "08"
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-extra
data:
  mode: practice
EOF'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/bundle.yaml"'''

[[lab.steps.alternatives]]
name = "an initial separator, reordered keys and equivalent quotes"
run = '''cat > "$PODDOJO_LAB_DIR/bundle.yaml" <<'EOF'
---
# Greeting first; the fields can be reordered.
data:
  message: "hello terminal"  
metadata:
  name: 'dojo-greeting'
kind: 'ConfigMap'
apiVersion: "v1"
--- # the second object
kind: ConfigMap
apiVersion: v1
metadata:
  name: "dojo-counter"
data:
  count: '08'
EOF'''
undo = '''test -n "$PODDOJO_LAB_DIR" && rm -f "$PODDOJO_LAB_DIR/bundle.yaml"'''
+++

## Meet the terminal

A **terminal** is a window that sends your keystrokes to a **shell**, a program that reads commands and starts other programs. A prompt such as `$` means the shell is ready; do not type that prompt. Enter runs a command. The Up arrow recalls previous commands, Tab completes many paths, and Ctrl-C interrupts a foreground command. These examples use portable shell syntax shared by `/bin/sh`, bash and zsh.

Commands have a name, options and arguments: `ls -a practice` asks `ls` to include hidden entries in `practice`. Names and paths are case-sensitive on Linux; practice treating them that way everywhere. Spaces separate arguments, so quote paths containing spaces.

## Find your files

Every shell has a **current directory**. A relative path starts there; an absolute path starts at `/`. `.` means here, `..` means the parent directory, and `~` expands to your home directory when unquoted at the start of a shell word.

Start the lab before trying these commands: PodDojo supplies `$PODDOJO_LAB_DIR`, a disposable workspace. No Docker engine or cluster is needed.

```bash
test -n "$PODDOJO_LAB_DIR" && cd "$PODDOJO_LAB_DIR"
pwd
ls -a
mkdir -p practice
cd practice
pwd
cd ..
```

`pwd` prints the current directory; `ls` lists entries; `cd` changes directory. `mkdir -p` creates missing directories without complaining if they already exist. A file is a named sequence of bytes; a directory holds names pointing to files or other directories.

Read a file with `cat practice/notes.txt`. `cp SOURCE DESTINATION` copies a file, and `mv SOURCE DESTINATION` renames or moves it. Both can overwrite an existing destination. `rm FILE` deletes that exact file without a recycle bin; inspect the path first. Keep practice work inside the lab folder.

## Direct text into files

Programs normally read **standard input** and write **standard output** to the terminal. Errors use a separate stream, **standard error**. The shell connects these streams:

| Operator | Meaning |
|---|---|
| `> file` | Create or overwrite a file with standard output |
| `>> file` | Append standard output to a file |
| `< file` | Read standard input from a file |
| `a \| b` | Feed a's standard output into b's standard input |

```bash
printf '%s\n' 'first line' > practice/notes.txt
printf '%s\n' 'another line' >> practice/notes.txt
cat practice/notes.txt
```

`printf` gives predictable formatting: `%s` prints a string and `\n` ends the line. Redirection happens **before** the program runs. Never read and overwrite the same file in a pipeline: `sort file > file` empties it first. Errors are not captured by `>`; `2> file` redirects standard error separately.

A **here-document** supplies several lines as standard input. This is how later lessons write manifests without opening an editor:

```bash
cat > practice/notes.txt <<'EOF'
Terminal practice
Keep YAML indentation consistent.
$HOME stays literal here.
EOF
```

The quoted opening delimiter prevents expansion inside the body. The closing `EOF` must be alone, unquoted, with no leading or trailing spaces. The shell may show a continuation prompt until you finish it.

## Variables and quotes

A shell variable stores text. Assignment has **no spaces** around `=`. `$NAME` or `${NAME}` reads it. `export` also passes it to programs started by this shell; an ordinary assignment alone does not export a new variable.

```bash
TOPIC='YAML practice'
export TOPIC
printf '%s\n' "$TOPIC"
printf '%s\n' '$TOPIC'
```

Double quotes expand variables while keeping the result as one argument. Single quotes preserve everything literally. Unquoted expansions can split on spaces and expand wildcard characters such as `*`, so habitually write `"$PODDOJO_LAB_DIR"`. Shell quotes are instructions to the shell; they are not normally passed to the program. YAML quotes, inside a file, are a separate language.

## Filter, count and diagnose

`grep PATTERN FILE` keeps matching lines. `^` anchors the beginning, `$` the end, and `-v` selects nonmatching lines. Quote patterns so the shell does not reinterpret them. `head -n 2` selects the first two lines; `tail -n 2` selects the last two. `wc -l` counts newline characters. `sort` orders text; `sort -n` compares numbers, and `-r` reverses the order.

After creating the lab's scores file, try:

```bash
head -n 2 practice/scores.txt
tail -n 2 practice/scores.txt
grep -v '^#' practice/scores.txt | wc -l
grep -v '^#' practice/scores.txt | sort -n | tail -n 1
```

Every command returns an **exit status**: zero means success; nonzero means failure or another documented outcome. `$?` holds the most recent status, so inspect it immediately:

```bash
grep '^ready$' practice/notes.txt
printf 'status: %s\n' "$?"
test -f practice/notes.txt && printf '%s\n' 'file exists'
test -f practice/missing.txt || printf '%s\n' 'file missing'
```

`&&` runs the next command only after success; `||` runs it after failure. In `a && b || c`, failure of either a or b can trigger c. A portable shell pipeline reports the **last** command's status, so a successful `tail` alone does not prove an earlier reader succeeded.

Use `man grep` for the local manual: Space advances, `/pattern` searches, and `q` quits the pager. Many programs offer `--help`, for example `kubectl --help` later. Some macOS tools do not support that option; use their man pages. Read error messages before retrying.

## YAML is a tree made visible

YAML describes data, not commands. A **mapping** associates keys with values; a **list** contains ordered items introduced by `- `. Spaces after colons separate keys from values. Indentation creates parent-child relationships. Use spaces only, consistently; this course uses two per level. Tabs are forbidden for indentation.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dojo-web
  labels:
    app: web
spec:
  containers:
    - name: web
      image: nginx:1.27-alpine
      ports:
        - containerPort: 80
```

`metadata` contains `name` and `labels`; `spec` contains `containers`, a list of mappings. `image` belongs to the same list item as `name`. Equal indentation means siblings. Moving a line can produce valid YAML with the **wrong meaning**.

A Kubernetes **manifest** describes a desired object: `apiVersion` selects its API, `kind` its type, `metadata` its identity, and, for a Pod, `spec` its desired configuration. Other kinds have different fields: a ConfigMap uses `data` for string entries. YAML syntax alone cannot tell you whether Kubernetes accepts an object.

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: dojo-greeting
data:
  message: hello terminal
```

Later, with kubectl installed, the imperative generator for that object is:

```bash
kubectl create configmap dojo-greeting --from-literal=message='hello terminal' --dry-run=client -o yaml
```

This prints a manifest without creating the object. Generated output can include extra metadata. `kubectl explain pod.spec.containers` later documents fields; `kubectl apply -f FILE` submits manifests to the selected cluster. This introductory lab only edits files.

## Strings and multiple documents

Plain scalars can be interpreted as numbers, booleans or null. Quote string identifiers such as `"08"`, and quote `"yes"`, `"no"`, `"on"` and `"off"`: YAML 1.1-style parsers can treat those words as booleans. Do not rely on every parser using YAML 1.2 rules. Quote text containing `: ` or a space followed by `#`, which otherwise introduces a comment. Duplicate mapping keys are errors to avoid, even when a tool silently keeps one value.

```yaml
data:
  enabled: "yes"
  code: "08"
  note: 'status: ready # literal text'
  script: |
    echo first
    echo second
  description: >
    These two lines become
    one paragraph.
```

`|` preserves line breaks; `>` folds ordinary adjacent lines into spaces. Both normally retain a final newline; `|-` and `>-` remove it. Indent every content line beneath its key. Single-quoted strings preserve backslashes; double-quoted YAML strings interpret escapes such as `\n`.

`---` at column 1 starts another YAML document. Each document is independent and needs its own object fields. A file can therefore hold a ConfigMap followed by another ConfigMap or a Pod; repeating top-level keys without a separator does not create a second object.

## Vim survival, then nano

Open `vim broken.yaml` from the lab folder. Vim starts in **Normal mode**, where keys perform actions. Press `i` for **Insert mode** and type text. Press Esc to return to Normal mode. In Normal mode, `:` opens the command line; finish its command with Enter.

| Keys from Normal mode | Effect |
|---|---|
| `:w`, `:q` | Save; quit if no unsaved changes |
| `:wq`, `:q!` | Save and quit; quit discarding unsaved changes |
| `dd`, `yy`, `p` | Delete a line; copy a line; paste after the cursor (below for whole lines) |
| `u` | Undo the last change |
| `/name` then Enter, `n` | Search forward for name; repeat the search |
| `:set number` | Display line numbers without adding them to the file |

Arrow keys move the cursor. If characters appear unexpectedly, press Esc before giving another command. To configure spaces for YAML, a minimal **`~/.vimrc`** contains:

```text
set expandtab
set tabstop=2
set shiftwidth=2
```

`expandtab` inserts spaces when you press Tab; `tabstop` controls displayed tab width; `shiftwidth` controls indentation operations. These settings **do not remove existing tabs**. This lab does not change your home files: enter the same settings temporarily with `:set expandtab tabstop=2 shiftwidth=2`. `:set list` exposes existing tabs as `^I`; replace them with spaces.

Before a terminal paste, Esc then `:set paste`, Enter, and `i` prevent auto-indentation from shifting pasted YAML. After pasting, Esc then `:set nopaste` restores normal editing. Inspect indentation before saving.

If installed, `nano broken.yaml` is a fallback: type directly, Ctrl-O then Enter writes the file, and Ctrl-X exits. Nano displays `^` for Ctrl in its shortcut bar. Practise saving and leaving both editors before relying on them under time pressure.

## Exam tips

Keep commands and YAML in the requested working directory, and verify filenames before overwriting. Generate a manifest with `--dry-run=client -o yaml`, edit the smallest necessary section, and check the object name and indentation. Use `kubectl explain` for field structure and `--help` for flags. Save the editor buffer before running a command against its file. A successful shell exit, valid YAML and a healthy Kubernetes workload are three different checks; later lessons teach all three.
