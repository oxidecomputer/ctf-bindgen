use std::convert::TryFrom;
use std::error::Error;
use std::ffi::CStr;
use std::fs;
use std::io::Read;
use std::mem::size_of;
use std::path::Path;

use arrayref::array_ref;
use flate2::read::ZlibDecoder;
use num_enum::TryFromPrimitive;
use object::{
    elf::FileHeader64, endian::LittleEndian, read::elf::ElfFile, Object,
    ObjectSection, ObjectSymbol,
};

/// The primary CTF type that holds all of the information extracted from an ELF
/// object file. This data structure contains minimally parsed CTF elements that
/// reference internal `ctf_data`. Functions in the Ctf implementation are
/// provided for resolving things like string and type references.
#[derive(Debug)]
pub struct Ctf {
    /// Library file this CTF came from
    pub libname: String,

    /// Version and flag information associated with this CTF.
    pub preamble: Preamble,

    /// A collection of indices into the CTF data that tells us where to find
    /// things.
    pub header: Header,

    /// Parsed out sections containing CTF data structures.
    pub sections: Sections,

    /// Names of functions from the ELF symtab data
    pub function_names: Vec<String>,

    ctf_data: Vec<u8>,
    uncompressed: Option<Vec<u8>>,
}

impl Ctf {
    /// Create a Ctf instance from an ELF object file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Ctf, Box<dyn Error>> {
        // parse the libname libfoo.so as foo
        let libname = path
            .as_ref()
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let libname = match libname.strip_prefix("lib") {
            Some(without_prefix) => without_prefix.into(),
            None => libname,
        };

        //
        // parse the object file
        //
        let bin_data = fs::read(path)?;
        let obj_file = ElfFile::<'_, FileHeader64<LittleEndian>, &[u8]>::parse(
            bin_data.as_slice(),
        )?;

        // XXX
        let mut function_names = Vec::new();
        for s in obj_file.symbols() {
            let name = match s.name() {
                Ok(n) => n,
                Err(_) => continue,
            };
            if s.kind() == object::SymbolKind::Text {
                function_names.push(name.to_owned());
            }
        }

        //
        // get the CTF section, bail if it's not there
        //
        let section = match obj_file.section_by_name(".SUNW_ctf") {
            Some(section) => section,
            None => Err("ctf section not found")?,
        };

        //
        // parse the raw CTF data
        //
        Self::parse_ctf_data(section.data()?, function_names, libname)
    }

    /// First parse out the premable and headers, then decompress (if needed)
    /// and parse section data.
    fn parse_ctf_data(
        data: &[u8],
        function_names: Vec<String>,
        libname: String,
    ) -> Result<Ctf, Box<dyn Error>> {
        //
        // parse the preamble and header
        //
        let (preamble, data) = Self::parse_preamble(data)?;
        let (header, data) = Self::parse_header(data)?;

        //
        // decompress the remaining data if needed and parse the CTF section
        // data
        //
        let (sections, uncompressed) = if preamble.compressed() {
            let mut d = ZlibDecoder::new(data);
            let mut uncompressed = Vec::new();
            d.read_to_end(&mut uncompressed)?;
            (Sections::parse(&uncompressed, &header)?, Some(uncompressed))
        } else {
            (Sections::parse(data, &header)?, None)
        };

        Ok(Ctf {
            preamble,
            header,
            sections,
            uncompressed,
            function_names,
            libname,
            ctf_data: data.to_owned(),
        })
    }

    /// Ensure the correct magic is in the preamble. Check the CTF is the
    /// expected version. Extract flags
    fn parse_preamble(
        data: &[u8],
    ) -> Result<(Preamble, &[u8]), Box<dyn Error>> {
        assert!(data.len() >= 4);

        //
        // check magic
        //
        let magic = u16::from_le_bytes([data[0], data[1]]);
        if magic != MAGIC {
            Err(format!("ctf magic {} is not magical enough", magic))?;
        }

        //
        // check version
        //
        let version = data[2];
        if version != VERSION {
            Err(format!(
                "ctf version {} not supported, only version 2",
                version
            ))?;
        }

        //
        // extract flags
        //
        let flags = data[3];

        Ok((Preamble { version, flags }, &data[4..]))
    }

    /// Parse the contents of the CTF header.
    fn parse_header(d: &[u8]) -> Result<(Header, &[u8]), Box<dyn Error>> {
        let size = size_of::<Header>();
        assert!(d.len() >= size);

        Ok((
            Header {
                parent_label_offset: u32::from_le_bytes(*array_ref!(d, 0, 4)),
                parent_name_offset: u32::from_le_bytes(*array_ref!(d, 4, 4)),
                label_offset: u32::from_le_bytes(*array_ref!(d, 8, 4)),
                object_offset: u32::from_le_bytes(*array_ref!(d, 12, 4)),
                function_offset: u32::from_le_bytes(*array_ref!(d, 16, 4)),
                type_offset: u32::from_le_bytes(*array_ref!(d, 20, 4)),
                string_offset: u32::from_le_bytes(*array_ref!(d, 24, 4)),
                string_section_length: u32::from_le_bytes(*array_ref!(
                    d, 28, 4
                )),
            },
            &d[size..],
        ))
    }

    pub fn string_at(&self, offset: u32) -> &str {
        let offset = offset as usize;

        let s_begin = self.header.string_offset as usize;
        let s_end = s_begin + self.header.string_section_length as usize;
        let d = match &self.uncompressed {
            Some(u) => &u[s_begin..s_end],
            None => &self.ctf_data[s_begin..s_end],
        };

        if offset > d.len() {
            return "?";
        }

        let mut end = offset;
        loop {
            if end == d.len() {
                break;
            }
            if d[end] == 0 {
                end += 1;
                break;
            }
            end += 1;
        }

        let cs = CStr::from_bytes_with_nul(&d[offset..end]).unwrap();
        cs.to_str().unwrap()
    }

    pub fn type_name(&self, type_id: u16) -> &str {
        let t = &self.sections.types[type_id as usize - 1];
        self.string_at(t.name_offset)
    }
}

#[derive(Debug, Default)]
pub struct Preamble {
    pub version: u8,
    pub flags: u8,
}

#[derive(Debug, Default)]
pub struct Sections {
    pub labels: Vec<Label>,
    pub objects: Vec<u16>,
    pub functions: Vec<Function>,
    pub types: Vec<Type>,
}

#[derive(Debug, Default)]
pub struct Header {
    pub parent_label_offset: u32,
    pub parent_name_offset: u32,
    pub label_offset: u32,
    pub object_offset: u32,
    pub function_offset: u32,
    pub type_offset: u32,
    pub string_offset: u32,
    pub string_section_length: u32,
}

/// A few bits that are expected to be in the CTF preamble.
const MAGIC: u16 = 0xcff1;

/// Only CTF version 2 is supported
const VERSION: u8 = 0x02;

/// An indicator that compression is used on the CTF data.
const CTF_F_COMPRESS: u8 = 0x01;

//XXX size of a serialized ctf_stype without the variable length section
const BASE_TYPE_SIZE: usize = 8;

/// The CTF element kinds.
#[repr(u8)]
#[derive(Debug, TryFromPrimitive, PartialEq)]
pub enum Kind {
    Unknown,
    Integer,
    Float,
    Pointer,
    Array,
    Function,
    Struct,
    Union,
    Enum,
    Forward,
    Typedef,
    Volatile,
    Const,
    Restrict,
    Max = 31,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Label {
    pub name_offset: u32,
    pub type_offset: u32,
}

/// A function starts with a u16 type encoding with the following format.
///
///     ------------------------
///     | kind | isroot | vlen |
///     ------------------------
///     15   11    10    9     0
///
/// Then, if kind indicates a typed function, the next u16 is the return type
/// followed by N more u16 values where N is the number of arguments in the
/// function.
#[derive(Debug)]
pub struct Function {
    pub type_encoding: TypeEncoding,
    pub types: Vec<u16>,
}

#[derive(Debug)]
pub struct Type {
    pub name_offset: u32,
    pub type_encoding: TypeEncoding,
    pub info: TypeInfo,
    pub repr: TypeRepr,
    pub lsize: Option<Lsize>,
}

#[derive(Debug)]
pub enum TypeRepr {
    Struct(Vec<StructMember>),
    Int(u32),
    Float(u32),
}

#[derive(Debug)]
pub struct StructMember {
    pub name_offset: u32,
    pub type_offset: u16,
    pub offset: u16,
    pub lsize: Option<Lsize>,
}

#[derive(Debug)]
pub enum TypeInfo {
    Size(u16),
    // NOTE the Type variant only appears to be used for Pointer, Typdev,
    // Volatile, Restrict and Const kinds (see man ctf Encoding of Pointers,
    // Typedefs ...).
    Type(u16),
}

#[derive(Debug)]
pub struct Lsize {
    pub hi: u32,
    pub lo: u32,
}

#[derive(Debug)]
pub struct TypeEncoding(u16);

impl TypeEncoding {
    /// Decode this types kind.
    pub fn kind(&self) -> Kind {
        let k = ((self.0 & 0xf800) >> 11) as u8;
        match Kind::try_from(k) {
            Ok(k) => k,
            Err(_) => {
                println!("unexpected kind {}, interpreting as unknown", k);
                Kind::Unknown
            }
        }
    }

    /// True if this is a root kind.
    pub fn is_root(&self) -> bool {
        (self.0 & 0x0400) == 0
    }

    /// Length of the associated type data.
    pub fn vlen(&self) -> u16 {
        (self.0 & 0x3ff) as u16
    }
}

impl Function {
    /// Initialize a new function object from an encoded type. Arguments
    /// initialized to empty.
    pub fn new(type_encoding: u16) -> Self {
        Function {
            type_encoding: TypeEncoding(type_encoding),
            types: Vec::new(),
        }
    }
}

impl Preamble {
    /// Check if the data associated with this CTF preamble is compressed.
    pub fn compressed(&self) -> bool {
        self.flags & CTF_F_COMPRESS != 0
    }
}

impl Sections {
    /// Parse out each of the CTF sections in the provided CTF data including
    ///     - labels
    ///     - objects
    ///     - functions
    ///     - types
    fn parse(data: &[u8], header: &Header) -> Result<Sections, Box<dyn Error>> {
        Ok(Sections {
            labels: Self::read_labels(
                data,
                header.label_offset as usize,
                header.object_offset as usize,
            ),
            objects: Self::read_objects(
                data,
                header.object_offset as usize,
                header.function_offset as usize,
            ),
            functions: Self::read_functions(
                data,
                header.function_offset as usize,
                header.type_offset as usize,
            )?,
            types: Self::read_types(
                data,
                header.type_offset as usize,
                header.string_offset as usize,
            )?,
        })
    }

    fn read_labels(data: &[u8], begin: usize, end: usize) -> Vec<Label> {
        //
        // for now just stamp an array of Label structs onto the bits.
        //
        let labels: &[Label] =
            unsafe { std::mem::transmute(&data[begin..end]) };
        labels.into()
    }

    fn read_objects(data: &[u8], begin: usize, end: usize) -> Vec<u16> {
        //
        // for now just stamp an array of u16 references onto the bits
        //
        let labels: &[u16] = unsafe { std::mem::transmute(&data[begin..end]) };
        labels.into()
    }

    fn read_functions(
        mut data: &[u8],
        begin: usize,
        end: usize,
    ) -> Result<Vec<Function>, Box<dyn Error>> {
        //
        // restrict data slice to just the functions section
        //
        data = &data[begin..end];

        let mut result = Vec::new();

        //
        // loop over the data creating a function instance for each function we
        // find.
        //
        loop {
            if data.is_empty() {
                break;
            }

            let ftype = u16::from_le_bytes(*array_ref!(data, 0, 2));
            data = &data[2..];

            let mut func = Function::new(ftype);

            match func.type_encoding.kind() {
                // The function is untyped, no more processing to be done
                Kind::Unknown => {
                    result.push(func);
                    continue;
                }

                // Function is typed, continue processing below
                Kind::Function => {}

                // Bail on unexpected kind
                kind => {
                    Err(format!("unexpected kind {:?}", kind))?;
                }
            }

            // gather up the argument types, +1 is for the return argument
            let len = func.type_encoding.vlen() + 1;
            for _ in 0..len {
                let arg_type = u16::from_le_bytes(*array_ref!(data, 0, 2));
                data = &data[2..];
                func.types.push(arg_type);
            }

            result.push(func);
        }

        Ok(result)
    }

    fn read_types(
        mut data: &[u8],
        begin: usize,
        end: usize,
    ) -> Result<Vec<Type>, Box<dyn Error>> {
        //
        // restrict data slice to just the functions section
        //
        data = &data[begin..end];

        let mut result = Vec::new();

        loop {
            if data.is_empty() {
                break;
            }

            let name_offset = u32::from_le_bytes(*array_ref!(data, 0, 4));
            data = &data[4..];

            //
            // Extract the type info
            //
            let type_encoding =
                TypeEncoding(u16::from_le_bytes(*array_ref!(data, 0, 2)));
            data = &data[2..];

            //
            // Extract the type size
            //
            let type_size = u16::from_le_bytes(*array_ref!(data, 0, 2));
            data = &data[2..];

            //
            // Iterate over each possible type and handle accordingly
            //
            match type_encoding.kind() {
                Kind::Unknown => continue,

                //TODO account for differences in unions?
                Kind::Struct | Kind::Union => {
                    let (repr, remaining) =
                        Self::parse_struct(type_encoding.vlen(), data);
                    data = remaining;
                    result.push(Type {
                        name_offset,
                        type_encoding,
                        info: TypeInfo::Size(type_size),
                        repr: TypeRepr::Struct(repr),
                        //TODO
                        lsize: None,
                    });
                }

                Kind::Integer => {
                    let repr = u32::from_le_bytes(*array_ref!(data, 0, 4));
                    data = &data[4..];
                    result.push(Type {
                        name_offset,
                        type_encoding,
                        info: TypeInfo::Size(type_size),
                        repr: TypeRepr::Int(repr),
                        //TODO
                        lsize: None,
                    });
                }

                Kind::Float => {
                    let repr = u32::from_le_bytes(*array_ref!(data, 0, 4));
                    data = &data[4..];
                    result.push(Type {
                        name_offset,
                        type_encoding,
                        info: TypeInfo::Size(type_size),
                        repr: TypeRepr::Float(repr),
                        //TODO
                        lsize: None,
                    });
                }

                Kind::Pointer
                | Kind::Typedef
                | Kind::Volatile
                | Kind::Const
                | Kind::Restrict => {
                    //TODO - skip for now
                    let len = BASE_TYPE_SIZE;
                    data = &data[len..];
                }

                Kind::Array => {
                    //TODO - skip for now
                    let len = BASE_TYPE_SIZE + 8;
                    data = &data[len..];
                }

                k => todo!("kind: {:?}", k),
            }
        }

        Ok(result)
    }

    fn parse_struct(
        member_count: u16,
        mut data: &[u8],
    ) -> (Vec<StructMember>, &[u8]) {
        let mut result = Vec::new();

        for _ in 0..member_count {
            let name_offset = u32::from_le_bytes(*array_ref!(data, 0, 4));
            data = &data[4..];

            let type_offset = u16::from_le_bytes(*array_ref!(data, 0, 2));
            data = &data[2..];

            let offset = u16::from_le_bytes(*array_ref!(data, 0, 2));
            data = &data[2..];

            let lsize = if offset == 0x2000 {
                let lo = u32::from_le_bytes(*array_ref!(data, 0, 4));
                data = &data[4..];

                let hi = u32::from_le_bytes(*array_ref!(data, 0, 4));
                data = &data[4..];

                Some(Lsize { hi, lo })
            } else {
                None
            };

            result.push(StructMember {
                name_offset,
                type_offset,
                offset,
                lsize,
            })
        }

        (result, data)
    }
}
