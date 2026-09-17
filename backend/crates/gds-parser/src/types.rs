pub const DTYPE_NO_DATA: u8  = 0x00;
pub const DTYPE_BIT_ARRAY: u8 = 0x01;
pub const DTYPE_INT2: u8     = 0x02;
pub const DTYPE_INT4: u8     = 0x03;
pub const DTYPE_REAL4: u8    = 0x04;
pub const DTYPE_REAL8: u8    = 0x05;
pub const DTYPE_ASCII: u8    = 0x06;

pub const RT_HEADER: u8       = 0x00;
pub const RT_BGNLIB: u8       = 0x01;
pub const RT_LIBNAME: u8      = 0x02;
pub const RT_UNITS: u8        = 0x03;
pub const RT_ENDLIB: u8       = 0x04;
pub const RT_BGNSTR: u8       = 0x05;
pub const RT_STRNAME: u8      = 0x06;
pub const RT_ENDSTR: u8       = 0x07;
pub const RT_BOUNDARY: u8     = 0x08;
pub const RT_PATH: u8         = 0x09;
pub const RT_SREF: u8         = 0x0A;
pub const RT_AREF: u8         = 0x0B;
pub const RT_TEXT: u8         = 0x0C;
pub const RT_LAYER: u8        = 0x0D;
pub const RT_DATATYPE: u8     = 0x0E;
pub const RT_WIDTH: u8        = 0x0F;
pub const RT_XY: u8           = 0x10;
pub const RT_ENDEL: u8        = 0x11;
pub const RT_SNAME: u8        = 0x12;
pub const RT_COLROW: u8       = 0x13;
pub const RT_TEXTTYPE: u8     = 0x16;
pub const RT_PRESENTATION: u8 = 0x17;
pub const RT_STRING: u8       = 0x19;
pub const RT_STRANS: u8       = 0x1A;
pub const RT_MAG: u8          = 0x1B;
pub const RT_ANGLE: u8        = 0x1C;
pub const RT_PATHTYPE: u8     = 0x21;
pub const RT_GENERATIONS: u8  = 0x22;
pub const RT_ELFLAGS: u8      = 0x26;
pub const RT_NODETYPE: u8     = 0x2A;
pub const RT_PROPATTR: u8     = 0x2B;
pub const RT_PROPVALUE: u8    = 0x2C;
pub const RT_BOX: u8          = 0x2D;
pub const RT_BOXTYPE: u8      = 0x2E;
pub const RT_PLEX: u8         = 0x2F;
pub const RT_FORMAT: u8       = 0x36;
pub const RT_MASK: u8         = 0x37;
pub const RT_ENDMASKS: u8     = 0x38;

#[derive(Debug, Clone, PartialEq)]
pub enum GdsRecord {
    Header(Vec<u8>),
    BgnLib(Vec<u8>),
    LibName(Vec<u8>),
    Units(Vec<u8>),
    EndLib,
    BgnStr(Vec<u8>),
    StrName(Vec<u8>),
    EndStr,
    Boundary,
    Path,
    Sref,
    Aref,
    Text,
    EndEl,
    Layer(Vec<u8>),
    DataType(Vec<u8>),
    Width(Vec<u8>),
    XY(Vec<u8>),
    SName(Vec<u8>),
    ColRow(Vec<u8>),
    STrans(Vec<u8>),
    Mag(Vec<u8>),
    Angle(Vec<u8>),
    TextType(Vec<u8>),
    Presentation(Vec<u8>),
    String(Vec<u8>),
    PathType(Vec<u8>),
    Generations(Vec<u8>),
    ElFlags(Vec<u8>),
    PropAttr(Vec<u8>),
    PropValue(Vec<u8>),
    Plex(Vec<u8>),
    Box,
    BoxType(Vec<u8>),
    Format(Vec<u8>),
    Mask(Vec<u8>),
    EndMasks,
    Unknown { record_type: u8, data: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GdsError {
    UnexpectedEof,
    TruncatedRecord,
    RecordTooShort { length: u16 },
    UnexpectedRecord { record_type: u8, state: &'static str },
    InvalidAscii { record_type: u8 },
    UnresolvedReference { cell_name: String },
    IoError(String),
}

impl std::fmt::Display for GdsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GdsError::UnexpectedEof =>
                write!(f, "GDS parse failed: unexpected end of file"),
            GdsError::TruncatedRecord =>
                write!(f, "GDS parse failed: stream ended mid-record"),
            GdsError::RecordTooShort { length } =>
                write!(f, "GDS parse failed: record length {length} < 4"),
            GdsError::UnexpectedRecord { record_type, state } =>
                write!(f, "GDS parse failed: record 0x{record_type:02X} invalid in state '{state}'"),
            GdsError::InvalidAscii { record_type } =>
                write!(f, "GDS parse failed: non-ASCII in record 0x{record_type:02X}"),
            GdsError::UnresolvedReference { cell_name } =>
                write!(f, "GDS parse failed: unknown cell '{cell_name}'"),
            GdsError::IoError(msg) =>
                write!(f, "GDS I/O error: {msg}"),
        }
    }
}

impl std::error::Error for GdsError {}

impl From<std::io::Error> for GdsError {
    fn from(e: std::io::Error) -> Self {
        GdsError::IoError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gds_record_is_debug_and_clone() {
        let r = GdsRecord::Header(vec![0x00, 0x06]);
        let cloned = r.clone();
        assert_eq!(r, cloned);
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn endel_has_no_data() {
        let r = GdsRecord::EndEl;
        assert!(matches!(r, GdsRecord::EndEl));
    }

    #[test]
    fn unknown_record_carries_type_byte_and_data() {
        let r = GdsRecord::Unknown { record_type: 0xFF, data: vec![1, 2, 3] };
        if let GdsRecord::Unknown { record_type, data } = r {
            assert_eq!(record_type, 0xFF);
            assert_eq!(data.len(), 3);
        } else {
            panic!("expected Unknown variant");
        }
    }

    #[test]
    fn gds_error_display_does_not_panic() {
        let errors = vec![
            GdsError::UnexpectedEof,
            GdsError::TruncatedRecord,
            GdsError::RecordTooShort { length: 2 },
            GdsError::UnexpectedRecord { record_type: 0x10, state: "InLibrary" },
            GdsError::InvalidAscii { record_type: 0x06 },
            GdsError::UnresolvedReference { cell_name: "MISSING_CELL".to_string() },
            GdsError::IoError("disk read failed".to_string()),
        ];
        for e in errors {
            let _ = format!("{e}");
        }
    }

    #[test]
    fn record_type_constants_are_correct() {
        assert_eq!(RT_HEADER,   0x00);
        assert_eq!(RT_BOUNDARY, 0x08);
        assert_eq!(RT_SREF,     0x0A);
        assert_eq!(RT_XY,       0x10);
        assert_eq!(RT_ENDEL,    0x11);
        assert_eq!(RT_SNAME,    0x12);
        assert_eq!(RT_STRANS,   0x1A);
    }
}
