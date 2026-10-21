# MM-022: Global memory exclusions

status: Resolved

The user requests exclusions for processes, actual process trees and paths,
applied globally, including Other processes and Other used RAM. Excluded
allocations may optionally be displayed in their own category.

- AC1: Session-only, case-insensitive substring rules for process names,
  executable paths and tree root names compose by union. Tree rules include
  descendants via lifetime-aware parent relationships. Remove individual rules
  or clear all; rejected commands are atomic. Allow up to 32 rules of 1–2048
  characters each.
- AC2: Exclusions remove matching table leaves and empty groups in every
  grouping. Surviving children remain reachable. Search, grouping, selection,
  pause, raw collection and history remain intact; copied snapshots honor
  exclusions. Retained group/inspector summaries retain full entity totals,
  consistent with the existing search filter.
- AC3: The overview removes matching allocations from their original bands
  without moving them into Other processes. Other processes and Other used RAM
  are independently excludable. A toggle displays all excluded allocations as
  one Excluded band, or hides them. Header totals remain actual system values.
  Both metrics and historical comparison baselines use the same partition.
  When Other processes is excluded, responsive overflow of still-included named
  bands uses More programs, preserving its bytes separately from Excluded.
  Missing retained detail must show an explicit attribution gap, never fabricate
  historical values or infer process exits.
- AC4: A global header control opens accessible filter management, including
  from expanded overview. Rules and category toggles are visible and reversible;
  keyboard submission, dismissal and small-window scrolling remain usable.

All automated UI checks are headless. Validate behavior before implementation;
inspect cosmetic layout without adding CSS-mirroring tests. Review the exact
candidate before publication; no merge, privilege changes or worktree cleanup.
