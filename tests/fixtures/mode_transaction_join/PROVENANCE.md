# Mode-transaction live fixture

This fixture preserves the validated read-only PC v2.01 capture
`mode-transaction-join-86872488-expedition-a-20260913`. It observed one owner-
reported one-person expedition transaction for seed `86872488` through the
parameterized-session producer, queue consumer, generator return, and
materializer input.

The target executable SHA-256 was
`4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159`.
The capture used VEH interface 2 and four owned read-only execute breakpoints.
Cleanup was verified with empty owned and global breakpoint inventories.

This fixture does not establish a native UI mode enum, physical-actor join,
persistent-task join, all-writers proof, or product oracle.
