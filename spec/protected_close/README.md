# Protected worker close

The model covers save and runtime workers. `ShutdownAck` represents a positive
`safe_to_shutdown` reply handled by `Worker::close`. `TransportLost` maps to
`Worker::fail` and its lifecycle transition to `Lost`. `ChildExits` maps to the
positive `try_wait` observation in `Worker::can_replace` after the pipe is lost.
`RequestClose` represents the protected portion of `Broker::shutdown`, including
a retry after a failed shutdown call.

`NoCloseWithoutRelease` keeps a running, unacknowledged owner fenced.
`ExitedWorkerCanClose` permits a later close with positive child-exit proof.
The two faulty configurations independently reproduce premature release and
the refusal to close after a retained worker has exited. The read-only search
worker has a separate, force-reap policy and is outside this model.
