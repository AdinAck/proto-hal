//! There are two fundamental actions that can be used to access a field:
//! 1. read
//! 1. write
//!
//! In addition to these fundamental actions, there are emergent properties
//! derived from these actions and special capabilities expressed by hardware.
//!
//! To illustrate this, consider writing the value `42` to some field `foo`.
//! If you were to then **read** field `foo`, what value would you get?
//!
//! Simply knowing whether or not a field is `read` and/or `write`, is insufficient
//! information to predict how the field will behave.
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

use crate::field::numericity::Numericity;

/// This modality indicates that from the software (CPU) perspective, the field
/// may only be *read*, as a way to view data written to the field by *hardware*.
#[derive(Debug, Clone, Default)]
pub struct Read {
    pub numericity: Numericity,
}

/// This modality indicates that from the software (CPU) perspective, the field
/// may only be *written*, as a way to send data to *hardware*.
///
/// Since the field cannot be read, the data ephemeral.
#[derive(Debug, Clone, Default)]
pub struct Write {
    pub numericity: Numericity,
}

/// This modality indicates that from the software (CPU) perspective, the field
/// may be both *read* from and *written* to, as a way to send and receive data
/// to and from hardware.
///
/// In this modality, the data is **still ephemeral**, as the data read and the
/// data written are not the same. In other words, writing the value `42` to a
/// `ReadWrite` field `foo`, does **not** mean reading the field would produce
/// `42`.
///
/// Fields with this modality can be thought of as two independent channels with
/// opposing direction. Because of this, the [`Numericity`] of *read* data and
/// *write* data are independent as well.
#[derive(Debug, Clone, Default)]
pub struct ReadWrite {
    pub read: Read,
    pub write: Write,
}

/// This modality indicates that from the software (CPU) perspective, the field
/// may be both *read* from and *written* to, as a way to store data in hardware.
///
/// Unlike [`ReadWrite`], hardware *only* has read access to the field. This
/// means that values written to the field are statically known to persist, as
/// hardware is incapable of mutating the field contents.
///
/// This modality endows fields with [resolvability](TODO: resolvability docs).
#[derive(Debug, Clone, Default)]
pub struct Store {
    pub numericity: Numericity,
}

/// This modality indicates that from the software (CPU) perspective, the field
/// may be both *read* from and *written* to, as a way to store data in hardware.
///
/// Unlike [`Store`], hardware *does* have write access.
///
/// Unlike [`ReadWrite`], the data read from and written to the field *are* the
/// same.
///
/// Fields with this modality can be thought of as a single bidirectional channel.
/// This means that the field has *one* [`Numericity`]. This also means that data
/// written to the field is **not** statically known to persist.
///
/// This modality endows fields with *conditional*
/// [resolvability](TODO: resolvability docs). This means that the field state can
/// only be resolved when the entitlements of the hardware write access are
/// unsatisfied.
#[derive(Debug, Clone, Default)]
pub struct VolatileStore {
    pub numericity: Numericity,
}

#[derive(Debug, Clone)]
pub enum Access {
    Read(Read),
    Write(Write),
    ReadWrite(ReadWrite),
    Store(Store),
    VolatileStore(VolatileStore),
}

impl Access {
    pub fn get_read(&self) -> Option<&Numericity> {
        match self {
            Self::Read(Read { numericity })
            | Self::ReadWrite(ReadWrite {
                read: Read { numericity },
                ..
            })
            | Self::Store(Store { numericity })
            | Self::VolatileStore(VolatileStore { numericity }) => Some(numericity),
            Self::Write(..) => None,
        }
    }

    pub fn get_read_mut(&mut self) -> Option<&mut Numericity> {
        match self {
            Self::Read(Read { numericity })
            | Self::ReadWrite(ReadWrite {
                read: Read { numericity },
                ..
            })
            | Self::Store(Store { numericity })
            | Self::VolatileStore(VolatileStore { numericity }) => Some(numericity),
            Self::Write(..) => None,
        }
    }

    pub fn get_write(&self) -> Option<&Numericity> {
        match self {
            Self::Write(Write { numericity })
            | Self::ReadWrite(ReadWrite {
                write: Write { numericity },
                ..
            })
            | Self::Store(Store { numericity })
            | Self::VolatileStore(VolatileStore { numericity }) => Some(numericity),
            Self::Read(..) => None,
        }
    }

    pub fn get_write_mut(&mut self) -> Option<&mut Numericity> {
        match self {
            Self::Write(Write { numericity })
            | Self::ReadWrite(ReadWrite {
                write: Write { numericity },
                ..
            })
            | Self::Store(Store { numericity })
            | Self::VolatileStore(VolatileStore { numericity }) => Some(numericity),
            Self::Read(..) => None,
        }
    }

    pub fn is_read(&self) -> bool {
        self.get_read().is_some()
    }

    pub fn is_write(&self) -> bool {
        self.get_write().is_some()
    }
}

impl From<Read> for Access {
    fn from(read: Read) -> Self {
        Self::Read(read)
    }
}

impl From<Write> for Access {
    fn from(write: Write) -> Self {
        Self::Write(write)
    }
}

impl From<ReadWrite> for Access {
    fn from(readwrite: ReadWrite) -> Self {
        Self::ReadWrite(readwrite)
    }
}

impl From<Store> for Access {
    fn from(store: Store) -> Self {
        Self::Store(store)
    }
}

impl From<VolatileStore> for Access {
    fn from(volatile_store: VolatileStore) -> Self {
        Self::VolatileStore(volatile_store)
    }
}

/// Marker trait for access modalities that expose write access.
#[doc(hidden)]
pub trait IsWrite {}

impl IsWrite for Write {}
impl IsWrite for ReadWrite {}
impl IsWrite for Store {}
impl IsWrite for VolatileStore {}
