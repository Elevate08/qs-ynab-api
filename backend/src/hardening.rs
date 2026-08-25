//! Process-level hardening, applied before anything sensitive is in memory.

/// Prevents this process from writing its own memory to disk, and from being
/// attached to by other processes running as the same user.
///
/// The helper holds a YNAB Personal Access Token in plaintext for as long as it
/// takes to seal, unseal, or send it. `Zeroizing` wipes that memory on the way
/// out of every ordinary path, but a fatal signal - a SIGSEGV inside a
/// dependency, an abort - skips destructors entirely and hands the whole
/// address space to the core dump handler. On a systemd desktop that means the
/// token sits in `/var/lib/systemd/coredump` in the clear, which is exactly the
/// plaintext-on-disk outcome the rest of this crate is built to avoid.
///
/// Both calls are advisory: if the kernel refuses either one, the process is no
/// worse off than it was, so neither failure is worth reporting to a user who
/// asked about their budget.
pub fn forbid_memory_disclosure() {
    // No core file, whatever the system or shell default happens to be.
    let no_core = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let _ = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &no_core) };

    // Belt to that braces, and it closes the other door as well: a
    // non-dumpable process cannot be `ptrace`d by a sibling process running as
    // you, which is the other way a token in memory leaves this address space.
    let _ = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    // Runs in the shared test process, which is fine: both settings are
    // one-way hardening that nothing else here depends on being absent.
    #[test]
    fn core_dumps_and_tracing_are_refused() {
        forbid_memory_disclosure();

        let mut limit = libc::rlimit {
            rlim_cur: 1,
            rlim_max: 1,
        };
        assert_eq!(0, unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) });
        assert_eq!(0, limit.rlim_cur, "a core dump could still be written");

        assert_eq!(
            0,
            unsafe { libc::prctl(libc::PR_GET_DUMPABLE) },
            "the process is still dumpable and ptrace-able"
        );
    }
}
