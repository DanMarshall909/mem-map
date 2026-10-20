# Implementation specification

Use the dark-grey/orange/green mockup as the target. The central change is the **stacked area chart showing overall system memory usage**, supported by a compact process table and an inspector for the selected group or process.

The instructions below define the appearance and behaviour of each element. Treat dimensions as starting values in device-independent pixels; populate all numbers, bar lengths and chart geometry from application data.

## 1. Window layout and visual theme

### Main layout

Keep three primary regions:

- **Application header:** system memory summary and global actions.
- **Main workspace:** process table on the left, overview chart and selected-process inspector on the right.
- **Status footer:** collection coverage and recording state.

At the reference window size, give the table approximately **68% of the workspace width** and the right pane **32%**. Add a draggable vertical splitter and remember its position.

Use approximately 16 px outer padding, 8–12 px between panels, and 12–16 px inside the right-hand panels. Use flat tonal separation between panels; retain clear keyboard-focus, selection and input affordances.

The right pane should contain two vertically stacked panels:

1. **Overall memory usage**, occupying roughly 45% of the available height.
2. **Selected-process inspector**, occupying the remainder.

When the window becomes smaller, allow toolbar wrapping, table scrolling and inspector scrolling. Preserve readable text and exact numeric values. Reduce decorative bar space before clipping numbers.

### Theme tokens

Define shared resources instead of assigning colours separately to individual controls.

| Token | Suggested value | Use |
|---|---|---|
| Window background | `#202327` | Application canvas |
| Panel background | `#282D32` | Table and inspector surfaces |
| Input background | `#202327` | Search, dropdowns and path field |
| Subtle divider | `#42494F` | Essential separators and resize affordances |
| Control boundary | `#727C84` | Inputs, checkboxes and other boundaries needed to identify controls |
| Primary text | `#ECEFF1` | Names, values and important labels |
| Secondary text | `#B8C0C6` | Supporting information and axis labels |
| Orange accent | `#F2A14A` | Selected options and selection edge |
| Green accent | `#6CBC8B` | Subtle row memory fill and selected-process history |
| Increasing change | `#FFA0A0` | Positive percentage text at full intensity |
| Decreasing change | `#87DCAA` | Negative percentage text at full intensity |
| Unchanged | `#FFFFFF` | Exact zero change |
| Selected row background | `#3D352C` | Restrained orange tint |
| Hover background | `#343A40` | Pointer hover |

Use dark text on bright orange or green filled controls. Keep secondary buttons dark and flat, with clear hover and keyboard-focus states.

As a readability target, use at least 4.5:1 contrast for normal text and 3:1 for essential control indicators and graphical boundaries. These are useful benchmarks for this desktop interface. The proposed primary and secondary text colours provide approximately 12.0:1 and 7.5:1 against the panel background, respectively. [W3C](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html?utm_source=chatgpt.com)

### Typography and density

Use the platform UI font, such as Segoe UI, with:

- 14 px table text and control labels.
- 12–13 px supporting text.
- 18–20 px panel titles.
- 26–28 px application title.
- Tabular digits for numeric columns.

Start with 32–34 px compact rows and 46–50 px expanded rows. Keep controls approximately 32–36 px high. Use modest corner rounding, around 3–4 px.

---

## 2. Application header

### Application title

Keep **Mem Map** at the upper left. Remove “Navigation and zoom” from the application’s visible title if it is development text.

Preserve normal window movement, resizing and native window controls.

### System memory summary

Replace the abbreviated RAM text with:

**System RAM used 24.20 / 31.70 GiB (76%)**

Bind the values to one displayed snapshot:

```text
Used physical memory = Total physical memory − Available physical memory
Usage percentage = Used physical memory / Total physical memory × 100
```

Use the same calculation for the displayed percentage and usage bar so they remain consistent.

### Usage bar

Place a compact horizontal bar immediately after the summary.

- The full track represents physical capacity.
- Green fill represents used physical memory.
- The scale always runs from zero to capacity.
- Expose the numeric summary to accessibility APIs.
- Keep the textual values visible alongside it.

### Available memory

Change **Free** to **Available** when using Windows’ available-memory counter. That counter includes immediately reusable standby, free and zeroed pages, so “Available” describes it more accurately. [Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-memorystatusex?utm_source=chatgpt.com)

Display:

**Available 7.50 GiB**

### Live status

Place this before the header actions:

**Live · Updating every 2 seconds**

Use the actual configured interval. Show a clear alternate state when the collector stops or becomes unavailable; “Live” should reflect successful collection.

### Pause display

Rename the existing **Pause** button to **Pause display** and use a flat button with orange text and a clear keyboard-focus state.

Define the behaviour precisely:

- Freeze the displayed memory readings, table values, automatic ordering and chart endpoints.
- Continue collecting history.
- Keep selection, filtering, path copying and historical inspection usable.
- Change the button label to **Resume display**.
- Show **Display paused at [time] · Recording continues**.

On resume, advance the current readings and overview to the latest completed snapshot. Preserve selection, expansion and filters. The selected chart should advance only if its **Follow latest** option is enabled.

### Export CSV

Keep **Export CSV** at the far right as a green action button.

Preserve the existing CSV contract where possible. Export from one coherent snapshot, and describe the export scope in the button’s tooltip.

For a currently undefined scope, use the current filtered data, including records outside the viewport. Expansion and scrolling must not accidentally change which records are exported or duplicate aggregate values.

While the display is paused, export the displayed snapshot. Include its timestamp and retain raw numeric memory values rather than exporting only formatted strings.

---

## 3. Grouping and filtering toolbar

Place these controls above the table, keeping their scope visually attached to it.

### Group by

Replace the existing Name / Path / Family / PID button collection with a **single-selection segmented control**:

**Name · Path · Process tree · Individual processes**

Default to **Name**.

Use radio-group behaviour: exactly one option is selected, arrow keys move between options, and the selected option has orange fill.

| Option | Required grouping |
|---|---|
| Name | Group process instances by executable name |
| Path | Group instances by complete executable path |
| Process tree | Show parent/descendant process relationships |
| Individual processes | Show a flat list of process instances |

Check the existing meaning of **Family** before renaming it. The new **Process tree** label requires actual process ancestry. If Family currently means something else, retain that capability under an accurate label until the intended grouping is implemented.

### Filter field

Add the visible label **Filter:** and the placeholder:

**Name, path or PID**

Use a search icon at the left and a clear button at the right when populated.

Required behaviour:

- Match full underlying names and paths, including portions truncated onscreen.
- Match PIDs as well as text.
- Apply changes after a short debounce, approximately 150–250 ms.
- Preserve the query when grouping changes.
- Keep ancestors visible when a child matches.
- Show **No matching processes** when the result is empty.

Treat filtering as a visibility operation. Keep a group’s memory totals based on its full membership. If only some children match, show a qualifier such as **1 matching of 18 processes**.

The filter must not change the system totals or the overall-memory chart.

### Show exited processes

Replace **Inactive** with a real checkbox labelled:

**Show exited processes**

Default to unchecked.

Verify that the old control actually refers to terminated processes. Idle, inaccessible and exited are different states.

When enabled, retain exited instances for historical inspection. Give them an **Exited** label and an exit or last-observed time. Keep historical readings distinguishable from current allocation.

### Compact rows

Add a checkbox labelled **Compact rows**, checked by default.

- **Checked:** show one line per row.
- **Unchecked:** show a second line containing the executable path or a multiple-path summary.

Changing density must preserve selection, expansion and scroll position.

### Change over

Move **Growth over** into this toolbar and rename it:

**Change over: 5 minutes**

Use a dropdown. A practical initial set is 1, 5, 15 and 30 minutes, subject to retained history.

This setting controls the table’s change column and the inspector’s change value. Update both labels when it changes.

Keep it independent of the chart’s visible time range.

---

## 4. Process table

### Table structure and columns

Use a virtualized tree grid with a fixed header.

| Column | Change required |
|---|---|
| **Executable / processes** | Show the executable name, hierarchy chevron and labelled process count |
| **PID** | Show a real PID for individual instances; use an em dash for aggregates |
| **Private working set** | Show current private resident memory as a full-row proportional background and exact value |
| **Peak (last 30m)** | Show the observed peak of the same metric over the requested window |
| **Private commit** | Show private committed memory as a separate value |
| **Change % (5m)** | Show signed percentage over the selected comparison interval; retain exact byte change in tooltip/accessibility text |

The first column should absorb available width. Give numeric columns stable widths and right-align all numbers.

Reasonable initial widths are approximately 300 px for identity, 55 px for PID, 190 px for private working set, 125 px each for peak and commit, and 165 px for change. Allow resizing.

### Verify the memory labels

Before changing **Private RAM** to **Private working set**, verify the collector’s actual counter.

Microsoft distinguishes current private working set from private commit: `PrivateWorkingSetSize` represents the former, while `PrivateUsage` represents private commit. Bind each new label to the matching measurement and retain an appropriate collection method for the application’s supported Windows versions. [Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2?utm_source=chatgpt.com)

### Executable names, counts and hierarchy

Move counts beside names:

**ChatGPT.exe 18 processes**

The PID column should contain **—** for that group. Expanding it reveals actual process instances and their PIDs.

For Name grouping, use path children when an executable name represents several different paths. For Path grouping, place individual instances beneath their full-path group.

In Process tree mode, define whether a row represents its own process or a subtree. I recommend showing each process’s own measurements; any subtree aggregate should have an explicit label.

Only show a disclosure chevron when children exist. Clicking the row selects it; clicking its chevron expands or collapses it.

### Selection and hover

Use:

- A restrained orange row tint.
- A 3 px orange edge at the left.
- A separate visible keyboard-focus outline.

Keep ordinary row text off-white. Avoid making every value orange when selected.

### Private working-set row backgrounds

Paint a subtle proportional background across the entire row behind its cells.

The numeric value must remain aligned at the same right edge regardless of fill length. Keep focus, selection and exact values readable over the fill.

For Name and Path grouping:

```text
Bar proportion =
    Row private working set
    / Largest matching root-group private working set
```

Use one shared denominator for all comparable rows, including expanded children. Calculate it across the filtered model rather than only the visible viewport.

For individual-process and process-tree modes, use the largest matching process value and update the explanatory label accordingly.

Keep the scale frozen while the display is paused.

### Percentage changes

Show signed percentage change relative to the prior reading:

```text
Baseline = current private working set - coherent byte change
Percentage = coherent byte change / baseline x 100
```

- Increases use red text; decreases use green text.
- Greater absolute percentage makes the color stronger, saturating at 100%.
- Exact zero is white and displays **0.0%**.
- Tiny nonzero changes preserve direction as **+<0.1%** or **−<0.1%**.
- Growth from a zero baseline displays **New**, without an invented finite percentage.
- Unknown or partial endpoints remain unavailable with an explanation.
- A valid confirmed exit uses zero current memory and can show **−100.0%**.

Use exact integer bytes for the calculation. Preserve raw byte values and reasons
in tooltips, keyboard-accessible descriptions and CSV. Color indicates direction
and magnitude; it does not establish a fault. Keep signed text meaningful without
color and readable over full-row backgrounds.

Retain a help strip explaining relative row fill and red increases, green decreases,
white unchanged. The change header must disclose that sorting continues to use
the backend's raw byte delta, rather than percentage magnitude.

### Sorting and live stability

Remove the separate **RAM ↓** toolbar button. Put sorting on column headers, defaulting to **Private working set descending**.

Sort using raw values, keep unavailable readings after valid readings, and use a deterministic tie-breaker.

Preserve selection and expansion by stable identity across updates. Refresh values in place rather than rebuilding the table.

Prevent moving click targets by holding automatic row reordering while the user is interacting with the table. An explicit header click should still sort immediately. Use a small status message such as **Order held while inspecting** when necessary.

### Missing and exited data

Use an em dash for an unavailable measurement, with an explanation available on hover and keyboard focus.

If a group has valid readings for only some members, display its known total with a **Partial** indicator and expose coverage, for example:

**16 of 18 processes measured**

Exited-process history must not continue contributing its last observed value to current totals.

---

## 5. Peak and change calculations

These calculations need to agree between the table and inspector.

### Peak

Compute the peak from the group’s total at each recorded sample:

```text
Group value at time t = Sum of that group’s member values at time t

Group peak =
    Maximum recorded group value within the last 30 minutes
```

Independent process peaks may occur at different times, so adding those peaks does not produce the group’s observed peak.

Keep the 30-minute peak window independent of the selected chart’s zoom. When less history exists, disclose the recorded duration in the inspector or peak tooltip.

Windows’ `PeakWorkingSetSize` is a separate working-set counter; it should not be substituted for the requested rolling private-working-set group peak. [Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2?utm_source=chatgpt.com)

### Change

Calculate:

```text
Change over D = Group value now − Group value at now minus D
```

Use a documented baseline rule. For example, choose the last valid sample at or immediately before the target time, allowing a maximum age of two collection intervals.

If there is insufficient history or incomplete endpoint coverage, show **—** and explain why. A five-minute comparison after three minutes of recording should say **Need five minutes of history**.

Include process starts and exits in group changes. The value describes the group’s net footprint change.

---

## 6. Overall memory usage panel

This replaces the complete **Top 5 · RAM** panel, including its Pie, Top 5 and Refresh controls.

### Heading and summary

Use:

**Overall memory usage**

Immediately beneath it, show:

**24.20 GiB used · 7.50 GiB available**

These values must agree with the header’s displayed snapshot.

Place **View last: 30m** at the upper right. Use the same range choices as the selected-process history: 5m, 15m, 30m and All retained history.

### Chart type and axes

Implement a **true cumulative stacked area chart in absolute GiB**.

Required geometry:

- X-axis: time.
- Y-axis: physical memory, starting at zero.
- Upper scale reference: OS-reported physical capacity.
- Every layer fills between its lower and upper cumulative boundaries.
- The top of the complete stack equals system used memory.
- The space above the stack represents available memory.

Use linear or step interpolation appropriate to the sampling model. Avoid curve smoothing that creates values beyond recorded measurements.

### Layers and colours

Use the following order from bottom to top:

| Layer | Colour direction | Meaning |
|---|---|---|
| **Other used RAM** | Medium neutral grey | System usage remaining after measured private working sets are attributed |
| **Other processes** | Dark green | Measured private working sets without their own named band |
| **Named process group 1** | Light green | First individually displayed executable group |
| **Named process group 2** | Orange | Second individually displayed executable group |
| **Named process group 3** | Amber/orange | Third individually displayed executable group |

ChatGPT.exe, node.exe and codex.exe are examples from the image. Populate the labels from the data.

For the initial implementation, select three prominent executable groups after the first collection cycle and keep those identities, colours and stack positions fixed for the recording session. Include every other measured process through **Other processes**.

This preserves the overall total while keeping the legend readable. Table sorting, filtering and selection should not silently replace chart bands.

### Memory accounting

Use physical-memory measurements throughout the stack.

Ordinary process working sets can contain shared pages, which makes their simple sum unsuitable for exclusive physical-memory attribution. Private commit also has a different meaning from resident physical memory. Use measured private working sets for the process-attribution bands. [Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/memory/working-set?utm_source=chatgpt.com)

For each coherent sample:

```text
Used RAM = Physical capacity − Available RAM

Measured process RAM =
    Sum of valid private working sets for unique live process instances

Other processes =
    Measured process RAM − Sum of the individually named bands

Other used RAM =
    Used RAM − Measured process RAM
```

Therefore:

```text
Other used RAM
+ Other processes
+ Named process bands
= Used RAM
```

Describe **Other used RAM** as residual or unattributed system usage. It can include system/shared allocations and usage belonging to processes whose readings are unavailable.

Each process instance must contribute once. Table aggregate rows and their children must not both enter the calculation.

### Capacity and available headroom

Draw a dashed horizontal capacity line, labelled:

**Capacity: 31.70 GiB**

At the latest displayed timestamp, show:

- A marker and label at the stack endpoint: **24.20 GiB**.
- A bracket between that endpoint and capacity: **Available 7.50 GiB**.

Position both annotations from the actual values. For roomy charts, reserve enough space at the right of the plot so labels remain inside the panel. For compact charts, reveal annotations through hover, keyboard focus or an info-button tap rather than reserving a label gutter.

### Legend and interaction

For roomy charts, place the legend below the plot in the same order as the layers. Let it wrap without truncating names.

Determine compactness from each chart's own width relative to UI text scale. At compact widths, hide the legend, readout, descriptions and SVG annotations from normal layout until hover, keyboard focus or an info-button tap. Reveal labels in a viewport-clamped popover beside the plot when possible, without changing plot/data coordinates or blocking plot input. Escape, outside pointer input and keyboard focus leaving the chart dismiss the labels. Preserve keyboard sample navigation and an accessible chart description.

Hovering or keyboard-focusing a band should reveal:

- Timestamp.
- Group or category name.
- Its own memory value.
- Total system usage.
- Available memory.
- Relevant measurement-coverage information.

The band value is its thickness, not its cumulative upper boundary.

Legend interaction may highlight a band, but avoid removing it from the stack: the complete area should continue representing overall used RAM.

### Recorded history and gaps

Keep the caption:

**All processes + other used RAM · 5m 06s recorded**

A requested 30-minute window may initially contain only five minutes of measurements. Label the actual time span accurately.

Handle unavailable data explicitly:

- A failed system-memory sample produces a gap.
- A failed process reading remains unknown.
- A confirmed process exit removes its subsequent current contribution.
- If a named group loses valid coverage, show an attribution gap or neutral total-usage region instead of an apparent release to zero.
- If measured process RAM exceeds system used RAM, retain the valid system total and mark the decomposition unavailable for that sample.

Do not force inconsistent samples to reconcile by silently rescaling the bands.

---

## 7. Selected-process identity and path

### Title and counts

Use the selected executable or process name as the inspector title:

**ChatGPT.exe**

Beneath it:

**18 processes · 1 executable path**

Pluralize correctly and derive the counts from the selected entity.

For an individual process, include its PID and lifecycle state.

### Show processes

Replace **Expand ↗** with **Show processes**.

The action should expand the selected group in the table and bring its members into view. Hide it for a leaf process.

### Path field

Use a selectable, read-only field containing the executable path.

Apply middle truncation when needed so the beginning and executable filename remain visible:

`C:\Program Files\WindowsApps\…\ChatGPT.exe`

Keep the complete underlying path available through keyboard-accessible help and copying.

If a group contains several paths, show **3 executable paths** and provide access through the existing path hierarchy or a small path list. Avoid presenting one arbitrary path as the identity of the entire group.

### Copy path

Add **Copy path** beside the field.

Copy the full underlying path, including any text hidden by ellipsis, and show a brief **Path copied** confirmation.

When several paths exist, require a specific path selection or make the action explicitly **Copy all paths**.

---

## 8. Inspector summary values

Use a compact label/value list with a fixed right-aligned value column:

| Label | Example |
|---|---:|
| Private working set | 2.47 GiB |
| Private commit | 3.49 GiB |
| Change over 5m | −6.7% |
| Peak (last 30m) | 2.74 GiB |

Bind these to the same selected entity, snapshot and calculations used by the table.

Use the same signed percentage, zero-baseline, confirmed-exit and missing/partial semantics as the table, retaining raw bytes in accessible help. Clarify in the peak help text that it refers to private working set.

The chart’s metric dropdown should control the chart; it should not silently change the meaning of these summary labels.

---

## 9. Selected-process history controls and chart

### Controls

Replace the ambiguous RAM and plus/minus control row with explicit controls.

| Element | Default | Required behaviour |
|---|---|---|
| **Metric** dropdown | Private working set | Select the plotted counter; include Private commit when available |
| **View last** segmented control | 30m | Offer 5m, 15m, 30m and All retained history |
| **Y-axis** segmented control | From zero | Choose From zero or Fit values |
| **Reset view** button | Enabled after navigation | Remove manual pan/zoom, preserve selected range and metric, and re-enable Follow latest |
| **Follow latest** checkbox | Checked | Keep the chart attached to the newest displayed sample |

Use orange fill for selected segmented options.

### From zero and Fit values

**From zero** sets the lower bound to zero and chooses a suitable upper bound above the visible maximum.

**Fit values** uses the visible value range with modest padding. Keep both axis bounds visible so the narrower scale is obvious.

The overall-memory chart should retain its zero-to-capacity scale independently.

### History chart

Use a single green filled area for the selected metric:

- A clear green outline.
- A restrained translucent green fill.
- Subtle gridlines.
- Labelled units.
- A current or displayed-endpoint marker and value.

Use the same metric definition for the line, endpoint label and tooltip.

Keep a practical minimum plot height. On short windows, let the inspector scroll instead of compressing the chart into an unreadable strip.

### Time navigation

Preserve wheel-based chart time zoom if it already exists.

Manual panning or zooming should turn **Follow latest** off. Re-enabling it returns the viewport to the newest displayed sample.

Use elapsed session time consistently on both charts, with wall-clock timestamps available in tooltips.

### History availability

Display:

**Requested: 30m · Recorded: 5m 06s**

For a newly observed process, use that process’s available history. **All** means all retained history for the selected entity.

Do not fill unobserved periods with invented zeroes.

---

## 10. Footer and diagnostics

Replace the compressed status string with three readable groups:

**92 executable groups**

**549 processes observed this session**

**171 with unavailable data · Details**

When filtering, use a count such as **12 of 92 executable groups**. Change the first label with the grouping mode.

Distinguish current unavailable readings from historical failures. The displayed count and **Details** view should use the same scope.

Make **Details** an orange, underlined action. Its panel should contain:

- Collection failures and their recorded reasons.
- Measurement coverage.
- Last successful collection time.
- Retained snapshot count.
- Omitted-track information.
- Mem Map’s own memory footprint.

This retains useful diagnostics from the original header and footer without giving them the same prominence as system usage.

At the far right, retain:

**Recording continues when display is paused**

---

## 11. Shared update behaviour

Apply one completed collection snapshot to the header, table, inspector and overview together. This prevents different parts of the interface displaying different moments as though they were one reading.

Keep collection work away from the UI thread. Update existing rows and chart buffers incrementally.

Use a stable process-instance identity that distinguishes restarts, such as PID plus creation time. Preserve selection and expansion across refreshes.

When the selected process exits, keep its inspector available with an **Exited** state and retained history. When a filter hides the selection, identify that condition rather than silently selecting an unrelated process.

While paused, retain the displayed snapshot and the history needed to inspect it, even as the collector continues.

Persist presentation preferences such as grouping, compact mode, column widths, splitter position and chart ranges.

---

## 12. Keyboard access, scaling and zoom

Provide a predictable tab order from header actions through the toolbar, table and inspector.

Implement:

- **Ctrl+F:** focus the filter.
- **Escape in the filter:** clear the query.
- **Up/Down:** move through table rows.
- **Right Arrow:** expand or enter children.
- **Left Arrow:** collapse or move to the parent.
- **Space:** toggle a focused checkbox.
- Standard keyboard activation for buttons and segmented controls.

Expose names, values, sort direction, selected state, hierarchy and expanded state through the platform accessibility APIs. Make chart values available without requiring a mouse.

Retain the existing UI zoom capability. Move its visible controls into a compact **View** menu or status control if necessary, and preserve existing shortcuts. **Ctrl+0** can restore normal application scale.

Keep application scaling and chart navigation distinct. Verify the layout with increased text scaling; allow wrapping and scrolling rather than shrinking text.

---

## 13. Implementation order and acceptance checks

Implement in this order:

1. Verify existing grouping, lifecycle and memory-counter definitions.
2. Add theme resources and the three-region layout.
3. Replace toolbar controls and table presentation.
4. Implement consistent peak, change and snapshot behaviour.
5. Replace the overview with the cumulative stacked area chart.
6. Update the inspector, history controls and footer.
7. Verify missing data, live interaction, keyboard access and scaling.

Use these cases to check the result:

| Check | Expected result |
|---|---|
| Capacity is 32 GiB, available is 8 GiB, named bands total 6 GiB, and other measured processes total 3 GiB | Used is 24 GiB; Other used RAM is 15 GiB; the stack reaches 24 GiB with 8 GiB headroom |
| Table is filtered to ChatGPT.exe | Overall system usage and the overview stack remain unchanged |
| A process has 6 GiB private commit and 1 GiB private working set | Its physical-memory contribution is 1 GiB |
| Two group members peak at different times | The group peak comes from simultaneous group totals |
| Five-minute change is requested after three minutes of recording | Show unavailable with an insufficient-history explanation |
| A process measurement fails | Show an unavailable or partial state without fabricating a release |
| Display is paused while usage changes | Displayed readings remain fixed; collection continues |
| A selected process moves in the memory ranking | Selection remains attached to the same process or group |
| A truncated path is copied | The complete original path is copied |
| Text scaling is increased | Every control and exact value remains accessible through layout adaptation or scrolling |

The resulting view should let someone answer four questions directly: **How much RAM is used, what accounts for it, what changed, and what is happening to the selected process?**