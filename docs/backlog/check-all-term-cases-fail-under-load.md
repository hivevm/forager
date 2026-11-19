# Two TERM cases of the check-all self-test fail under a heavy load

**Observed:** while ticket revision-2026-11-18/12 was being worked (2026-11-19), at a natural load
average near 100 on 24 processors. Two cases of `scripts/test-check-all.sh` failed: "and returns
once a check that cleans up on TERM has done so" and "and no check it started outlives it". Run
alone three times afterwards, the self-test passed, failed one of its 33 cases, and passed. The
cause of the ticket's flake, a job bash kept after the plain `wait` with which `settle` waited
before that ticket's fix, is not theirs. The TERM case stops the runner while its first checks
run. With the default of one check per processor, fewer than the 56 checks before the settle line
on 24 processors, the runner has not reached that line then.

**Where:** the TERM case of `scripts/test-check-all.sh` (the fixture `stopped`), `stop` and the
subshell's TERM trap of `run` in `scripts/check-all.sh`. The fix of a7e10c2b addressed an earlier
flake of the same case.

**Unconfirmed leads:**

- The cleaning stub waits in `sleep 5 & wait $!` and then ends in `exec sleep 60`, which a TERM
  kills without the cleanup. A TERM that reaches the stub more than 5 s after it set its trap
  leaves no `cleaned` file.
- The case gives up waiting for `started` and `trapped` after 300 polls of 0.1 s and sends the
  TERM anyway. A cleaning stub that has not set its trap by then dies without the cleanup.
- Nothing above explains a stub that outlives the runner; no reproduction of that failure exists.

**What a change weighs:** a reproduction under a known load comes first, as for the ticket, then
a fix of the cause it shows. No retry, longer sleep, or looser bound passes for a fix.
