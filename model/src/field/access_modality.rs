//! There are two fundamental actions that can be used to access a field:
//! 1. read
//! 1. write
//!
//! There are emergent properties derived from these actions and the constraints imposed on them that affect how the
//! field behaves.
//!
//! To illustrate this, consider writing the value `42` to some field `foo`. If you were to then *read* field `foo`,
//! what value would you get? Usually, the answer to such a question is "it depends on the exact
//! field/register/peripheral used". Knowing whether a field is `read` and/or `write` is insufficient information to
//! model how the field will behave.
//!
//! **Access modalities** provide a richer expression of field access that can be used to describe fields generally, and
//! are as follows:
//!
//! | Name          | Software Access      | Hardware Access | Symmetry     |
//! | ------------- | -------------------- | --------------- | ------------ |
//! | Read          | Read                 | Write           | -            |
//! | Write         | Write                | Read            | -            |
//! | ReadWrite     | Read/Write           | Read/Write      | Asymmetrical |
//! | Store         | Read/Write           | Read            | Symmetrical  |
//! | VolatileStore | Read/Write           | Read/Write      | Symmetrical  |
//!
//! *Symmetry* refers to the semantics of data *read* vs data *written*. A *symmetrical* access modality indicates that
//! the semantics of the data read and written are one in the same. An *asymmetrical* access modality indicates that the
//! semantics of the data read and written are *different*.

/// The access modality of a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessModality {
    /// This modality indicates that from the software (CPU) perspective, the field may only be *read*, as a way to view
    /// data written to the field by *hardware*.
    Read,
    /// This modality indicates that from the software (CPU) perspective, the field may only be *written*, as a way to
    /// send data to *hardware*.
    ///
    /// Since the field cannot be read, the data written is *ephemeral*.
    Write,
    /// This modality indicates that from the software (CPU) perspective, the field may be both *read* from and
    /// *written* to, as a way to send and receive data to and from hardware.
    ///
    /// In this modality, the data is *ephemeral* (like [`Write`](AccessModality::Write)), as the data read and the data
    /// written have different semantics. In other words, writing the value `42` to a `ReadWrite` field `foo`, does
    /// **not** mean reading the field would produce `42`. And even if it *did*, it would not mean the data written is
    /// being read back.
    ///
    /// Fields with this modality can be thought of as two independent channels with opposing direction. Because of
    /// this, the variants of *read* data and *write* data are independent as well.
    ReadWrite,
    /// This modality indicates that from the software (CPU) perspective, the field may be both *read* from and
    /// *written* to, as a way to store data in hardware.
    ///
    /// Unlike [`ReadWrite`](AccessModality::ReadWrite), hardware *only* has read access to the field.
    ///
    /// Fields with this modality can be thought of as a value store. This means that the field (if enumerated) has
    /// *one* set of variants.
    ///
    /// This modality endows fields with *resolvability*. This means that the values written to the field are statically
    /// known to persist.
    Store,
    /// This modality indicates that from the software (CPU) perspective, the field may be both *read* from and
    /// *written* to, as a way to store data in hardware.
    ///
    /// Unlike [`Store`](AccessModality::Store), hardware *does* have write access.
    ///
    /// Unlike [`ReadWrite`](AccessModality::ReadWrite), the data read and the data written have *the same* semantics.
    ///
    /// Fields with this modality can be thought of as a single bidirectional channel. This means that the field (if
    /// enumerated) has *one* set of variants.
    ///
    /// This modality endows fields with *conditional resolvability*. This means that the values written to the field
    /// are statically known to persist iff the entitlements of the hardware write access are *unsatisfied*.
    VolatileStore,
}
