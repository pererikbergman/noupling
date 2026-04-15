# CI/CD Integration

## GitHub Actions

### Basic setup

```yaml
- name: Install noupling
  run: |
    curl -sL "https://github.com/pererikbergman/noupling/releases/latest/download/noupling-linux-x86_64" -o noupling
    chmod +x noupling
    sudo mv noupling /usr/local/bin/

- name: Scan
  run: noupling scan .

- name: Audit
  run: noupling audit . --fail-below 80
```

### Diff mode (PR gating)

Only flag violations introduced in the current PR:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0  # Need full history for diff

- name: Scan (diff mode)
  run: noupling scan . --diff-base origin/main

- name: Audit
  run: noupling audit . --fail-below 80
```

### PR comment bot

Copy `.github/workflows/noupling-pr.yml` from the noupling repository to your project. It automatically comments on PRs with:

- Health score
- Total and new violations
- Violation details

The workflow installs noupling from the latest release, runs a diff scan, and posts/updates a comment.

### Generate SonarCloud report

```yaml
- name: Scan
  run: noupling scan .

- name: Generate Sonar report
  run: noupling report . --format sonar
```

Then add to `sonar-project.properties`:

```properties
sonar.externalIssuesReportPaths=.noupling/noupling-sonar.json
```

## Baseline workflow

For projects with existing violations, use baselines to adopt noupling incrementally:

```bash
# First time: save current violations as accepted
noupling scan .
noupling baseline save .

# In CI: only fail on NEW violations
noupling scan . --diff-base origin/main
noupling audit . --baseline
```

The baseline is stored in `.noupling/baseline.json`. Commit this file to your repository.

## Git pre-commit hook

```bash
noupling hook install .
```

This installs a pre-commit hook that scans changed files and fails the commit if new violations are introduced. Bypass with `git commit --no-verify`.

```bash
noupling hook uninstall .
```
