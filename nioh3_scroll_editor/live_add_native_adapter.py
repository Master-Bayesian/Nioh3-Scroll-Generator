"""Independent Windows executor using the shared verified live-add plan contract."""
from .live_add_adapter import LiveAddAdapter
from .live_add_native_transport import NativeLiveAddTransport


class NativeLiveAddAdapter(LiveAddAdapter):
    """Reuse version checks, inspection and evidence; replace only the transport."""

    def __init__(self, directory):
        super().__init__(transport=NativeLiveAddTransport(directory))
