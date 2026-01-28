// Copyright © 2019 Intel Corporation
// Copyright 2023 Rivos, Inc.
// Copyright 2024 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

use vm_memory::{GuestAddress, GuestMemory, GuestMemoryError};

pub mod aml;
pub mod dsdt;
pub mod fadt;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod xsdt;

pub use aml::Aml;
pub use dsdt::Dsdt;
pub use fadt::Fadt;
pub use madt::Madt;
pub use mcfg::Mcfg;
pub use rsdp::Rsdp;
pub use xsdt::Xsdt;
use zerocopy::little_endian::{U32, U64};
use zerocopy::{Immutable, IntoBytes};

// This is the creator ID that we will embed in ACPI tables that are created using this crate.
const FC_ACPI_CREATOR_ID: [u8; 4] = *b"FCAT";
// This is the created ID revision that we will embed in ACPI tables that are created using this
// crate.
const FC_ACPI_CREATOR_REVISION: u32 = 0x20240119;

/// Calculate ACPI checksum for the given byte slices
///
/// The checksum is calculated such that the sum of all bytes including
/// the checksum byte equals zero when wrapped in u8 arithmetic.
///
/// # Performance
/// Optimized to minimize iterator overhead and use efficient wrapping arithmetic
#[inline]
fn checksum(buf: &[&[u8]]) -> u8 {
    let sum = buf
        .iter()
        .flat_map(|slice| slice.iter().copied())
        .fold(0u8, u8::wrapping_add);
    sum.wrapping_neg()
}

#[derive(Debug, thiserror::Error, displaydoc::Display)]
pub enum AcpiError {
    /// Guest memory error: {0}
    GuestMemory(#[from] GuestMemoryError),
    /// Invalid guest address
    InvalidGuestAddress,
    /// Invalid register size
    InvalidRegisterSize,
}

/// Result type for ACPI operations
pub type Result<T> = std::result::Result<T, AcpiError>;

/// Generic Address Structure (GAS) - ACPI type representing memory/IO addresses
///
/// This structure is used throughout ACPI tables to describe register locations
/// in various address spaces (System Memory, System I/O, PCI Configuration Space, etc.)
///
/// # Layout
/// The structure is packed (no padding) and follows the ACPI specification exactly.
///
/// # Examples
/// ```ignore
/// use acpi_tables::GenericAddressStructure;
///
/// // System Memory address at 0x1000
/// let gas = GenericAddressStructure::new(0, 32, 0, 3, 0x1000);
/// ```
#[repr(C, packed)]
#[derive(IntoBytes, Immutable, Clone, Copy, Debug, Default)]
pub struct GenericAddressStructure {
    /// Address space where the register exists (0=System Memory, 1=System I/O, etc.)
    pub address_space_id: u8,
    /// Size in bits of the register
    pub register_bit_width: u8,
    /// Bit offset of the register within the address
    pub register_bit_offset: u8,
    /// Access size (0=Undefined, 1=Byte, 2=Word, 3=DWord, 4=QWord)
    pub access_size: u8,
    /// 64-bit address of the register
    pub address: U64,
}

impl GenericAddressStructure {
    pub fn new(
        address_space_id: u8,
        register_bit_width: u8,
        register_bit_offset: u8,
        access_size: u8,
        address: u64,
    ) -> Self {
        Self {
            address_space_id,
            register_bit_width,
            register_bit_offset,
            access_size,
            address: U64::new(address),
        }
    }
}

/// System Descriptor Table Header
///
/// This is the standard header included at the beginning of all ACPI System Descriptor Tables
/// (XSDT, FADT, MADT, etc.). It contains table identification and checksum information.
///
/// # Layout
/// The structure is packed (36 bytes) and follows the ACPI specification.
///
/// # Checksum
/// The checksum byte is calculated such that the sum of all bytes in the entire table
/// (including this header) equals zero when wrapped in u8 arithmetic.
#[repr(C, packed)]
#[derive(Clone, Debug, Copy, Default, IntoBytes, Immutable)]
pub struct SdtHeader {
    /// Table signature (e.g., b"XSDT", b"FACP", b"APIC")
    pub signature: [u8; 4],
    /// Length of the entire table including this header
    pub length: U32,
    /// Table revision number
    pub revision: u8,
    /// Checksum of entire table (sum of all bytes = 0)
    pub checksum: u8,
    /// OEM-supplied string that identifies the OEM
    pub oem_id: [u8; 6],
    /// OEM-supplied string that identifies the specific data table
    pub oem_table_id: [u8; 8],
    /// OEM-supplied revision number
    pub oem_revision: U32,
    /// Vendor ID of utility that created the table
    pub creator_id: [u8; 4],
    /// Revision of utility that created the table
    pub creator_revision: U32,
}

impl SdtHeader {
    pub(crate) fn new(
        signature: [u8; 4],
        length: u32,
        table_revision: u8,
        oem_id: [u8; 6],
        oem_table_id: [u8; 8],
        oem_revision: u32,
    ) -> Self {
        SdtHeader {
            signature,
            length: U32::new(length),
            revision: table_revision,
            checksum: 0,
            oem_id,
            oem_table_id,
            oem_revision: U32::new(oem_revision),
            creator_id: FC_ACPI_CREATOR_ID,
            creator_revision: U32::new(FC_ACPI_CREATOR_REVISION),
        }
    }
}

/// Trait for ACPI System Descriptor Table operations
///
/// This trait provides a common interface for all ACPI tables (XSDT, FADT, MADT, etc.)
/// allowing them to be written to guest memory and queried for their size.
///
/// # Implementation Note
/// Implementers must ensure the checksum is properly calculated before writing to guest memory.
pub trait Sdt {
    /// Get the total length of the table in bytes
    ///
    /// This includes the header and all table-specific data
    fn len(&self) -> usize;

    /// Check if the table is empty
    ///
    /// Returns true if the table length is zero (which should never happen for valid tables)
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write the complete table to guest memory at the specified address
    ///
    /// # Arguments
    /// * `mem` - The guest memory region to write to
    /// * `address` - The guest physical address where the table should be written
    ///
    /// # Errors
    /// Returns `AcpiError::GuestMemory` if writing to guest memory fails
    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::checksum;

    #[test]
    fn test_checksum() {
        assert_eq!(checksum(&[&[]]), 0u8);
        assert_eq!(checksum(&[]), 0u8);
        assert_eq!(checksum(&[&[1, 2, 3]]), 250u8);
        assert_eq!(checksum(&[&[1, 2, 3], &[]]), 250u8);
        assert_eq!(checksum(&[&[1, 2], &[3]]), 250u8);
        assert_eq!(checksum(&[&[1, 2], &[3], &[250]]), 0u8);
        assert_eq!(checksum(&[&[255]]), 1u8);
        assert_eq!(checksum(&[&[1, 2], &[3], &[250], &[255]]), 1u8);
    }
}
