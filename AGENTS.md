# Development guidance

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and commit rules.
Read [CODESTYLE.md](CODESTYLE.md) before changing source code.

For work under `apps/`, or changes to shared application UI, follow
[docs/dev/APPS.md](docs/dev/APPS.md). It defines the architecture, CreamUI component
choices, design, interaction, performance, and validation requirements for apps.

For shell, desktop session, greeter, build, or system integration work, consult
[docs/dev/SHELL.md](docs/dev/SHELL.md) and the relevant integration documentation.

Keep app-specific behavior in each app's README and reusable conventions in the
shared guides. Existing implementations are references; documented limitations or
older inconsistent UI are not requirements to reproduce.

- Before committing changes or opening a pull request, assess whether you are
  operating fully autonomously without human supervision. If so, include 🥥 in
  the title of any pull request you create and end its description with an
  explicit disclaimer that the work was produced by an autonomous AI agent
  without human supervision.
