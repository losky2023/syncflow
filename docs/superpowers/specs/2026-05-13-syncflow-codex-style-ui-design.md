# SyncFlow Codex-Style UI Refresh Design

Date: 2026-05-13

## Summary

SyncFlow will refresh the existing workbench UI toward a Codex-inspired desktop app style: quiet, focused, light, and durable for daily file work. The approved visual direction is based on the `v7` mockup:

`E:\workspace\wjtb\.superpowers\brainstorm\ui-review-manual\content\codex-inspired-workbench-v7.html`

This design is for review and implementation planning only. It does not change product behavior or backend logic.

## Approved Direction

Use a two-zone workbench:

- Left: a soft gray-blue file/navigation area.
- Right: a single white rounded main work area for preview, tabs, editing, and file metadata entry points.

The right details pane is hidden by default. Users open it from the preview header through an info/details icon. File tree actions remain available but must not make the tree visually noisy.

## Goals

- Make the app feel closer to the Codex desktop app: calm, restrained, and structurally clear.
- Improve the visual connection between top, left, and main content areas.
- Keep the file tree functional while preserving a clean default state.
- Reduce decorative gradients, heavy shadows, nested cards, and oversized radii.
- Preserve existing workflows: repository switching, file tree browsing, preview tabs, Markdown editing, sync status, conflict/status diagnostics, and cloud settings.

## Non-goals

- Rewriting application behavior.
- Changing Tauri command contracts.
- Redesigning sync algorithms or cloud integration.
- Adding a new design framework.
- Making a dark theme in this pass.

## Layout

### Window Shell

The outer shell uses a gray-blue background similar to Codex's sidebar surface. The shell has a modest border and a `14-16px` radius. Avoid large shadows; use a subtle 1px border and, at most, a very light shadow.

The left sidebar and the surrounding app background should feel like one continuous surface. The main white work area should be the visual anchor.

### Left Sidebar

The sidebar has four regions:

1. Compact brand row.
2. Files header with small icon actions.
3. File tree.
4. Bottom repository/sync status control.

The sidebar must not place the active repository as a large card at the top. Repository name, switching, and sync summary belong in the bottom status control.

Target width: about `280-300px` on desktop. It can reduce slightly on narrower screens, but file names should remain readable.

### Main Work Area

The main area is a single white rounded container with:

1. Preview header.
2. Preview tabs.
3. Preview/editor content.
4. Bottom file status bar.

The main container should start slightly below the floating path row, giving the top controls breathing room without creating a separate heavy topbar.

### Top Path Row

The path row is lightweight and sits above the main white container. It contains:

- Current repository and relative path.
- Small global actions, such as refresh and settings.

It should look like a quiet contextual toolbar, not a full navigation bar.

### Details Pane

Details are hidden by default. Opening details should reveal a right-side inspector connected to the main work area. The preferred implementation is a right-side drawer/inspector, not a permanently allocated third grid column.

The details pane should support:

- File metadata.
- Location information.
- Sync/cloud state.
- Conflict details and actions.

It should close easily and return the workbench to the two-zone layout.

## Visual System

### Color Tokens

Use a Codex-like light palette:

- App background: `#f5f7fa`
- Sidebar surface: `#eaf0f7`
- Sidebar selected: `#dfe6ee`
- Main panel: `#ffffff`
- Border: `#e5e7eb`
- Strong border: `#d5dbe3`
- Text: `#24292f`
- Muted text: `#7b8490`
- Hover surface: `#e4ebf2`
- Success/synced: `#16a34a`
- Warning/dirty: `#f97316`
- Primary blue: use sparingly for focus rings or selected command emphasis.

Avoid dominant purple, beige, heavy gradients, or glassmorphism.

### Radius

- Main shell and main work area: `14-16px`.
- Cards/drawers/menus: `10-12px`.
- Buttons and row controls: `6-9px`.
- File tree rows: about `8px`.

Avoid stacking many large rounded cards inside other rounded cards.

### Borders and Shadows

Use 1px borders as the primary layer separator. Shadows should be rare:

- No shadow for ordinary panels.
- Light shadow only for popovers, menus, and inspectors.
- Avoid deep floating-card shadows in the base layout.

### Typography

Use the existing system stack, prioritizing Windows-friendly UI fonts:

`"Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI", "PingFang SC", system-ui, sans-serif`

Text sizes:

- Main document title in preview mock state: about `30px`.
- Section/header labels: `12-13px`.
- File tree rows: `12px`.
- Supporting/status text: `11px`.

Do not use viewport-scaled font sizing.

## Components

### File Tree

The file tree default state should be quiet:

- Expand/collapse chevron.
- Folder/file icon.
- Name.
- Optional unsaved/dirty dot.

Use recognizable folder and file icons. In implementation, prefer lucide icons such as `Folder`, `FolderOpen`, `FileText`, `Image`, `File`, `ChevronRight`, and `ChevronDown`.

Avoid square placeholder icons.

Actions:

- Default: no visible action buttons except selected/hover affordances.
- Hover: show lightweight quick actions, such as open/reveal and more.
- Selected: actions may remain visible.
- More menu: rename, move, copy relative path, refresh, delete.
- Dangerous actions, especially delete, must stay inside the more menu and use danger styling.
- New folder/file and import actions belong in the file header action group or more menu, not as large buttons.

Inline states:

- Rename/new item uses an inline input row.
- Loading and errors should occupy row-level or compact inline space when possible.
- Empty tree uses a simple muted message, not a large illustration.

### Repository Status Control

The bottom sidebar control combines:

- Current repository name.
- Switch/manage affordance.
- Sync status summary.
- File count, queue count, conflict count.

It should be compact and visually connected to the sidebar, not a large independent card. Clicking it opens the repository manager.

### Preview Header

The preview header contains:

- Section title, such as "预览".
- Current relative path.
- Details/info icon.
- Open/reveal icon when relevant.

The header should be calm and compact. File path text truncates cleanly.

### Tabs

Tabs are low-contrast and utilitarian:

- Active tab uses white background and a subtle border connection to content.
- Inactive tabs use the light gray tab strip.
- Dirty/saving indicators are small and do not shift layout.
- Close controls appear on hover or selected tab.

### Markdown Editor/Preview

The document area should feel like a clean editor canvas:

- Main content centered with readable measure.
- Generous horizontal padding.
- No nested card around the editor unless the editor component requires a border.
- Focus ring appears only when editing.

### Details Inspector

The inspector should attach visually to the right side of the main area. It may overlay part of the main work area rather than shrinking the layout every time.

Use compact sections:

- File.
- Location.
- Sync.
- Conflicts.

Avoid showing the inspector by default.

### Menus and Popovers

Menus should use:

- White background.
- `10px` radius.
- 1px border.
- Light shadow.
- `26-30px` item height.

Menu item labels should be concise. Destructive items use red text and hover treatment.

## States

Required UI states:

- No repository configured.
- Repository selected with empty tree.
- Tree loading.
- Tree load error.
- File selected.
- Folder selected.
- Unsupported preview.
- Markdown dirty.
- Markdown saving.
- Save error.
- Sync stopped.
- Sync running/synced.
- Sync pending.
- Sync issue/conflict.
- Cloud disconnected.
- Cloud connected.
- Details inspector open.
- Repository manager open.

These states should be represented without making the default workbench visually crowded.

## Accessibility

- All icon-only controls need accessible labels and tooltips.
- Focus states use a visible outline or ring with sufficient contrast.
- File tree rows must be keyboard navigable.
- More menus close with Escape and support keyboard navigation.
- Status colors cannot be the only signal; include text labels.
- Truncated paths and file names should preserve full values in `title` or equivalent accessible text.

## Implementation Notes

The refresh should be mostly CSS and component structure changes in:

- `syncflow/packages/client/src/styles/workbench.css`
- `syncflow/packages/client/src/app/Workbench.tsx`
- `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
- `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
- `syncflow/packages/client/src/components/sidebar/SpaceList.tsx`
- `syncflow/packages/client/src/components/details/DetailsPane.tsx`
- Preview components only as needed for header/status consistency.

Prefer adding lucide icons if the project can accept the dependency. If avoiding a new dependency, use a tiny local icon component set with consistent `16px` stroke icons. Do not keep text symbols as final UI icons.

## Acceptance Criteria

- Default workbench visually matches the v7 direction.
- Left file tree uses recognizable folder/file icons.
- File tree actions are available without cluttering the default state.
- Details are hidden by default and open as an inspector/drawer.
- Repository and sync status are combined at the bottom of the sidebar.
- The top/left/main area connections feel calm and continuous.
- UI remains usable at common desktop widths and does not overlap at narrower widths.
- `npm --prefix syncflow/packages/client run build` passes after implementation.
