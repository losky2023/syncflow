# SyncFlow WeChat Article Import Design

Date: 2026-05-07

## Summary

SyncFlow will support importing WeChat Official Account articles into a selected sync space as normal Markdown files. The first version focuses on clipboard import: users open an article in WeChat or a browser, copy the article content, return to SyncFlow, and import it into the current folder. A later version will add URL import as a best-effort convenience path.

The feature should not create a separate article database. Imported articles become regular files under the sync-space root so the existing workbench file tree, Markdown editor, filesystem safety model, local sync runtime, conflict handling, and Baidu Netdisk cloud sync can continue to operate without special article-specific storage.

## Approved Direction

Build C first, then B:

- **C: Clipboard import** is the primary path for the first version.
- **B: URL import** follows as a best-effort enhancement.

Clipboard import is the stable baseline because it does not require a WeChat Official Account backend account, does not depend on private or unstable WeChat page APIs, and avoids most anti-bot behavior on `mp.weixin.qq.com`. URL import should never be the only path because public WeChat article pages can be rate-limited, login-gated, altered, or protected from direct image retrieval.

## Goals

- Import a copied WeChat article into the selected sync space as Markdown.
- Preserve useful article metadata in YAML front matter.
- Save article images as local files and rewrite Markdown image references to relative paths.
- Reuse the existing `spaceId + relativePath` filesystem safety boundary.
- Refresh the file tree and open the imported article after a successful import.
- Provide a clear fallback when URL import fails: ask the user to use clipboard import.

## Non-Goals

- Bulk syncing an owned Official Account through AppID/AppSecret in this phase.
- Bypassing WeChat login, anti-bot, paid-article, or permission restrictions.
- Importing comments, likes, read counts, recommendations, ads, or interactive widgets.
- Reconstructing exact WeChat visual styling.
- Creating a proprietary article database separate from files.
- Automatically publishing back to WeChat.

## User Experience

### Clipboard Import

The user flow:

1. User opens a WeChat Official Account article.
2. User selects and copies the article content.
3. User selects a target sync space or folder in SyncFlow.
4. User clicks "Import WeChat Article".
5. SyncFlow reads clipboard HTML first, falling back to plain text.
6. SyncFlow shows a small confirmation state with detected title and target filename when practical.
7. SyncFlow writes the Markdown file and image assets.
8. SyncFlow refreshes the target directory and opens the imported Markdown file.

The entry point should live near existing file creation/import actions:

- file tree toolbar: import into selected folder or current space root,
- directory row actions: import into this directory.

If no folder is selected, import into the selected sync-space root or a default `WeChat Articles/YYYY-MM/` directory, depending on the product decision during implementation. The first implementation should prefer a predictable default directory so imported articles do not clutter the root.

### URL Import

The later URL flow:

1. User chooses "Import WeChat Article from URL".
2. User pastes an `https://mp.weixin.qq.com/...` URL.
3. SyncFlow fetches the article page with a normal HTTP client.
4. If parsing succeeds, SyncFlow uses the same Markdown/file writing pipeline.
5. If fetching or parsing fails, SyncFlow displays a concise failure and suggests clipboard import.

URL import is best effort. It should not promise compatibility with every article.

## File Layout

Default layout:

```text
WeChat Articles/
  YYYY-MM/
    Article Title.md
    assets/
      Article Title-01.jpg
      Article Title-02.png
```

If the user imports into a specific folder, use that folder as the base:

```text
Selected Folder/
  Article Title.md
  assets/
    Article Title-01.jpg
```

Filename generation rules:

- Prefer the parsed article title.
- Trim whitespace and collapse repeated spaces.
- Remove path separators, device names, control characters, and characters rejected by Windows filenames.
- Limit filename length before adding `.md`.
- If a file already exists, append `-2`, `-3`, and so on.
- Keep all generated paths relative to the selected sync space.

## Markdown Format

Generated files should use YAML front matter:

```markdown
---
source: wechat
title: "Article title"
account: "Official Account name"
author: "Author name"
published_at: "2026-05-07"
original_url: ""
imported_at: "2026-05-07T12:34:56Z"
import_method: "clipboard"
---

# Article title

Article body...
```

Rules:

- Include fields only when known, except `source`, `title`, `imported_at`, and `import_method`.
- Escape front matter values safely.
- Preserve headings, paragraphs, block quotes, lists, links, inline code, code blocks, and tables when the source HTML supports them.
- Strip scripts, style tags, tracking pixels, comments, hidden elements, ad blocks, and unsupported widgets.
- Keep text readable even when rich formatting cannot be preserved.

## Image Handling

Clipboard import should first look for image data or image URLs in the copied HTML:

- If an image is embedded in the clipboard payload, write it directly.
- If an image is referenced by URL, try to download it with bounded timeouts and size limits.
- If a download fails, keep a Markdown placeholder with the original URL or a short failure note.

Image rules:

- Accept common raster types: JPEG, PNG, GIF, and WEBP.
- Reject SVG in the first version unless a later implementation adds sanitization.
- Use a per-image size limit and a total import size limit.
- Deduplicate identical image URLs within one import.
- Rewrite Markdown image references to local relative paths.

## Architecture

Add a small import pipeline on the Tauri backend:

```text
ClipboardReader / UrlFetcher
  -> WeChatArticleParser
  -> ArticleMarkdownRenderer
  -> ArticleAssetWriter
  -> TreeNode response
```

The core data shape:

```text
ParsedArticle {
  title,
  account,
  author,
  published_at,
  original_url,
  body_blocks or sanitized_html,
  images
}
```

Keep parsing and rendering separate from filesystem writing:

- Parser: understands copied WeChat HTML and URL-fetched HTML.
- Renderer: converts sanitized article content into Markdown.
- Writer: resolves safe target paths, writes Markdown and assets, and returns a `TreeNode`.

This separation allows URL import to reuse almost all of the clipboard implementation.

## Tauri Commands

First version:

```text
import_wechat_article_from_clipboard(request) -> TreeNode
```

Request:

```text
{
  spaceId: string,
  parentRelativePath?: string | null
}
```

Later URL version:

```text
import_wechat_article_from_url(request) -> TreeNode
```

Request:

```text
{
  spaceId: string,
  parentRelativePath?: string | null,
  url: string
}
```

Frontend wrappers belong in `src/lib/tauriClient.ts`. React components should not call raw `invoke(...)`.

## Frontend Changes

Expected files touched in implementation:

- `syncflow/packages/client/src/app/Workbench.tsx`
- `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
- `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
- `syncflow/packages/client/src/lib/tauriClient.ts`
- `syncflow/packages/client/src/types/workbench.ts`
- `syncflow/packages/client/src/styles/workbench.css`

Keep the UI compact and workbench-like:

- Add an import action near file creation actions.
- Show busy state while parsing and writing.
- On success, refresh only the target directory if possible.
- Select and preview the new Markdown file.
- On failure, show a concise actionable error.

No separate landing page, wizard, or article library view is needed for the first version.

## Backend Changes

Expected files touched in implementation:

- `syncflow/packages/client/src-tauri/src/commands.rs`
- `syncflow/packages/client/src-tauri/src/main.rs`
- optional new module: `syncflow/packages/client/src-tauri/src/wechat_import.rs`
- optional parser tests beside the new module

The first implementation should use established Rust crates where practical:

- HTML parsing with a real parser rather than ad hoc string slicing.
- Markdown rendering through a small controlled converter or a crate if it fits the dependency profile.
- Clipboard access through a Tauri-compatible mechanism available in the desktop backend.

If clipboard HTML access is not reliable through Tauri APIs on Windows, the implementation may accept clipboard payload from the frontend as a second-best design:

```text
read clipboard in frontend
-> pass html/text to Tauri command
-> backend parses and writes files
```

The backend must remain responsible for parsing, sanitization, path safety, and writing.

## Filesystem Safety

All writes must preserve SyncFlow's existing sync-space safety model:

1. Parse `spaceId`.
2. Load the sync space from storage.
3. Validate `parentRelativePath`.
4. Resolve and canonicalize the target parent.
5. Verify the target parent remains inside the sync-space root.
6. Generate filenames internally.
7. Reject or rename on collisions; never overwrite silently.
8. Write Markdown and assets only under the chosen sync-space root.

The frontend must never provide absolute output paths or asset paths.

## Error Handling

Required errors:

- Clipboard empty or unreadable.
- Clipboard does not look like an article.
- Article title missing and no usable fallback filename.
- Target folder missing or outside the sync space.
- Filesystem permission failure.
- Image download timeout or unsupported image type.
- URL is not a valid `https://mp.weixin.qq.com/` article URL.
- URL fetch blocked, redirected unexpectedly, or not parseable.

Image failures should not fail the entire import unless every body extraction path also fails. The Markdown should remain useful even if some images cannot be imported.

## URL Import Constraints

URL import must be conservative:

- Accept only `https://mp.weixin.qq.com/` URLs.
- Use redirect and response-size limits.
- Do not send user cookies unless a later design explicitly covers user consent and security.
- Do not attempt to bypass login, payment, anti-bot, or access controls.
- Treat parsing selectors as versioned heuristics that may break.

The UI copy should frame URL import as convenience, not guaranteed archival.

## Testing Strategy

Backend tests:

- Parse representative copied WeChat HTML into `ParsedArticle`.
- Parse plain-text fallback content.
- Strip scripts and style content.
- Convert headings, paragraphs, lists, quotes, links, and images to Markdown.
- Generate safe filenames for Chinese, English, punctuation, and reserved Windows names.
- Avoid overwriting existing files by suffixing names.
- Reject parent traversal in target paths.
- Keep generated files inside the sync-space root.
- Continue import when one image fails.

Frontend verification:

- Import action appears in file tree controls.
- Busy, success, and failure states render correctly.
- Successful import refreshes the target folder and selects the created Markdown file.
- Error copy recommends clipboard fallback for URL failures.

Verification commands:

```bash
cargo test --workspace --manifest-path syncflow/Cargo.toml
npm --prefix syncflow/packages/client run build
```

Manual checks:

- Copy article from WeChat desktop and import.
- Copy article from browser and import.
- Import an article with multiple images.
- Import an article with no images.
- Import the same article twice and verify filenames do not overwrite.
- Try URL import with a public article and a failing/blocked article after the URL version exists.

## Implementation Order

1. Add parser and Markdown renderer tests with fixture HTML.
2. Add backend import module and safe filename/path writer.
3. Add `import_wechat_article_from_clipboard` command and register it in Tauri.
4. Add `tauriClient.ts` wrapper and TypeScript types.
5. Add the workbench import action and loading/error state.
6. Refresh/select the imported file after success.
7. Run Rust tests and frontend build.
8. Add URL import on top of the same parser/writer pipeline.

## Open Decisions

- Whether the default clipboard import target is the selected folder, current space root, or `WeChat Articles/YYYY-MM/`.
- Whether to show a confirmation preview before writing, or import immediately and show an undo-like reveal action.
- Which clipboard API gives the most reliable HTML payload on Windows Tauri.
- Whether failed image downloads should retain remote image URLs or local textual placeholders.
