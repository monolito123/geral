// SPDX-License-Identifier: Apache-2.0

//! NottOS policy models.
//!
//! This crate holds decisions that must be the same wherever they are made:
//! on a build host, in a porting tool, or on the device. It contains no I/O.

pub mod kernel;
pub mod tier;

pub use tier::{assess, Assessment, Descriptor, DescriptorError, PostureItem, State, Tier};
