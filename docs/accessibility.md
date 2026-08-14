# Rebost accessibility reference

Keyboard-first notes for VoiceOver on macOS (verified against Rebost 0.2.26, Tauri 2 webview). Windows Narrator has not been verified. Gaps are listed at the end.

This file is a factual reference, not prompt instructions.

## Platform

| Item | Value |
|------|--------|
| App | Rebost desktop |
| OS | macOS, Windows 10/11 |
| Screen reader | VoiceOver (macOS). Narrator not verified. |
| UI | Custom webview (not native AppKit controls) |

## Landmarks

| Region | How to get there |
|--------|------------------|
| Sidebar | First tab stop after the window; Chat, Shelves, Recipes, Settings. An Update control appears above Settings only when a newer GitHub release was confirmed |
| Main | Next to the sidebar; heading depends on the view |
| Chat composer | Bottom of Chat; `textarea` |
| Document drawer | Dialog-like overlay on Shelves when a row is opened |

Document drawer, source panel, the Reset Rebost dialog, the model More info card, and the Update window trap Tab, move initial focus into the dialog, and close on Escape. Icon-only Send / Stop / New conversation expose `aria-label`. Copy and delete controls are visible without hover.

## Keyboard

| Action | Keys |
|--------|------|
| Move between sidebar and main | `Tab` / `Shift+Tab` |
| Activate a button | `Space` or `Return` |
| Send chat | `Return` (composer; Shift+Return for newline, see ChatView) |
| Close drawers, source panel, Reset dialog, model More info, Update window | `Escape` |
| Open citation | `Return` on a citation chip inside the answer |

VoiceOver rotor: form controls and buttons. Headings exist in Shelves/Settings; Chat thread list is a list of buttons.

## Known gaps (0.2.26)

- No VoiceOver custom actions for "copy without personal information" beyond the visible button label.
- Windows Narrator has not been verified.

Form fields on Shelves, Recipes, Settings, and Chat (composer, Shelf picker, Reset confirmation) have labels. Icon-only controls expose `aria-label`. Drawers, the Reset dialog, the model More info card, and the Update window trap Tab.

## Settings relevant to AT

House rules and Diagnostics are ordinary text. Increasing the OS font size scales the webview with the OS.
