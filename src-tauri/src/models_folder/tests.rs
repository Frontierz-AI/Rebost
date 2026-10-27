//! Where AIs are kept: space, relations, preflight, and the move matrix.
//!
//! Every move case ends by checking the rule: no AI's bytes became
//! unreachable, whatever happened in between.

use super::mover::{redundant, stage_with, Strategy};
use super::*;
use std::path::Path;

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), bytes).unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn files(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn disk_is_the_longest_matching_mount() {
    let disks = [
        (Path::new("/"), 10),
        (Path::new("/Volumes/Data"), 500),
        (Path::new("/Volumes/Data Two"), 7),
    ];
    let pick = |path: &str| disk_for(Path::new(path), disks.iter().copied());
    assert_eq!(
        pick("/Volumes/Data/AI"),
        Some((Path::new("/Volumes/Data"), 500))
    );
    assert_eq!(
        pick("/Volumes/Data Two/AI"),
        Some((Path::new("/Volumes/Data Two"), 7))
    );
    assert_eq!(pick("/Users/me/Library"), Some((Path::new("/"), 10)));
    // A sibling whose name only starts the same is a different disk.
    assert_eq!(pick("/Volumes/DataX"), Some((Path::new("/"), 10)));
    assert_eq!(disk_for(Path::new("/x"), []), None);
}

#[cfg(windows)]
#[test]
fn windows_drive_letters_match_in_any_case() {
    let disks = [(Path::new("C:\\"), 3), (Path::new("D:\\"), 900)];
    let pick = |path: &str| disk_for(Path::new(path), disks.iter().copied()).map(|d| d.1);
    assert_eq!(pick(r"D:\Rebost\Modelos"), Some(900));
    assert_eq!(pick(r"d:\Rebost\Modelos"), Some(900));
    assert_eq!(pick(r"C:\Users\me\AppData\Roaming"), Some(3));
}

#[test]
fn free_space_measures_the_nearest_existing_folder() {
    let dir = tempfile::tempdir().unwrap();
    let not_yet = dir.path().join("a").join("b");
    // Free space moves while other programs write, so compare disks, not bytes.
    assert!(free_space(&not_yet).is_some_and(|free| free > 0));
    assert_eq!(existing_ancestor(&not_yet), dir.path());
    assert!(same_disk(dir.path(), &not_yet));
}

#[test]
fn room_check_never_blocks_on_unknown_space() {
    assert!(has_room(Some(10), 10));
    assert!(!has_room(Some(9), 10));
    assert!(has_room(None, u64::MAX));
}

#[test]
fn plain_path_drops_the_verbatim_prefix() {
    assert_eq!(
        plain_path(PathBuf::from(r"\\?\D:\Rebost\Modelos")),
        PathBuf::from(r"D:\Rebost\Modelos")
    );
    assert_eq!(
        plain_path(PathBuf::from(r"\\?\UNC\nas\share\AI")),
        PathBuf::from(r"\\nas\share\AI")
    );
    assert_eq!(
        plain_path(PathBuf::from("/Volumes/Data/AI")),
        PathBuf::from("/Volumes/Data/AI")
    );
}

#[test]
fn plan_sorts_files_by_where_they_are() {
    let dir = tempfile::tempdir().unwrap();
    let (from, to) = (dir.path().join("from"), dir.path().join("to"));
    write(&from, "a.gguf", b"aaaa");
    write(&to, "b.gguf", b"bb");
    let plan = MovePlan::new(&files(&["a.gguf", "b.gguf", "c.gguf"]), &from, &to);
    assert_eq!(plan.to_move, [("a.gguf".to_string(), 4)]);
    assert_eq!(plan.already_there, ["b.gguf"]);
    assert_eq!(plan.missing, ["c.gguf"]);
    assert_eq!(plan.bytes(), 4);
    assert_eq!(plan.bytes_needed(true), 0);
    assert_eq!(plan.bytes_needed(false), 4 + SPARE_BYTES);
    let nothing = MovePlan::new(&files(&["b.gguf"]), &from, &to);
    assert_eq!(nothing.bytes_needed(false), 0);
}

#[test]
fn preflight_spots_folders_that_cannot_hold_the_ais() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
    paths.ensure().unwrap();
    let current = paths.default_models_dir();
    write(&current, "a.gguf", b"aaaa");
    let installed = files(&["a.gguf"]);
    let check = |to: &Path, missing: bool| {
        Preflight::check(&paths, to, &installed, &current, missing).problem
    };

    let drive = dir.path().join("Drive");
    std::fs::create_dir(&drive).unwrap();
    let ok = Preflight::check(&paths, &drive, &installed, &current, false);
    assert_eq!(ok.problem, None);
    assert_eq!(ok.plan.to_move, [("a.gguf".to_string(), 4)]);
    assert!(ok.free.is_some());

    assert_eq!(check(&current, false), None, "staying put is fine");
    assert_eq!(
        check(&paths.shelves_dir(), false),
        Some(FolderProblem::InsideAppData)
    );
    assert_eq!(
        check(&dir.path().join("Unplugged"), false),
        Some(FolderProblem::NotWritable)
    );
    // The current folder is gone and the picked one is empty.
    let gone = dir.path().join("Gone");
    let lost = Preflight::check(&paths, &drive, &installed, &gone, true);
    assert_eq!(lost.problem, Some(FolderProblem::NoAisThere));
    // ...but a folder that holds them is the fix.
    write(&drive, "a.gguf", b"aaaa");
    let found = Preflight::check(&paths, &drive, &installed, &gone, true);
    assert_eq!(found.problem, None);
    assert_eq!(found.plan.already_there, ["a.gguf"]);
}

#[test]
fn preflight_recreates_only_the_default_folder() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
    let gone = dir.path().join("Gone");
    let default = paths.default_models_dir();
    let back = Preflight::check(&paths, &default, &files(&["a.gguf"]), &gone, true);
    assert_eq!(
        back.problem, None,
        "going back is allowed even with nothing there"
    );
    assert_eq!(back.plan.missing, ["a.gguf"]);
    assert!(default.is_dir());
}

#[test]
fn preflight_spots_a_different_file_with_an_ai_name() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
    paths.ensure().unwrap();
    let current = paths.default_models_dir();
    write(&current, "a.gguf", b"aaaa");
    let drive = dir.path().join("Drive");
    write(&drive, "a.gguf", b"someone else's");
    let check = Preflight::check(&paths, &drive, &files(&["a.gguf"]), &current, false);
    assert_eq!(check.problem, Some(FolderProblem::NameTaken));
    // Same size could be the same AI: the move compares it then.
    write(&drive, "a.gguf", b"aaaa");
    let check = Preflight::check(&paths, &drive, &files(&["a.gguf"]), &current, false);
    assert_eq!(check.problem, None);
}
#[test]
fn writable_check_needs_an_existing_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(check_writable(dir.path()), Ok(()));
    assert!(names(dir.path()).is_empty(), "probe file is removed");
    assert_eq!(
        check_writable(&dir.path().join("unplugged")),
        Err(FolderProblem::NotWritable)
    );
}

#[cfg(unix)]
#[test]
fn writable_check_spots_a_read_only_folder() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = check_writable(&locked);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result, Err(FolderProblem::NotWritable));
}

// ---- Folder relations ------------------------------------------------------

fn case_insensitive(dir: &Path) -> bool {
    let probe = dir.join("CaseProbe");
    std::fs::create_dir_all(&probe).unwrap();
    let insensitive = dir.join("caseprobe").exists();
    std::fs::remove_dir(&probe).unwrap();
    insensitive
}

#[test]
fn folder_relations_follow_identity_not_spelling() {
    use Relation::*;
    let dir = tempfile::tempdir().unwrap();
    let models = dir.path().join("Models");
    let inner = models.join("Inner");
    let other = dir.path().join("Other");
    for folder in [&inner, &other] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let trailing = PathBuf::from(format!("{}{}", models.display(), std::path::MAIN_SEPARATOR));
    let mut cases = vec![
        ("same path", models.clone(), models.clone(), Same),
        ("trailing separator", models.clone(), trailing, Same),
        ("dot-dot spelling", models.clone(), inner.join(".."), Same),
        ("inside", inner.clone(), models.clone(), Nested),
        ("holds", models.clone(), inner.clone(), Nested),
        ("separate", models.clone(), other.clone(), Separate),
        (
            "unreachable",
            dir.path().join("Gone"),
            models.clone(),
            Separate,
        ),
        (
            "unreachable inside",
            models.join("Gone"),
            models.clone(),
            Nested,
        ),
    ];
    if case_insensitive(dir.path()) {
        cases.push((
            "other casing",
            models.clone(),
            dir.path().join("MODELS"),
            Same,
        ));
    }
    #[cfg(unix)]
    {
        let alias = dir.path().join("Alias");
        std::os::unix::fs::symlink(&models, &alias).unwrap();
        cases.push(("symlink", alias.clone(), models.clone(), Same));
        cases.push((
            "inside through a symlink",
            alias.join("Inner"),
            models.clone(),
            Nested,
        ));
    }
    #[cfg(windows)]
    {
        let junction = dir.path().join("Junction");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&models)
            .output()
            .is_ok_and(|out| out.status.success());
        if made {
            cases.push(("junction", junction.clone(), models.clone(), Same));
            cases.push((
                "inside through a junction",
                junction.join("Inner"),
                models.clone(),
                Nested,
            ));
        }
    }
    for (name, a, b, want) in cases {
        assert_eq!(
            relation(&a, &b),
            want,
            "{name}: {} vs {}",
            a.display(),
            b.display()
        );
    }
}

#[test]
fn preflight_refuses_a_folder_inside_or_around_the_current_one() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
    let current = dir.path().join("Drive").join("AI");
    write(&current, "a.gguf", b"aaaa");
    let inside = current.join("More");
    std::fs::create_dir(&inside).unwrap();
    let installed = files(&["a.gguf"]);
    for to in [&inside, &dir.path().join("Drive")] {
        let check = Preflight::check(&paths, to, &installed, &current, false);
        assert_eq!(
            check.problem,
            Some(FolderProblem::Nested),
            "{}",
            to.display()
        );
    }
}

#[cfg(unix)]
#[test]
fn rebosts_own_folder_under_another_path_is_still_rebosts() {
    let dir = tempfile::tempdir().unwrap();
    let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
    paths.ensure().unwrap();
    let alias = dir.path().join("Alias");
    std::os::unix::fs::symlink(paths.default_models_dir(), &alias).unwrap();
    let gone = dir.path().join("Gone");
    let back = Preflight::check(&paths, &alias, &files(&["a.gguf"]), &gone, true);
    assert_eq!(back.problem, None);
}

// ---- The rule at removal time ---------------------------------------------

#[test]
fn redundant_needs_two_regular_files_of_one_size_in_two_places() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("A"), dir.path().join("B"));
    write(&a, "x.gguf", b"same");
    write(&b, "x.gguf", b"same");
    write(&b, "short.gguf", b"sam");
    write(&a, "short.gguf", b"same");
    write(&a, "y.gguf", b"same");
    let cases = vec![
        (
            "copy in another folder",
            a.join("x.gguf"),
            b.join("x.gguf"),
            true,
        ),
        (
            "other size",
            a.join("short.gguf"),
            b.join("short.gguf"),
            false,
        ),
        ("same folder", a.join("x.gguf"), a.join("y.gguf"), false),
        ("the file itself", a.join("x.gguf"), a.join("x.gguf"), false),
        (
            "nothing to keep",
            a.join("x.gguf"),
            b.join("none.gguf"),
            false,
        ),
        (
            "nothing to remove",
            a.join("none.gguf"),
            b.join("x.gguf"),
            false,
        ),
    ];
    for (name, remove, keep, want) in cases {
        assert_eq!(redundant(&remove, &keep), want, "{name}");
    }
    #[cfg(unix)]
    {
        let alias = dir.path().join("Alias");
        std::os::unix::fs::symlink(&a, &alias).unwrap();
        std::os::unix::fs::symlink(a.join("x.gguf"), b.join("link.gguf")).unwrap();
        assert!(
            !redundant(&a.join("x.gguf"), &alias.join("x.gguf")),
            "same folder, other spelling"
        );
        assert!(
            !redundant(&a.join("x.gguf"), &b.join("link.gguf")),
            "a link is never a copy"
        );
        assert!(
            !redundant(&b.join("link.gguf"), &a.join("x.gguf")),
            "nor removed as one"
        );
    }
}

// ---- Moves -----------------------------------------------------------------

struct Fixture {
    _dir: tempfile::TempDir,
    base: PathBuf,
    from: PathBuf,
    to: PathBuf,
    models: Vec<(String, Vec<u8>)>,
}

/// AIs in `From`, an empty `To`, and app data for the journal.
fn fixture(models: &[(&str, &[u8])]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("appdata");
    let (from, to) = (dir.path().join("From"), dir.path().join("To"));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&to).unwrap();
    for (name, bytes) in models {
        write(&from, name, bytes);
    }
    Fixture {
        base,
        from,
        to,
        models: models
            .iter()
            .map(|(n, b)| (n.to_string(), b.to_vec()))
            .collect(),
        _dir: dir,
    }
}

impl Fixture {
    fn plan(&self) -> MovePlan {
        let installed: Vec<String> = self.models.iter().map(|(n, _)| n.clone()).collect();
        MovePlan::new(&installed, &self.from, &self.to)
    }

    fn stage(&self, strategy: Strategy) -> anyhow::Result<Mover> {
        self.stage_watching(strategy, &mut |_, _| {})
    }

    fn stage_watching(
        &self,
        strategy: Strategy,
        progress: &mut dyn FnMut(u64, u64),
    ) -> anyhow::Result<Mover> {
        stage_with(
            &self.plan(),
            &journal_path(&self.base),
            strategy,
            progress,
            &|| false,
        )
    }

    fn journal_left(&self) -> bool {
        journal_path(&self.base).exists()
    }

    /// The rule's promise: every AI is still whole in one of the folders.
    fn assert_no_ai_lost(&self) {
        for (name, bytes) in &self.models {
            let whole = [&self.from, &self.to]
                .iter()
                .any(|dir| std::fs::read(dir.join(name)).is_ok_and(|b| &b == bytes));
            assert!(whole, "{name} was lost");
        }
    }
}

const BIG: usize = 8 * 1024 * 1024 + 3;

#[test]
fn a_move_keeps_the_originals_until_finished_on_one_drive_or_two() {
    let big = vec![7u8; BIG];
    // `Copy` stands in for another drive; `LinkOrCopy` links on this one.
    for strategy in [Strategy::LinkOrCopy, Strategy::Copy] {
        let fx = fixture(&[("a.gguf", &big), ("vision-1.gguf", b"vv")]);
        let mut last = (0, 0);
        let mover = fx
            .stage_watching(strategy, &mut |d, t| last = (d, t))
            .unwrap();
        assert_eq!(last, (fx.plan().bytes(), fx.plan().bytes()), "{strategy:?}");
        assert_eq!(names(&fx.from), ["a.gguf", "vision-1.gguf"], "{strategy:?}");
        assert_eq!(names(&fx.to), ["a.gguf", "vision-1.gguf"], "{strategy:?}");
        assert!(
            fx.journal_left(),
            "the journal covers a crash before the switch"
        );
        assert_eq!(mover.finish(), 0);
        assert!(names(&fx.from).is_empty(), "{strategy:?}");
        assert!(!fx.journal_left());
        assert_eq!(std::fs::read(fx.to.join("a.gguf")).unwrap(), big);
    }
}

#[test]
fn the_only_ai_survives_every_failed_move() {
    let only: &[u8] = b"the only copy";
    #[allow(clippy::type_complexity)]
    let failures: Vec<(&str, Box<dyn Fn(&Fixture)>, bool)> = vec![
        ("nothing in the way", Box::new(|_| {}), false),
        (
            "another file, same size",
            Box::new(|fx| write(&fx.to, "a.gguf", b"not the same!")),
            true,
        ),
        (
            "another file, other size",
            Box::new(|fx| write(&fx.to, "a.gguf", b"other")),
            true,
        ),
        (
            "a folder with its name",
            Box::new(|fx| std::fs::create_dir(fx.to.join("a.gguf")).unwrap()),
            true,
        ),
    ];
    for strategy in [Strategy::LinkOrCopy, Strategy::Copy] {
        for (name, setup, fails) in &failures {
            let fx = fixture(&[("a.gguf", only)]);
            setup(&fx);
            let before = names(&fx.to);
            match fx.stage(strategy) {
                Ok(mover) => {
                    assert!(!fails, "{name} should fail");
                    mover.roll_back();
                }
                Err(error) => {
                    assert!(*fails, "{name}: {error:#}");
                    assert!(
                        error.to_string().contains(FolderProblem::NameTaken.code()),
                        "{name}"
                    );
                }
            }
            assert_eq!(
                std::fs::read(fx.from.join("a.gguf")).unwrap(),
                only,
                "{name}"
            );
            assert_eq!(
                names(&fx.to),
                before,
                "{name}: only what the move made is removed"
            );
            assert!(!fx.journal_left(), "{name}");
        }
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_in_the_new_folder_is_never_a_copy() {
    let fx = fixture(&[("a.gguf", b"only copy")]);
    std::os::unix::fs::symlink(fx.from.join("a.gguf"), fx.to.join("a.gguf")).unwrap();
    let paths = crate::paths::Paths::new(&fx.base);
    let installed = files(&["a.gguf"]);
    assert_eq!(
        Preflight::check(&paths, &fx.to, &installed, &fx.from, false).problem,
        Some(FolderProblem::NameTaken)
    );
    let error = fx.stage(Strategy::LinkOrCopy).unwrap_err();
    assert!(error.to_string().contains(FolderProblem::NameTaken.code()));
    fx.assert_no_ai_lost();
    assert_eq!(std::fs::read(fx.from.join("a.gguf")).unwrap(), b"only copy");
}

#[cfg(unix)]
#[test]
fn the_same_folder_under_another_path_never_loses_a_file() {
    // Callers refuse this (`relation`); the mover must hold on its own.
    let fx = fixture(&[("a.gguf", b"only copy")]);
    let alias = fx.base.parent().unwrap().join("Alias");
    std::os::unix::fs::symlink(&fx.from, &alias).unwrap();
    let plan = MovePlan::new(&files(&["a.gguf"]), &fx.from, &alias);
    let mover = stage_with(
        &plan,
        &journal_path(&fx.base),
        Strategy::Copy,
        &mut |_, _| {},
        &|| false,
    )
    .unwrap();
    assert_eq!(mover.finish(), 1, "kept, not removed");
    assert_eq!(std::fs::read(fx.from.join("a.gguf")).unwrap(), b"only copy");
}

#[test]
fn files_already_in_the_new_folder_are_reused_and_never_rolled_back() {
    // A hard link or an identical copy left by an earlier, interrupted move.
    for leftover in ["hard link", "identical copy"] {
        let fx = fixture(&[("a.gguf", b"weights")]);
        match leftover {
            "hard link" => {
                std::fs::hard_link(fx.from.join("a.gguf"), fx.to.join("a.gguf")).unwrap()
            }
            _ => write(&fx.to, "a.gguf", b"weights"),
        }
        fx.stage(Strategy::LinkOrCopy).unwrap().roll_back();
        assert_eq!(names(&fx.to), ["a.gguf"], "{leftover}: not ours to remove");
        assert_eq!(names(&fx.from), ["a.gguf"], "{leftover}");

        assert_eq!(
            fx.stage(Strategy::LinkOrCopy).unwrap().finish(),
            0,
            "{leftover}"
        );
        assert!(names(&fx.from).is_empty(), "{leftover}");
        assert_eq!(std::fs::read(fx.to.join("a.gguf")).unwrap(), b"weights");
    }
}

#[test]
fn a_partial_download_is_left_alone() {
    let fx = fixture(&[("a.gguf", b"weights")]);
    write(&fx.from, "b.part", b"half a download");
    fx.stage(Strategy::Copy).unwrap().finish();
    assert_eq!(names(&fx.from), ["b.part"]);
    assert_eq!(names(&fx.to), ["a.gguf"]);
}

/// Accepts `room` bytes, then reports a full disk.
struct FullDisk {
    room: usize,
}

impl std::io::Write for FullDisk {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.room == 0 {
            return Err(std::io::ErrorKind::StorageFull.into());
        }
        let n = buf.len().min(self.room);
        self.room -= n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_full_disk_mid_copy_is_reported_and_undone() {
    let mut input: &[u8] = &[1u8; 4096];
    let error = mover::copy_stream(
        &mut input,
        &mut FullDisk { room: 1000 },
        &mut |_| {},
        &|| false,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), FolderProblem::NotEnoughSpace.code());

    // Any failure after the first file undoes the first one too.
    let fx = fixture(&[("a.gguf", b"first"), ("b.gguf", b"second")]);
    write(&fx.to, "b.gguf", b"in the way");
    assert!(fx.stage(Strategy::Copy).is_err());
    assert_eq!(names(&fx.to), ["b.gguf"]);
    fx.assert_no_ai_lost();
}

#[test]
fn a_cancelled_move_is_undone() {
    let fx = fixture(&[("a.gguf", &vec![1u8; BIG])]);
    let plan = fx.plan();
    let error = stage_with(
        &plan,
        &journal_path(&fx.base),
        Strategy::Copy,
        &mut |_, _| {},
        &|| true,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "cancelled");
    assert!(names(&fx.to).is_empty(), "no half-written copy left");
    assert!(!fx.journal_left());
    fx.assert_no_ai_lost();
}

#[cfg(unix)]
#[test]
fn the_old_drive_vanishing_mid_move_keeps_the_new_copies() {
    let fx = fixture(&[
        ("a.gguf", b"first"),
        ("b.gguf", b"second"),
        ("c.gguf", b"third"),
    ]);
    let saved = fx.models.clone();
    let from = fx.from.clone();
    // Unplug the old drive once the first file is across.
    let mut progress = |done: u64, _| {
        if done >= 5 && from.exists() {
            std::fs::remove_dir_all(&from).unwrap();
        }
    };
    assert!(fx.stage_watching(Strategy::Copy, &mut progress).is_err());
    // The undo can't prove the originals are still there, so the copies stay.
    assert_eq!(std::fs::read(fx.to.join("a.gguf")).unwrap(), saved[0].1);
    assert!(!fx.journal_left());
}

#[cfg(unix)]
#[test]
fn the_new_drive_vanishing_mid_move_keeps_the_originals() {
    let fx = fixture(&[("a.gguf", &vec![2u8; BIG]), ("b.gguf", b"second")]);
    let to = fx.to.clone();
    let mut progress = |done: u64, _| {
        if done > 0 && to.exists() {
            std::fs::remove_dir_all(&to).unwrap();
        }
    };
    assert!(fx.stage_watching(Strategy::Copy, &mut progress).is_err());
    assert_eq!(names(&fx.from), ["a.gguf", "b.gguf"]);
    fx.assert_no_ai_lost();
}

#[test]
fn a_restart_mid_move_finishes_or_undoes_it() {
    for strategy in [Strategy::LinkOrCopy, Strategy::Copy] {
        for switched in [false, true] {
            let fx = fixture(&[("a.gguf", b"weights"), ("b.gguf", b"more")]);
            let crashed = fx.stage(strategy).unwrap();
            drop(crashed); // the app stopped here
            if !switched {
                // ...or earlier: halfway through copying `b`, before the
                // journal recorded it.
                let path = journal_path(&fx.base);
                let mut journal: mover::Journal = crate::paths::read_json(&path).unwrap();
                journal.placed.pop();
                crate::paths::write_json(&path, &journal).unwrap();
                std::fs::remove_file(fx.to.join("b.gguf")).unwrap();
                write(&fx.to, ".b.gguf.rebost-moving", b"ha");
            }
            let current = if switched { &fx.to } else { &fx.from };
            let outcome = recover(&fx.base, current).unwrap();
            let case = format!("{strategy:?}, switched: {switched}");
            if switched {
                assert_eq!(outcome, Recovered::Finished { kept: 0 }, "{case}");
                assert!(names(&fx.from).is_empty(), "{case}");
                assert_eq!(names(&fx.to), ["a.gguf", "b.gguf"], "{case}");
            } else {
                assert_eq!(outcome, Recovered::RolledBack, "{case}");
                assert_eq!(names(&fx.from), ["a.gguf", "b.gguf"], "{case}");
                assert!(names(&fx.to).is_empty(), "{case}");
            }
            fx.assert_no_ai_lost();
            assert!(!fx.journal_left(), "{case}");
            assert_eq!(
                recover(&fx.base, current),
                None,
                "{case}: nothing left to do"
            );
        }
    }
}

#[test]
fn a_folder_holding_app_data_is_fine_one_inside_it_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let paths = crate::paths::Paths::new(home.join("Library").join("io.rebost.desktop"));
    paths.ensure().unwrap();
    let current = dir.path().join("Drive");
    write(&current, "a.gguf", b"aaaa");
    let installed = files(&["a.gguf"]);
    let check = |to: &Path| Preflight::check(&paths, to, &installed, &current, false).problem;
    assert_eq!(
        check(&home),
        None,
        "the home folder holds app data; that's fine"
    );
    assert_eq!(check(paths.base()), Some(FolderProblem::InsideAppData));
    assert_eq!(
        check(&paths.shelves_dir()),
        Some(FolderProblem::InsideAppData)
    );
}
