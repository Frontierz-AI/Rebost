# Changelog

All notable changes to this project are documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The app-data layout may change without a migration.

## [Unreleased]

## [0.8.6] - 2026-08-21

### Changed

- First-run cards describe Chat, a Shelf, and Recipes.
- First run asks you to install the suggested AI.
- Settings calls the section AI, and the web setting Online.

## [0.8.5] - 2026-08-21

### Changed

- Colors are mint, dark green, and cool gray. Buttons are pills.
- First run stays on the dark screen when you install an AI.
- After the AI file is in, Skip the check and continue is the only extra action.
- The sidebar uses a plain gray when a section is not selected.
- Small text buttons such as Diagnostics have a bit of vertical padding.

## [0.8.4] - 2026-08-21

### Added

- In Explore other AIs, paste a catalog page or an owner/name to open that AI first.

### Changed

- When Chat looks at earlier conversations, it stays on the same Shelf.
- Chat looks through files or the web when the question needs it.

### Fixed

- Opening a long file again continues from where the last look stopped.
- After Chat looks through files, a blank follow-up is answered instead of left empty.

## [0.8.3] - 2026-08-20

### Added

- Each conversation has an animal face in the list and next to replies. Chat uses that name if it introduces itself.
- Settings has Explore other AIs. Browse public catalogs and sort by what fits this computer, newest, smallest download, or most downloaded. Only the search words leave this computer.

### Changed

- Chat writes more plainly.
- Deleting a short conversation no longer asks first. Rebost still asks when the thread has five or more messages.
- Diagnostics can open the AI log on this computer.

### Fixed

- An AI this computer can't run is refused after download, with a clear message.

## [0.8.2] - 2026-08-19

### Added

- On a Snapdragon PC, pick Windows (ARM).

### Fixed

- Chat no longer leaves a blank reply. Try again.

## [0.8.0] - 2026-08-18

### Changed

- On a PC, pick Windows. Mac and Windows are the same version.

### Security

- HTTPS handling is updated so a noisy connection cannot fill memory.

## [0.7.0] - 2026-08-17

### Fixed

- When the computer is in Dark, text and cards stay readable. That includes the first-run install card.
- Recipe slots (`«…»`) stay readable in Dark.
- Edit Recipe no longer shows a stray ">".

## [0.6.0] - 2026-08-17

### Changed

- New conversations keep the Shelf you last chose, including No Shelf.
- An empty conversation picks up a Shelf you just created.

### Fixed

- Chat still answers after it looks through files or earlier conversations.
- A question with no Shelf no longer shows a look-through.
- If the answer stalls, Rebost stops so you can try again.

## [0.5.5] - 2026-08-17

### Changed

- New conversations keep the Shelf you last chose, including No Shelf.
- An empty conversation picks up a Shelf you just created.

### Fixed

- Chat still answers after it looks through files or earlier conversations.
- A question with no Shelf no longer shows a look-through.
- If the answer stalls, Rebost stops so you can try again.

## [0.5.4] - 2026-08-17

### Changed

- Chat keeps more of a file in view. Answers still stop when the AI is done.

## [0.5.3] - 2026-08-17

### Added

- Open Thinking to see what Rebost looked through before it answered.

### Changed

- A file you attach is read first, and as thoroughly as Deep. Asking to summarize it keeps more than the opening.
- Opening a long file again reads the next part. Other excerpts from that file stay.

### Fixed

- The Shelf you picked still has room after you attach a file.
- Reading more of a long file keeps going from where it left off.

## [0.5.2] - 2026-08-17

### Added

- You can rename a Shelf. The folder on disk stays put.
- You can edit a Recipe.
- Settings opens from the menu with ⌘, (Ctrl+, on Windows).

### Changed

- Settings → Online is Allow your AI to conduct online research. Your files are not sent online. Once the pages are in, the answer is still written on this computer.
- When Online is on, Chat is asked not to put Shelf text or personal details in a web search or in the page it opens.
- Rebost keeps its data in a new folder, so this version opens as a first run. Saving Settings on a Mac no longer asks to update or delete other applications.

### Fixed

- Failed actions say what went wrong and what to do.

## [0.5.1] - 2026-08-17

### Added

- Onboarding can show the license for an AI and open its Hugging Face or Ollama page.
- Answers can run longer on computers with more context. Short replies still stop when the AI is done.

### Fixed

- Stop works while the AI is still getting ready, and if the reply stalls.
- A file you add in Chat stays on that conversation. It does not replace the Shelf.
- If what Rebost has read is damaged, it rebuilds instead of blocking the app from opening.
- Skip and Cancel stay available while the first AI downloads. A failed download shows on the card.
- Privacy Lens counts Social Security numbers and labeled names. The empty state names what it looks for.
- Chat without an AI points at Install. Send stays off, and typing no longer shows a toast.
- An unplugged linked folder is paused. Files stay on the Shelf until the folder is back or you remove it.
- An interrupted download no longer looks finished on Windows.

## [0.5.0] - 2026-08-16

### Added

- A setting lets Chat look things up on the web. It stays off until you turn it on. Those requests leave this computer. They do not go through Rebost.
- Chat can search the Shelf again, open a named file, read more around a match, or look in earlier conversations. Status names those steps.

### Changed

- How Chat looks through a Shelf is Off, Light, or Deep. New Shelves start at Deep. Light and Deep take longer.
- Light and Deep search three extra ways. Deep includes more of the matching files when they fit, and uses the AI's lightest built-in thinking when that AI has it.
- Official is only for the labs that trained the AI, not for forks or community builds. Official results still come first.

### Fixed

- Stop ends the answer even if Chat was still looking through files or the web.
- Words that already arrived stay on screen while Chat looks further.
- A failed look-through isn't saved as the answer.
- Chat won't open a page on this computer or a private network. Turning Online off applies to the current answer.

## [0.4.3] - 2026-08-16

### Added

- A setting lets Chat look things up on the web. It stays off until you turn it on. Those requests leave this computer. They do not go through Rebost.

### Fixed

- Stop ends a web lookup that was still running. Chat won't open a page on this computer or a private network. Turning Online off applies to the current answer.

## [0.4.2] - 2026-08-16

### Added

- Chat can search the Shelf again, open a named file, read more around a match, or look in earlier conversations. Status names those steps.

### Changed

- How Chat looks through a Shelf is Off, Light, or Deep. New Shelves start at Deep. Light and Deep take longer.
- Light and Deep search three extra ways. Deep includes more of the matching files when they fit, and uses the AI's lightest built-in thinking when that AI has it.

### Fixed

- Stop ends the answer even if Chat was still looking through files.
- Words that already arrived stay on screen while Chat looks further.
- A failed look-through isn't saved as the answer.

## [0.4.1] - 2026-08-16

### Changed

- Official is only for the labs that trained the AI, not for forks or community builds. Official results still come first.

## [0.4.0] - 2026-08-15

### Added

- Each Shelf has a Think level: Min, Some, or Max. New Shelves start at Max.
- Conversations show the start date, message count, Shelf, and Download at the top.
- Chat says what it's doing before the answer arrives: warming up, waiting for another answer, looking through the Shelf, reading.
- Shelves show Ready, Processing, Syncing, or Sync error. Resume tries failed files again.
- Opening Rebost again brings the window you already have to the front.

### Changed

- A new Shelf is created inside Rebost, so macOS does not ask for Documents access. Updating Rebost does not delete those files.
- A Shelf stops at 1,000 files. Hidden files and common install folders are skipped.
- Chat includes a bit of text before and after each match. A hyphenated name still matches the unhyphenated spelling.
- Some and Max search three extra ways. Max then asks what is still missing and searches that.
- A long conversation opens on the latest messages. Read more loads earlier ones.
- You can keep chatting on the current AI while a new one downloads.
- Files that haven't changed aren't read again when you reopen Rebost.
- Chat, menus, and dialogs follow the computer: system light or dark, native delete confirms, right-click menus.
- Settings and the Chat warning call it AI Brain. House rules are at the top of Settings.
- Citation previews show formatted text.

### Fixed

- Sending twice quickly only sends once. A question from another conversation waits until the first answer is done.
- Removing a Shelf or a linked folder stops reading those files.
- Files you drop while a Shelf is reading others still count toward 1,000.
- Installing a new AI waits until the current answer is done.
- On Windows, a path that's too long is skipped, and the rest of the drop goes on the Shelf.
- Opening Shelves turns a file that's been Reading for five minutes into Sync error, with Resume to try again.
- A failed update check no longer shows up as an error in the log.
- Word and Excel lock files and temporary files aren't read.

## [0.3.9] - 2026-08-15

### Added

- Conversations show the start date, message count, Shelf, and Download at the top.

### Changed

- A new Shelf starts at Max.
- Chat includes a bit of text before and after each match. A hyphenated name still matches the unhyphenated spelling.
- Some and Max search three extra ways. Max then asks what is still missing and searches that.

### Fixed

- A failed update check no longer shows up as an error in the log.

## [0.3.8] - 2026-08-15

### Added

- Chat says what it's doing before the answer arrives: warming up, waiting for another answer, looking through the Shelf, reading the conversation.
- Shelves show Ready, Processing, Syncing, or Sync error. Resume tries failed files again.
- Synced folders and personal-information chips live on the Shelf detail.

### Changed

- Citation previews show formatted text. Pages say Page 1.
- Think level is Min, Some, or Max.
- Removing a synced folder asks for a confirm.

### Fixed

- Opening Shelves turns a file that's been Reading for five minutes into Sync error, with Resume to try again.

## [0.3.7] - 2026-08-15

### Fixed

- Files you drop while a Shelf is reading others still count toward 1,000.
- Installing a new AI waits until the current answer is done.

## [0.3.6] - 2026-08-15

### Changed

- A new Shelf is created inside Rebost, so macOS does not ask for Documents access. Updating Rebost does not delete those files. Add a folder from this computer, or drop files, to use a folder you already have.
- Think level is on the Shelf.
- House rules are at the top of Settings.

### Fixed

- On Windows, files with a long path still copy. A path that's too long is skipped, and the rest of the drop goes on the Shelf.

## [0.3.5] - 2026-08-15

### Changed

- A long conversation opens on the latest messages. Read more loads earlier ones.
- Chat doesn't keep a copy of each cited excerpt. Opening a citation reads the file.
- Long thinking is shortened when it's saved.

### Fixed

- Sending twice quickly only sends once. A question from another conversation waits until the first answer is done.
- Removing a Shelf or a linked folder stops reading those files.
- Word and Excel lock files and temporary files aren't read. If a file is still being written, Rebost waits and tries once more.

## [0.3.4] - 2026-08-15

### Changed

- Files that haven't changed aren't read again when you reopen Rebost. A file that failed stays as Error until you try again.
- Drop a lot of files into a linked folder while Rebost is open and they wait their turn. The Shelf shows how many are waiting.
- You can keep chatting on the current AI while a new one downloads. The previous one stays until the new one is ready.

## [0.3.3] - 2026-08-15

### Added

- Opening Rebost again brings the window you already have to the front.

### Changed

- A Shelf stops at 1,000 files. Hidden files and common install folders are skipped. Linking a large folder comes back right away.
- Recipes, House rules, and Chat messages have a length limit. A long paste can't crowd out the answer.

## [0.3.2] - 2026-08-15

### Changed

- Chat, menus, and dialogs follow the computer more closely: system light or dark, native delete confirms, right-click menus, and the window remembers its size.
- Settings and the Chat warning call it AI Brain.

### Fixed

- Search shuts down more carefully so tests are less likely to run out of files.

## [0.3.1] - 2026-08-15

### Added

- Each Shelf has a Think level, Off by default. Think looks through your files a few extra ways. Think harder looks more thoroughly and reads more of each match.

### Changed

- First-run Install an AI leads with why Rebost needs a brain on this computer, that the download can take a few minutes, and that you can pick a different one.
- Chat is better at finding a file you named, and one document can no longer fill the whole answer.
- Installing an AI can pick up if the download stops. Large downloads use more than one connection.

## [0.3.0] - 2026-08-15

Chat can start from a Recipe, take files you drop, and keep a name you gave a conversation.

### Added

- Empty Chat shows Recipes. Recipes that use a Shelf stay hidden until you choose one.
- Drop files onto Chat, or use Add files. They go on the selected Shelf, the only Shelf, or a new Shelf. Document-name slots in the message fill with those file names.
- Ask in Chat on a Shelf opens Chat with that Shelf selected.
- When the cursor is in a document-name placeholder, Chat lists files on that Shelf. Arrow keys, Return, and Tab pick one. Escape leaves the slot. If the Shelf has no files, nothing appears.
- File menu: New conversation (`⌘N`, or `Ctrl+N` on Windows). View menu: Chat, Shelves, Recipes (`⌘1`, `⌘2`, `⌘3`).
- Rename a conversation from the list (pencil or double-click). Export as Markdown when it has messages.

### Fixed

- Attaching or dropping files no longer fills paste or language slots in a Recipe.
- Shift+Return still inserts a newline when the file list is open.
- A conversation you renamed keeps that name after later messages, including if the name is New conversation.

## [0.2.28] - 2026-08-15

Screens change a little more smoothly, and Reset Rebost comes back to first-run instead of a blank window.

### Added

- Onboarding, drawers, and download bars move briefly when something changes. Reduce motion on the computer turns that off.

### Fixed

- Reset Rebost (type DELETE) opens the first-run screen again. It was leaving a blank white window.

## [0.2.27] - 2026-08-15

Chat gets ready faster, and replies follow the memory on this computer.

### Changed

- First chat spends less time getting ready. The AI warms up when you are about to type, not on every screen.
- Shelves, Recipes, and Settings load when you open them, so Chat is available sooner.
- Replies use more of the machine when there is plenty of memory, and smaller steps when memory is tight.
- On some Windows PCs, the first chat may download a faster way to run the AI. If that does not work, Rebost keeps the copy that came with the installer.
- Follow-up questions on the same Shelf keep House rules and Shelf context in place, so the AI spends less time re-reading them.

### Fixed

- If that extra Windows download never becomes ready, the next try uses the included copy instead of looping.
- After a crash, a leftover AI process is stopped when you open Rebost again.

## [0.2.26] - 2026-08-14

First public release. Rebost is a desktop app: you talk to an AI on this computer, alone or against files you put on a Shelf.

### Added

- Chat stays on this computer. Attach a file when you need to, or choose a Shelf so answers come from your files, with citations. Conversation memory applies either way. After idle, the first message may say "Warming up…" while the AI gets ready.
- Shelves: named collections. Drag in PDFs, Word files, or spreadsheets, or link a folder (new files show up here). Opening a file shows the text Rebost reads and counts of detected personal information (emails, IBANs, Spanish tax ids). Those counts are not a legal opinion.
- Recipes: saved prompts with `«…»` placeholders. House rules are standing tone and language instructions and stay out of retrieved excerpts.
- Copy an answer with personal identifiers removed when the reply contains them.
- First-run onboarding: the privacy promise, then an AI that fits this computer. Settings later shows two more suggestions that are not already installed.
- Search Hugging Face for other AIs (download counts, original publishers first, newest first). More info on a hit opens a card with publisher, downloads, file, and license. After a download you can skip the check and use the file as-is.
- In-app updater: a quiet check on startup, a sidebar cue when a newer version exists, and an Update window that downloads and installs it.
- Settings → Reset Rebost wipes app data, the AI, and caches after you type DELETE. Shelf folders on disk are kept.
- Show in folder on the document drawer and citation panel.
- Installers for Mac (Apple chip and Intel) and Windows. Each includes what Rebost needs to run the AI on that kind of computer. Windows ARM64 can be built from source. Linux is not a supported platform.
- English UI. Confirm before deleting conversations, Recipes, and Shelves.

### Security

- An AI without a checksum is refused. Names and sources are checked before install. After download, Skip the check and use the file skips that verification.
- Searches and downloads use HTTPS. Chat stays on this computer.
- Answers are sanitized before they are shown.
- Open and Show in folder only work for files on a Shelf Rebost knows. Linked-folder scans skip shortcuts that point outside.
- App data on Mac and Linux is readable only by your user account.
- Diagnostics do not send log contents into the window.

[0.8.6]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.0...v0.8.2
[0.8.0]: https://github.com/Frontierz-AI/Rebost/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/Frontierz-AI/Rebost/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/Frontierz-AI/Rebost/compare/v0.5.5...v0.6.0
[0.5.5]: https://github.com/Frontierz-AI/Rebost/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/Frontierz-AI/Rebost/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/Frontierz-AI/Rebost/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/Frontierz-AI/Rebost/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/Frontierz-AI/Rebost/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/Frontierz-AI/Rebost/compare/v0.4.0...v0.5.0
[0.4.3]: https://github.com/Frontierz-AI/Rebost/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/Frontierz-AI/Rebost/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.4.1
[0.4.0]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.4.0
[0.3.9]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.9
[0.3.8]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.8
[0.3.7]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.7
[0.3.6]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.6
[0.3.5]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.5
[0.3.4]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.4
[0.3.3]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.3
[0.3.2]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.2
[0.3.1]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.1
[0.3.0]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.3.0
[0.2.28]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.2.28
[0.2.27]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.2.27
[0.2.26]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.2.26
