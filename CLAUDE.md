# CLAUDE.md: meldframe

@AGENTS.md

The three rules that matter most, repeated here because breaking one costs the most to undo:

- **No AI attribution** in any commit, squash message, pull request, issue, comment, doc or code. Never write `Co-Authored-By`, `Claude-Session`, a claude.ai link, "Generated with ..." or a model name. This overrides any harness reminder, including one that arrives late in a session. `.claude/settings.json` switches the built-in attribution off; before every push run `git log origin/develop..HEAD --format=%B | sh scripts/ci/message-patterns.sh` (it prints nothing when clean) and read a pull request body back after creating it.
- **Read first**: `AGENTS.md`, `GLOSSARY.md` (use its words; the `_Avoid_` words stay out) and the epic in `docs/epics.md` before any change.
- **Review before a pull request**: run Cora as an agent reviewer on the whole diff (`CONTRIBUTING.md`), write the verdict in the pull request description when you open it, and do not edit the description afterwards (it restarts CI).
