//! There are two fundamental actions that can be used to access a field:
//! 1. read
//! 1. write
//!
//! In addition to these fundamental actions, there are emergent properties derived from these actions and special
//! capabilities expressed by hardware.
//!
//! To illustrate this, consider writing the value `42` to some field `foo`. If you were to then **read** field `foo`,
//! what value would you get?
//!
//! Simply knowing whether or not a field is `read` and/or `write` is insufficient information to predict how the field
//! will behave.
//!
//! **Access modalities** fully express the behavior of field accesses and are as follows:
//!
//! | Name          | Software Access      | Hardware Access | Symmetry     |
//! | ------------- | -------------------- | --------------- | ------------ |
//! | Read          | Read                 | Write           | -            |
//! | Write         | Write                | Read            | -            |
//! | ReadWrite     | Read/Write           | Read/Write      | Asymmetrical |
//! | Store         | Read/Write           | Read            | Symmetrical  |
//! | VolatileStore | Read/Write           | Read/Write      | Symmetrical  |

/// The access modality of a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    /// This modality indicates that from the software (CPU) perspective, the field may only be *read*, as a way to view
    /// data written to the field by *hardware*.
    Read,
    /// This modality indicates that from the software (CPU) perspective, the field may only be *written*, as a way to
    /// send data to *hardware*.
    ///
    /// Since the field cannot be read, the data written is ephemeral.
    Write,
    /// This modality indicates that from the software (CPU) perspective, the field may be both *read* from and
    /// *written* to, as a way to send and receive data to and from hardware.
    ///
    /// In this modality, the data is **still ephemeral**, as the data read and the data written have different
    /// semantics. In other words, writing the value `42` to a `ReadWrite` field `foo`, does **not** mean reading the
    /// field would produce `42`.
    ///
    /// Fields with this modality can be thought of as two independent channels with opposing direction. Because of
    /// this, the variants of *read* data and *write* data are independent as well.
    ReadWrite,
    /// This modality indicates that from the software (CPU) perspective, the field may be both *read* from and
    /// *written* to, as a way to store data in hardware.
    ///
    /// Unlike [`ReadWrite`](Access::ReadWrite), hardware *only* has read access to the field. This means that values
    /// written to the field are statically known to persist, as hardware is incapable of mutating the field contents.
    ///
    /// This modality endows fields with *resolvability*.
    Store,
    /// This modality indicates that from the software (CPU) perspective, the field may be both *read* from and
    /// *written* to, as a way to store data in hardware.
    ///
    /// Unlike [`Store`](Access::Store), hardware *does* have write access.
    ///
    /// Unlike [`ReadWrite`](Access::ReadWrite), the data read and the data written have *the same* semantics.
    ///
    /// Fields with this modality can be thought of as a single bidirectional channel. This means that the field has
    /// *one* set of variants. This also means that data written to the field is **not** statically known to persist.
    ///
    /// This modality endows fields with *conditional resolvability*. This means that the field state can only be
    /// resolved when the entitlements of the hardware write access are *unsatisfied*.
    VolatileStore,
}
