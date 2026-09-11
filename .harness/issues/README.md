# Issues

Bug reports filed from inside the harness with **Report an issue**. One
directory per issue, kept in the project so an agent can read them, and so a
fix and the report that asked for it land in the same commit.

```
0007-short-title/
  issue.json         the record: title, description, status, history
  screenshot.png     the annotated capture, if there was one
  attachments/       whatever was dropped onto the form
  fix-technical.md   how the fix was made, for whoever maintains this
  fix-plain.md       the same fix with no technical language, for whoever asked
```

`status` is one of `open`, `fix-applied` or `resolved`. An agent moves an issue
to `fix-applied` once it has changed the code and written both documents. Only a
person moves it to `resolved`, after checking the program actually behaves; a
resolved issue can be reopened.

Run the `issues` skill in Claude Code to work through the open ones.
