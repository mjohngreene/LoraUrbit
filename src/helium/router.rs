//! Helium Packet Router client
//!
//! The Helium Packet Router uses gRPC to forward packets from hotspots
//! to registered LNS endpoints. In GWMP mode, it speaks the same
//! Semtech UDP protocol as a local gateway — meaning our existing
//! UDP server can receive Helium packets with zero changes.
//!
//! In Packet Router mode (more efficient), it uses a gRPC stream.
//! This module will implement the gRPC client for Phase 4.
//!
//! ## Protocol options:
//! - **GWMP**: Helium Packet Router sends Semtech UDP to our bind address
//!   - Pro: Reuses existing UDP server code, zero changes needed
//!   - Con: Less efficient, no streaming
//! - **Packet Router (gRPC)**: Direct streaming connection
//!   - Pro: More efficient, bidirectional, supports downlinks
//!   - Con: Requires protobuf/gRPC setup
//!
//! For Phase 4 MVP, we'll use GWMP mode (our UDP server already handles it).
//! gRPC Packet Router mode is a Phase 5 optimization.
//!
//! Reference: https://github.com/helium/gateway-rs

// Phase 4: Implement gRPC Packet Router client
// Will use helium/proto definitions and tonic for gRPC
