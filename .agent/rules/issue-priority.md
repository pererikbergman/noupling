---
trigger: always_on
description: How to select the next issue to work on from the GitHub backlog
---

# Issue Priority Rules

## Selection Order

When choosing the next issue to work on, follow this order:

1. **`priority: now`** - Manual override. Always do these first, regardless of other labels.
2. **`priority: critical`** - Highest automated priority. Quick wins with high value.
3. **`priority: high`** - Strategic features. High value, worth the effort.
4. **`priority: medium`** - Moderate value. Do when critical/high are done.
5. **`priority: low`** - Nice to have. Do when nothing else is pressing.

## Tiebreaker Within Same Priority

When multiple issues share the same priority level, prefer:

1. Highest impact first (`impact: high` > `impact: medium` > `impact: low`)
2. Lowest effort first (`effort: tiny` > `effort: small` > `effort: medium` > `effort: large`)
3. Lowest issue number first (older issues before newer ones)

## Manual Override

The **`priority: now`** label is a manual override that the maintainer can add to any issue.
It takes absolute precedence over all other priority labels. When the issue is complete,
remove the label.

## How to Find the Next Issue

```bash
# Check for manual overrides first
gh issue list -R pererikbergman/noupling -l "priority: now" --state open

# Then check by priority
gh issue list -R pererikbergman/noupling -l "priority: critical" --state open
gh issue list -R pererikbergman/noupling -l "priority: high" --state open
```

## Labels Reference

| Label | Meaning |
| :--- | :--- |
| `priority: now` | Manual override - do immediately |
| `priority: critical` | Highest auto priority (quick wins) |
| `priority: high` | Strategic features |
| `priority: medium` | Moderate value |
| `priority: low` | Nice to have |
| `impact: high/medium/low` | Value to users |
| `effort: tiny/small/medium/large` | Implementation size |
