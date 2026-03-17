# Report: Breez SDK with LDK Node

## Executive Summary

The project has transformed the Breez SDK from a Greenlight-dependent architecture into a standalone Lightning implementation powered by LDK Node. Over the course of development, a production-grade foundation was deliverd with complete payment functionality, robust backup and recovery systems, comprehensive developer tooling, and automated quality assurance. The SDK now supports the full lifecycle of Lightning operations from node initialization through payment sending, channel management, and disaster recovery.

## Research and Technology Selection
The project began with extensive research into the Breez SDK and Lightning Development Kit stack, specifically evaluating LDK Node, Versioned Storage Service (VSS), and Rapid Gossip Sync (RGS) as core infrastructure components. This investigation identified what would be required to embed a full Lightning node directly into the SDK rather than relying on external custodial infrastructure. Limitations and concerns discovered during this phase were systematically documented and communicated to upstream projects, contributing to ecosystem improvements.

Based on this analysis, LDK Node was selected as the Lightning backend for its balance of functionality and embeddability, with VSS chosen to provide remote encrypted storage of Lightning node state. This architecture enables non-custodial operation without requiring users to manage their own infrastructure.

## Prototype Development and Validation

The initial prototype phase focused on proving core technical concepts through working implementation. LDK Node was successfully embedded into the Breez SDK architecture, followed by comprehensive testing on both Regtest and Mainnet networks. Validation covered all fundamental Lightning operations: sending and receiving BOLT11 invoices, sending BOLT12 offers, receiving payments with Just-In-Time channel creation, bidirectional swaps between on-chain and Lightning funds, and full LNURL functionality.

During prototype development, gaps in the LDK stack were identified and addressed through direct implementation work. Additionally, a containerized Regtest environment was created to streamline developer onboarding. This environment provides one-command setup including Lightning nodes, Breez services, an LSP implementing LSPS2 protocol, RGS, and VSS—enabling complete SDK functionality testing without external dependencies.

The prototype code lives here: <https://github.com/andrei-21/breez-sdk/tree/prototype>.

## Architecture Refactoring and Greenlight Deprecation

A significant restructuring effort was undertaken to transition the SDK from its historical Greenlight foundation. The refactoring proceeded through two distinct phases.

Initially, the SDK was re-architected to support dual backends through feature flags, with generic interfaces cleaned of Greenlight-specific assumptions. This allowed parallel operation while the LDK implementation matured.

Subsequently, following Breez's strategic decision to deprecate Greenlight entirely, all Greenlight code, dependencies, and feature flags were completely removed. The SDK now presents a unified, simplified architecture built exclusively around LDK Node patterns.

## Delivered LDK Implementation

The current implementation provides comprehensive Lightning node functionality:

Node Lifecycle Management

- Basic node startup and initialization
- Locking mechanism preventing concurrent node access
- Node restoration from encrypted remote backup

Payment Operations

- Receiving payments via BOLT11 invoices
- Sending payments to BOLT11 invoices
- Sending spontaneous payments (keysend)
- Receiving payments with automatic JIT channel creation
- Receiving normal payments through existing channels

Channel Management

- LSP integration for inbound liquidity
- Peer channel closure

Node Information

- Basic node-state queries and status information

## Storage Architecture

A mirroring store system was developed and merged to provide resilient data management. This component implements local-first storage for LDK Node with transparent encrypted remote backup to VSS. The design ensures operational continuity during connectivity interruptions while protecting against device loss through encrypted state preservation.

## Developer Infrastructure and Quality Assurance

Two major infrastructure investments support ongoing development and reliability:

### Regtest Environment
The containerized development environment enables rapid local testing of complete SDK functionality including BOLT11 and BOLT12 payments, JIT channels, swaps, and LSP interactions without external network dependencies or cost.

### Automated Testing
A Testcontainers-based integration testing framework was prototyped, then fully implemented and integrated into continuous integration. All developed features are now automatically tested on every change, with tests running in CI covering node operations, payment flows, and channel management.

## Outstanding Work and Blockers

Several items remain incomplete, blocking production deployment:

Missing Core Features

- On-chain swaps (both directions: to Lightning and from Lightning)
- Paying BOLT12 offers (PR: <https://github.com/breez/breez-sdk-ldk/pull/54>)

External Dependencies

- LSP Integration: No LSPS5-compatible LSP is currently available. Olympus from Zeus has been identified as the intended partner but LSPS5 implementation is pending, with possible availability in Q2 2025.
- VSS Service: Deployment and operation of the VSS service by Breez infrastructure team requires internal coordination with undefined timeline.

Technical Debt

- Final refactoring to simplify the SDK model for cleaner LDK Node integration remains ongoing.
