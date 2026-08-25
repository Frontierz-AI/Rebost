# Changelog

All notable changes to this project are documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The app-data layout may change without a migration.

## [Unreleased]

## [0.8.11] - 2026-08-26

### Fixed

- Citations stay attached to the same files for the rest of a conversation. Opening one opens the file that answer used, not a different file from the Shelf.
- Chat answers in the language of the question. Files it looked up on a Shelf are not treated as something the person typed or pasted.

### Changed

- A short follow-up still looks through the last couple of questions and the files already cited.

## [0.8.10] - 2026-08-25

### Changed

- Conversation faces are food instead of animals. A conversation that still had an animal face gets a food one. Chat uses that name if it introduces itself.
- The README screenshots match the current windows.

## [0.8.9] - 2026-08-25

### Changed

- First launch offers an AI sized for the machine's memory, and the suggestion now uses more of that memory when the machine has it. The pick is Qwen3.8 27B on 32 GB or more, Ornith-1.5 9B on 16 to 24 GB, Qwen3.5 4B on 8 to 12 GB, and Qwen3.5 2B on about 6 GB.
- Settings still shows two more suggestions that are not already installed. The first extra is a stronger smaller pick from another family. The second is the nearest other family that still fits. A machine offered Qwen3.8 27B sees Ornith-1.5 9B and Muse Glimmer; one offered Ornith-1.5 9B sees Qwen3.5 4B and Ministral 3 8B; one offered Qwen3.5 4B sees Ministral 3 3B and Phi-4 Mini.
- Opening a citation, or View the text on a file, shows a window around that spot instead of the whole file. Earlier and Later page through the rest.

## [0.8.8] - 2026-08-22

### Added

- A Code of Conduct, adapted from the Contributor Covenant, covers issues, pull requests, and discussions.

### Changed

- Deleting a Shelf now says that Rebost will stop using it, the same wording used when a linked folder is removed. No files are deleted either way.
- The security, contributing, branding, and licensing pages were rewritten in the register the README now uses, and notes meant for whoever maintains the project moved out of them.
- The privacy page says what runs locally and then what the network is used for, rather than marking a broad claim false and leaving it there. Every user agent Rebost sends is quoted there in full.
- Questions about the project, and security reports, go to a project address rather than a personal one. Web lookups carry that address too. Security reports can also come in as a private advisory on GitHub.
- Earlier changelog entries were rewritten to read the way the rest of the documentation does, and links to releases that no longer exist were dropped.
- The version inside the user agent comes from the build, so it cannot disagree with the release.
- The screenshot set used to build the project site is no longer in the repository. The images the README shows are still there.

## 0.8.7 - 2026-08-22

### Changed

- The README opens with what the application does, then Download, then Chat, Shelves, Recipes, and House rules, with a screenshot of each.
- The FAQ, team, and privacy pages were rewritten to match. They name the machine where Rebost is installed rather than pointing at whichever computer the reader is on.

## [0.8.6] - 2026-08-21

### Changed

- First-run cards describe Chat, a Shelf, and Recipes.
- First run asks to install the suggested AI, which Chat needs before it can answer.
- Settings calls the section AI, and the web setting Online.

## [0.8.5] - 2026-08-21

### Changed

- Colors are mint, dark green, and cool gray. Buttons are pills.
- First run stays on the dark screen while an AI installs.
- Once the AI file is in place, Skip the check and continue is the only extra action.
- The sidebar uses a plain gray when a section is not selected.
- Small text buttons such as Diagnostics have a bit of vertical padding.

## [0.8.4] - 2026-08-21

### Added

- In Explore other AIs, pasting a catalog page or an owner and name opens that AI first.

### Changed

- When Chat looks at earlier conversations, it stays on the same Shelf.
- Chat looks through files or the web when the question needs it.

### Fixed

- Opening a long file again continues from where the last look stopped.
- After Chat looks through files, a blank follow-up is answered instead of left empty.

## [0.8.3] - 2026-08-20

### Added

- Each conversation has an animal face in the list and next to replies. Chat uses that name if it introduces itself.
- Settings has Explore other AIs. It browses public catalogs and sorts by what fits the machine, newest, smallest download, or most downloaded. Only the search words leave that machine.

### Changed

- Chat writes more plainly.
- Deleting a short conversation no longer asks first. Rebost still asks when the thread has five or more messages.
- Diagnostics can open the AI log stored on that machine.

### Fixed

- An AI the machine cannot run is refused after download, with a clear message.

## [0.8.2] - 2026-08-19

### Added

- Snapdragon PCs have a Windows (ARM) installer.

### Fixed

- Chat no longer leaves a blank reply.

## [0.8.0] - 2026-08-18

### Changed

- Windows has its own installer. Mac and Windows ship at the same version.

### Security

- HTTPS handling is updated so a noisy connection cannot fill memory.

## [0.7.0] - 2026-08-17

### Fixed

- When the operating system is set to Dark, text and cards stay readable. That includes the first-run install card.
- Recipe slots (`«…»`) stay readable in Dark.
- Edit Recipe no longer shows a stray ">".

## [0.6.0] - 2026-08-17

### Changed

- New conversations keep the last Shelf chosen, including No Shelf.
- An empty conversation picks up a newly created Shelf.

### Fixed

- Chat still answers after it looks through files or earlier conversations.
- A question with no Shelf no longer shows a look-through.
- If the answer stalls, Rebost stops so the question can be sent again.

## 0.5.4 - 2026-08-17

### Changed

- Chat keeps more of a file in view. Answers still stop when the AI is done.

## 0.5.3 - 2026-08-17

### Added

- Thinking shows what Rebost looked through before it answered.

### Changed

- An attached file is read first, and as thoroughly as Deep. Asking for a summary keeps more than the opening.
- Opening a long file again reads the next part. Other excerpts from that file stay.

### Fixed

- The selected Shelf still has room after a file is attached.
- Reading more of a long file keeps going from where it left off.

## 0.5.2 - 2026-08-17

### Added

- A Shelf can be renamed. The folder on disk stays put.
- A Recipe can be edited.
- Settings opens from the menu with ⌘, (Ctrl+, on Windows).

### Changed

- Settings → Online reads Allow your AI to conduct online research. Documents on a Shelf are not part of those lookups, and the answer is still written on the machine where Rebost is installed.
- When Online is on, Chat is asked not to put Shelf text or personal details in a web search or in the page it opens.
- Rebost keeps its data in a new folder, so this version opens as a first run. Saving Settings on a Mac no longer asks to update or delete other applications.

### Fixed

- Failed actions say what went wrong and what to do.

## 0.5.1 - 2026-08-17

### Added

- Onboarding can show the license for an AI and open its Hugging Face or Ollama page.
- Answers can run longer on machines with more context. Short replies still stop when the AI is done.

### Fixed

- Stop works while the AI is still getting ready, and if the reply stalls.
- A file added in Chat stays on that conversation. It does not replace the Shelf.
- If what Rebost has read is damaged, it rebuilds instead of blocking the application from opening.
- Skip and Cancel stay available while the first AI downloads. A failed download shows on the card.
- Privacy Lens counts Social Security numbers and labeled names. The empty state names what it looks for.
- Chat without an AI points at Install. Send stays off, and typing no longer shows a toast.
- An unplugged linked folder is paused. Files stay on the Shelf until the folder is back or the folder is removed.
- An interrupted download no longer looks finished on Windows.

## 0.5.0 - 2026-08-16

### Added

- A setting lets Chat look things up on the web. It stays off until it is turned on. Those requests leave the machine directly and do not go through Rebost.
- Chat can search the Shelf again, open a named file, read more around a match, or look in earlier conversations. Status names those steps.

### Changed

- How Chat looks through a Shelf is Off, Light, or Deep. New Shelves start at Deep. Light and Deep take longer.
- Light and Deep search three extra ways. Deep includes more of the matching files when they fit, and uses the AI's lightest built-in thinking when that AI has it.
- Official is only for the labs that trained the AI, not for forks or community builds. Official results still come first.

### Fixed

- Stop ends the answer even if Chat was still looking through files or the web.
- Words that already arrived stay on screen while Chat looks further.
- A failed look-through is not saved as the answer.
- Chat will not open a page on the local machine or a private network. Turning Online off applies to the current answer.

## 0.4.3 - 2026-08-16

### Added

- A setting lets Chat look things up on the web. It stays off until it is turned on. Those requests leave the machine directly and do not go through Rebost.

### Fixed

- Stop ends a web lookup that was still running. Chat will not open a page on the local machine or a private network. Turning Online off applies to the current answer.

## 0.4.2 - 2026-08-16

### Added

- Chat can search the Shelf again, open a named file, read more around a match, or look in earlier conversations. Status names those steps.

### Changed

- How Chat looks through a Shelf is Off, Light, or Deep. New Shelves start at Deep. Light and Deep take longer.
- Light and Deep search three extra ways. Deep includes more of the matching files when they fit, and uses the AI's lightest built-in thinking when that AI has it.

### Fixed

- Stop ends the answer even if Chat was still looking through files.
- Words that already arrived stay on screen while Chat looks further.
- A failed look-through is not saved as the answer.

## 0.4.1 - 2026-08-16

### Changed

- Official is only for the labs that trained the AI, not for forks or community builds. Official results still come first.

## 0.4.0 - 2026-08-15

### Added

- Each Shelf has a Think level: Min, Some, or Max. New Shelves start at Max.
- Conversations show the start date, message count, Shelf, and Download at the top.
- Chat says what it is doing before the answer arrives: warming up, waiting for another answer, looking through the Shelf, reading.
- Shelves show Ready, Processing, Syncing, or Sync error. Resume tries failed files again.
- Opening Rebost again brings the existing window to the front.

### Changed

- A new Shelf is created inside Rebost, so macOS does not ask for Documents access. Updating Rebost does not delete those files.
- A Shelf stops at 1,000 files. Hidden files and common install folders are skipped.
- Chat includes a bit of text before and after each match. A hyphenated name still matches the unhyphenated spelling.
- Some and Max search three extra ways. Max then asks what is still missing and searches that.
- A long conversation opens on the latest messages. Read more loads earlier ones.
- Chat keeps working on the current AI while a new one downloads.
- Files that have not changed are not read again when Rebost reopens.
- Chat, menus, and dialogs follow the operating system: light or dark, native delete confirms, right-click menus.
- Settings and the Chat warning use the same name for the installed AI. House rules are at the top of Settings.
- Citation previews show formatted text.

### Fixed

- Sending twice quickly only sends once. A question from another conversation waits until the first answer is done.
- Removing a Shelf or a linked folder stops reading those files.
- Files dropped while a Shelf is reading others still count toward 1,000.
- Installing a new AI waits until the current answer is done.
- On Windows, a path that is too long is skipped, and the rest of the drop goes on the Shelf.
- Opening Shelves turns a file that has been Reading for five minutes into Sync error, with Resume to try again.
- A failed update check no longer shows up as an error in the log.
- Word and Excel lock files and temporary files are not read.

## 0.3.9 - 2026-08-15

### Added

- Conversations show the start date, message count, Shelf, and Download at the top.

### Changed

- A new Shelf starts at Max.
- Chat includes a bit of text before and after each match. A hyphenated name still matches the unhyphenated spelling.
- Some and Max search three extra ways. Max then asks what is still missing and searches that.

### Fixed

- A failed update check no longer shows up as an error in the log.

## 0.3.8 - 2026-08-15

### Added

- Chat says what it is doing before the answer arrives: warming up, waiting for another answer, looking through the Shelf, reading the conversation.
- Shelves show Ready, Processing, Syncing, or Sync error. Resume tries failed files again.
- Synced folders and personal-information chips live on the Shelf detail.

### Changed

- Citation previews show formatted text. Pages say Page 1.
- Think level is Min, Some, or Max.
- Removing a synced folder asks for a confirm.

### Fixed

- Opening Shelves turns a file that has been Reading for five minutes into Sync error, with Resume to try again.

## 0.3.7 - 2026-08-15

### Fixed

- Files dropped while a Shelf is reading others still count toward 1,000.
- Installing a new AI waits until the current answer is done.

## 0.3.6 - 2026-08-15

### Changed

- A new Shelf is created inside Rebost, so macOS does not ask for Documents access. Updating Rebost does not delete those files. Adding a folder from the machine, or dropping files in, uses a folder that already exists.
- Think level is on the Shelf.
- House rules are at the top of Settings.

### Fixed

- On Windows, files with a long path still copy. A path that is too long is skipped, and the rest of the drop goes on the Shelf.

## 0.3.5 - 2026-08-15

### Changed

- A long conversation opens on the latest messages. Read more loads earlier ones.
- Chat does not keep a copy of each cited excerpt. Opening a citation reads the file.
- Long thinking is shortened when it is saved.

### Fixed

- Sending twice quickly only sends once. A question from another conversation waits until the first answer is done.
- Removing a Shelf or a linked folder stops reading those files.
- Word and Excel lock files and temporary files are not read. If a file is still being written, Rebost waits and tries once more.

## 0.3.4 - 2026-08-15

### Changed

- Files that have not changed are not read again when Rebost reopens. A file that failed stays as Error until it is tried again.
- A large batch dropped into a linked folder while Rebost is open waits its turn. The Shelf shows how many are waiting.
- Chat keeps working on the current AI while a new one downloads. The previous one stays until the new one is ready.

## 0.3.3 - 2026-08-15

### Added

- Opening Rebost again brings the existing window to the front.

### Changed

- A Shelf stops at 1,000 files. Hidden files and common install folders are skipped. Linking a large folder comes back right away.
- Recipes, House rules, and Chat messages have a length limit. A long paste cannot crowd out the answer.

## 0.3.2 - 2026-08-15

### Changed

- Chat, menus, and dialogs follow the operating system more closely: light or dark, native delete confirms, right-click menus, and the window remembers its size.
- Settings and the Chat warning use the same name for the installed AI.

### Fixed

- Search shuts down more carefully so tests are less likely to run out of files.

## 0.3.1 - 2026-08-15

### Added

- Each Shelf has a Think level, Off by default. Think looks through Shelf files a few extra ways. Think harder looks more thoroughly and reads more of each match.

### Changed

- First-run Install an AI leads with why an AI has to be on the machine, that the download can take a few minutes, and that a different one can be chosen.
- Chat is better at finding a file named in the question, and one document can no longer fill the whole answer.
- Installing an AI can pick up if the download stops. Large downloads use more than one connection.

## 0.3.0 - 2026-08-15

Chat can start from a Recipe, take dropped files, and keep a name given to a conversation.

### Added

- Empty Chat shows Recipes. Recipes that use a Shelf stay hidden until one is chosen.
- Files dropped onto Chat, or added with Add files, go on the selected Shelf, the only Shelf, or a new Shelf. Document-name slots in the message fill with those file names.
- Ask in Chat on a Shelf opens Chat with that Shelf selected.
- When the cursor is in a document-name placeholder, Chat lists files on that Shelf. Arrow keys, Return, and Tab pick one. Escape leaves the slot. If the Shelf has no files, nothing appears.
- File menu: New conversation (`⌘N`, or `Ctrl+N` on Windows). View menu: Chat, Shelves, Recipes (`⌘1`, `⌘2`, `⌘3`).
- A conversation can be renamed from the list (pencil or double-click), and exported as Markdown once it has messages.

### Fixed

- Attaching or dropping files no longer fills paste or language slots in a Recipe.
- Shift+Return still inserts a newline when the file list is open.
- A renamed conversation keeps that name after later messages, including when the name is New conversation.

## 0.2.28 - 2026-08-15

Screens change a little more smoothly, and Reset Rebost comes back to first run instead of a blank window.

### Added

- Onboarding, drawers, and download bars move briefly when something changes. Reduce motion on the operating system turns that off.

### Fixed

- Reset Rebost (type DELETE) opens the first-run screen again. It was leaving a blank white window.

## 0.2.27 - 2026-08-15

Chat gets ready faster, and replies follow the memory available on the machine.

### Changed

- First chat spends less time getting ready. The AI warms up when the composer is about to be used, not on every screen.
- Shelves, Recipes, and Settings load when they are opened, so Chat is available sooner.
- Replies use more of the machine when there is plenty of memory, and smaller steps when memory is tight.
- On some Windows PCs, the first chat may download a faster way to run the AI. If that does not work, Rebost keeps the copy that came with the installer.
- Follow-up questions on the same Shelf keep House rules and Shelf context in place, so the AI spends less time re-reading them.

### Fixed

- If that extra Windows download never becomes ready, the next try uses the included copy instead of looping.
- After a crash, a leftover AI process is stopped the next time Rebost opens.

## 0.2.26 - 2026-08-14

First release. Rebost is a desktop application that runs an AI on the machine where it is installed, either on its own or against documents placed on a Shelf.

### Added

- Chat runs on that machine. A file can be attached, or a Shelf chosen so answers come from those documents, with citations. Conversation memory applies either way. After idle, the first message may say "Warming up…" while the AI gets ready.
- Shelves: named collections. PDFs, Word files, and spreadsheets can be dragged in, or a folder linked so new files show up. Opening a file shows the text Rebost reads and counts of detected personal information (emails, IBANs, Spanish tax ids). Those counts are not a legal opinion.
- Recipes: saved prompts with `«…»` placeholders. House rules are standing tone and language instructions and stay out of retrieved excerpts.
- An answer can be copied with personal identifiers removed when the reply contains them.
- First-run onboarding: the privacy promise, then an AI that fits the machine. Settings later shows two more suggestions that are not already installed.
- Hugging Face search for other AIs (download counts, original publishers first, newest first). More info on a hit opens a card with publisher, downloads, file, and license. After a download, the check can be skipped and the file used as-is.
- In-app updater: a quiet check on startup, a sidebar cue when a newer version exists, and an Update window that downloads and installs it.
- Settings → Reset Rebost wipes app data, the AI, and caches once DELETE is typed. Shelf folders on disk are kept.
- Show in folder on the document drawer and citation panel.
- Installers for Mac (Apple chip and Intel) and Windows. Each includes what Rebost needs to run the AI on that kind of machine. Windows ARM64 can be built from source. Linux is not a supported platform.
- English UI. Deleting conversations, Recipes, and Shelves asks for a confirm.

### Security

- An AI without a checksum is refused. Names and sources are checked before install. After download, Skip the check and use the file skips that verification.
- Searches and downloads use HTTPS. Chat runs on the machine where Rebost is installed.
- Answers are sanitized before they are shown.
- Open and Show in folder only work for files on a Shelf Rebost knows. Linked-folder scans skip shortcuts that point outside.
- App data on Mac and Linux is readable only by the operating-system user account that installed it.
- Diagnostics do not send log contents into the window.

[Unreleased]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.11...HEAD
[0.8.11]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.10...v0.8.11
[0.8.10]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.9...v0.8.10
[0.8.9]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.8...v0.8.9
[0.8.8]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.6...v0.8.8
[0.8.6]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/Frontierz-AI/Rebost/compare/v0.8.0...v0.8.2
[0.8.0]: https://github.com/Frontierz-AI/Rebost/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/Frontierz-AI/Rebost/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/Frontierz-AI/Rebost/releases/tag/v0.6.0
