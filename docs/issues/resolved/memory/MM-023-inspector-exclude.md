# MM-023: Exclude from process details

status: Resolved

The user requests a button in the selected-process details to filter that item
out directly, using the global exclusion feature.

- AC1: An accessible funnel button in details adds the current program-name
  rule, tree-root-name rule in Process tree grouping, or known single executable
  path rule in Path grouping. Its tooltip explains the existing case-insensitive
  contains semantics. Zero known paths or multiple known paths fall back to the
  program-name rule. A single known path stays usable with partial metadata.
- AC2: No selection, an empty value or an already-applied identical rule disables
  the action. Selection updates change the target; the selected identity remains
  inspectable after exclusion and the rule is removable in Global exclusions.
- AC3: Use existing collection, session-only filtering and Excluded-band behavior;
  do not alter backend accounting, process identities or persistence. Verify the
  button's actual headless journey and relevant rendered layout.
- AC4: Remove the visible Group by caption while preserving the fieldset's
  accessible name. Add subtle separators between toolbar, metric, axis, range,
  navigation and zoom control groups. Explain Family's launcher boundary in its
  tooltip. These cosmetic changes need rendered/syntax checks, not CSS tests.
