/// Sync module — reserved for post-MVP.
///
/// The data model (UUIDs, timestamps, `synced` flag) is designed
/// from the start to support sync. This module will house:
///
/// - Device pairing / token exchange
/// - Transcript delta computation
/// - Conflict resolution (last-write-wins → CRDT upgrade path)
/// - Transport layer (iCloud Drive / Google Drive file sync, or custom P2P)

// Nothing to implement in MVP — placeholder to keep the module tree intact.
