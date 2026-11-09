# TQL and the update language

`tatrs ls` takes a query written in the Tatr Query Language (TQL). It is designed to be typed
directly into a shell: no quoting beyond normal shell quoting, `<` and `>` are spelled `lt`/`gt`, and
grouping uses square brackets instead of parentheses.

```console
$ tatrs ls :bug and priority lt 50
$ tatrs ls [:ui or :cli] and not :wip
$ tatrs ls -a -u close,-wip :scope
```

## Grammar

```
expr      ::= or
or        ::= and *('or' and)
and       ::= compare *('and' compare)
compare   ::= primary *(cmp primary)
cmp       ::= 'lt' | 'le' | 'gt' | 'ge' | 'eq' | 'ne'
primary   ::= ':' tag
            | '[' expr ']'
            | 'not' primary
            | 'any'
            | 'tagged'
            | 'priority'
            | number
            | huid
tag       ::= 1*<any character except whitespace and square brackets>
number    ::= ['-'] 1*digit
huid      ::= 8digit '-' 6digit ['-' suffix]
```

Tokens are separated by whitespace; `[` and `]` are always separate tokens. That is why tags cannot
contain square brackets, and why `:` is enough to introduce a tag.

## Semantics

| Expression | Value | Meaning |
|-|-|-|
| `:tag` | boolean | the task's `TAGS` contains exactly `tag` |
| `tagged` | boolean | the task has at least one tag |
| `any` | boolean | always true |
| `priority` | integer | the task's `PRIORITY` |
| `42`, `-7` | integer | a literal |
| `<huid>` | boolean | the task's id equals `<huid>` |
| `[expr]` | same as `expr` | grouping |
| `not p` | boolean | negation of a primary expression |
| `a and b`, `a or b` | boolean | short-circuit is not observable, but precedence is |
| `a lt b` and friends | boolean | integer comparison; both sides must be integers |

Precedence, tightest first: `not`, comparisons, `and`, `or`; within a level, left to right.

### `not` binds to a primary, not to a comparison

This follows the grammar and matches upstream:

```console
$ tatrs ls not priority lt 50        # error: `not` gets an integer
$ tatrs ls not [priority lt 50]      # correct
$ tatrs ls not :bug and :ui          # (not :bug) and :ui
```

### Types

Only two types exist: boolean and integer. Predicates produce booleans, `priority` and literals
produce integers, and comparisons accept integers only. Comparing booleans is a compile error, so
`tagged eq :bug` and `:a eq :b` are rejected. Type errors are reported at compile time with a caret
under the offending operator:

```console
$ tatrs ls :bug lt 5
:bug lt 5
     ^
comparisons expect integer operands, got boolean
```

A query must evaluate to a single boolean; `tatrs ls priority` fails with "compare it, for example
`priority lt 50`". An empty query is `any`.

### Interaction with the status filter

`ls` filters by status before the query, exactly like upstream:

- default: open tasks only;
- `-c/--closed`: closed tasks only;
- `-a/--all`: both.

So a query cannot see closed tasks unless you ask for them. Reopening work therefore needs the flag
included:

```console
$ tatrs ls -a -u open :scope     # reopen every scope task
```

## Update language

`-u/--update` applies a list of actions to every matched task:

```
update ::= action *( ("," | whitespace) action )
action ::= 'open' | 'close' | '+' tag | '-' tag
```

- Actions apply in order; `+tag` is idempotent and `-tag` removes every occurrence.
- The query is mandatory: updating everything requires the explicit query `any`, so a typo can never
  turn into a mass update.
- The whole batch holds `tasks/.tatrs.lock`; each file is written atomically, and only tasks that
  actually changed are rewritten. Unknown properties and the body survive untouched.
- A task whose header could not be parsed is never rewritten: the batch aborts with exit `3` and the
  `tatrs check` diagnostics, because rendering a half-parsed task would replace its body.
- `--dry-run` computes the same result and prints it without writing. It needs no lock and is
  allowed under `--read-only`.
- `--read-only` rejects a real update with exit code 2.

Human output annotates changed tasks and ends with a summary:

```console
$ tatrs ls -u close,-wip :bug
tasks/20261109-133012/TASK.md:1: <UPDATED: close,-wip> CLOSED [PRIORITY:  10] [bug] Fix parser
1 task(s) updated (1 matched)
```

JSON adds three fields to the usual list payload:

```json
{"schema": 1, "updated": 1, "dry_run": false, "update": "close,-wip", "tasks": [...]}
```

`tasks` reflects the post-update state (or the would-be state under `--dry-run`).

## Errors and exit codes

| Situation | Exit | Output |
|-|-|-|
| invalid query or update syntax | `2` | source, caret, message on stderr |
| `--update` without a query | `2` | usage message |
| `--update` under `--read-only` without `--dry-run` | `2` | usage message |
| lock held for more than 5 s | `4` | lock message |
| no matches with `--fail-if-empty` | `5` | message |
| anything else | `0` | results |

## Differences from upstream

- `.tag` (the deprecated tag prefix) is rejected; use `:tag`.
- Diagnostics are produced at compile time, so a type error is reported once rather than per task.
- Boolean comparisons are rejected (upstream has the same limitation; it is documented here).
- Query and update syntax are the same; the update language also accepts whitespace separators,
  matching upstream's tokenizer.
