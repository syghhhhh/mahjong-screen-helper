//! Test-only directory ownership: clock ticks are not unique on Windows.
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub fn create(prefix: &str, clock: u128) -> io::Result<PathBuf> {
    create_in(&std::env::temp_dir(), prefix, clock, &NEXT_ID)
}

fn create_in(root: &Path, prefix: &str, clock: u128, sequence: &AtomicU64) -> io::Result<PathBuf> {
    for _ in 0..64 {
        let id = sequence.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!("{prefix}-{}-{clock}-{id}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "test directory collision limit reached",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_paths_are_never_reused_or_removed() {
        let root = create("msh-dir-contract", 0).unwrap();
        let occupied = root.join(format!("child-{}-7-0", std::process::id()));
        fs::create_dir(&occupied).unwrap();
        let sentinel = occupied.join("keep.txt");
        fs::write(&sentinel, b"owned by another test").unwrap();
        let created = create_in(&root, "child", 7, &AtomicU64::new(0)).unwrap();
        assert_ne!(created, occupied);
        assert_eq!(fs::read(&sentinel).unwrap(), b"owned by another test");
        fs::remove_dir(created).unwrap();
        fs::remove_file(sentinel).unwrap();
        fs::remove_dir(occupied).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn identical_clock_ticks_are_unique_across_simultaneous_creators() {
        let barrier = std::sync::Barrier::new(16);
        let paths = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        create("msh-dir-parallel", 0).unwrap()
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            paths.iter().collect::<std::collections::HashSet<_>>().len(),
            16
        );
        for path in paths {
            fs::remove_dir(path).unwrap();
        }
    }

    #[test]
    fn collision_budget_is_finite_and_other_io_errors_return_immediately() {
        let root = create("msh-dir-budget", 0).unwrap();
        let sequence = AtomicU64::new(0);
        let paths: Vec<_> = (0..64)
            .map(|id| {
                let path = root.join(format!("child-{}-9-{id}", std::process::id()));
                fs::create_dir(&path).unwrap();
                path
            })
            .collect();
        assert_eq!(
            create_in(&root, "child", 9, &sequence).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(sequence.load(Ordering::Relaxed), 64);
        let sequence = AtomicU64::new(0);
        assert_eq!(
            create_in(&root.join("missing"), "child", 9, &sequence)
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(sequence.load(Ordering::Relaxed), 1);
        for path in paths {
            fs::remove_dir(path).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }
}
